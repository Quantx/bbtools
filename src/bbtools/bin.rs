use std::fs::File;
use std::io::BufReader;
use std::io::Read;
use std::io::Seek;
use std::io::Write;
use std::path::Path;

use byteorder::{LittleEndian, ReadBytesExt};

use crate::bbtools::FileSlice;

const BIN_MAGIC: [u8; 4] = *b"BIN0";

pub struct BIN<R: Read + Seek> {
    reader: BufReader<R>,
    files: Vec<FileSlice>,
}

impl<R: Read + Seek> BIN<R> {
    pub fn from_reader(mut reader: BufReader<R>) -> Result<BIN<R>, std::io::Error> {
        let file_count = reader.read_u32::<LittleEndian>()?;

        let mut file_list_bytes = vec![0; FileSlice::buffer_size(file_count)];
        reader.read_exact(&mut file_list_bytes)?;

        let files = FileSlice::from_bytes(&file_list_bytes);

        return Ok(BIN {
            reader: reader,
            files: files,
        });
    }

    pub fn open(path: &Path) -> Result<BIN<File>, std::io::Error> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);

        let mut magic: [u8; 4] = [0; _];
        reader.read_exact(&mut magic)?;

        if magic != BIN_MAGIC {
            return Err(std::io::Error::other("BadMagic"));
        }

        return BIN::from_reader(reader);
    }

    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    pub fn file_length(&self, file_index: usize) -> usize {
        self.files[file_index].length
    }

    pub fn copy_file(
        &mut self,
        file_index: usize,
        writer: &mut impl Write,
    ) -> Result<bool, std::io::Error> {
        if file_index >= self.files.len() {
            return Err(std::io::Error::other("invalid file_index"));
        }

        // Need a reference to the reader to avoid a move from .take()
        let reader = &mut self.reader;

        let file = &self.files[file_index];

        reader.seek(file.into())?;
        let mut take = reader.take(file.length as u64);

        return Ok(std::io::copy(&mut take, writer)? == file.length as u64);
    }

    pub fn export_file(&mut self, file_index: usize, path: &Path) -> Result<bool, std::io::Error> {
        if file_index >= self.files.len() {
            return Err(std::io::Error::other("invalid file_index"));
        }

        let mut file = File::create(path)?;
        return self.copy_file(file_index, &mut file);
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
