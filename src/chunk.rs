use fast_stream::bytes::Bytes;
use fast_stream::derive::NumToEnum;
use fast_stream::endian::Endian;
use fast_stream::enum_to_bytes;
use fast_stream::stream::{Stream};
use std::io::{Seek};
use crate::error::Error;

#[repr(u32)]
#[derive(Debug, Clone, PartialEq, Eq, NumToEnum)]
pub enum ChunkType {
    CgBI = 0x43674249_u32,
    IhDr = 0x49484452_u32, //图像头部
    IdAt = 0x49444154_u32, //图像数据
    IEND = 0x49454E44_u32, //文件结束
    Unknown(u32),
}
enum_to_bytes!(ChunkType, u32);

#[derive(Debug)]
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
        let file_length = stream.length();

        let length: u32 = stream.read_value()?;
        if length as u64 > file_length - 4 {
            return Err(Error::Error(format!(
                "informational: chunk length {} larger than file",
                length
            )));
        }
        let position = stream.stream_position()? as usize;
        let chunk_data: Vec<u8> = stream
            .drain(position..position + length as usize + 4)?;
        let mut data = Stream::new(chunk_data.into());
        data.with_endian(Endian::Big);
        let data = Self {
            length,
            id: data.read_value()?,
            data,
            crc32: stream.read_value()?,
        };
        Ok(data)
    }
}
