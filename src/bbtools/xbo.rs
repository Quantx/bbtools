use byteorder::{ByteOrder, LittleEndian};
use gltf::json::material::AlphaCutoff;
use gltf::json::material::PbrBaseColorFactor;
use gltf::json::material::PbrMetallicRoughness;
use gltf::json::material::StrengthFactor;
use gltf::json::texture::Info;
use pathdiff::diff_paths;

use std::collections::BTreeMap;
use std::fmt;
use std::fs::File;
use std::io::BufWriter;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::bbtools::FileSlice;
use crate::bbtools::*;
use lmt::LMT;
use ppd::PPD;
use xpr::XPR;

use gltf::json::accessor::*;
use gltf::json::buffer::*;
use gltf::json::extensions::root::Root as RootExtension;
use gltf::json::extensions::scene::Node as NodeExtension;
use gltf::json::extras::Void;
use gltf::json::image::Image;
use gltf::json::material::*;
use gltf::json::mesh::*;
use gltf::json::scene::*;
use gltf::json::validation::*;
use gltf::json::*;

use serde_json::{Map, Number, Value, json};

use glam::EulerRot;
use glam::Quat;
use glam::Vec4Swizzles;
use glam::f32::Vec2;
use glam::f32::Vec3A as Vec3; // Vec3A is 16-bytes so that it can function with SIMD
use glam::i16::{I16Vec2, I16Vec3};
use glam::u8::U8Vec4;

const VERTEX_ATTRIBUTE_POSITION: u32 = 0x0002;
const VERTEX_ATTRIBUTE_NORMAL: u32 = 0x0010;
const VERTEX_ATTRIBUTE_COLOR: u32 = 0x0040;
const VERTEX_ATTRIBUTE_UV: u32 = 0x0100;

const JOINT_SIZE: usize = 8;
struct Vertex {
    position: Option<Vec3>, // 0x0002
    normal: Option<Vec3>,   // 0x0010
    color: Option<U8Vec4>,  // 0x0040
    uv: Option<Vec2>,       // 0x0100
}

impl Vertex {
    fn write(&self, mesh_idx: u8, writer: &mut impl Write) -> Result<(), std::io::Error> {
        let node_idx = mesh_idx + 1;
        let joint_buf: [u8; JOINT_SIZE] = [
            node_idx, 0, 0, 0, // Joints
            0xFF, 0, 0, 0, // Weights
        ];

        writer.write_all(&joint_buf)?;

        if let Some(position) = self.position {
            for v in position.to_array() {
                writer.write_f32::<LittleEndian>(v)?;
            }
        }

        if let Some(normal) = self.normal {
            for v in normal.to_array() {
                writer.write_f32::<LittleEndian>(v)?;
            }
        }

        if let Some(color) = self.color {
            writer.write_all(&color.to_array())?;
        }

        if let Some(uv) = self.uv {
            for v in uv.to_array() {
                writer.write_f32::<LittleEndian>(v)?;
            }
        }

        return Ok(());
    }

    fn stride_size(&self) -> usize {
        let mut stride: usize = JOINT_SIZE;

        if self.position.is_some() {
            stride += 12;
        }

        if self.normal.is_some() {
            stride += 12;
        }

        if self.color.is_some() {
            stride += 4;
        }

        if self.uv.is_some() {
            stride += 8;
        }

        return stride;
    }
}

struct MeshXBO {
    index: usize,
    primative_mode: Mode,
    position: Vec3,
    rotation: Vec3,
    triangle_count: u16,
    vertices: Vec<Vertex>,
    indices: Vec<u16>,
    pos_min: Vec3,
    pos_max: Vec3,
}

impl MeshXBO {
    fn new(index: usize, buf: &[u8], indices: Vec<u16>, scale: f32) -> Self {
        let position = Vec3::new(
            LittleEndian::read_f32(&buf[0..4]),
            LittleEndian::read_f32(&buf[4..8]),
            LittleEndian::read_f32(&buf[8..12]),
        ) * scale;

        let rotation = Vec3::new(
            LittleEndian::read_f32(&buf[12..16]),
            LittleEndian::read_f32(&buf[16..20]),
            LittleEndian::read_f32(&buf[20..24]),
        );

        let vertex_attributes = LittleEndian::read_u32(&buf[24..28]);

        let mesh_data_size = LittleEndian::read_u32(&buf[28..32]) as usize;

        let d3d_primative_type = LittleEndian::read_u16(&buf[32..36]); // D3DPRIMITIVETYPE
        let primative_mode = match d3d_primative_type {
            1 => Mode::Points,
            2 => Mode::Lines,
            3 => Mode::LineLoop,
            4 => Mode::LineStrip,
            5 => Mode::Triangles,
            6 => Mode::TriangleStrip,
            7 => Mode::TriangleFan,
            _ => unimplemented!("Unsupported D3D primative type"),
        };

        let mesh_header_size = LittleEndian::read_u16(&buf[36..38]) as usize; // Always 48

        assert!(buf.len() == mesh_header_size + mesh_data_size);

        let triangle_count = LittleEndian::read_u16(&buf[38..40]);
        let vertex_count = LittleEndian::read_u16(&buf[40..42]) as usize;

        let _attr_a = buf[42];
        let _attr_b = buf[43];

        let vertex_stride = buf[44] as usize;

        assert!(mesh_data_size == vertex_stride * vertex_count as usize);

        let _unknown_format = buf[45];
        let _unknown_data = LittleEndian::read_u16(&buf[46..48]);

        let mut pos_max = Vec3::NEG_INFINITY;
        let mut pos_min = Vec3::INFINITY;

        let data_buf = &buf[mesh_header_size..mesh_header_size + mesh_data_size];

        let mut vertices: Vec<Vertex> = Vec::with_capacity(vertex_count);
        for mut ptr in data_buf.chunks_exact(vertex_stride) {
            let position =
                if vertex_attributes & VERTEX_ATTRIBUTE_POSITION == VERTEX_ATTRIBUTE_POSITION {
                    let mut position_len = vertex_stride;
                    if vertex_attributes & VERTEX_ATTRIBUTE_NORMAL == VERTEX_ATTRIBUTE_NORMAL {
                        position_len -= 6; // I16Vec3
                    }

                    if vertex_attributes & VERTEX_ATTRIBUTE_COLOR == VERTEX_ATTRIBUTE_COLOR {
                        position_len -= 4; // U8Vec4
                    }

                    if vertex_attributes & VERTEX_ATTRIBUTE_UV == VERTEX_ATTRIBUTE_UV {
                        position_len -= 4; // I16Vec2
                    }

                    assert!(
                        position_len == 6 || position_len == 12,
                        "position_len {}",
                        position_len
                    );

                    let position = if position_len == 12 {
                        Vec3::new(
                            LittleEndian::read_f32(&ptr[0..4]),
                            LittleEndian::read_f32(&ptr[4..8]),
                            LittleEndian::read_f32(&ptr[8..12]),
                        )
                    } else {
                        I16Vec3::new(
                            LittleEndian::read_i16(&ptr[0..2]),
                            LittleEndian::read_i16(&ptr[2..4]),
                            LittleEndian::read_i16(&ptr[4..6]),
                        )
                        .as_vec3a()
                    } * scale;

                    pos_min = pos_min.min(position);
                    pos_max = pos_max.max(position);

                    ptr = &ptr[position_len..];

                    Some(position)
                } else {
                    None
                };

            let normal = if vertex_attributes & VERTEX_ATTRIBUTE_NORMAL == VERTEX_ATTRIBUTE_NORMAL {
                let normal = I16Vec3::new(
                    LittleEndian::read_i16(&ptr[0..2]),
                    LittleEndian::read_i16(&ptr[2..4]),
                    LittleEndian::read_i16(&ptr[4..6]),
                )
                .as_vec3a()
                .normalize_or(Vec3::Y);

                ptr = &ptr[6..];

                Some(normal)
            } else {
                None
            };

            let color = if vertex_attributes & VERTEX_ATTRIBUTE_COLOR == VERTEX_ATTRIBUTE_COLOR {
                let color = U8Vec4::from_slice(&ptr[0..4]).zyxw();

                ptr = &ptr[4..];

                Some(color)
            } else {
                None
            };

            let uv = if vertex_attributes & VERTEX_ATTRIBUTE_UV == VERTEX_ATTRIBUTE_UV {
                let uv = I16Vec2::new(
                    LittleEndian::read_i16(&ptr[0..2]),
                    LittleEndian::read_i16(&ptr[2..4]),
                )
                .as_vec2()
                    / i16::MAX as f32; // Convert to [-1, 1] range

                Some(uv)
            } else {
                None
            };

            vertices.push(Vertex {
                position,
                normal,
                color,
                uv,
            });
        }

        return MeshXBO {
            index,
            primative_mode,
            position,
            rotation,
            triangle_count,
            vertices,
            indices,
            pos_max,
            pos_min,
        };
    }

    pub fn generate_attributes(
        &self,
        root: &mut Root,
        vertex_view_idx: Index<View>,
        index_view_idx_opt: Option<Index<View>>,
    ) -> Result<Primitive, Box<dyn std::error::Error>> {
        if self.vertices.is_empty() {
            return Err("MeshVerticesEmpty".into());
        }
        let vertex = &self.vertices[0];

        let mut attributes = BTreeMap::new();
        let mut byte_offset: u64 = 0;

        let joint_acc_idx = root.push(Accessor {
            name: Some(format!("Mesh {} - Joints", self.index)),
            buffer_view: Some(vertex_view_idx),
            byte_offset: Some(USize64(byte_offset)),
            count: USize64::from(self.vertices.len()),
            component_type: Checked::Valid(GenericComponentType(ComponentType::U8)),
            extensions: None,
            extras: Extras::default(),
            type_: Checked::Valid(Type::Vec4),
            min: None,
            max: None,
            normalized: false,
            sparse: None,
        });

        byte_offset += 4;

        attributes.insert(Checked::Valid(Semantic::Joints(0)), joint_acc_idx);

        let weight_acc_idx = root.push(Accessor {
            name: Some(format!("Mesh {} - Weights", self.index)),
            buffer_view: Some(vertex_view_idx),
            byte_offset: Some(USize64(byte_offset)),
            count: USize64::from(self.vertices.len()),
            component_type: Checked::Valid(GenericComponentType(ComponentType::U8)),
            extensions: None,
            extras: Extras::default(),
            type_: Checked::Valid(Type::Vec4),
            min: None,
            max: None,
            normalized: true,
            sparse: None,
        });

        byte_offset += 4;

        attributes.insert(Checked::Valid(Semantic::Weights(0)), weight_acc_idx);

        if vertex.position.is_some() {
            let pos_acc_idx = root.push(Accessor {
                name: Some(format!("Mesh {} - Position", self.index)),
                buffer_view: Some(vertex_view_idx),
                byte_offset: Some(USize64(byte_offset)),
                count: USize64::from(self.vertices.len()),
                component_type: Checked::Valid(GenericComponentType(ComponentType::F32)),
                extensions: None,
                extras: Extras::default(),
                type_: Checked::Valid(Type::Vec3),
                min: Some(json!([self.pos_min.x, self.pos_min.y, self.pos_min.z])),
                max: Some(json!([self.pos_max.x, self.pos_max.y, self.pos_max.z])),
                normalized: false,
                sparse: None,
            });

            byte_offset += 12;

            attributes.insert(Checked::Valid(Semantic::Positions), pos_acc_idx);
        }

        if vertex.normal.is_some() {
            let norm_acc_idx = root.push(Accessor {
                name: Some(format!("Mesh {} - Normal", self.index)),
                buffer_view: Some(vertex_view_idx),
                byte_offset: Some(USize64(byte_offset)),
                count: USize64::from(self.vertices.len()),
                component_type: Checked::Valid(GenericComponentType(ComponentType::F32)),
                extensions: None,
                extras: Extras::default(),
                type_: Checked::Valid(Type::Vec3),
                min: None,
                max: None,
                normalized: false,
                sparse: None,
            });

            byte_offset += 12;

            attributes.insert(Checked::Valid(Semantic::Normals), norm_acc_idx);
        }

        if vertex.color.is_some() {
            let color_acc_idx = root.push(Accessor {
                name: Some(format!("Mesh {} - Colors", self.index)),
                buffer_view: Some(vertex_view_idx),
                byte_offset: Some(USize64(byte_offset)),
                count: USize64::from(self.vertices.len()),
                component_type: Checked::Valid(GenericComponentType(ComponentType::U8)),
                extensions: None,
                extras: Extras::default(),
                type_: Checked::Valid(Type::Vec4),
                min: None,
                max: None,
                normalized: true,
                sparse: None,
            });

            byte_offset += 4;

            attributes.insert(Checked::Valid(Semantic::Colors(0)), color_acc_idx);
        }

        if vertex.uv.is_some() {
            let uv_acc_idx = root.push(Accessor {
                name: Some(format!("Mesh {} - TexCoords", self.index)),
                buffer_view: Some(vertex_view_idx),
                byte_offset: Some(USize64(byte_offset)),
                count: USize64::from(self.vertices.len()),
                component_type: Checked::Valid(GenericComponentType(ComponentType::F32)),
                extensions: None,
                extras: Extras::default(),
                type_: Checked::Valid(Type::Vec2),
                min: None,
                max: None,
                normalized: false,
                sparse: None,
            });

            //byte_offset += 8;

            attributes.insert(Checked::Valid(Semantic::TexCoords(0)), uv_acc_idx);
        }

        let indices = index_view_idx_opt.map(|index_view_idx| {
            root.push(Accessor {
                name: Some(format!("Mesh {} - Indicies", self.index)),
                buffer_view: Some(index_view_idx),
                byte_offset: None,
                count: USize64::from(self.indices.len()),
                component_type: Checked::Valid(GenericComponentType(ComponentType::U16)),
                extensions: None,
                extras: Extras::default(),
                type_: Checked::Valid(Type::Scalar),
                min: None,
                max: None,
                normalized: false,
                sparse: None,
            })
        });

        let mode = if self.vertices.len() < 3 {
            Mode::Points
        } else {
            self.primative_mode
        };

        return Ok(Primitive {
            attributes,
            extensions: None,
            extras: Extras::default(),
            indices,
            material: None,
            mode: Checked::Valid(mode),
            targets: None,
        });
    }

    fn stride_size(&self) -> usize {
        self.vertices
            .first()
            .map(|v| v.stride_size())
            .unwrap_or_default()
    }

    fn has_texcoords(&self) -> bool {
        self.vertices
            .first()
            .map(|v| v.uv.is_some())
            .unwrap_or(false)
    }

    fn vertices_size(&self) -> usize {
        self.vertices.len() * self.stride_size()
    }

    fn indices_size(&self) -> usize {
        self.indices.len() * 2
    }

    fn write_vertices(&self, mesh_idx: u8, writer: &mut impl Write) -> Result<(), std::io::Error> {
        for vertex in self.vertices.iter() {
            vertex.write(mesh_idx, writer)?;
        }

        return Ok(());
    }

    fn write_indices(&self, writer: &mut impl Write) -> Result<(), std::io::Error> {
        let mut buf = vec![0; self.indices.len() * 2];
        LittleEndian::write_u16_into(&self.indices, &mut buf);
        writer.write_all(&buf)?;

        return Ok(());
    }
}

// These aren't actually magic values but they're consistent across all files so it works
const XBO_MAGIC0: [u8; 4] = [0x50, 0, 0, 0]; // @0x00
const XBO_MAGIC1: [u8; 4] = [0x08, 0, 0, 0]; // @0x58
const XBO_MAGIC2: [u8; 4] = [0x18, 0, 0, 0]; // @0x5C
pub struct XBO<'a> {
    motion: Option<&'a LMT<'a>>,
    texture: Option<&'a XPR>,
    hitbox: Option<&'a PPD>,
    surface_effects_path: Option<PathBuf>,
    pub ignore_root_transform: bool,
    meshes: Vec<MeshXBO>,
    parents: Vec<u8>,
    specials: Vec<u8>,
    mirrors: Option<Vec<u8>>,
    position: Vec3,
    rotation: Vec3,
    glbin_path: Option<PathBuf>,
}

impl fmt::Display for XBO<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(
            f,
            "Mesh count {}, Position {:.03}, Rotation {:.03}",
            self.meshes.len(),
            self.position,
            self.rotation
        )?;

        for (i, mesh) in self.meshes.iter().enumerate() {
            let mut attribute_strings: Vec<String> = Vec::with_capacity(4);
            if let Some(vertex) = mesh.vertices.first() {
                if vertex.position.is_some() {
                    attribute_strings.push("POSITION".to_string());
                }
                if vertex.normal.is_some() {
                    attribute_strings.push("NORMAL".to_string());
                }
                if vertex.color.is_some() {
                    attribute_strings.push("COLOR".to_string());
                }
                if vertex.uv.is_some() {
                    attribute_strings.push("UV".to_string());
                }
            } else {
                attribute_strings.push("NONE".to_string());
            }
            writeln!(
                f,
                "  Mesh {:02}, Primative {:?}, Attributes [{}], Vertices {}, Triangles {}, Indices {}, Position {:.03}, Rotation {:.03}",
                i,
                mesh.primative_mode,
                attribute_strings.join(", "),
                mesh.vertices.len(),
                mesh.triangle_count,
                mesh.indices.len(),
                mesh.position,
                mesh.rotation
            )?;
        }

        return Ok(());
    }
}

impl<'a> XBO<'a> {
    pub fn import(buf: &[u8], scale: f32) -> Result<Self, Box<dyn std::error::Error>> {
        if buf.len() < 0x60 {
            return Err("MissingHeader".into());
        }

        if &buf[0x00..0x04] != XBO_MAGIC0 {
            return Err("BadMagic0".into());
        }

        if &buf[0x58..0x5C] != XBO_MAGIC1 {
            return Err("BadMagic1".into());
        }

        if &buf[0x5C..0x60] != XBO_MAGIC2 {
            return Err("BadMagic2".into());
        }

        let mut model_file = FileSlice::from(&buf[0..8]);
        let special_file = FileSlice::from(&buf[8..16]);
        let mirror_file = FileSlice::from(&buf[16..24]);
        let mut indices_file = FileSlice::from(&buf[24..32]);

        // Fix a weird bug in SB ADVANCE.BIN's XBO files
        let unreliable_sizes = model_file.length == 0x19A && indices_file.length == 0;
        if unreliable_sizes {
            model_file.length = buf.len() - model_file.offset;
        }

        // Process model file header
        let model_buf = &buf[model_file.as_range()];

        let node_count = model_buf[0];
        let mesh_count = node_count - 1;

        let _mesh_attr = model_buf[1];

        let _model_header_size = LittleEndian::read_u16(&model_buf[2..4]);

        let _extra_data_offset = LittleEndian::read_u32(&model_buf[4..8]);

        let mesh_files_end = 16 + FileSlice::buffer_size(mesh_count as u32);
        let mut mesh_files = FileSlice::from_bytes(&model_buf[16..mesh_files_end]);

        if unreliable_sizes {
            for mesh_file in mesh_files.iter_mut() {
                if mesh_file.get_end() > model_buf.len() {
                    mesh_file.set_end(model_buf.len());
                }
            }
        }

        let parent_buf_end = mesh_files_end + node_count as usize;
        let parent_buf = &model_buf[mesh_files_end..parent_buf_end];

        let pos_rot_buf = &model_buf[parent_buf_end..];
        let position = Vec3::new(
            LittleEndian::read_f32(&pos_rot_buf[0..4]),
            LittleEndian::read_f32(&pos_rot_buf[4..8]),
            LittleEndian::read_f32(&pos_rot_buf[8..12]),
        ) * scale;

        let rotation = Vec3::new(
            LittleEndian::read_f32(&pos_rot_buf[12..16]),
            LittleEndian::read_f32(&pos_rot_buf[16..20]),
            LittleEndian::read_f32(&pos_rot_buf[20..24]),
        );

        let mut indices_bufs: Vec<&[u8]>;
        if indices_file.length > 0 {
            // Fix indices_file length (programming error with XBOs?)
            let indices_header_size = FileSlice::buffer_size(mesh_count as u32);
            indices_file.length += indices_header_size;

            if buf.len() != indices_file.get_end() as usize {
                eprintln!(
                    "Buf len {}, Index end {}",
                    buf.len(),
                    indices_file.get_end()
                );
                return Err("BufWrongSize".into());
            }

            // Process index file header
            let indices_buf = &buf[indices_file.as_range()];
            let indices_files = FileSlice::from_bytes(&indices_buf[0..indices_header_size]);
            indices_bufs = Vec::with_capacity(indices_files.len());
            for f in indices_files {
                indices_bufs.push(&indices_buf[f.as_range()]);
            }
        } else {
            indices_bufs = Vec::new();
        };

        let mut meshes: Vec<MeshXBO> = Vec::with_capacity(mesh_count as usize);
        for i in 0..mesh_count as usize {
            let indices = if indices_bufs.is_empty() {
                Vec::new()
            } else {
                let index_buf = indices_bufs[i];

                assert!(index_buf.len() % 2 == 0);
                let index_count = index_buf.len() / 2;

                // The extra +1 here is incase we need to flip face orientation by duplicating the first index
                let mut indices: Vec<u16> = Vec::with_capacity(index_count + 1);
                indices.resize(index_count, 0);

                LittleEndian::read_u16_into(&index_buf, &mut indices);

                indices
            };

            let mesh_file = &mesh_files[i];
            let mesh_buf = &model_buf[mesh_file.as_range()];
            meshes.push(MeshXBO::new(i, mesh_buf, indices, scale));
        }

        let specials_max = u32::BITS as usize;
        let mut specials: Vec<u8> = vec![0xFF; specials_max];
        if special_file.length > 0 {
            let special_buf = &buf[special_file.as_range()];
            let special_count = LittleEndian::read_u32(&special_buf[0..4]);
            let special_flags = LittleEndian::read_u32(&special_buf[4..8]);

            let mut special_idx: usize = 0;
            for i in 0..specials_max {
                let mask: u32 = 1 << i;
                if (special_flags & mask) == mask {
                    specials[i] = special_buf[8 + special_idx];
                    special_idx += 1;
                }
            }

            assert!(special_idx <= special_count as usize);
        }

        let mirrors: Option<Vec<u8>> = if mirror_file.length > 0 {
            assert!(mirror_file.length == node_count as usize);

            let mut mirrors = vec![0xFF; node_count as usize];
            mirrors.copy_from_slice(&buf[mirror_file.as_range()]);

            // Fix up mirror nodes
            for i in 0..node_count {
                let m = mirrors[i as usize];
                if m != i && mirrors[m as usize] == m {
                    mirrors[m as usize] = i;
                }
            }

            Some(mirrors)
        } else {
            None
        };

        return Ok(XBO {
            motion: None,
            texture: None,
            hitbox: None,
            surface_effects_path: None,
            ignore_root_transform: false,
            meshes,
            parents: parent_buf.to_vec(),
            specials,
            mirrors,
            position,
            rotation,
            glbin_path: None,
        });
    }

    pub fn get_node_count(&self) -> usize {
        self.meshes.len() + 1
    }

    pub fn set_animation(&mut self, lmt: &'a LMT) -> Result<(), Box<dyn std::error::Error>> {
        if lmt.get_node_count() != self.get_node_count() {
            println!(
                "LMT {}, XBO {}",
                lmt.get_node_count(),
                self.get_node_count()
            );
            return Err("AnimationNodeCountIncorrect".into());
        }

        self.motion = Some(lmt);

        return Ok(());
    }

    pub fn set_hitbox(&mut self, hbx: &'a PPD) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(hbx_node_count) = hbx.get_node_count()
            && hbx_node_count != self.get_node_count()
        {
            println!("HBX {}, XBO {}", hbx_node_count, self.get_node_count());
            return Err("HitboxNodeCountIncorrect".into());
        }

        self.hitbox = Some(hbx);

        return Ok(());
    }

    pub fn set_texture(&mut self, xpr: &'a XPR) -> Result<(), Box<dyn std::error::Error>> {
        self.texture = Some(xpr);
        return Ok(());
    }

    pub fn set_surface_effects_path(&mut self, path: &Path) {
        self.surface_effects_path = Some(path.to_owned());
    }

    pub fn path_to_surface_effects(&self, path: &Path) -> Option<PathBuf> {
        let mut gltf_path = PathBuf::from(path);
        gltf_path.set_file_name("");

        return diff_paths(self.surface_effects_path.as_ref()?, gltf_path);
    }

    pub fn flip_faces(&mut self) {
        // Clone the first index on every mesh to flip all face orientations
        for mesh in self.meshes.iter_mut() {
            if !mesh.indices.is_empty() {
                mesh.indices.insert(0, mesh.indices[0]);
            }
        }
    }

    pub fn size(&self) -> (usize, usize) {
        let vertices_size = self
            .meshes
            .iter()
            .fold(0, |acc, mesh| acc + mesh.vertices_size());
        let indices_size = self
            .meshes
            .iter()
            .fold(0, |acc, mesh| acc + mesh.indices_size());

        return (vertices_size, indices_size);
    }

    pub fn write_to_glbin(&mut self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let file = File::create(&path)?;
        let mut writer = BufWriter::new(file);

        for (i, mesh) in self.meshes.iter().enumerate() {
            mesh.write_vertices(i as u8, &mut writer)?;
        }

        for mesh in self.meshes.iter() {
            mesh.write_indices(&mut writer)?;
        }

        self.glbin_path = Some(path.to_path_buf());

        return Ok(());
    }

    pub fn path_to_glbin(&self, path: &Path) -> Option<PathBuf> {
        let mut gltf_path = PathBuf::from(path);
        gltf_path.set_file_name("");

        return diff_paths(self.glbin_path.as_ref()?, gltf_path);
    }

    pub fn write_to_gltf(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        self.write_to_gltf_with_node_meta(path, BTreeMap::new())
    }

    pub fn write_to_gltf_with_node_meta(
        &self,
        path: &Path,
        node_metas: BTreeMap<usize, Value>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let glbin_rel_path = if let Some(p) = self.path_to_glbin(path) {
            p
        } else {
            return Err("Cannot get relative path to mesh glbin".into());
        };

        let mut root = Root::default();

        root.extensions_used = vec![String::from("bbtools")];

        root.asset = Asset {
            copyright: None,
            extensions: None,
            generator: Some(String::from("bbtools")),
            min_version: None,
            extras: Void::default(),
            version: String::from("2.0"),
        };

        let material_idx = if let Some(texture) = self.texture {
            let texture_rel_path = if let Some(p) = texture.path_to_dds(&path) {
                p
            } else {
                return Err("Cannot get relative path to texture dds".into());
            };

            let texture_rel_path_str = texture_rel_path.to_string_lossy().into_owned();

            let image_idx = root.push(Image {
                buffer_view: None,
                mime_type: None,
                name: None,
                uri: Some(texture_rel_path_str),
                extensions: None,
                extras: Void::default(),
            });

            let texture_idx = root.push(Texture {
                name: None,
                sampler: None,
                source: image_idx,
                extensions: None,
                extras: Void::default(),
            });

            Some(root.push(Material {
                alpha_cutoff: Some(AlphaCutoff(0.5)),
                alpha_mode: Checked::Valid(material::AlphaMode::Mask),
                double_sided: false,
                name: None,
                pbr_metallic_roughness: PbrMetallicRoughness {
                    base_color_factor: PbrBaseColorFactor([1.0, 1.0, 1.0, 1.0]),
                    base_color_texture: Some(Info {
                        index: texture_idx,
                        tex_coord: 0,
                        extensions: None,
                        extras: Void::default(),
                    }),
                    metallic_factor: StrengthFactor(1.0),
                    roughness_factor: StrengthFactor(1.0),
                    metallic_roughness_texture: None,
                    extensions: None,
                    extras: Void::default(),
                },
                normal_texture: None,
                occlusion_texture: None,
                emissive_factor: EmissiveFactor([0.0, 0.0, 0.0]),
                emissive_texture: None,
                extensions: None,
                extras: Void::default(),
            }))
        } else {
            None
        };

        let glbin_rel_path_str = String::from(glbin_rel_path.to_str().unwrap());

        let gltf_name = String::from(path.file_prefix().unwrap().to_str().unwrap());

        let (vertices_size, indices_size) = self.size();

        // Buffer for the XBO data
        let vertex_index_buf_idx = root.push(Buffer {
            name: Some(String::from("Vertices & Indices")),
            uri: Some(glbin_rel_path_str),
            byte_length: USize64::from(vertices_size + indices_size),
            extensions: None,
            extras: Void::default(),
        });

        let mesh_count = self.meshes.len();
        let mut primitives: Vec<Primitive> = Vec::with_capacity(mesh_count);
        let mut node_indices: Vec<Index<Node>> = Vec::with_capacity(mesh_count + 1);

        let root_position = (!self.ignore_root_transform).then_some(self.position.to_array());

        let root_rotation = (!self.ignore_root_transform).then_some(UnitQuaternion(
            Quat::from_euler(
                EulerRot::XYZEx,
                self.rotation.x,
                self.rotation.y,
                self.rotation.z,
            )
            .to_array(),
        ));

        // Add the root node
        node_indices.push(root.push(Node {
            name: Some(String::from("0")),
            camera: None,
            children: None, // This is processed later
            extensions: None,
            extras: Extras::default(),
            matrix: None,
            mesh: None, // This is set later for node 0
            rotation: root_rotation,
            scale: None,
            translation: root_position,
            skin: None,
            weights: None,
        }));

        let mut vertex_offsets: Vec<usize> = vec![0; mesh_count + 1];
        let mut index_offsets: Vec<usize> = vec![0; mesh_count + 1];

        for (i, mesh) in self.meshes.iter().enumerate() {
            vertex_offsets[i + 1] = vertex_offsets[i] + mesh.vertices_size();
            index_offsets[i + 1] = index_offsets[i] + mesh.indices_size();
        }

        assert!(vertex_offsets.last() == Some(&vertices_size));
        assert!(index_offsets.last() == Some(&indices_size));

        for (i, mesh) in self.meshes.iter().enumerate() {
            /*** VERTICIES ***/
            let vertex_byte_offset = vertex_offsets[i];
            let vertex_byte_length = vertex_offsets[i + 1] - vertex_byte_offset;

            let vertex_stride = mesh.stride_size();

            let vertex_view_idx = root.push(View {
                name: Some(format!("Mesh {} - Verticies", i)),
                buffer: vertex_index_buf_idx,
                byte_length: USize64::from(vertex_byte_length),
                byte_offset: Some(USize64::from(vertex_byte_offset)),
                byte_stride: Some(Stride(vertex_stride)),
                target: Some(Checked::Valid(Target::ArrayBuffer)),
                extensions: None,
                extras: Extras::default(),
            });

            /*** INDICIES ***/
            let index_byte_offset = index_offsets[i];
            let index_byte_length = index_offsets[i + 1] - index_byte_offset;

            let index_view_idx_opt = (!mesh.indices.is_empty()).then(|| {
                root.push(View {
                    name: Some(format!("Mesh {} - Indicies", i)),
                    buffer: vertex_index_buf_idx,
                    byte_length: USize64::from(index_byte_length),
                    byte_offset: Some(USize64::from(vertices_size + index_byte_offset)),
                    byte_stride: None,
                    target: Some(Checked::Valid(Target::ElementArrayBuffer)),
                    extensions: None,
                    extras: Extras::default(),
                })
            });

            let mut primative =
                mesh.generate_attributes(&mut root, vertex_view_idx, index_view_idx_opt)?;
            if mesh.has_texcoords() {
                primative.material = material_idx;
            }
            primitives.push(primative);

            let quat = Quat::from_euler(
                EulerRot::XYZEx,
                mesh.rotation.x,
                mesh.rotation.y,
                mesh.rotation.z,
            );

            node_indices.push(root.push(Node {
                name: Some(format!("{}", i + 1)),
                camera: None,
                children: None, // This is processed later
                extensions: None,
                extras: Extras::default(),
                matrix: None,
                mesh: None,
                rotation: Some(UnitQuaternion(quat.to_array())),
                scale: None,
                translation: Some(mesh.position.to_array()),
                skin: None,
                weights: None,
            }));
        }

        let mesh_idx = root.push(Mesh {
            name: Some(gltf_name.clone()),
            extensions: None,
            extras: Extras::default(),
            primitives,
            weights: None,
        });

        let skin_idx = root.push(Skin {
            name: Some(gltf_name.clone()),
            extensions: None,
            extras: Extras::default(),
            inverse_bind_matrices: None,
            joints: node_indices.clone(),
            skeleton: Some(node_indices[0]),
        });

        for (i, node) in root.nodes.iter_mut().enumerate() {
            if i == 0 {
                node.mesh = Some(mesh_idx);
                node.skin = Some(skin_idx);
            }

            if let Some(si) = self.specials.iter().position(|&sn| sn == i as u8) {
                node.name = Some(format!("special_{}", si));
            };

            let mut children: Vec<Index<Node>> = Vec::new();
            for (j, &parent) in self.parents.iter().enumerate() {
                if parent == i as u8 {
                    children.push(node_indices[j]);
                }
            }

            if !children.is_empty() {
                node.children = Some(children);
            }

            if let Some(mirrors) = self.mirrors.as_ref() {
                let mn = mirrors[i] as usize;
                if mn != i {
                    let extensions = node
                        .extensions
                        .get_or_insert(NodeExtension { others: Map::new() });

                    if let &mut Value::Object(ref mut bbtools_ext) = extensions
                        .others
                        .entry("bbtools")
                        .or_insert(Value::Object(Map::new()))
                    {
                        bbtools_ext.insert("mirror".to_owned(), Value::Number(Number::from(mn)));
                    }
                }
            }

            if let Some(meta) = node_metas.get(&i) {
                let extensions = node
                    .extensions
                    .get_or_insert(NodeExtension { others: Map::new() });

                if let &mut Value::Object(ref mut bbtools_ext) = extensions
                    .others
                    .entry("bbtools")
                    .or_insert(Value::Object(Map::new()))
                {
                    bbtools_ext.insert("meta".to_owned(), meta.clone());
                }
            }
        }

        let scene_idx = root.push(Scene {
            name: Some(gltf_name.clone()),
            extensions: None,
            extras: Extras::default(),
            nodes: vec![node_indices[0]],
        });

        // Set this as the primary scene
        root.scene = Some(scene_idx);

        // Add animations from motion file if one is present
        if let Some(motion) = self.motion {
            let mn: Option<&[u8]> = self.mirrors.as_ref().map(|x| x.as_slice());
            motion.append_to_gltf(&mut root, &path, mn)?;
        }

        if let Some(hitbox) = self.hitbox {
            hitbox.append_to_gltf(&mut root, &path)?;
        }

        if self.surface_effects_path.is_some() {
            let surface_effects_rel_path = if let Some(p) = self.path_to_surface_effects(path) {
                p
            } else {
                return Err("Cannot get relative path to surface effects".into());
            };

            let surface_effects_rel_path_str =
                surface_effects_rel_path.to_string_lossy().into_owned();

            const SURFACE_EFFECT_SIZE: usize = 28 * 9 * 4;

            let surface_effects_buf_idx = root.push(Buffer {
                name: Some(String::from("Surface Effects")),
                uri: Some(surface_effects_rel_path_str),
                byte_length: USize64::from(SURFACE_EFFECT_SIZE),
                extensions: None,
                extras: Void::default(),
            });

            let surface_effects_view_idx = root.push(View {
                name: Some(String::from("Surface Effects")),
                buffer: surface_effects_buf_idx,
                byte_length: USize64::from(SURFACE_EFFECT_SIZE),
                byte_offset: None,
                byte_stride: None,
                target: None,
                extensions: None,
                extras: Extras::default(),
            });

            let extensions = root
                .extensions
                .get_or_insert(RootExtension { others: Map::new() });

            if let &mut Value::Object(ref mut bbtools_ext) = extensions
                .others
                .entry("bbtools")
                .or_insert(Value::Object(Map::new()))
            {
                bbtools_ext.insert(
                    String::from("surface_effects"),
                    Value::Number(Number::from(surface_effects_view_idx.value())),
                );
            }
        }

        let file = File::create(path)?;
        let writer = BufWriter::new(file);
        root.to_writer_pretty(writer)?;

        return Ok(());
    }
}
