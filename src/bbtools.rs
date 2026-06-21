pub mod bin;
pub mod eff;
pub mod lmt;
pub mod lsq;
pub mod obj;
pub mod ppd;
pub mod stg;
pub mod text;
pub mod x86;
pub mod xact;
pub mod xbe;
pub mod xbo;
pub mod xpr;

use byteorder::{ByteOrder, LittleEndian, WriteBytesExt};
use glam::f32::Vec3A as Vec3; // Vec3A is 16-bytes so that it can function with SIMD
use std::f32;
use std::fmt;
use std::io::{SeekFrom, Write};
use std::ops::Range;
use std::path::{Path, PathBuf};

pub type FnIndexToPath = fn(usize) -> PathBuf;

pub fn write_pascal_string(pstr: &str, writer: &mut impl Write) -> Result<(), std::io::Error> {
    let bytes = pstr.as_bytes();
    writer.write_u32::<LittleEndian>(bytes.len() as u32)?;
    return writer.write_all(&bytes);
}

pub fn write_pascal_option_string(
    pstr_option: Option<&str>,
    writer: &mut impl Write,
) -> Result<(), std::io::Error> {
    write_pascal_string(pstr_option.unwrap_or(""), writer)
}

pub fn write_godot_path(path: &Path, writer: &mut impl Write) -> Result<(), std::io::Error> {
    let path_str = path.to_str().unwrap();
    let path_string = if path.has_root() {
        "res://".to_string() + path_str
    } else {
        path_str.to_string()
    };
    write_pascal_string(&path_string, writer)
}

pub fn write_godot_option_path(
    path_option: Option<&Path>,
    writer: &mut impl Write,
) -> Result<(), std::io::Error> {
    if let Some(path) = path_option {
        write_godot_path(path, writer)
    } else {
        writer.write_u32::<LittleEndian>(0)
    }
}

// Sin wave approximation of HSL to RGB as reverse engineered from: LOC@0x0004bed0
pub fn hsl_to_rgb(hsl: Vec3) -> Vec3 {
    let mut rgb = ((hsl.x + Vec3::new(0.0, 1.0 / 3.0, 1.0 / 1.5)) * f32::consts::TAU).sin();

    let s = hsl.y * 65.0;
    rgb *= s;
    rgb += s;

    let avg = rgb.element_sum() / 3.0;
    let l = 1.0 - (hsl.z * 0.5);

    rgb += (avg - rgb) * l;

    (rgb / 255.0).clamp(Vec3::ZERO, Vec3::ONE)
}

const FILE_VEC_SIZE: usize = 8;

#[derive(Default)]
struct FileSlice {
    offset: usize,
    length: usize,
}

impl Into<SeekFrom> for &FileSlice {
    fn into(self) -> SeekFrom {
        SeekFrom::Start(self.offset as u64)
    }
}

impl From<&[u8; 8]> for FileSlice {
    fn from(buf: &[u8; 8]) -> Self {
        FileSlice {
            offset: LittleEndian::read_u32(&buf[0..4]) as usize,
            length: LittleEndian::read_u32(&buf[4..8]) as usize,
        }
    }
}

impl From<&[u8]> for FileSlice {
    fn from(buf: &[u8]) -> Self {
        assert!(buf.len() == 8);
        FileSlice {
            offset: LittleEndian::read_u32(&buf[0..4]) as usize,
            length: LittleEndian::read_u32(&buf[4..8]) as usize,
        }
    }
}

impl FileSlice {
    fn get_end(&self) -> usize {
        return self.offset + self.length;
    }

    fn set_end(&mut self, end: usize) {
        if end >= self.offset {
            self.length = end - self.offset;
        }
    }

    fn as_range(&self) -> Range<usize> {
        return self.offset..(self.offset + self.length);
    }

    fn buffer_size(file_count: u32) -> usize {
        return file_count as usize * FILE_VEC_SIZE;
    }

    fn from_bytes(buf: &[u8]) -> Vec<FileSlice> {
        let (file_list, []) = buf.as_chunks::<FILE_VEC_SIZE>() else {
            panic!("file_list_bytes length not a multiple of FILE_VEC_SIZE");
        };

        let mut files: Vec<FileSlice> = Vec::with_capacity(file_list.len());

        for file_bytes in file_list {
            files.push(FileSlice::from(file_bytes));
        }

        return files;
    }
}

impl fmt::Display for FileSlice {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "[{:08X}..{:08X}|{}]",
            self.offset,
            self.get_end(),
            self.length
        )
    }
}
