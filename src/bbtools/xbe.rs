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

use num_bigint::BigUint;
use sha1::{Digest, Sha1, digest::DynDigest};

use byteorder::{ByteOrder, LittleEndian, ReadBytesExt};

const XBE_MAGIC: [u8; 4] = *b"XBEH";
const XBE_SECTION_SIZE: usize = 56;
const XBE_HEADER_SHA_OFFSET: usize = XBE_MAGIC.len() + 256; // MAGIC + Signature size

const XBE_PUBLIC_KEY_EXPONENT: [u8; 4] = [0x01, 0x00, 0x01, 0x00];
const XBE_PUBLIC_KEY_MODULUS: [u8; 256] = [
    0xd3, 0xd7, 0x4e, 0xe5, 0x66, 0x3d, 0xd7, 0xe6, 0xc2, 0xd4, 0xa3, 0xa1, 0xf2, 0x17, 0x36, 0xd4,
    0x2e, 0x52, 0xf6, 0xd2, 0x02, 0x10, 0xf5, 0x64, 0x9c, 0x34, 0x7b, 0xff, 0xef, 0x7f, 0xc2, 0xee,
    0xbd, 0x05, 0x8b, 0xde, 0x79, 0xb4, 0x77, 0x8e, 0x5b, 0x8c, 0x14, 0x99, 0xe3, 0xae, 0xc6, 0x73,
    0x72, 0x73, 0xb5, 0xfb, 0x01, 0x5b, 0x58, 0x46, 0x6d, 0xfc, 0x8a, 0xd6, 0x95, 0xda, 0xed, 0x1b,
    0x2e, 0x2f, 0xa2, 0x29, 0xe1, 0x3f, 0xf1, 0xb9, 0x5b, 0x64, 0x51, 0x2e, 0xa2, 0xc0, 0xf7, 0xba,
    0xb3, 0x3e, 0x8a, 0x75, 0xff, 0x06, 0x92, 0x5c, 0x07, 0x26, 0x75, 0x79, 0x10, 0x5d, 0x47, 0xbe,
    0xd1, 0x6a, 0x52, 0x90, 0x0b, 0xae, 0x6a, 0x0b, 0x33, 0x44, 0x93, 0x5e, 0xf9, 0x9d, 0xfb, 0x15,
    0xd9, 0xa4, 0x1c, 0xcf, 0x6f, 0xe4, 0x71, 0x94, 0xbe, 0x13, 0x00, 0xa8, 0x52, 0xca, 0x07, 0xbd,
    0x27, 0x98, 0x01, 0xa1, 0x9e, 0x4f, 0xa3, 0xed, 0x9f, 0xa0, 0xaa, 0x73, 0xc4, 0x71, 0xf3, 0xe9,
    0x4e, 0x72, 0x42, 0x9c, 0xf0, 0x39, 0xce, 0xbe, 0x03, 0x76, 0xfa, 0x2b, 0x89, 0x14, 0x9a, 0x81,
    0x16, 0xc1, 0x80, 0x8c, 0x3e, 0x6b, 0xaa, 0x05, 0xec, 0x67, 0x5a, 0xcf, 0xa5, 0x70, 0xbd, 0x60,
    0x0c, 0xe8, 0x37, 0x9d, 0xeb, 0xf4, 0x52, 0xea, 0x4e, 0x60, 0x9f, 0xe4, 0x69, 0xcf, 0x52, 0xdb,
    0x68, 0xf5, 0x11, 0xcb, 0x57, 0x8f, 0x9d, 0xa1, 0x38, 0x0a, 0x0c, 0x47, 0x1b, 0xb4, 0x6c, 0x5a,
    0x53, 0x6e, 0x26, 0x98, 0xf1, 0x88, 0xae, 0x7c, 0x96, 0xbc, 0xf6, 0xbf, 0xb0, 0x47, 0x9a, 0x8d,
    0xe4, 0xb3, 0xe2, 0x98, 0x85, 0x61, 0xb1, 0xca, 0x5f, 0xf7, 0x98, 0x51, 0x2d, 0x83, 0x81, 0x76,
    0x0c, 0x88, 0xba, 0xd4, 0xc2, 0xd5, 0x3c, 0x14, 0xc7, 0x72, 0xda, 0x7e, 0xbd, 0x1b, 0x4b, 0xa4,
];

struct Section {
    // Virtual Address
    vaddr: u32,
    vsize: u32,
    // File Address
    faddr: u32,
    fsize: u32,
    // Provided SHA-1 hash
    hash_provided: [u8; 20],
    // Actual SHA-1 hash of the section (should match hash_proivded)
    hash_actual: [u8; 20],
}

impl Section {
    fn compute_sha1<R: Read + Seek>(
        &mut self,
        reader: &mut R,
        offset: u32,
    ) -> Result<(), std::io::Error> {
        reader.seek(SeekFrom::Start((self.faddr + offset) as u64))?;
        let length = self.fsize - offset;
        let mut take = reader.take(length as u64);

        let mut data: Vec<u8> = Vec::with_capacity(length as usize);
        if take.read_to_end(&mut data)? != length as usize {
            return Err(std::io::Error::other("failed to read entire section"));
        }

        let mut length_prefix: [u8; 4] = [0; _];
        LittleEndian::write_u32(&mut length_prefix, length);

        if DynDigest::finalize_into(
            Sha1::new_with_prefix(length_prefix).chain_update(data),
            &mut self.hash_actual,
        )
        .is_err()
        {
            return Err(std::io::Error::other("Failed to compute SHA-1 digest"));
        }

        return Ok(());
    }

    pub fn is_hash_ok(&self) -> bool {
        self.hash_actual == self.hash_provided
    }
}

impl fmt::Display for Section {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Virtual [{:08X}..{:08X}], File [{:08X}..{:08X}] | Hash {:>3}",
            self.vaddr,
            self.vaddr + self.vsize,
            self.faddr,
            self.faddr + self.fsize,
            if self.is_hash_ok() { "OK" } else { "BAD" }
        )
    }
}

const HEADERS_NAME: &str = "headers";

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

        let mut unsigned_signature: [u8; 256] = [0; _];
        reader.read_exact(&mut unsigned_signature)?;

        let signature = {
            let s = BigUint::from_bytes_le(&unsigned_signature);
            let e = BigUint::from_bytes_le(&XBE_PUBLIC_KEY_EXPONENT);
            let n = BigUint::from_bytes_le(&XBE_PUBLIC_KEY_MODULUS);

            s.modpow(&e, &n).to_bytes_be()
        };

        let header_addr = reader.read_u32::<LittleEndian>()?;
        let header_size = reader.read_u32::<LittleEndian>()?;

        let mut header_section = Section {
            vaddr: header_addr,
            vsize: header_size,

            faddr: 0,
            fsize: header_size,

            hash_provided: [0; 20],
            hash_actual: [0; 20],
        };

        reader.seek_relative(0xC)?; // Seek to 0x118

        let certificate_address = reader.read_u32::<LittleEndian>()?;

        let section_count = reader.read_u32::<LittleEndian>()?;
        let section_address = reader.read_u32::<LittleEndian>()?;

        // Last 20 bytes contain the provided hash
        header_section
            .hash_provided
            .copy_from_slice(&signature[235..]);
        header_section.compute_sha1(&mut reader, XBE_HEADER_SHA_OFFSET as u32)?;

        let mut xbe = XBE {
            reader,
            title_id: 0,
            sections: HashMap::with_capacity(section_count as usize + 1),
        };

        xbe.sections
            .insert(HEADERS_NAME.to_string(), header_section);

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
            xbe.sections[HEADERS_NAME], HEADERS_NAME
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

                hash_provided: [0; 20],
                hash_actual: [0; 20],
            };

            section.hash_provided.copy_from_slice(&section_bytes[36..]);
            section.compute_sha1(&mut xbe.reader, 0)?;

            println!("  Section {:02} | {} | Name {}", i + 1, section, name);

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

    pub fn is_valid(&self) -> bool {
        !self.sections.iter().any(|(_, s)| !s.is_hash_ok())
    }
}
