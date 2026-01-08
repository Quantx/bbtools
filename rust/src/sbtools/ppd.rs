use std::fmt;
use std::path::Path;

use byteorder::ByteOrder;
use byteorder::LittleEndian;

use glam::f32::Vec3A as Vec3; // Vec3A is 16-bytes so that it can function with SIMD
use glam::i32::IVec3;
use glam::u16::U16Vec3;

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
                    let mut aabb = AABB::new(&base_buf[offset..offset+AABB_SIZE], scale, level);
                    aabb.children.import(base_buf, offset + AABB_SIZE, scale, level + 1);
                    
                    let next_offset = aabb.next_offset; 
                    children.push(aabb);
                    
                    if next_offset == 0 {break}
                    offset += next_offset;
                }
            },
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
    flags: u64,
    vertices: [Vec3; 4],
    normal: Vec3,
    level: usize
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
        
        let _zero0 = LittleEndian::read_u32(&buf[52..56]);
        let flags = LittleEndian::read_u64(&buf[56..64]);
        
        let normal = Vec3::new(
            LittleEndian::read_f32(&buf[64..68]),
            LittleEndian::read_f32(&buf[68..72]),
            LittleEndian::read_f32(&buf[72..76]),
        ).normalize();
        
        let _zero0 = LittleEndian::read_u32(&buf[76..80]);
        
        return Quad{
            flags: flags,
            vertices: vertices,
            normal: normal,
            level: level,
        };
    }
}

impl fmt::Display for Quad {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let indent = self.level * 2;
        
        writeln!(f, "{:indent$}Quad - Flags {:016X}, Normal {}", "", self.flags, self.normal, indent=indent)?;
        
        for (i, vertex) in self.vertices.iter().enumerate() {
            writeln!(f, "{:indent$}  Vertex {} {}", "", i, vertex, indent=indent)?;
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
        ).as_vec3a() * scale;
        
        let extents = U16Vec3::new(
            LittleEndian::read_u16(&buf[12..14]),
            LittleEndian::read_u16(&buf[14..16]),
            LittleEndian::read_u16(&buf[16..18]),
        ).as_vec3a() * scale;
        
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
        
        return AABB{
            min: corner_a.min(corner_b),
            max: corner_a.max(corner_b),
            children: children,
            next_offset: next_offset,
            level: level,
        };
    }

    pub fn import_tree(base_buf: &[u8], first_header_offset: usize, scale : f32) -> AABB {
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
        
        return AABB{
            min: min,
            max: max,
            children: children_type,
            next_offset: 0,
            level: 0,
        };
    }
}

impl fmt::Display for AABB {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let indent = self.level * 2;
        writeln!(f, "{:indent$}AABB - Min {}, Max {}", "", self.min, self.max, indent=indent)?;
        match &self.children {
            ChildType::AABB(children) => {
                for child in children {
                    write!(f, "{}", child)?;
                }
            },
            ChildType::Quad(quads) => {
                for quad in quads {
                    write!(f, "{}", quad)?;
                }
            },
        }
        return Ok(());
    }
}

const PPD_HEADER_SIZE: usize = 16;
pub struct PPD {
    margin: f32,
    parts: Vec<AABB>,
}

impl PPD {
    pub fn import(buf: &[u8], bone_names_opt: Option<Vec<String>>) -> Result<Self, Box<dyn std::error::Error>> {
        Self::import_scaled(buf, 1.0, bone_names_opt)
    }
    
    pub fn import_scaled(buf: &[u8], scale: f32, bone_names_opt: Option<Vec<String>>) -> Result<Self, Box<dyn std::error::Error>> {
        // No MAGIC for this file type unfortunately
        let margin = LittleEndian::read_f32(&buf[0..4]) as f32 * scale;
        let part_count = LittleEndian::read_u32(&buf[4..8]) as usize;
        
        // &buf[8..16] is always zero
        
        let part_buf = &buf[PPD_HEADER_SIZE..];
        
        let mut part_offsets: Vec<u32> = vec![0; part_count];
        let part_offsets_end = part_count * 4;
        LittleEndian::read_u32_into(&part_buf[..part_offsets_end], &mut part_offsets);
        
        let bone_offset = LittleEndian::read_u32(&part_buf[part_offsets_end..(part_offsets_end + 4)]) as usize;
        
        let mut parts: Vec<AABB> = Vec::with_capacity(part_count);
        for part_offset in part_offsets {
            let aabb = AABB::import_tree(&part_buf, part_offset as usize, scale);
            parts.push(aabb);
        }
        
        if bone_offset > 0 {
            let bone_names = if let Some(bn) = bone_names_opt {bn} else {
                return Err("Bone offset present, but no bone names provided".into());
            };
        
            let bone_buf = &buf[PPD_HEADER_SIZE+bone_offset..];
            
            let bone_count = bone_buf[0] as usize;
            let bone_parts = bone_buf[1] as usize;
            assert!(bone_parts == part_count);
            
            if bone_count + 1 != bone_names.len() {
                return Err(format!("Bone count {} does not match bone name count {}",
                    bone_count + 1, bone_names.len()).into());
            }
            
            let bone_buf_end = 2 + bone_count * 2;
            let (bone_list, []) = buf[2..bone_buf_end].as_chunks::<2>() else {
                panic!("bone_buf length not a multiple of 2");
            };
            
            for bone in bone_list {
                let bone_id = bone[0] as usize;
                let part_id = bone[1] as usize;
                
                if part_id == 99 {continue};
                assert!(bone_id < bone_count + 1); // There's an extra bone including the root one
                assert!(part_id < part_count);
                
                
            }
        }
        
        return Ok(PPD{
            margin: margin,
            parts: parts,
        });
    }
    
    pub fn write_to_hbx(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        
    
        return Ok(());
    }
}

impl fmt::Display for PPD {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "Margin {:.01}, Part Count {}", self.margin, self.parts.len())?;
        for (i, part) in self.parts.iter().enumerate() {
            writeln!(f, "Part {:02}", i)?;
            write!(f, "{}", part)?;
        }
        return Ok(())
    }
}
