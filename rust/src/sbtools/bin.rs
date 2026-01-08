use std::path::Path;
use std::fs::File;
use std::io::BufReader;
use std::io::Read;
use std::io::Write;
use std::io::Seek;
use byteorder::ByteOrder;
use byteorder::LittleEndian;

use crate::sbtools::FileSlice;

const BIN_MAGIC: [u8; 4] = *b"BIN0";

pub struct BIN {
    reader: BufReader<File>,
    files: Vec<FileSlice>,
}

impl BIN {
    pub fn open(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        
        let mut magic: [u8; 4] = [0; _];
        reader.read_exact(&mut magic)?;
        
        if magic != BIN_MAGIC {
            return Err("BadMagic".into());
        }
        
        let mut file_count_bytes: [u8; 4] = [0; _];
        reader.read_exact(&mut file_count_bytes)?;
        let file_count = LittleEndian::read_u32(&file_count_bytes);
        
        let file_list_bytes_size = FileSlice::buffer_size(file_count); 
        let mut file_list_bytes = vec![0; file_list_bytes_size];
        
        reader.read_exact(&mut file_list_bytes)?;
        
        let files = FileSlice::from_bytes(&file_list_bytes);
        
        return Ok(BIN{
            reader: reader,
            files: files,
        });
    }
    
    pub fn file_count(&self) -> usize {self.files.len()}
    
    pub fn copy_file(&mut self, file_index: usize, mut writer: impl Write) -> Result<bool, std::io::Error> {
        if file_index >= self.files.len() {
            return Err(std::io::Error::other("invalid file_index"));
        }
        
        // Need a reference to the reader to avoid a move from .take()
        let reader = &mut self.reader;
        
        let file = &self.files[file_index];
        
        reader.seek(file.into())?;
        let mut take = reader.take(file.length as u64);
        
        return Ok(std::io::copy(&mut take, &mut writer)? == file.length as u64);
    }
    
    pub fn export_file(&mut self, file_index: usize, path: &Path) -> Result<bool, std::io::Error> {
        if file_index >= self.files.len() {
            return Err(std::io::Error::other("invalid file_index"));
        }
        
        let file = File::create(path)?;
        return self.copy_file(file_index, file);
    }
    
    pub fn file_as_bytes(&mut self, file_index: usize) -> Result<Vec<u8>, std::io::Error> {
        if file_index >= self.files.len() {
            return Err(std::io::Error::other("invalid file_index"));
        }
        
        let file = &self.files[file_index];
        
        self.reader.seek(file.into())?;
        
        let mut buf = vec![0; file.length];
        self.reader.read_exact(&mut buf)?;
        
        return Ok(buf);
    }
}
