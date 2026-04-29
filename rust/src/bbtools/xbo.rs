use byteorder::ByteOrder;
use byteorder::LittleEndian;
use pathdiff::diff_paths;

use std::collections::BTreeMap;
use std::fmt;
use std::fs::File;
use std::io::BufWriter;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::bbtools::FileSlice;
use crate::bbtools::lmt::LMT;
use crate::bbtools::ppd::PPD;

use gltf::json::accessor::*;
use gltf::json::buffer::*;
use gltf::json::extensions::root::Root as RootExtension;
use gltf::json::extensions::scene::Node as NodeExtension;
use gltf::json::extras::Void;
use gltf::json::mesh::*;
use gltf::json::scene::*;
use gltf::json::validation::*;
use gltf::json::*;

use serde_json::{Map, Number, Value, json};

use glam::EulerRot;
use glam::Quat;
use glam::f32::Vec2;
use glam::f32::Vec3A as Vec3; // Vec3A is 16-bytes so that it can function with SIMD
use glam::i16::I16Vec2;
use glam::i16::I16Vec3;

enum VertexFormat {
    XBO(Vec<VertexXBO>),
    XBO2(Vec<VertexXBO2>),
    SHA(Vec<VertexSHA>),
}

impl From<u32> for VertexFormat {
    fn from(format: u32) -> Self {
        match format {
            0x0152 => VertexFormat::XBO(Vec::new()),
            0x0052 => VertexFormat::XBO2(Vec::new()),
            0x0012 => VertexFormat::SHA(Vec::new()),
            _ => todo!("Unknown VertexFormat"),
        }
    }
}

impl fmt::Display for VertexFormat {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let name = match self {
            VertexFormat::XBO(_) => "XBO",
            VertexFormat::XBO2(_) => "XBO2",
            VertexFormat::SHA(_) => "SHA",
        };
        write!(f, "{}", name)
    }
}

const JOINT_SIZE: usize = 8;

impl VertexFormat {
    fn get_stride(&self) -> usize {
        match self {
            VertexFormat::XBO(_) => JOINT_SIZE + VERTEX_XBO_SIZE,
            VertexFormat::XBO2(_) => JOINT_SIZE + VERTEX_XBO2_SIZE,
            VertexFormat::SHA(_) => JOINT_SIZE + VERTEX_SHA_SIZE,
        }
    }

    fn write(&self, mesh_idx: u8, mut writer: impl Write) -> Result<usize, std::io::Error> {
        let mut bytes_written: usize = 0;

        let node_idx = mesh_idx + 1;
        let joint_buf = [
            node_idx, 0, 0, 0, // Joints
            0xFF, 0, 0, 0, // Weights
        ];

        match self {
            VertexFormat::XBO(v) => {
                for vertex in v {
                    let vertex_bytes = vertex.as_array();
                    writer.write_all(&joint_buf)?;
                    writer.write_all(&vertex_bytes)?;
                    bytes_written += joint_buf.len() + vertex_bytes.len();
                }
            }
            VertexFormat::XBO2(v) => {
                for vertex in v {
                    let vertex_bytes = vertex.as_array();
                    writer.write_all(&joint_buf)?;
                    writer.write_all(&vertex_bytes)?;
                    bytes_written += joint_buf.len() + vertex_bytes.len();
                }
            }
            VertexFormat::SHA(v) => {
                for vertex in v {
                    let vertex_bytes = vertex.as_array();
                    writer.write_all(&joint_buf)?;
                    writer.write_all(&vertex_bytes)?;
                    bytes_written += joint_buf.len() + vertex_bytes.len();
                }
            }
        }

        return Ok(bytes_written);
    }

    fn size(&self) -> usize {
        let stride = self.get_stride();
        match self {
            VertexFormat::XBO(v) => stride * v.len(),
            VertexFormat::XBO2(v) => stride * v.len(),
            VertexFormat::SHA(v) => stride * v.len(),
        }
    }
}

const VERTEX_XBO_SIZE: usize = 36;
struct VertexXBO {
    pos: Vec3,
    norm: Vec3,
    color: [u8; 4],
    uv: Vec2,
}

impl VertexXBO {
    fn as_array(&self) -> [u8; VERTEX_XBO_SIZE] {
        let mut buf: [u8; VERTEX_XBO_SIZE] = [0; _];
        LittleEndian::write_f32(&mut buf[0..4], self.pos.x);
        LittleEndian::write_f32(&mut buf[4..8], self.pos.y);
        LittleEndian::write_f32(&mut buf[8..12], self.pos.z);

        LittleEndian::write_f32(&mut buf[12..16], self.norm.x);
        LittleEndian::write_f32(&mut buf[16..20], self.norm.y);
        LittleEndian::write_f32(&mut buf[20..24], self.norm.z);

        buf[24..28].copy_from_slice(&self.color);

        LittleEndian::write_f32(&mut buf[28..32], self.uv.x);
        LittleEndian::write_f32(&mut buf[32..36], self.uv.y);

        return buf;
    }
}

const VERTEX_XBO2_SIZE: usize = 28;
struct VertexXBO2 {
    pos: Vec3,
    norm: Vec3,
    color: [u8; 4],
}

impl VertexXBO2 {
    fn as_array(&self) -> [u8; VERTEX_XBO2_SIZE] {
        let mut buf: [u8; VERTEX_XBO2_SIZE] = [0; _];
        LittleEndian::write_f32(&mut buf[0..4], self.pos.x);
        LittleEndian::write_f32(&mut buf[4..8], self.pos.y);
        LittleEndian::write_f32(&mut buf[8..12], self.pos.z);

        LittleEndian::write_f32(&mut buf[12..16], self.norm.x);
        LittleEndian::write_f32(&mut buf[16..20], self.norm.y);
        LittleEndian::write_f32(&mut buf[20..24], self.norm.z);

        buf[24..28].copy_from_slice(&self.color);

        return buf;
    }
}

const VERTEX_SHA_SIZE: usize = 24;
struct VertexSHA {
    pos: Vec3,
    norm: Vec3,
}

impl VertexSHA {
    fn as_array(&self) -> [u8; VERTEX_SHA_SIZE] {
        let mut buf: [u8; VERTEX_SHA_SIZE] = [0; _];
        LittleEndian::write_f32(&mut buf[0..4], self.pos.x);
        LittleEndian::write_f32(&mut buf[4..8], self.pos.y);
        LittleEndian::write_f32(&mut buf[8..12], self.pos.z);

        LittleEndian::write_f32(&mut buf[12..16], self.norm.x);
        LittleEndian::write_f32(&mut buf[16..20], self.norm.y);
        LittleEndian::write_f32(&mut buf[20..24], self.norm.z);

        return buf;
    }
}

struct MeshXBO {
    position: Vec3,
    rotation: Vec3,
    triangle_count: u16,
    vertex_count: usize,
    vertices: VertexFormat,
    indices: Vec<u16>,
    pos_min: Vec3,
    pos_max: Vec3,
}

fn pos_norm_from_slize(buf: &[u8]) -> (Vec3, Vec3) {
    assert!(buf.len() == 12 || buf.len() == 18);

    let pos = if buf.len() == 18 {
        Vec3::new(
            LittleEndian::read_f32(&buf[0..4]),
            LittleEndian::read_f32(&buf[4..8]),
            LittleEndian::read_f32(&buf[8..12]),
        )
    } else {
        I16Vec3::new(
            LittleEndian::read_i16(&buf[0..2]),
            LittleEndian::read_i16(&buf[2..4]),
            LittleEndian::read_i16(&buf[4..6]),
        )
        .as_vec3a()
    };

    let norm = I16Vec3::new(
        LittleEndian::read_i16(&buf[0..2]),
        LittleEndian::read_i16(&buf[2..4]),
        LittleEndian::read_i16(&buf[4..6]),
    )
    .as_vec3a()
    .normalize_or(Vec3::Y);

    return (pos, norm);
}

impl MeshXBO {
    fn new(buf: &[u8], indices: Vec<u16>, scale: f32) -> Self {
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

        let mut vertices = VertexFormat::from(LittleEndian::read_u32(&buf[24..28]));

        let mesh_data_size = LittleEndian::read_u32(&buf[28..32]) as usize;

        let _primative_type = LittleEndian::read_u16(&buf[32..36]); // D3DPRIMITIVETYPE

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
        for vertex_buf in data_buf.chunks_exact(vertex_stride) {
            match vertices {
                VertexFormat::XBO(ref mut v) => {
                    assert!(vertex_stride == 26 || vertex_stride == 20);
                    let pos_norm_size = vertex_stride - 8;
                    let (mut pos, norm) = pos_norm_from_slize(&vertex_buf[..pos_norm_size]);

                    pos *= scale;

                    let color_uv_buf = &vertex_buf[pos_norm_size..];

                    let color = [
                        color_uv_buf[2], // R
                        color_uv_buf[1], // G
                        color_uv_buf[0], // B
                        color_uv_buf[3], // A
                    ];

                    let uv = I16Vec2::new(
                        LittleEndian::read_i16(&color_uv_buf[4..6]),
                        LittleEndian::read_i16(&color_uv_buf[6..8]),
                    )
                    .as_vec2()
                        / i16::MAX as f32; // Convert to [-1, 1] range

                    pos_min = pos_min.min(pos);
                    pos_max = pos_max.max(pos);

                    v.push(VertexXBO {
                        pos,
                        norm,
                        uv,
                        color,
                    });
                }
                VertexFormat::XBO2(ref mut v) => {
                    assert!(vertex_stride == 22 || vertex_stride == 16);
                    let pos_norm_size = vertex_stride - 4;
                    let (mut pos, norm) = pos_norm_from_slize(&vertex_buf[..pos_norm_size]);

                    pos *= scale;

                    let color_buf = &vertex_buf[pos_norm_size..];

                    let color = [
                        color_buf[2], // R
                        color_buf[1], // G
                        color_buf[0], // B
                        color_buf[3], // A
                    ];

                    pos_min = pos_min.min(pos);
                    pos_max = pos_max.max(pos);

                    v.push(VertexXBO2 { pos, norm, color });
                }
                VertexFormat::SHA(ref mut v) => {
                    assert!(vertex_stride == 12);
                    let (mut pos, norm) = pos_norm_from_slize(&vertex_buf);

                    pos *= scale;

                    pos_min = pos_min.min(pos);
                    pos_max = pos_max.max(pos);

                    v.push(VertexSHA { pos, norm });
                }
            };
        }

        return MeshXBO {
            position,
            rotation,
            triangle_count,
            vertex_count,
            vertices,
            indices,
            pos_max,
            pos_min,
        };
    }
}

// These aren't actually magic values but they're consistent across all files so it works
const XBO_MAGIC0: [u8; 4] = [0x50, 0, 0, 0]; // @0x00
const XBO_MAGIC1: [u8; 4] = [0x08, 0, 0, 0]; // @0x58
const XBO_MAGIC2: [u8; 4] = [0x18, 0, 0, 0]; // @0x5C
pub struct XBO<'a> {
    motion: Option<&'a LMT<'a>>,
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
            "Mesh count {}, Pos {}, Rot {}",
            self.meshes.len(),
            self.position,
            self.rotation
        )?;

        for (i, mesh) in self.meshes.iter().enumerate() {
            writeln!(
                f,
                "  Mesh {:02}, Format {}, Triangle Count {}, Pos {}, Rot {}",
                i, mesh.vertices, mesh.triangle_count, mesh.position, mesh.rotation
            )?;
        }

        return Ok(());
    }
}

impl<'a> XBO<'a> {
    pub fn import(buf: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        Self::import_scaled(buf, 1.0)
    }

    pub fn import_scaled(buf: &[u8], scale: f32) -> Result<Self, Box<dyn std::error::Error>> {
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

        let model_file = FileSlice::from(&buf[0..8]);
        let special_file = FileSlice::from(&buf[8..16]);
        let mirror_file = FileSlice::from(&buf[16..24]);
        let mut indices_file = FileSlice::from(&buf[24..32]);

        // Process model file header
        let model_buf = &buf[model_file.as_range()];

        let node_count = model_buf[0];
        let mesh_count = node_count - 1;

        let _mesh_attr = model_buf[1];

        let _model_header_size = LittleEndian::read_u16(&model_buf[2..4]);

        let _extra_data_offset = LittleEndian::read_u32(&model_buf[4..8]);

        let mesh_files_end = 16 + FileSlice::buffer_size(mesh_count as u32);
        let mesh_files = FileSlice::from_bytes(&model_buf[16..mesh_files_end]);

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

        let mut meshes: Vec<MeshXBO> = Vec::with_capacity(mesh_count as usize);

        assert!(mesh_files.len() == indices_files.len());
        for i in 0..mesh_count as usize {
            let mesh_file = &mesh_files[i];
            let index_file = &indices_files[i];

            assert!(index_file.length % 2 == 0);
            let index_count = index_file.length / 2;

            // The extra +1 here is incase we need to flip face orientation by duplicating the first index
            let mut indices: Vec<u16> = Vec::with_capacity(index_count + 1);
            indices.resize(index_count, 0);

            LittleEndian::read_u16_into(&indices_buf[index_file.as_range()], &mut indices);

            let mesh_buf = &model_buf[mesh_file.as_range()];

            meshes.push(MeshXBO::new(mesh_buf, indices, scale));
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
            mesh.indices.insert(0, mesh.indices[0]);
        }
    }

    pub fn size(&self) -> (usize, usize) {
        let vertices_size = self
            .meshes
            .iter()
            .fold(0, |acc, mesh| acc + mesh.vertices.size());
        let indices_size = self
            .meshes
            .iter()
            .fold(0, |acc, mesh| acc + mesh.indices.len() * 2);

        return (vertices_size, indices_size);
    }

    pub fn write_to_glbin(&mut self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let file = File::create(&path)?;
        let mut writer = BufWriter::new(file);

        for (i, mesh) in self.meshes.iter().enumerate() {
            mesh.vertices.write(i as u8, &mut writer)?;
        }

        for mesh in self.meshes.iter() {
            let mut buf = vec![0; mesh.indices.len() * 2];
            LittleEndian::write_u16_into(&mesh.indices, &mut buf);

            writer.write_all(&buf)?;
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
        let mut node_idxs: Vec<Index<Node>> = Vec::with_capacity(mesh_count + 1);

        let root_position = if self.ignore_root_transform {
            None
        } else {
            Some(self.position.into())
        };

        let root_rotation = if self.ignore_root_transform {
            None
        } else {
            Some(UnitQuaternion(
                Quat::from_euler(
                    EulerRot::XYZEx,
                    self.rotation.x,
                    self.rotation.y,
                    self.rotation.z,
                )
                .into(),
            ))
        };

        node_idxs.push(root.push(Node {
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
            vertex_offsets[i + 1] = vertex_offsets[i] + mesh.vertices.size();
            index_offsets[i + 1] = index_offsets[i] + mesh.indices.len() * 2;
        }

        for (i, mesh) in self.meshes.iter().enumerate() {
            /*** VERTICIES ***/
            let vertex_byte_offset = vertex_offsets[i];
            let vertex_byte_length = vertex_offsets[i + 1] - vertex_byte_offset;

            let vertex_stride = mesh.vertices.get_stride();

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

            let mut attributes = BTreeMap::new();

            let joint_acc_idx = root.push(Accessor {
                name: Some(format!("Mesh {} - Joints", i)),
                buffer_view: Some(vertex_view_idx),
                byte_offset: Some(USize64::from(0u64)),
                count: USize64::from(mesh.vertex_count),
                component_type: Checked::Valid(GenericComponentType(ComponentType::U8)),
                extensions: None,
                extras: Extras::default(),
                type_: Checked::Valid(Type::Vec4),
                min: None,
                max: None,
                normalized: false,
                sparse: None,
            });

            attributes.insert(Checked::Valid(Semantic::Joints(0)), joint_acc_idx);

            let weight_acc_idx = root.push(Accessor {
                name: Some(format!("Mesh {} - Weights", i)),
                buffer_view: Some(vertex_view_idx),
                byte_offset: Some(USize64::from(4u64)),
                count: USize64::from(mesh.vertex_count),
                component_type: Checked::Valid(GenericComponentType(ComponentType::U8)),
                extensions: None,
                extras: Extras::default(),
                type_: Checked::Valid(Type::Vec4),
                min: None,
                max: None,
                normalized: true,
                sparse: None,
            });

            attributes.insert(Checked::Valid(Semantic::Weights(0)), weight_acc_idx);

            let pos_acc_idx = root.push(Accessor {
                name: Some(format!("Mesh {} - Position", i)),
                buffer_view: Some(vertex_view_idx),
                byte_offset: Some(USize64::from(8u64)),
                count: USize64::from(mesh.vertex_count),
                component_type: Checked::Valid(GenericComponentType(ComponentType::F32)),
                extensions: None,
                extras: Extras::default(),
                type_: Checked::Valid(Type::Vec3),
                min: Some(json!([mesh.pos_min.x, mesh.pos_min.y, mesh.pos_min.z])),
                max: Some(json!([mesh.pos_max.x, mesh.pos_max.y, mesh.pos_max.z])),
                normalized: false,
                sparse: None,
            });

            attributes.insert(Checked::Valid(Semantic::Positions), pos_acc_idx);

            let norm_acc_idx = root.push(Accessor {
                name: Some(format!("Mesh {} - Normal", i)),
                buffer_view: Some(vertex_view_idx),
                byte_offset: Some(USize64::from(20u64)),
                count: USize64::from(mesh.vertex_count),
                component_type: Checked::Valid(GenericComponentType(ComponentType::F32)),
                extensions: None,
                extras: Extras::default(),
                type_: Checked::Valid(Type::Vec3),
                min: None,
                max: None,
                normalized: false,
                sparse: None,
            });

            attributes.insert(Checked::Valid(Semantic::Normals), norm_acc_idx);

            if matches!(mesh.vertices, VertexFormat::XBO(_) | VertexFormat::XBO2(_)) {
                let color_acc_idx = root.push(Accessor {
                    name: Some(format!("Mesh {} - Colors", i)),
                    buffer_view: Some(vertex_view_idx),
                    byte_offset: Some(USize64::from(32u64)),
                    count: USize64::from(mesh.vertex_count),
                    component_type: Checked::Valid(GenericComponentType(ComponentType::U8)),
                    extensions: None,
                    extras: Extras::default(),
                    type_: Checked::Valid(Type::Vec4),
                    min: None,
                    max: None,
                    normalized: true,
                    sparse: None,
                });

                attributes.insert(Checked::Valid(Semantic::Colors(0)), color_acc_idx);
            }

            if matches!(mesh.vertices, VertexFormat::XBO(_)) {
                let uv_acc_idx = root.push(Accessor {
                    name: Some(format!("Mesh {} - TexCoords", i)),
                    buffer_view: Some(vertex_view_idx),
                    byte_offset: Some(USize64::from(36u64)),
                    count: USize64::from(mesh.vertex_count),
                    component_type: Checked::Valid(GenericComponentType(ComponentType::F32)),
                    extensions: None,
                    extras: Extras::default(),
                    type_: Checked::Valid(Type::Vec2),
                    min: None,
                    max: None,
                    normalized: false,
                    sparse: None,
                });

                attributes.insert(Checked::Valid(Semantic::TexCoords(0)), uv_acc_idx);
            }

            /*** INDICIES ***/
            let index_byte_offset = index_offsets[i];
            let index_byte_length = index_offsets[i + 1] - index_byte_offset;

            assert!(index_byte_length == mesh.indices.len() * 2);

            let index_view_idx = root.push(View {
                name: Some(format!("Mesh {} - Indicies", i)),
                buffer: vertex_index_buf_idx,
                byte_length: USize64::from(index_byte_length),
                byte_offset: Some(USize64::from(vertices_size + index_byte_offset)),
                byte_stride: None,
                target: Some(Checked::Valid(Target::ElementArrayBuffer)),
                extensions: None,
                extras: Extras::default(),
            });

            let index_acc_idx = root.push(Accessor {
                name: Some(format!("Mesh {} - Indicies", i)),
                buffer_view: Some(index_view_idx),
                byte_offset: None,
                count: USize64::from(mesh.indices.len()),
                component_type: Checked::Valid(GenericComponentType(ComponentType::U16)),
                extensions: None,
                extras: Extras::default(),
                type_: Checked::Valid(Type::Scalar),
                min: None,
                max: None,
                normalized: false,
                sparse: None,
            });

            primitives.push(Primitive {
                attributes: attributes,
                extensions: None,
                extras: Extras::default(),
                indices: Some(index_acc_idx),
                material: None,
                mode: Checked::Valid(Mode::TriangleStrip),
                targets: None,
            });

            let quat = Quat::from_euler(
                EulerRot::XYZEx,
                mesh.rotation.x,
                mesh.rotation.y,
                mesh.rotation.z,
            );

            node_idxs.push(root.push(Node {
                name: Some(format!("{}", i + 1)),
                camera: None,
                children: None, // This is processed later
                extensions: None,
                extras: Extras::default(),
                matrix: None,
                mesh: None,
                rotation: Some(UnitQuaternion(quat.into())),
                scale: None,
                translation: Some(mesh.position.into()),
                skin: None,
                weights: None,
            }));
        }

        let mesh_idx = root.push(Mesh {
            name: Some(gltf_name.clone()),
            extensions: None,
            extras: Extras::default(),
            primitives: primitives,
            weights: None,
        });

        let skin_idx = root.push(Skin {
            name: Some(gltf_name.clone()),
            extensions: None,
            extras: Extras::default(),
            inverse_bind_matrices: None,
            joints: node_idxs.clone(),
            skeleton: Some(node_idxs[0]),
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
                    children.push(node_idxs[j]);
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
                        bbtools_ext.insert(String::from("mirror"), Value::Number(Number::from(mn)));
                    }
                }
            }
        }

        let scene_idx = root.push(Scene {
            name: Some(gltf_name.clone()),
            extensions: None,
            extras: Extras::default(),
            nodes: vec![node_idxs[0]],
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
