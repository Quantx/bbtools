use std::collections::HashMap;
use std::ffi::CString;
use std::fmt;
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::path::Path;

use byteorder::{ByteOrder, LittleEndian, ReadBytesExt};

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
}

impl fmt::Display for Section {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Virtual [{:08X}..{:08X}], File [{:08X}..{:08X}]",
            self.vaddr,
            self.vaddr + self.vsize,
            self.faddr,
            self.faddr + self.fsize
        )
    }
}

pub struct XBE {
    pub title_id: u32,
    pub reader: BufReader<File>,
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

        let header_addr = reader.read_u32::<LittleEndian>()?;
        let header_size = reader.read_u32::<LittleEndian>()?;

        let header_section = Section {
            vaddr: header_addr,
            vsize: header_size,

            faddr: 0,
            fsize: header_size,

            checksum: [0; 20],
        };

        reader.seek_relative(0xC)?; // Seek to 0x118

        let certificate_address = reader.read_u32::<LittleEndian>()?;

        let section_count = reader.read_u32::<LittleEndian>()?;
        let section_address = reader.read_u32::<LittleEndian>()?;

        let mut xbe = XBE {
            reader: reader,
            title_id: 0,
            sections: HashMap::with_capacity(section_count as usize + 1),
        };

        let header_name = "headers".to_string();

        xbe.sections.insert(header_name.clone(), header_section);

        // Get the Title ID
        xbe.seek_pointer_offset(certificate_address + 0x8)?;
        xbe.title_id = xbe.reader.read_u32::<LittleEndian>()?;

        println!(
            "Title {:08X}, Section Count {}, Section Table Address {:08X}",
            xbe.title_id,
            section_count + 1,
            section_address
        );

        println!(
            "  Section 00 | {} | Name {}",
            xbe.sections[&header_name], header_name
        );

        let section_list_byte_size = section_count as usize * XBE_SECTION_SIZE;
        let mut section_list_bytes = vec![0; section_list_byte_size];

        xbe.seek_pointer_offset(section_address)?;
        xbe.reader.read_exact(&mut section_list_bytes)?;

        let (section_list, []) = section_list_bytes.as_chunks::<XBE_SECTION_SIZE>() else {
            panic!("section_list_bytes length not a multiple of XBE_SECTION_SIZE");
        };

        for (i, section_bytes) in section_list.into_iter().enumerate() {
            let name_address = LittleEndian::read_u32(&section_bytes[20..24]);
            xbe.seek_pointer_offset(name_address)?;

            let mut name_bytes: Vec<u8> = Vec::with_capacity(16);
            xbe.reader.read_until(0, &mut name_bytes)?;

            let name_cstr = CString::from_vec_with_nul(name_bytes)?;
            let name = name_cstr.into_string()?;

            let mut section = Section {
                vaddr: LittleEndian::read_u32(&section_bytes[4..8]),
                vsize: LittleEndian::read_u32(&section_bytes[8..12]),

                faddr: LittleEndian::read_u32(&section_bytes[12..16]),
                fsize: LittleEndian::read_u32(&section_bytes[16..20]),

                checksum: [0; 20],
            };

            println!("  Section {:02} | {} | Name {}", i + 1, section, name);

            section.checksum.copy_from_slice(&section_bytes[36..]);

            xbe.sections.insert(name, section);
        }

        return Ok(xbe);
    }

    pub fn get_section_data(&mut self, section_name: &str) -> Result<Vec<u8>, std::io::Error> {
        let section = self
            .sections
            .get_mut(section_name)
            .ok_or(std::io::Error::other("invalid section"))?;

        // Need a reference to the reader to avoid a move from .take()
        let reader = &mut self.reader;

        reader.seek(SeekFrom::Start(section.faddr as u64))?;
        let mut take = reader.take(section.fsize as u64);

        let mut data: Vec<u8> = Vec::with_capacity(section.vsize as usize);
        if take.read_to_end(&mut data)? != section.fsize as usize {
            return Err(std::io::Error::other("failed to read entire section"));
        }

        data.resize(section.vsize as usize, 0); // Initialize the rest of the data to zeros

        return Ok(data);
    }

    pub fn seek_section_offset(
        &mut self,
        section_name: &str,
        offset: u32,
    ) -> Result<u64, std::io::Error> {
        let section = self
            .sections
            .get(section_name)
            .ok_or(std::io::Error::other("section not found"))?;
        if offset >= section.fsize {
            return Err(std::io::Error::other("offset larger than section"));
        }

        let fpos = (offset + section.faddr) as u64;
        return self.reader.seek(SeekFrom::Start(fpos));
    }

    pub fn get_pointer_offset(&self, pointer: u32) -> Result<u64, std::io::Error> {
        if pointer == 0 {
            return Err(std::io::Error::other("null pointer"));
        }

        if let Some((_, section)) = self
            .sections
            .iter()
            .find(|(_, s)| pointer >= s.vaddr && pointer < s.vaddr + s.vsize)
        {
            let offset = pointer - section.vaddr;
            if offset >= section.fsize {
                return Err(std::io::Error::other("offset larger than section"));
            }

            let fpos = (offset + section.faddr) as u64;

            return Ok(fpos);
        }

        return Err(std::io::Error::other("pointer outside all sections"));
    }

    pub fn seek_pointer_offset(&mut self, pointer: u32) -> Result<u64, std::io::Error> {
        let offset = self.get_pointer_offset(pointer)?;
        return self.reader.seek(SeekFrom::Start(offset));
    }
}
