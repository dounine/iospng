use crate::bytes::stream::Stream;
use crate::error::Error;
use crate::{fast_read, from_bytes};

#[repr(u32)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChunkType {
    CgBI = 0x43674249_u32,
    IhDr = 0x49484452_u32, //图像头部
    IdAt = 0x49444154_u32, //图像数据
    IEND = 0x49454E44_u32, //文件结束
    Unknown(u32),
}
from_bytes!(ChunkType, u32, 4);
fast_read!(ChunkType, 4);
impl Into<ChunkType> for u32 {
    fn into(self) -> ChunkType {
        match self {
            0x43674249_u32 => ChunkType::CgBI,
            0x49484452_u32 => ChunkType::IhDr,
            0x49444154_u32 => ChunkType::IdAt,
            0x49454E44_u32 => ChunkType::IEND,
            _ => ChunkType::Unknown(self),
        }
    }
}
#[derive(Debug, Clone)]
pub struct Chunk {
    pub length: u32,
    pub id: ChunkType,
    pub data: Stream,
    pub crc32: u32,
}
impl Chunk {
    pub fn parse(stream: &mut Stream) -> Result<Vec<Self>, Error> {
        let mut chunks = vec![];
        loop {
            let chunk = Self::init(stream)?;
            if chunk.id == ChunkType::IEND {
                chunks.push(chunk);
                break;
            } else {
                chunks.push(chunk);
            }
        }
        Ok(chunks)
    }
    fn init(stream: &mut Stream) -> Result<Self, Error> {
        let file_length = stream.len();

        let length: u32 = stream.read()?;
        if length as u64 > file_length - 4 {
            return Err(Error::Error(format!(
                "informational: chunk length {} larger than file",
                length
            )));
        }
        let position = stream.position() as usize;
        let chunk_data: Vec<u8> = stream
            .data_mut()
            .drain(position..position + length as usize + 4)
            .collect();
        let mut data_stream = Stream::from(chunk_data[..4].to_vec());
        data_stream.with_big_endian();
        let mut data = Stream::from(chunk_data);
        data.with_big_endian();

        Ok(Self {
            length,
            id: data.read()?,
            data,
            crc32: stream.read()?,
        })
    }
}
