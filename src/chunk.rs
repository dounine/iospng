use crate::error::PngError;
use std::io::{Cursor, Read};

#[derive(Debug)]
pub struct Chunk {
    pub length: i32,
    pub id: i32,
    pub data: Vec<u8>,
    pub crc32: i32,
}
impl Chunk {
    fn move_u8(value: u8, size: u8) -> i32 {
        let value = value as i32;
        if value == 0 {
            return 0;
        };
        value << size
    }
    pub fn init(data: Vec<u8>) -> Result<Self, PngError> {
        let png_magic_bytes = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        let file_length = data.len();
        let mut stream = Cursor::new(data);
        let mut magic_data = vec![0_u8; 8];
        stream.read_exact(&mut magic_data)?;
        if magic_data != png_magic_bytes {
            return Err(PngError::NotIosPng);
        }

        if file_length < 8 {
            return Err(PngError::NotIosPng);
        }
        let mut buf = [0; 4];
        stream.read_exact(&mut buf)?;

        let mut info = Self {
            length: Self::move_u8(buf[0], 24)
                + Self::move_u8(buf[1], 16)
                + Self::move_u8(buf[2], 8)
                + buf[3] as i32,
            id: 0,
            data: vec![],
            crc32: 0,
        };
        if info.length as usize > file_length - 4 {
            return Err(PngError::Error(format!(
                "informational: chunk length {} larger than file",
                info.length
            )));
        }
        let mut chunk_data = vec![0_u8; info.length as usize + 4];
        stream.read_exact(&mut chunk_data)?;
        info.data = chunk_data;
        info.id = Self::move_u8(info.data[0], 24)
            + Self::move_u8(info.data[1], 16)
            + Self::move_u8(info.data[2], 8)
            + info.data[3] as i32;

        let mut buf = [0; 4];
        stream.read_exact(&mut buf)?;
        info.crc32 = Self::move_u8(buf[0], 24)
            + Self::move_u8(buf[1], 16)
            + Self::move_u8(buf[2], 8)
            + buf[3] as i32;

        if info.id != 0x43674249 {
            return Err(PngError::NotIosPng);
        }

        Ok(info)
    }
}
