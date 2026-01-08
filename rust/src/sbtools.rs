pub mod xbe;
pub mod bin;
pub mod xpr;
pub mod xbo;
pub mod lmt;
pub mod ppd;
pub mod xact;
pub mod text;

use std::io::SeekFrom;
use byteorder::ByteOrder;
use byteorder::LittleEndian;
use std::ops::Range;

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
        FileSlice{
            offset: LittleEndian::read_u32(&buf[0..4]) as usize,
            length: LittleEndian::read_u32(&buf[4..8]) as usize,
        }
    }
}

impl From<&[u8]> for FileSlice {
    fn from(buf: &[u8]) -> Self {
        assert!(buf.len() == 8);
        FileSlice{
            offset: LittleEndian::read_u32(&buf[0..4]) as usize,
            length: LittleEndian::read_u32(&buf[4..8]) as usize,
        }
    }
}

impl FileSlice {
    fn get_end(&self) -> usize {return self.offset + self.length}
    
    fn as_range(&self) -> Range<usize> {return self.offset..(self.offset + self.length)}

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
