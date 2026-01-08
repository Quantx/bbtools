use std::fs::File;
use std::io::BufWriter;
use std::io::Write;
use std::fs;
use std::fmt;
use std::path::Path;

use byteorder::ByteOrder;
use byteorder::LittleEndian;

const DXT1_SIZE: usize = 8;
struct DXT1 {
    color0: u16,
    color1: u16,
    codes: u32,
}

impl From<&[u8; DXT1_SIZE]> for DXT1 {
    fn from(buf: &[u8; DXT1_SIZE]) -> Self {
        DXT1{
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
        DXT3{
            alpha: LittleEndian::read_u64(&buf[0..8]),
            dxt1: DXT1{
                color0: LittleEndian::read_u16(&buf[8..10]),
                color1: LittleEndian::read_u16(&buf[10..12]),
                codes: LittleEndian::read_u32(&buf[12..16]),
            }
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
        
        DXT3{
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
        
        if bit < width  {x |= mask_bit; mask_bit <<= 1; done = false;}
        if bit < height {y |= mask_bit; mask_bit <<= 1; done = false;}
        if bit < depth  {z |= mask_bit; mask_bit <<= 1; done = false;}
        
        bit <<= 1;
        
        if done { break }
    }
    assert!((x ^ y ^ z) == (mask_bit - 1)); /* masks are mutually exclusive */
    
    return (x, y, z);
}

fn unswizzle_box(src_base: &[u8], dst_base: &mut [u8],
    width: u32, height: u32, depth: u32,
    row_pitch: usize, slice_pitch: u32, bytes_per_pixel: usize)
{
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
                
                let src = &src_ptr[src_idx..src_idx+bytes_per_pixel];
                let dst = &mut dst_ptr[dst_idx..dst_idx+bytes_per_pixel];
                
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
            _ => XPRFormat::Unknown,
        }
    }
}

impl XPRFormat {
    fn has_ext_dimensions(&self) -> bool {
        match self {
            XPRFormat::ARGB_LIN => true,
            _ => false
        }
    }
    
    fn is_swizzled(&self) -> bool {
        match self {
            XPRFormat::ARGB => true,
            _ => false
        }
    }
    
    fn bytes_per_pixel(&self) -> usize {
        match self {
            XPRFormat::ARGB => 4,
            XPRFormat::ARGB_LIN => 4,
            XPRFormat::GB_LIN => 2,
            XPRFormat::GB => 2,
            _ => 0,
        }
    }
}

pub struct XPR {
    pub format: XPRFormat,
    levels: usize,
    width: usize,
    height: usize,
    depth: usize,
    
    data: Vec<u8>,
}

impl fmt::Display for XPR {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Levels {}, Width {}, Height {}, Depth {}",
            self.levels, self.width, self.height, self.depth)
    }
}

impl XPR {
    pub fn open(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let bytes = fs::read(path)?;
        match path.extension() {
            None => Err("Missing file extension for XPR".into()),
            Some(os_ext) => match os_ext.to_str() {
                Some("xpr") => XPR::import_xpr(bytes.as_slice()),
                Some("xraw") => XPR::import_xraw(bytes.as_slice()),
                Some(ext) => Err(format!("Unknown file extension for XPR: '{}'", ext).into()),
                None => Err("Failed to decode extension for XPR".into())
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
        
        let mut xpr = XPR{
            format: XPRFormat::from(header_bytes[21]),
            levels: (header_bytes[22] & 0xF) as usize,
            width: 1 << ((header_bytes[22] >> 4) & 0xF),
            height: 1 << (header_bytes[23] & 0xF),
            depth: 1 << ((header_bytes[23] >> 4) & 0xF),
            data: vec![0; file_end - file_start],
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
        
        xpr.data.copy_from_slice(&buf[file_start..file_end]);
        
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
        
        let data_buf = &buf[XRAW_HEAD_SIZE..];
        
        return Ok(XPR{
            format: XPRFormat::from(buf[5]),
            levels: (buf[6] & 0xF) as usize,
            width: 1 << ((buf[6] >> 4) & 0xF),
            height: 1 << (buf[7] & 0xF),
            depth: 1 << ((buf[7] >> 4) & 0xF),
            data: Vec::from(data_buf),
        });
    }
    
    pub fn convert_dxt1_to_dxt3(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.format != XPRFormat::DXT1 {
            return Err("XPR Format not DXT1".into())
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
        
        unswizzle_box(&self.data, &mut unswizzled_data,
            self.width as u32, self.height as u32, 1,
            self.width * bpp, 0, bpp);
        
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
        
        unswizzle_box(&self.data, &mut unswizzled_data,
            self.width as u32, self.height as u32, 1,
            self.width * bpp, 0, bpp);
        
        self.data = unswizzled_data;
        self.format = XPRFormat::GB_LIN;
        
        return Ok(());
    }
    
    pub fn write_to_dds(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        
        writer.write(&DDS_MAGIC)?;
        
        let dds = DDS::from(self);
        writer.write(&Into::<[u8; DDS_SIZE]>::into(dds))?;
        
        writer.write(&self.data)?;
        
        return Ok(());
    }
}

const DDS_PIXELFORMAT_SIZE: usize = 32;
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
    fn new() -> Self {Default::default()}
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
}

impl From<&XPR> for DDS {
    fn from(xpr: &XPR) -> Self {
        let mut dds = DDS{
            flags: 0x1 | 0x2 | 0x4 | 0x1000,
            height: xpr.height as u32,
            width: xpr.width as u32,
            pitch: 0,
            depth: xpr.depth as u32,
            levels: xpr.levels as u32,
            format: DDS_PixelFormat::new(),
            caps: [0; _],
        };
        
        match xpr.format {
            XPRFormat::DXT1 => {
                dds.flags |= 0x80000; // Compressed texture linear size
                dds.format.flags |= 0x4; // Compressed Texture
                dds.format.code = *b"DXT1";
                
                let block_w = (xpr.width + 3) / 4;
                let block_h = (xpr.height + 3) / 4;
                
                dds.pitch = (block_w * block_h * DXT1_SIZE) as u32;
            },
            XPRFormat::DXT3 => {
                dds.flags |= 0x80000; // Compressed texture linear size
                dds.format.flags |= 0x4; // Compressed Texture
                dds.format.code = *b"DXT3";
                
                let block_w = (xpr.width + 3) / 4;
                let block_h = (xpr.height + 3) / 4;
                
                dds.pitch = (block_w * block_h * DXT3_SIZE) as u32;
            },
            XPRFormat::ARGB_LIN => {
                dds.flags |= 0x8; // Uncompressed texture pitch
                dds.format.flags |= 0x1 | 0x40; // Alpha Channel | Uncompressed Texture
                
                dds.format.rgb_bit_count = 32;
                dds.format.a_bit_mask = 0xFF000000;
                dds.format.r_bit_mask = 0x00FF0000;
                dds.format.g_bit_mask = 0x0000FF00;
                dds.format.b_bit_mask = 0x000000FF;
                
                dds.pitch = ((xpr.width * xpr.height * dds.format.rgb_bit_count as usize + 7) / 8) as u32;
            },
            XPRFormat::GB_LIN => {
                dds.flags |= 0x8; // Uncompressed texture pitch
                dds.format.flags |= 0x40; // Uncompressed Texture
                
                dds.format.rgb_bit_count = 16;
                dds.format.g_bit_mask = 0xFF00;
                dds.format.b_bit_mask = 0x00FF;
                
                dds.pitch = ((xpr.width * xpr.height * dds.format.rgb_bit_count as usize + 7) / 8) as u32;
            }
            _ => {},
        };
        
        return dds;
    }
}

impl Into<[u8; DDS_SIZE]> for DDS {
    fn into(self) -> [u8; DDS_SIZE] {
        let mut buf: [u8; DDS_SIZE] = [0; _];
        
        // Serialize DDS struct
        LittleEndian::write_u32(&mut buf[0..4], DDS_SIZE as u32);
        LittleEndian::write_u32(&mut buf[4..8], self.flags);
        LittleEndian::write_u32(&mut buf[8..12], self.height);
        LittleEndian::write_u32(&mut buf[12..16], self.width);
        LittleEndian::write_u32(&mut buf[16..20], self.pitch);
        LittleEndian::write_u32(&mut buf[20..24], self.depth);
        LittleEndian::write_u32(&mut buf[24..28], self.levels);
        
        // Serialize DDS_PixelFormat struct
        let format = self.format;
        LittleEndian::write_u32(&mut buf[72..76], DDS_PIXELFORMAT_SIZE as u32);
        LittleEndian::write_u32(&mut buf[76..80], format.flags);
        buf[80..84].copy_from_slice(&format.code);
        LittleEndian::write_u32(&mut buf[84..88], format.rgb_bit_count);
        LittleEndian::write_u32(&mut buf[88..92], format.r_bit_mask);
        LittleEndian::write_u32(&mut buf[92..96], format.g_bit_mask);
        LittleEndian::write_u32(&mut buf[96..100], format.b_bit_mask);
        LittleEndian::write_u32(&mut buf[100..104], format.a_bit_mask);
        
        // Serialize rest of DDS struct
        LittleEndian::write_u32(&mut buf[104..108], self.caps[0]);
        LittleEndian::write_u32(&mut buf[108..112], self.caps[1]);
        LittleEndian::write_u32(&mut buf[112..116], self.caps[2]);
        LittleEndian::write_u32(&mut buf[116..120], self.caps[3]);
        
        return buf;
    }
}
