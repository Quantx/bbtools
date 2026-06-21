use std::cmp;
use std::fmt;
use std::fs::File;
use std::io::BufWriter;
use std::io::Write;
use std::path::{Path, PathBuf};

use byteorder::ByteOrder;
use byteorder::LittleEndian;
use byteorder::WriteBytesExt;

use pathdiff::diff_paths;

use glam::f32::Vec3A as Vec3; // Vec3A is 16-bytes so that it can function with SIMD
use glam::i32::IVec3;
use glam::u16::U16Vec3;

use gltf::json::buffer::*;
use gltf::json::extensions::scene::Node as NodeExtension;
use gltf::json::extras::Void;
use gltf::json::validation::*;
use gltf::json::*;

use serde_json::{Map, Value, json};

#[derive(Clone, Copy)]
pub struct Surface(pub u16, pub u8); // layers, surface_type

impl Surface {
    pub fn new(layers: u16, surface_bit: u64) -> Self {
        if surface_bit == 0 {
            return Surface(layers, 0xFF);
        }

        let surface = cmp::min(surface_bit.trailing_zeros(), 48) as u8;
        assert!(surface < 28, "Invalid surface: {:016X}", surface_bit);
        Surface(layers, surface)
    }
}

enum ChildType {
    AABB(Vec<AABB>),
    Quad(Vec<Quad>),
}

impl ChildType {
    fn import(&mut self, base_buf: &[u8], mut offset: usize, scale: f32, level: usize) {
        if level == 0xFF {
            panic!("PPD recursive overflow");
        }

        match self {
            ChildType::AABB(children) => {
                loop {
                    //println!("Level {:02} Import AABB at offset {}", level, offset);
                    let mut aabb = AABB::new(&base_buf[offset..offset + AABB_SIZE], scale, level);
                    aabb.children
                        .import(base_buf, offset + AABB_SIZE, scale, level + 1);

                    let next_offset = aabb.next_offset;
                    children.push(aabb);

                    if next_offset == 0 {
                        break;
                    }
                    offset += next_offset;
                }
            }
            ChildType::Quad(quads) => {
                //println!("Level {:02} Import QUADs at offset {}", level, offset);
                // Quad data immediately follows
                let quad_count = quads.capacity();
                let quad_data_buf_end = offset + quad_count * QUAD_SIZE;
                let quad_data_buf = &base_buf[offset..quad_data_buf_end];

                let (quad_list, []) = quad_data_buf.as_chunks::<QUAD_SIZE>() else {
                    panic!("header_buf length not a multiple of QUAD_SIZE");
                };

                for quad_buf in quad_list {
                    quads.push(Quad::new(quad_buf, scale, level));
                }
            }
        }
    }
}

const QUAD_SIZE: usize = 80;
pub struct Quad {
    surface: Surface,
    vertices: [Vec3; 4],
    normal: Vec3,
    level: usize,
}

impl Quad {
    pub fn new(buf: &[u8], scale: f32, level: usize) -> Quad {
        let _val = LittleEndian::read_i32(&buf[0..4]);

        let (vert_list, []) = buf[4..52].as_chunks::<12>() else {
            panic!("header_buf length not a multiple of 12");
        };

        let mut vertices: [Vec3; 4] = [Vec3::ZERO; _];
        for (i, vert_buf) in vert_list.iter().enumerate() {
            vertices[i].x = LittleEndian::read_f32(&vert_buf[0..4]);
            vertices[i].y = LittleEndian::read_f32(&vert_buf[4..8]);
            vertices[i].z = LittleEndian::read_f32(&vert_buf[8..12]);
            vertices[i] *= scale;
        }

        assert!(LittleEndian::read_u32(&buf[52..56]) == 0);
        let surface_bit = LittleEndian::read_u48(&buf[56..62]);
        let layers = LittleEndian::read_u16(&buf[62..64]);

        let normal = Vec3::new(
            LittleEndian::read_f32(&buf[64..68]),
            LittleEndian::read_f32(&buf[68..72]),
            LittleEndian::read_f32(&buf[72..76]),
        )
        .normalize();

        assert!(LittleEndian::read_u32(&buf[76..80]) == 0);

        return Quad {
            surface: Surface::new(layers, surface_bit),
            vertices,
            normal,
            level,
        };
    }

    fn write_triangles(&self, writer: &mut impl Write) -> Result<(), std::io::Error> {
        // Alter the winding order
        let vertices: [Vec3; 6] = [
            self.vertices[1],
            self.vertices[0],
            self.vertices[2], // Triangle 1
            self.vertices[3],
            self.vertices[1],
            self.vertices[2],
        ]; // Triangle 2

        for vertex in vertices {
            writer.write_f32::<LittleEndian>(vertex.x)?;
            writer.write_f32::<LittleEndian>(vertex.y)?;
            writer.write_f32::<LittleEndian>(vertex.z)?;
        }

        return Ok(());
    }

    fn write_layer(&self, writer: &mut impl Write) -> Result<(), std::io::Error> {
        writer.write_u16::<LittleEndian>(self.surface.0)?;
        return Ok(());
    }

    fn write_surface(&self, writer: &mut impl Write) -> Result<(), std::io::Error> {
        writer.write_u8(self.surface.1)?;
        return Ok(());
    }
}

impl fmt::Display for Quad {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let indent = self.level * 2;

        writeln!(
            f,
            "{:indent$}Quad - Layers {:04X}, Surface Type {:02X}, Normal {}",
            "",
            self.surface.0,
            self.surface.1,
            self.normal,
            indent = indent
        )?;

        for (i, vertex) in self.vertices.iter().enumerate() {
            writeln!(
                f,
                "{:indent$}  Vertex {} {}",
                "",
                i,
                vertex,
                indent = indent
            )?;
        }

        return Ok(());
    }
}

const AABB_SIZE: usize = 32;
pub struct AABB {
    min: Vec3,
    max: Vec3,
    children: ChildType,
    next_offset: usize,
    level: usize,
}

impl AABB {
    pub fn new(buf: &[u8], scale: f32, level: usize) -> AABB {
        let center = IVec3::new(
            LittleEndian::read_i32(&buf[0..4]),
            LittleEndian::read_i32(&buf[4..8]),
            LittleEndian::read_i32(&buf[8..12]),
        )
        .as_vec3a()
            * scale;

        let extents = U16Vec3::new(
            LittleEndian::read_u16(&buf[12..14]),
            LittleEndian::read_u16(&buf[14..16]),
            LittleEndian::read_u16(&buf[16..18]),
        )
        .as_vec3a()
            * scale;

        let corner_a = center + extents;
        let corner_b = center - extents;

        let child_aabb_count = LittleEndian::read_u16(&buf[18..20]) as usize;
        let child_quad_count = LittleEndian::read_u16(&buf[20..22]) as usize;

        //&buf[22..24] is always zero

        let next_offset = LittleEndian::read_u32(&buf[24..28]) as usize;
        let _data_index = LittleEndian::read_u32(&buf[28..32]) as usize;

        let children = if child_aabb_count > 0 {
            ChildType::AABB(Vec::with_capacity(child_aabb_count))
        } else {
            ChildType::Quad(Vec::with_capacity(child_quad_count))
        };

        return AABB {
            min: corner_a.min(corner_b),
            max: corner_a.max(corner_b),
            children: children,
            next_offset: next_offset,
            level: level,
        };
    }

    pub fn import_tree(base_buf: &[u8], first_header_offset: usize, scale: f32) -> AABB {
        let mut children_type = ChildType::AABB(Vec::with_capacity(100));
        children_type.import(base_buf, first_header_offset, scale, 1);

        let mut min = Vec3::INFINITY;
        let mut max = Vec3::NEG_INFINITY;

        if let ChildType::AABB(ref children) = children_type {
            for child in children {
                min = min.min(child.min);
                max = max.max(child.max);
            }
        }

        return AABB {
            min: min,
            max: max,
            children: children_type,
            next_offset: 0,
            level: 0,
        };
    }

    pub fn quad_count(&self) -> usize {
        match self.children {
            ChildType::AABB(ref children) => children
                .iter()
                .fold(0, |count, child| count + child.quad_count()),
            ChildType::Quad(ref children) => children.len(),
        }
    }

    pub fn write_triangles(&self, writer: &mut impl Write) -> Result<(), std::io::Error> {
        match self.children {
            ChildType::AABB(ref children) => {
                for child in children.iter() {
                    child.write_triangles(writer)?;
                }
            }
            ChildType::Quad(ref children) => {
                for child in children.iter() {
                    child.write_triangles(writer)?;
                }
            }
        }

        return Ok(());
    }

    pub fn write_surfaces(&self, writer: &mut impl Write) -> Result<(), std::io::Error> {
        match self.children {
            ChildType::AABB(ref children) => {
                for child in children.iter() {
                    child.write_surfaces(writer)?;
                }
            }
            ChildType::Quad(ref children) => {
                for child in children.iter() {
                    child.write_surface(writer)?;
                }
            }
        }

        return Ok(());
    }

    pub fn write_layers(&self, writer: &mut impl Write) -> Result<(), std::io::Error> {
        match self.children {
            ChildType::AABB(ref children) => {
                for child in children.iter() {
                    child.write_layers(writer)?;
                }
            }
            ChildType::Quad(ref children) => {
                for child in children.iter() {
                    child.write_layer(writer)?;
                }
            }
        }

        return Ok(());
    }
}

impl fmt::Display for AABB {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let indent = self.level * 2;
        writeln!(
            f,
            "{:indent$}AABB - Min {}, Max {}",
            "",
            self.min,
            self.max,
            indent = indent
        )?;
        match &self.children {
            ChildType::AABB(children) => {
                for child in children {
                    write!(f, "{}", child)?;
                }
            }
            ChildType::Quad(quads) => {
                for quad in quads {
                    write!(f, "{}", quad)?;
                }
            }
        }
        return Ok(());
    }
}

const COLLIDER_QUAD_TRIANGLES_SIZE: usize = 72;
const COLLIDER_QUAD_LAYERS_SIZE: usize = 2;
pub struct Collider {
    aabb: AABB,
    bone: Option<u8>,
}

impl Collider {
    pub fn size(&self) -> usize {
        self.triangles_size() + self.layers_size() + self.surfaces_size()
    }

    pub fn triangles_size(&self) -> usize {
        self.aabb.quad_count() * COLLIDER_QUAD_TRIANGLES_SIZE
    }

    pub fn layers_size(&self) -> usize {
        self.aabb.quad_count() * COLLIDER_QUAD_LAYERS_SIZE
    }

    pub fn surfaces_size(&self) -> usize {
        self.aabb.quad_count()
    }

    pub fn write(&self, writer: &mut impl Write) -> Result<(), std::io::Error> {
        self.aabb.write_triangles(writer)?;
        self.aabb.write_layers(writer)?;
        self.aabb.write_surfaces(writer)?;
        return Ok(());
    }
}

const PPD_HEADER_SIZE: usize = 16;
pub struct PPD {
    margin: f32,
    colliders: Vec<Collider>,
    node_count: Option<usize>,
    glbin_path: Option<PathBuf>,
}

impl PPD {
    pub fn import_scaled(
        buf: &[u8],
        scale: f32,
        node_count_fix: usize,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // No MAGIC for this file type unfortunately
        let margin = LittleEndian::read_f32(&buf[0..4]) as f32 * scale;
        let collider_count = LittleEndian::read_u32(&buf[4..8]) as usize;

        // &buf[8..16] is always zero

        let collider_buf = &buf[PPD_HEADER_SIZE..];

        let mut collider_offsets: Vec<u32> = vec![0; collider_count];
        let collider_offsets_end = collider_count * 4;
        LittleEndian::read_u32_into(&collider_buf[..collider_offsets_end], &mut collider_offsets);

        let bone_offset =
            LittleEndian::read_u32(&collider_buf[collider_offsets_end..(collider_offsets_end + 4)])
                as usize;

        let mut colliders: Vec<Collider> = Vec::with_capacity(collider_count);
        for collider_offset in collider_offsets {
            colliders.push(Collider {
                aabb: AABB::import_tree(&collider_buf, collider_offset as usize, scale),
                bone: None,
            });
        }

        let node_count = if bone_offset > 0 {
            let bones_buf = &buf[PPD_HEADER_SIZE + bone_offset..];

            let bone_count = bones_buf[0] as usize;
            assert!(collider_count == bones_buf[1] as usize);

            let bones_buf_end = 2 + bone_count * 2;
            let (bones_list, []) = bones_buf[2..bones_buf_end].as_chunks::<2>() else {
                panic!("bones_buf length not a multiple of 2");
            };

            let node_count = bone_count + node_count_fix;

            let mut valid_collider_count: usize = 0;
            for bone in bones_list {
                if bone[1] == 99 {
                    continue;
                }

                if valid_collider_count != bone[1] as usize {
                    println!(
                        "Collider ID {} missmatched index {}",
                        bone[1], valid_collider_count
                    );
                }

                let bone_id = bone[0] as usize;
                let collider_id = valid_collider_count;

                assert!(
                    bone_id < node_count,
                    "Bone ID {}, Bone Count {}",
                    bone_id,
                    node_count
                );
                assert!(
                    collider_id < collider_count,
                    "Collider ID {}, Collider Count {}",
                    collider_id,
                    collider_count
                );

                assert!(
                    colliders[collider_id].bone.is_none(),
                    "Collider {} bone already set to: {}, cannot set to: {}",
                    collider_id,
                    colliders[collider_id].bone.unwrap(),
                    bone_id,
                );
                colliders[collider_id].bone = Some(bone[0]);

                valid_collider_count += 1;
            }

            assert!(valid_collider_count == collider_count);

            Some(node_count)
        } else {
            assert!(colliders.len() == 1);

            None
        };

        return Ok(PPD {
            margin: margin,
            colliders: colliders,
            node_count: node_count,
            glbin_path: None,
        });
    }

    pub fn get_node_count(&self) -> Option<usize> {
        self.node_count
    }

    pub fn path_to_glbin(&self, path: &Path) -> Option<PathBuf> {
        let mut gltf_path = PathBuf::from(path);
        gltf_path.set_file_name("");

        return diff_paths(self.glbin_path.as_ref()?, gltf_path);
    }

    pub fn size(&self) -> usize {
        self.colliders
            .iter()
            .fold(0, |size, collider| size + collider.size())
    }

    pub fn write_to_glbin(&mut self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        for collider in self.colliders.iter() {
            collider.write(&mut writer)?;
        }

        self.glbin_path = Some(path.to_path_buf());

        return Ok(());
    }

    pub fn append_to_gltf(
        &self,
        root: &mut Root,
        gltf_path: &Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let glbin_rel_path = if let Some(p) = self.path_to_glbin(gltf_path) {
            p
        } else {
            return Err("Cannot get relative path to hitbox glbin".into());
        };

        let glbin_rel_path_str = glbin_rel_path.to_string_lossy().into_owned();

        let buf_idx = root.push(Buffer {
            name: Some(String::from("Hitbox")),
            uri: Some(glbin_rel_path_str),
            byte_length: USize64::from(self.size()),
            extensions: None,
            extras: Void::default(),
        });

        let mut view_offset: usize = 0;
        for (i, collider) in self.colliders.iter().enumerate() {
            let triangles_view_size = collider.triangles_size();
            let triangles_view = root.push(View {
                name: Some(format!("Collider {} - Triangles", i)),
                buffer: buf_idx,
                byte_length: USize64::from(triangles_view_size),
                byte_offset: Some(USize64::from(view_offset)),
                byte_stride: None,
                extensions: None,
                extras: Extras::default(),
                target: None,
            });
            view_offset += triangles_view_size;

            let layers_view_size = collider.layers_size();
            let layers_view = root.push(View {
                name: Some(format!("Collider {} - Layers", i)),
                buffer: buf_idx,
                byte_length: USize64::from(layers_view_size),
                byte_offset: Some(USize64::from(view_offset)),
                byte_stride: None,
                extensions: None,
                extras: Extras::default(),
                target: None,
            });
            view_offset += layers_view_size;

            let surfaces_view_size = collider.surfaces_size();
            let surfaces_view = root.push(View {
                name: Some(format!("Collider {} - Surfaces", i)),
                buffer: buf_idx,
                byte_length: USize64::from(surfaces_view_size),
                byte_offset: Some(USize64::from(view_offset)),
                byte_stride: None,
                extensions: None,
                extras: Extras::default(),
                target: None,
            });
            view_offset += surfaces_view_size;

            let node = &mut root.nodes[collider.bone.unwrap_or(0) as usize];
            let extensions = node
                .extensions
                .get_or_insert(NodeExtension { others: Map::new() });

            if let &mut Value::Object(ref mut bbtools_ext) = extensions
                .others
                .entry("bbtools")
                .or_insert(Value::Object(Map::new()))
            {
                let hitbox = json!({
                    "triangles": triangles_view,
                    "layers": layers_view,
                    "surfaces": surfaces_view,
                });

                assert!(!bbtools_ext.contains_key("hitbox"));
                bbtools_ext.insert(String::from("hitbox"), hitbox);
            }
        }

        return Ok(());
    }
}

impl fmt::Display for PPD {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(
            f,
            "Margin {:.01}, Part Count {}",
            self.margin,
            self.colliders.len()
        )?;
        for (i, collider) in self.colliders.iter().enumerate() {
            if let Some(bone) = collider.bone {
                writeln!(f, "Collider {:02} - Bone {:02}", i, bone)?;
            } else {
                writeln!(f, "Collider {:02}", i)?;
            }
            write!(f, "{}", collider.aabb)?;
        }
        return Ok(());
    }
}
