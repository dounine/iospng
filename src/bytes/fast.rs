use crate::bytes::endian::Endian;
use crate::bytes::stream::Stream;
use crate::error::Error;

pub trait VecExt {
    fn align(&mut self, align: usize);
}
impl VecExt for Vec<u8> {
    fn align(&mut self, align: usize) {
        let remainder = self.len() % align;
        if remainder != 0 {
            let padding = align - remainder;
            self.resize(self.len() + padding, 0u8);
        }
    }
}
pub trait FastWriter {
    fn write(&self, endian: &Endian) -> Result<Vec<u8>, Error>;
}
pub trait FastReader: Sized {
    fn read(stream: &mut Stream) -> Result<Self, Error>;
}
pub trait FromBytes<const N: usize> {
    fn from_be_bytes(data: [u8; N]) -> Self;
    fn from_le_bytes(data: [u8; N]) -> Self;
}
#[macro_export]
macro_rules! fast_writer {
   ($($typ:ty),*) => {
      $(
          impl FastWriter for $typ {
                fn write(&self, endian: &Endian) -> Result<Vec<u8>, Error> {
                    Ok(match endian {
                        Endian::Big => self.to_be_bytes().as_ref().to_vec(),
                        Endian::Little => self.to_le_bytes().as_ref().to_vec(),
                    })
                }
          }
      )*
   };
}
fast_writer!(u8, u16, u32, u64);
#[macro_export]
macro_rules! fast_read {
    ($($typ:ty, $size:expr),*) => {
        $(
            impl crate::bytes::fast::FastReader for $typ {
                fn read(stream: &mut crate::bytes::stream::Stream) -> Result<Self, crate::error::Error> {
                    #[allow(unused_imports)]
                    use crate::bytes::fast::FromBytes;
                    let mut data = [0; $size];
                    stream.read_exact(&mut data)?;
                    Ok(match stream.endian() {
                        crate::bytes::endian::Endian::Big => <$typ>::from_be_bytes(data),
                        crate::bytes::endian::Endian::Little => <$typ>::from_le_bytes(data),
                    })
                }
            }
        )*
    };
}
fast_read!(u8, 1, u16, 2, u32, 4, u64, 8);
#[macro_export]
macro_rules! from_bytes {
    ($typ:ty,$btyp:ty,$size:expr) => {
        impl crate::bytes::fast::FromBytes<$size> for $typ {
            fn from_be_bytes(data: [u8; $size]) -> Self {
                <$btyp>::from_be_bytes(data).into()
            }

            fn from_le_bytes(data: [u8; $size]) -> Self {
                <$btyp>::from_le_bytes(data).into()
            }
        }
    };
}

#[macro_export]
macro_rules! enum_to_bytes {
    ($typ:ty,$btyp:ty) => {
        impl crate::bytes::fast::FastWriter for $typ {
            fn write(&self, endian: &Endian) -> Result<Vec<u8>, Error> {
                let value: $btyp = self.clone() as $btyp;
                value.write(endian)
            }
        }
    };
}
impl FastReader for String {
    fn read(reader: &mut Stream) -> Result<Self, Error> {
        let mut bytes = vec![];
        reader.read_until(0, &mut bytes)?;
        String::from_utf8(bytes).map_err(|e| Error::Error(format!("bytes to string {}", e)))
    }
}
impl FastReader for [u8; 16] {
    fn read(stream: &mut Stream) -> Result<Self, Error> {
        let mut data = [0; 16];
        stream.read_exact(&mut data)?;
        Ok(data)
    }
}
impl FastWriter for Vec<u8> {
    fn write(&self, _endian: &Endian) -> Result<Vec<u8>, Error> {
        Ok(self.to_vec())
    }
}
impl FastWriter for [u8; 16] {
    fn write(&self, _endian: &Endian) -> Result<Vec<u8>, Error> {
        Ok(self.to_vec())
    }
}

#[derive(Debug)]
pub struct ULEB128(pub u64);
impl Into<u64> for ULEB128 {
    fn into(self) -> u64 {
        self.0
    }
}
impl FastReader for ULEB128 {
    fn read(stream: &mut Stream) -> Result<Self, Error> {
        let mut value = 0u64;
        let mut shift = 0u8;
        loop {
            let byte: u8 = stream.read()?;
            value |= (u64::from(byte & 0x7f)) << shift;
            shift += 7;
            if byte & 0x80 == 0 {
                break;
            }
        }
        Ok(ULEB128(value))
    }
}
impl FastWriter for ULEB128 {
    fn write(&self, _endian: &Endian) -> Result<Vec<u8>, Error> {
        let mut data = vec![];
        let mut value = self.0;
        loop {
            let mut byte = (value & 0x7f) as u8;
            value >>= 7;
            if value != 0 {
                byte |= 0x80; // 设置最高位为1，表示后续还有字节
            }
            data.push(byte);
            if value == 0 {
                break;
            }
        }
        Ok(data)
    }
}
#[derive(Debug)]
pub struct SLEB128(pub i64);
impl Into<i64> for SLEB128 {
    fn into(self) -> i64 {
        self.0
    }
}
impl FastReader for SLEB128 {
    fn read(stream: &mut Stream) -> Result<Self, Error> {
        let mut value = 0_i64;
        let mut shift = 0_u32;
        let mut byte: u8;
        loop {
            byte = stream.read()?;
            value += (i64::from(byte & 0x7f)) << shift;
            shift += 7;
            if byte < 128 {
                break;
            }
        }
        if byte & 0x40 != 0 {
            value |= -1_i64.wrapping_shl(shift);
        }
        Ok(SLEB128(value))
    }
}
impl FastWriter for SLEB128 {
    fn write(&self, _endian: &Endian) -> Result<Vec<u8>, Error> {
        let mut data = vec![];
        let mut more;
        let mut byte;
        let mut value = self.0;
        let is_neg = value < 0;

        loop {
            byte = (value & 0x7F) as u8;
            value >>= 7;

            if is_neg {
                more = (value != -1) || ((byte & 0x40) == 0);
            } else {
                more = (value != 0) || ((byte & 0x40) != 0);
            }
            if more {
                byte |= 0x80;
            }
            data.push(byte);
            if !more {
                break;
            }
        }
        Ok(data)
    }
}
impl FastWriter for [u8; 8] {
    fn write(&self, _endian: &Endian) -> Result<Vec<u8>, Error> {
        Ok(self.to_vec())
    }
}
impl FastWriter for String {
    fn write(&self, _endian: &Endian) -> Result<Vec<u8>, Error> {
        let mut data = self.as_bytes().to_vec();
        data.push(0);
        Ok(data)
    }
}
