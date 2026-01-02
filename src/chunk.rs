use crate::error::Error;
use binrw::io::read::ReadExt;
use binrw::io::{Read, Seek, Write};
use binrw::{BinRead, BinReaderExt, BinResult, BinWrite, BinWriterExt, Endian};

// #[binrw]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChunkType {
    // #[brw(magic = 0x43674249_u32)]
    CgBI,
    // #[brw(magic = 0x49484452_u32)]
    IhDr, //图像头部
    // #[brw(magic = 0x49444154_u32)]
    IdAt, //图像数据
    // #[brw(magic = 0x49454E44_u32)]
    IEND, //文件结束
    Unknown(u32),
}
impl BinRead for ChunkType {
    type Args<'a> = ();

    fn read_options<R: Read + Seek + Send>(
        reader: &mut R,
        endian: Endian,
        _args: Self::Args<'_>,
    ) -> impl Future<Output = BinResult<Self>> + Send
    where
        Self: Send,
    {
        async move {
            let value: u32 = reader.read_type(endian).await?;
            let value = match value {
                0x43674249_u32 => Self::CgBI,
                0x49484452_u32 => Self::IhDr,
                0x49444154_u32 => Self::IdAt,
                0x49454E44_u32 => Self::IEND,
                _ => Self::Unknown(value),
            };
            Ok(value)
        }
    }
}
impl BinWrite for ChunkType {
    type Args<'a> = ();
    fn write_options<W: Write + Seek + Send>(
        &self,
        writer: &mut W,
        endian: Endian,
        _args: Self::Args<'_>,
    ) -> impl Future<Output = BinResult<()>> + Send
    where
        Self: Sync,
    {
        async move {
            let value: u32 = match self {
                ChunkType::CgBI => 0x43674249_u32,
                ChunkType::IhDr => 0x49484452_u32,
                ChunkType::IdAt => 0x49444154_u32,
                ChunkType::IEND => 0x49454E44_u32,
                ChunkType::Unknown(value) => *value,
            };
            writer.write_type(&value, endian).await?;
            Ok(())
        }
    }
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
    T: Read + Write + Seek + Send + Default,
{
    pub fn parse<I>(
        stream: &mut I,
        file_length: u64,
    ) -> impl Future<Output = Result<Vec<Self>, Error>> + Send
    where
        I: Read + Seek + Send,
    {
        async move {
            let mut chunks = vec![];
            loop {
                let chunk = Self::init(stream, file_length).await?;
                if chunk.id == ChunkType::IEND {
                    chunks.push(chunk);
                    break;
                } else {
                    chunks.push(chunk);
                }
            }
            Ok(chunks)
        }
    }
    fn init<I>(stream: &mut I, file_length: u64) -> impl Future<Output = Result<Self, Error>> + Send
    where
        I: Read + Seek + Send,
    {
        async move {
            let pos = stream.position().await?;
            stream.set_position(pos).await?;

            let length: u32 = stream.read_be().await?;
            if length as u64 > file_length - 4 {
                return Err(Error::Error(format!(
                    "informational: chunk length {} larger than file {}",
                    length, file_length
                )));
            }
            // let position = stream.position()? as usize;
            // stream.take((length + 4) as u64)
            let mut reader = stream.take(length as u64 + 4);
            let mut chunk_data = vec![];
            reader.read_to_end(&mut chunk_data).await?;
            let mut data = T::default(); // Stream::new(chunk_data.into());
            data.write_all(&chunk_data).await?;
            data.seek_start().await?;
            let crc32 = stream.read_be().await?;
            let id = data.read_be().await?;
            let data = Self {
                length,
                id,
                data,
                crc32,
            };
            Ok(data)
        }
    }
}
