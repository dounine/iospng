use crate::bytes::endian::Endian;
use crate::bytes::stream::Stream;
use crate::error::Error;

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
impl FastReader for [u8; 8] {
    fn read(stream: &mut Stream) -> Result<Self, Error> {
        let mut data = [0; 8];
        stream.read_exact(&mut data)?;
        Ok(data)
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

impl FastWriter for [u8; 8] {
    fn write(&self, _endian: &Endian) -> Result<Vec<u8>, Error> {
        Ok(self.to_vec())
    }
}
