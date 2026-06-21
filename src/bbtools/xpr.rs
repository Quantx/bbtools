use std::cmp;
use std::fmt;
use std::fs;
use std::fs::File;
use std::io::BufWriter;
use std::io::Write;
use std::path::{Path, PathBuf};

use byteorder::{ByteOrder, LittleEndian, WriteBytesExt};
use pathdiff::diff_paths;

const DXT1_SIZE: usize = 8;
struct DXT1 {
    color0: u16,
    color1: u16,
    codes: u32,
}

impl From<&[u8; DXT1_SIZE]> for DXT1 {
    fn from(buf: &[u8; DXT1_SIZE]) -> Self {
        DXT1 {
            color0: LittleEndian::read_u16(&buf[0..2]),
            color1: LittleEndian::read_u16(&buf[2..4]),
            codes: LittleEndian::read_u32(&buf[4..8]),
        }
    }
}

impl Into<[u8; DXT1_SIZE]> for DXT1 {
    fn into(self) -> [u8; DXT1_SIZE] {
        let mut buf: [u8; DXT1_SIZE] = [0; _];
        LittleEndian::write_u16(&mut buf[0..2], self.color0);
        LittleEndian::write_u16(&mut buf[2..4], self.color1);
        LittleEndian::write_u32(&mut buf[4..8], self.codes);
        return buf;
    }
}

const DXT3_SIZE: usize = 16;
struct DXT3 {
    alpha: u64,
    dxt1: DXT1,
}

impl From<&[u8; DXT3_SIZE]> for DXT3 {
    fn from(buf: &[u8; DXT3_SIZE]) -> Self {
        DXT3 {
            alpha: LittleEndian::read_u64(&buf[0..8]),
            dxt1: DXT1 {
                color0: LittleEndian::read_u16(&buf[8..10]),
                color1: LittleEndian::read_u16(&buf[10..12]),
                codes: LittleEndian::read_u32(&buf[12..16]),
            },
        }
    }
}

impl Into<[u8; DXT3_SIZE]> for DXT3 {
    fn into(self) -> [u8; DXT3_SIZE] {
        let mut buf: [u8; DXT3_SIZE] = [0; _];
        LittleEndian::write_u64(&mut buf[0..8], self.alpha);
        buf[8..16].copy_from_slice(&Into::<[u8; DXT1_SIZE]>::into(self.dxt1));
        return buf;
    }
}

impl From<DXT1> for DXT3 {
    fn from(dxt1: DXT1) -> Self {
        let mut alpha = u64::MAX;
        if dxt1.color0 <= dxt1.color1 {
            alpha = 0;
            for i in 0..16 {
                let c = i * 2;
                if ((dxt1.codes >> c) & 3) != 3 {
                    let a = i * 4;
                    alpha |= 0xFu64 << a;
                }
            }
        }

        DXT3 {
            alpha: alpha,
            dxt1: dxt1,
        }
    }
}

// Ported from: https://github.com/xemu-project/xemu/blob/master/hw/xbox/nv2a/pgraph/swizzle.c
fn generate_swizzle_masks(width: u32, height: u32, depth: u32) -> (u32, u32, u32) {
    let mut x = 0u32;
    let mut y = 0u32;
    let mut z = 0u32;

    let mut bit = 1u32;
    let mut mask_bit = 1u32;

    loop {
        let mut done = true;

        if bit < width {
            x |= mask_bit;
            mask_bit <<= 1;
            done = false;
        }
        if bit < height {
            y |= mask_bit;
            mask_bit <<= 1;
            done = false;
        }
        if bit < depth {
            z |= mask_bit;
            mask_bit <<= 1;
            done = false;
        }

        bit <<= 1;

        if done {
            break;
        }
    }
    assert!((x ^ y ^ z) == (mask_bit - 1)); // masks are mutually exclusive

    return (x, y, z);
}

fn unswizzle_box(
    src_base: &[u8],
    dst_base: &mut [u8],
    width: u32,
    height: u32,
    depth: u32,
    row_pitch: usize,
    slice_pitch: u32,
    bytes_per_pixel: usize,
) {
    let (mask_x, mask_y, mask_z) = generate_swizzle_masks(width, height, depth);

    let mut off_z = 0u32;
    for z in 0..depth {
        let mut off_y = 0u32;
        for y in 0..height {
            let mut off_x = 0u32;

            let src_offset = (off_y + off_z) as usize * bytes_per_pixel;
            let dst_offset = (z * slice_pitch + y) as usize * row_pitch;

            let src_ptr = &src_base[src_offset..];
            let dst_ptr = &mut dst_base[dst_offset..];

            for x in 0..width {
                let src_idx = off_x as usize * bytes_per_pixel;
                let dst_idx = x as usize * bytes_per_pixel;

                let src = &src_ptr[src_idx..src_idx + bytes_per_pixel];
                let dst = &mut dst_ptr[dst_idx..dst_idx + bytes_per_pixel];

                dst.copy_from_slice(&src);

                off_x = off_x.wrapping_sub(mask_x) & mask_x;
            }
            off_y = off_y.wrapping_sub(mask_y) & mask_y;
        }
        off_z = off_z.wrapping_sub(mask_z) & mask_z;
    }
}

const DDS_MAGIC: [u8; 4] = *b"DDS ";
const XPR_MAGIC: [u8; 4] = *b"XPR0";
const XRAW_MAGIC: [u8; 4] = *b"XRAW";

const XPR_HEADER_SIZE: usize = 36;
const XRAW_HEAD_SIZE: usize = 16;

// These correspond to the XBOX format: X_D3DFMT_WHATEVER
#[derive(PartialEq)]
#[expect(non_camel_case_types)]
pub enum XPRFormat {
    Unknown,

    ARGB,
    DXT1,
    DXT3,
    ARGB_LIN,
    GB_LIN,
    GB,
    L16_LIN,
}

impl From<u8> for XPRFormat {
    fn from(format: u8) -> Self {
        match format {
            0x06 => XPRFormat::ARGB,
            0x0C => XPRFormat::DXT1,
            0x0E => XPRFormat::DXT3,
            0x12 => XPRFormat::ARGB_LIN,
            0x17 => XPRFormat::GB_LIN,
            0x28 => XPRFormat::GB,
            0x35 => XPRFormat::L16_LIN,
            _ => XPRFormat::Unknown,
        }
    }
}

impl XPRFormat {
    pub fn has_ext_dimensions(&self) -> bool {
        match self {
            XPRFormat::ARGB_LIN => true,
            _ => false,
        }
    }

    pub fn is_swizzled(&self) -> bool {
        match self {
            XPRFormat::ARGB | XPRFormat::GB => true,
            _ => false,
        }
    }

    pub fn bytes_per_pixel(&self) -> usize {
        match self {
            XPRFormat::ARGB_LIN | XPRFormat::ARGB => 4,
            XPRFormat::GB_LIN | XPRFormat::GB => 2,
            _ => 0,
        }
    }
}

pub struct XPR {
    pub format: XPRFormat,
    pub levels: usize, // MipMaps
    pub width: usize,  // X
    pub height: usize, // Y
    pub depth: usize,  // Z
    pub layers: usize, // ArraySize

    data: Vec<u8>,

    dds_path: Option<PathBuf>,
}

impl fmt::Display for XPR {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Levels {}, Width {}, Height {}, Depth {}",
            self.levels, self.width, self.height, self.depth
        )
    }
}

impl XPR {
    pub fn open(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let bytes = fs::read(path)?;
        match path.extension() {
            None => Err("Missing file extension for XPR".into()),
            Some(os_ext) => match os_ext.to_str() {
                Some("xpr") => XPR::import_xpr(bytes.as_slice()),
                Some("raw") => XPR::import_xraw(bytes.as_slice()),
                Some(ext) => Err(format!("Unknown file extension for XPR: '{}'", ext).into()),
                None => Err("Failed to decode extension for XPR".into()),
            },
        }
    }

    pub fn import_xpr(buf: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        if buf.len() < XPR_HEADER_SIZE {
            return Err("MissingHeader".into());
        }

        let magic = &buf[0..4];
        if magic != XPR_MAGIC {
            return Err("BadMagic".into());
        }

        let header_bytes = &buf[4..XPR_HEADER_SIZE];

        let file_end = LittleEndian::read_u32(&header_bytes[0..4]) as usize;
        let file_start = LittleEndian::read_u32(&header_bytes[4..8]) as usize;

        if file_end <= file_start {
            return Err("FileEmpty".into());
        }

        if buf.len() < file_end as usize {
            return Err("BufTooSmall".into());
        }

        let xpr_class = LittleEndian::read_u16(&header_bytes[10..12]);
        if xpr_class != 4 {
            return Err("XPR does not contain a texture".into());
        }

        let dimension_count = (header_bytes[20] >> 4) & 0xF;
        if dimension_count != 2 {
            return Err("XPR dimension count is not 2".into());
        }

        let mut xpr = XPR {
            format: XPRFormat::from(header_bytes[21]),
            levels: (header_bytes[22] & 0xF) as usize,
            width: 1 << ((header_bytes[22] >> 4) & 0xF),
            height: 1 << (header_bytes[23] & 0xF),
            depth: 1 << ((header_bytes[23] >> 4) & 0xF),
            layers: 1,
            data: Vec::from(&buf[file_start..file_end]),
            dds_path: None,
        };

        if xpr.format == XPRFormat::Unknown {
            return Err("XPR unknown format".into());
        }

        if xpr.format.has_ext_dimensions() {
            let ext_dimension = LittleEndian::read_u32(&header_bytes[24..28]);

            xpr.width = (ext_dimension & 0xFFF) as usize + 1;
            xpr.height = ((ext_dimension >> 12) & 0xFFF) as usize + 1;
            xpr.depth = ((ext_dimension >> 24) & 0xFFF) as usize + 1;
        }

        return Ok(xpr);
    }

    pub fn import_xraw(buf: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        if buf.len() < XRAW_HEAD_SIZE {
            return Err("MissingHeader".into());
        }

        let magic = &buf[0..4];
        if magic != XRAW_MAGIC {
            return Err("BadMagic".into());
        }

        let dimension_count = (buf[4] >> 4) & 0xF;
        if dimension_count != 2 {
            return Err("XRAW dimension count is not 2".into());
        }

        let width = 1 << ((buf[6] >> 4) & 0xF);
        let height = 1 << (buf[7] & 0xF);

        let top_level_end = XRAW_HEAD_SIZE + width * height * 2;
        let data_buf = &buf[XRAW_HEAD_SIZE..top_level_end];

        return Ok(XPR {
            format: XPRFormat::from(buf[5]),
            levels: 1, // (buf[6] & 0xF) as usize, // Force disable mipmaps
            width,
            height,
            depth: 1 << ((buf[7] >> 4) & 0xF),
            layers: 1,
            data: Vec::from(data_buf),
            dds_path: None,
        });
    }

    pub fn new_argb_lin(width: usize, height: usize, data: Vec<u8>) -> Self {
        return XPR {
            format: XPRFormat::ARGB_LIN,
            levels: 1,
            width,
            height,
            depth: 1,
            layers: 1,
            data,
            dds_path: None,
        };
    }

    pub fn new_l16_lin(width: usize, height: usize, values: Vec<u16>) -> Self {
        let mut data: Vec<u8> = vec![0; values.len() * 2];
        LittleEndian::write_u16_into(&values, &mut data);

        return XPR {
            format: XPRFormat::L16_LIN,
            levels: 1,
            width,
            height,
            depth: 1,
            layers: 1,
            data,
            dds_path: None,
        };
    }

    pub fn get_pitch(&self) -> usize {
        match self.format {
            XPRFormat::DXT1 => {
                let block_w = cmp::max(1, (self.width + 3) / 4);
                let block_h = cmp::max(1, (self.height + 3) / 4);

                block_w * block_h * DXT1_SIZE
            }
            XPRFormat::DXT3 => {
                let block_w = cmp::max(1, (self.width + 3) / 4);
                let block_h = cmp::max(1, (self.height + 3) / 4);

                block_w * block_h * DXT3_SIZE
            }
            XPRFormat::ARGB_LIN => (self.width * self.height * 32 as usize + 7) / 8,
            XPRFormat::GB_LIN | XPRFormat::L16_LIN => {
                (self.width * self.height * 16 as usize + 7) / 8
            }
            _ => todo!("Format not yet supported"),
        }
    }

    pub fn discard_top_mipmap(&mut self) {
        if self.levels <= 1 {
            return;
        }

        if self.depth > 1 {
            return;
        }

        if self.layers > 1 {
            return;
        }

        let pitch = self.get_pitch();

        drop(self.data.drain(0..pitch));
        self.levels -= 1;
        self.width /= 2;
        self.height /= 2;
    }

    pub fn discard_mipmaps(&mut self) {
        if self.levels <= 1 {
            return;
        }

        if self.depth > 1 {
            return;
        }

        if self.layers > 1 {
            return;
        }

        let pitch = self.get_pitch();

        self.data.truncate(pitch);
        self.levels = 1;
    }

    pub fn extend_layers(&mut self, layer: XPR) -> Result<(), Box<dyn std::error::Error>> {
        if self.width != layer.width {
            return Err("XPR width missmatch".into());
        }

        if self.height != layer.height {
            return Err("XPR height missmatch".into());
        }

        if self.levels != layer.levels {
            return Err("XPR levels missmatch".into());
        }

        if self.depth != layer.depth {
            return Err("XPR depth missmatch".into());
        }

        self.layers += layer.layers;

        self.data.extend(layer.data);

        return Ok(());
    }

    pub fn convert_dxt1_to_dxt3(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.format != XPRFormat::DXT1 {
            return Err("XPR Format not DXT1".into());
        }

        let (dxt1_list_bytes, []) = self.data.as_chunks::<DXT1_SIZE>() else {
            return Err("XPR data length not a multiple of DXT1".into());
        };

        let mut dxt3_list_bytes: Vec<[u8; DXT3_SIZE]> = Vec::with_capacity(dxt1_list_bytes.len());

        for dxt1_bytes in dxt1_list_bytes.into_iter() {
            let dxt1 = DXT1::from(dxt1_bytes);
            let dxt3 = DXT3::from(dxt1);
            dxt3_list_bytes.push(dxt3.into());
        }

        self.data = dxt3_list_bytes.into_flattened();
        self.format = XPRFormat::DXT3;

        return Ok(());
    }

    pub fn convert_argb_to_argb_lin(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.format != XPRFormat::ARGB {
            return Err("XPR Format not ARGB".into());
        }

        let mut unswizzled_data: Vec<u8> = vec![0; self.data.len()];

        let bpp = self.format.bytes_per_pixel();

        unswizzle_box(
            &self.data,
            &mut unswizzled_data,
            self.width as u32,
            self.height as u32,
            1,
            self.width * bpp,
            0,
            bpp,
        );

        self.data = unswizzled_data;
        self.format = XPRFormat::ARGB_LIN;

        return Ok(());
    }

    pub fn convert_gb_to_gb_lin(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.format != XPRFormat::GB {
            return Err("XPR Format not GB".into());
        }

        let mut unswizzled_data: Vec<u8> = vec![0; self.data.len()];

        let bpp = self.format.bytes_per_pixel();

        unswizzle_box(
            &self.data,
            &mut unswizzled_data,
            self.width as u32,
            self.height as u32,
            1,
            self.width * bpp,
            0,
            bpp,
        );

        self.data = unswizzled_data;
        self.format = XPRFormat::GB_LIN;

        return Ok(());
    }

    pub fn path_to_dds(&self, path: &Path) -> Option<PathBuf> {
        let mut from_path = path.to_path_buf();
        if from_path.extension().is_some() {
            from_path.pop();
        }

        return diff_paths(self.dds_path.as_ref()?, from_path);
    }

    pub fn write_to_dds(&mut self, path: &Path) -> Result<(), std::io::Error> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        writer.write_all(&DDS_MAGIC)?;

        let dds = DDS::from(&*self);
        dds.write(&mut writer)?;

        writer.write_all(&self.data)?;

        self.dds_path = Some(path.to_owned());

        return Ok(());
    }
}

const DDS_PIXELFORMAT_SIZE: usize = 32;
const DDS_4CC_DX10: [u8; 4] = *b"DX10";
#[expect(non_camel_case_types)]
#[derive(Default)]
struct DDS_PixelFormat {
    // size: u32 == DDS_PIXELFORMAT_SIZE
    flags: u32,
    code: [u8; 4],
    rgb_bit_count: u32,
    r_bit_mask: u32,
    g_bit_mask: u32,
    b_bit_mask: u32,
    a_bit_mask: u32,
}

impl DDS_PixelFormat {
    pub fn write(&self, writer: &mut impl Write) -> Result<(), std::io::Error> {
        writer.write_u32::<LittleEndian>(DDS_PIXELFORMAT_SIZE as u32)?;
        writer.write_u32::<LittleEndian>(self.flags)?;
        writer.write_all(self.code.as_slice())?;
        writer.write_u32::<LittleEndian>(self.rgb_bit_count)?;
        writer.write_u32::<LittleEndian>(self.r_bit_mask)?;
        writer.write_u32::<LittleEndian>(self.g_bit_mask)?;
        writer.write_u32::<LittleEndian>(self.b_bit_mask)?;
        writer.write_u32::<LittleEndian>(self.a_bit_mask)?;

        return Ok(());
    }
}

#[expect(non_camel_case_types)]
struct DDS_dx10 {
    dxgi_format: u32,
    resource_dimension: u32,
    misc_flags: [u32; 2],
    array_size: u32,
}

impl DDS_dx10 {
    pub fn write(&self, writer: &mut impl Write) -> Result<(), std::io::Error> {
        writer.write_u32::<LittleEndian>(self.dxgi_format)?;
        writer.write_u32::<LittleEndian>(self.resource_dimension)?;
        writer.write_u32::<LittleEndian>(self.misc_flags[0])?;
        writer.write_u32::<LittleEndian>(self.array_size)?;
        writer.write_u32::<LittleEndian>(self.misc_flags[1])?;
        return Ok(());
    }
}

const DDS_SIZE: usize = 124;
struct DDS {
    // size: u32 == DDS_SIZE
    flags: u32,
    height: u32,
    width: u32,
    pitch: u32,
    depth: u32,
    levels: u32,
    // 11 null-words
    format: DDS_PixelFormat,
    caps: [u32; 4],
    // 4 null-words
    dx10: DDS_dx10,
}

impl From<&XPR> for DDS {
    fn from(xpr: &XPR) -> Self {
        let mut dds = DDS {
            flags: 0x1 | 0x2 | 0x4 | 0x1000,
            height: xpr.height as u32,
            width: xpr.width as u32,
            pitch: 0,
            depth: xpr.depth as u32,
            levels: xpr.levels as u32,
            format: DDS_PixelFormat::default(),
            caps: [0x1000, 0, 0, 0],
            dx10: DDS_dx10 {
                dxgi_format: 0,
                resource_dimension: 3,
                misc_flags: [0, 0],
                array_size: xpr.layers as u32,
            },
        };

        if dds.depth > 1 || dds.levels > 1 {
            dds.caps[0] |= 0x8; // Mark this as a complex texture
        }

        if dds.levels > 1 {
            // Mark this as a mipmap texture
            dds.caps[0] |= 0x400000;
            // dds.flags |= 0x20000; Don't enable this due to a bug in Godot
        }

        if dds.depth > 1 {
            // Mark this as a 3D volume texture
            dds.caps[1] |= 0x200000;
            dds.flags |= 0x800000;
        }

        // Set 4CC code
        dds.format.code = DDS_4CC_DX10;
        dds.format.flags |= 0x4;

        match xpr.format {
            XPRFormat::DXT1 => {
                dds.flags |= 0x80000; // Compressed texture pitch
                dds.dx10.dxgi_format = 71; // DXGI_FORMAT_BC1_UNORM

                let block_w = cmp::max(1, (xpr.width + 3) / 4);
                let block_h = cmp::max(1, (xpr.height + 3) / 4);

                dds.pitch = (block_w * block_h * DXT1_SIZE) as u32;
            }
            XPRFormat::DXT3 => {
                dds.flags |= 0x80000; // Compressed texture pitch
                dds.dx10.dxgi_format = 74; // DXGI_FORMAT_BC2_UNORM

                let block_w = cmp::max(1, (xpr.width + 3) / 4);
                let block_h = cmp::max(1, (xpr.height + 3) / 4);

                dds.pitch = (block_w * block_h * DXT3_SIZE) as u32;
            }
            XPRFormat::ARGB_LIN => {
                dds.flags |= 0x8; // Uncompressed texture pitch
                dds.format.flags |= 0x1 | 0x40; // Alpha Channel | Uncompressed Texture
                dds.dx10.dxgi_format = 87; // DXGI_FORMAT_B8G8R8A8_UNORM

                dds.format.rgb_bit_count = 32;
                dds.format.a_bit_mask = 0xFF000000;
                dds.format.r_bit_mask = 0x00FF0000;
                dds.format.g_bit_mask = 0x0000FF00;
                dds.format.b_bit_mask = 0x000000FF;

                dds.pitch =
                    ((xpr.width * xpr.height * dds.format.rgb_bit_count as usize + 7) / 8) as u32;
            }
            XPRFormat::GB_LIN => {
                dds.flags |= 0x8; // Uncompressed texture pitch
                dds.format.flags |= 0x40; // Uncompressed Texture
                dds.dx10.dxgi_format = 49; // DXGI_FORMAT_R8G8_UNORM

                dds.format.rgb_bit_count = 16;
                dds.format.g_bit_mask = 0xFF00;
                dds.format.r_bit_mask = 0x00FF;

                dds.pitch =
                    ((xpr.width * xpr.height * dds.format.rgb_bit_count as usize + 7) / 8) as u32;
            }
            XPRFormat::L16_LIN => {
                dds.flags |= 0x8; // Uncompressed texture pitch
                dds.format.flags |= 0x40; // Uncompressed Texture
                dds.dx10.dxgi_format = 57; // DXGI_FORMAT_R16_UINT

                dds.format.rgb_bit_count = 16;
                dds.format.r_bit_mask = 0xFFFF;

                dds.pitch =
                    ((xpr.width * xpr.height * dds.format.rgb_bit_count as usize + 7) / 8) as u32;
            }
            _ => todo!("Cannot convert unknown XPR format to DDS"),
        };

        return dds;
    }
}

impl DDS {
    pub fn write(&self, writer: &mut impl Write) -> Result<(), std::io::Error> {
        writer.write_u32::<LittleEndian>(DDS_SIZE as u32)?;
        writer.write_u32::<LittleEndian>(self.flags)?;
        writer.write_u32::<LittleEndian>(self.height)?;
        writer.write_u32::<LittleEndian>(self.width)?;
        writer.write_u32::<LittleEndian>(self.pitch)?;
        writer.write_u32::<LittleEndian>(self.depth)?;
        writer.write_u32::<LittleEndian>(self.levels)?;

        for _ in 0..11 {
            // Write reserved
            writer.write_u32::<LittleEndian>(0)?;
        }

        self.format.write(writer)?;

        for cap in self.caps {
            writer.write_u32::<LittleEndian>(cap)?;
        }
        writer.write_u32::<LittleEndian>(0)?; // Write reserved 2

        if self.format.code == DDS_4CC_DX10 {
            self.dx10.write(writer)?;
        }

        return Ok(());
    }
}
