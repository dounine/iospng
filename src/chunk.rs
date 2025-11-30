use crate::error::Error;
use binrw::{BinReaderExt, binrw};
use std::io::{Read, Seek, SeekFrom, Write};

#[binrw]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChunkType {
    #[brw(magic = 0x43674249_u32)]
    CgBI,
    #[brw(magic = 0x49484452_u32)]
    IhDr, //图像头部
    #[brw(magic = 0x49444154_u32)]
    IdAt, //图像数据
    #[brw(magic = 0x49454E44_u32)]
    IEND, //文件结束
    Unknown(u32),
}
// impl From<u32> for ChunkType {
//     fn from(value: u32) -> Self {
//         match value {
//             0x43674249_u32 => ChunkType::CgBI,
//             0x49484452_u32 => ChunkType::IhDr,
//             0x49444154_u32 => ChunkType::IdAt,
//             0x49454E44_u32 => ChunkType::IEND,
//             _ => ChunkType::Unknown,
//         }
//     }
// }
// enum_to_bytes!(ChunkType, u32);

#[derive(Debug)]
pub struct Chunk<T>
where
    T: Read + Write + Seek + Default,
{
    pub length: u32,
    pub id: ChunkType,
    pub data: T,
    pub crc32: u32,
}
impl<T> Chunk<T>
where
    T: Read + Write + Seek + Default,
{
    pub fn parse<I>(stream: &mut I) -> Result<Vec<Self>, Error>
    where
        I: Read + Seek,
    {
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
    fn init<I>(stream: &mut I) -> Result<Self, Error>
    where
        I: Read + Seek,
    {
        let pos = stream.stream_position()?;
        let file_length = stream.seek(SeekFrom::End(0))?;
        stream.seek(SeekFrom::Start(pos))?;

        let length: u32 = stream.read_be()?;
        if length as u64 > file_length - 4 {
            return Err(Error::Error(format!(
                "informational: chunk length {} larger than file",
                length
            )));
        }
        // let position = stream.stream_position()? as usize;
        // stream.take((length + 4) as u64)
        let mut reader = stream.take(length as u64 + 4);
        let mut chunk_data = vec![];
        reader.read_to_end(&mut chunk_data)?;
        let mut data = T::default(); // Stream::new(chunk_data.into());
        data.write_all(&chunk_data)?;
        data.seek(SeekFrom::Start(0))?;
        let crc32 = stream.read_be()?;
        let id = data.read_be()?;
        let data = Self {
            length,
            id,
            data,
            crc32,
        };
        Ok(data)
    }
}
