use std::fs::File;
use std::path::Path;
use std::ffi::CString;
use std::collections::HashMap;
use std::io::SeekFrom;
use std::io::BufReader;
use std::io::Read;
use std::io::Write;
use std::io::Seek;
use std::io::BufRead;

use byteorder::ByteOrder;
use byteorder::LittleEndian;

const XBE_MAGIC: [u8; 4] = *b"XBEH";
const XBE_SECTION_SIZE: usize = 56;

struct Section {
    // Virtual Address
    vaddr: u32,
    vsize: u32,
    // File Address
    faddr: u32,
    fsize: u32,
    // Section SHA-1 checksum
    checksum: [u8; 20],

    // Data
    data: Option<Vec<u8>>,
}

pub struct XBE {
    reader: BufReader<File>,
    base_address: u32,
    pub title_id: u32,
    sections: HashMap<String, Section>,
}

impl XBE {
    pub fn open(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        
        let mut magic: [u8; 4] = [0; _];
        reader.read_exact(&mut magic)?;
        
        if magic != XBE_MAGIC {
            return Err("BadMagic".into());
        }
        
        reader.seek_relative(0x100)?; // Seek to 0x104
        
        let mut base_address_bytes: [u8; 4] = [0; _];
        reader.read_exact(&mut base_address_bytes)?;
        let base_address = LittleEndian::read_u32(&base_address_bytes);
        
        reader.seek_relative(0x10)?; // Seek to 0x118
        
        let mut certificate_address_bytes: [u8; 4] = [0; _];
        reader.read_exact(&mut certificate_address_bytes)?;
        let certificate_address = LittleEndian::read_u32(&certificate_address_bytes);
        
        let mut section_count_bytes: [u8; 4] = [0; _];
        reader.read_exact(&mut section_count_bytes)?;
        let section_count = LittleEndian::read_u32(&section_count_bytes);
        
        let mut section_address_bytes: [u8; 4] = [0; _];
        reader.read_exact(&mut section_address_bytes)?;
        let section_address = LittleEndian::read_u32(&section_address_bytes);
        
        let mut xbe = XBE{
            reader: reader,
            base_address: base_address,
            title_id: 0,
            sections: HashMap::with_capacity(section_count as usize),
        };
        
        // Get the Title ID
        xbe.seek_pointer(certificate_address + 0x8)?;
        
        let mut title_id_bytes: [u8; 4] = [0; _];
        xbe.reader.read_exact(&mut title_id_bytes)?;
        xbe.title_id = LittleEndian::read_u32(&title_id_bytes);
        
        println!("Title {:08X}, Base Address {:08X}, Section Count {}, Section Table Address {:08X}",
            xbe.title_id, xbe.base_address, section_count, section_address);
        
        xbe.seek_pointer(section_address)?;
        
        let section_list_byte_size = section_count as usize * XBE_SECTION_SIZE;
        let mut section_list_bytes = vec![0; section_list_byte_size];
        
        xbe.reader.read_exact(&mut section_list_bytes)?;
        
        let (section_list, []) = section_list_bytes.as_chunks::<XBE_SECTION_SIZE>() else {
            panic!("section_list_bytes length not a multiple of XBE_SECTION_SIZE");
        };
        
        for (i, section_bytes) in section_list.into_iter().enumerate() {
            let name_address = LittleEndian::read_u32(&section_bytes[20..24]);
            
            xbe.seek_pointer(name_address)?;
            
            let mut name_bytes = Vec::new();
            
            xbe.reader.read_until(0, &mut name_bytes)?;
            
            let name_cstr = CString::from_vec_with_nul(name_bytes)?;
            let name = name_cstr.into_string()?;
            
            let mut section = Section{
                vaddr: LittleEndian::read_u32(&section_bytes[4..8]),
                vsize: LittleEndian::read_u32(&section_bytes[8..12]),
                
                faddr: LittleEndian::read_u32(&section_bytes[12..16]),
                fsize: LittleEndian::read_u32(&section_bytes[16..20]),
                
                checksum: [0; 20],

                data: None,
            };
            
            println!("Section {:02}, Virtual [{:08X}..{:08X}], File [{:08X}..{:08X}], Name {}",
                i,
                section.vaddr, section.vaddr + section.vsize,
                section.faddr, section.faddr + section.fsize,
                name);
            
            section.checksum.copy_from_slice(&section_bytes[36..]);
            
            xbe.sections.insert(name, section);
        }
        
        return Ok(xbe);
    }
    
    pub fn seek_pointer(&mut self, pointer: u32) -> Result<u64, std::io::Error> {
        if pointer < self.base_address {
            return Err(std::io::Error::other("pointer below base address"));
        }
        
        let mut offset = pointer - self.base_address;
        
        for (name, section) in &self.sections {
            if pointer >= section.vaddr && pointer < section.vaddr + section.vsize {
                offset = pointer - section.vaddr;
                if offset >= section.fsize {
                    return Err(std::io::Error::other("offset larger than section"));
                }
                offset += section.faddr;
                break;
            }
        }
        
        return self.reader.seek(SeekFrom::Start(offset as u64));
    }
    
    pub fn seek_section(&mut self, name: &str, offset: u32) -> Result<u64, std::io::Error> {
        let section = self.sections.get(name).ok_or(std::io::Error::other("invalid section"))?;

        if offset >= section.fsize {
            return Err(std::io::Error::other("offset larger than section"));
        }
        return self.reader.seek(SeekFrom::Start((section.faddr + offset) as u64));
    }

    pub fn get_section_data(&mut self, name: &str) -> Result<&[u8], std::io::Error> {

    }
    
    pub fn copy_section(&mut self, name: &str, mut writer: impl Write) -> Result<bool, std::io::Error> {
        // Need a reference to the reader to avoid a move from .take()
        let reader = &mut self.reader;

        let section = self.sections.get(name).ok_or(std::io::Error::other("invalid section"))?;

        reader.seek(SeekFrom::Start(section.faddr as u64))?;
        let mut take = reader.take(section.fsize as u64);

        return Ok(std::io::copy(&mut take, &mut writer)? == section.fsize as u64);
    }
}

