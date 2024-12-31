use crate::bytes::endian::Endian;
use crate::bytes::fast::{FastReader, FastWriter};
use crate::error::Error;
use std::io::{BufRead, Cursor, Read, Seek, Write};

#[derive(Debug, Clone)]
pub struct Stream {
    data: Cursor<Vec<u8>>,
    endian: Endian,
    pin_positions: Vec<u64>,
}
impl Into<Result<Vec<u8>, Error>> for &mut Stream {
    fn into(self) -> Result<Vec<u8>, Error> {
        Ok(self.take_data())
    }
}
impl Stream {
    pub fn empty() -> Self {
        Stream {
            data: Cursor::new(vec![]),
            endian: Endian::Little,
            pin_positions: vec![],
        }
    }
    pub fn from(data: Vec<u8>) -> Self {
        Stream {
            data: Cursor::new(data),
            endian: Endian::Little,
            pin_positions: vec![],
        }
    }
    pub fn with_little_endian(&mut self) {
        self.endian = Endian::Little;
    }
    pub fn with_big_endian(&mut self) {
        self.endian = Endian::Big;
    }
    pub fn with_endian(&mut self, endian: &Endian) -> &mut Self {
        self.endian = endian.clone();
        self
    }
    pub fn insert(&mut self, mut data: Stream) -> &mut Self {
        let data = data.take_data();
        self.data.get_mut().splice(0..0, data);
        self
    }
    pub fn insert_data(&mut self, data: Vec<u8>) -> &mut Self {
        self.data.get_mut().splice(0..0, data);
        self
    }
    pub fn position_end(&mut self) -> &mut Self {
        let len = self.data.get_ref().len();
        self.set_position(len as u64);
        self
    }
    pub fn data(&self) -> &Vec<u8> {
        self.data.get_ref()
    }
    pub fn endian(&self) -> &Endian {
        &self.endian
    }
    pub fn data_mut(&mut self) -> &mut Vec<u8> {
        self.data.get_mut()
    }
    pub fn restore(&mut self) -> &mut Self {
        self.data.set_position(0);
        self
    }
    /*
    保存当前位置
     */
    pub fn pin(&mut self) -> &mut Self {
        self.pin_positions.push(self.data.position());
        self
    }
    /*
    清空保存位置数据
     */
    pub fn pin_clear(&mut self) -> &mut Self {
        self.pin_positions.clear();
        self
    }
    /*
    恢复当前位置
     */
    pub fn un_pin(&mut self) -> &mut Self {
        if let Some(position) = self.pin_positions.pop() {
            self.data.set_position(position)
        }
        self
    }
    /*
    恢复到pin+size位置
     */
    pub fn un_pin_size(&mut self, size: u64) -> &mut Self {
        let current_position = self.data.position();
        if let Some(position) = self.pin_positions.pop() {
            if current_position - position != size {
                self.data.set_position(position + size);
            }
        }
        self
    }
    pub fn set_position(&mut self, position: u64) -> &mut Self {
        self.data.set_position(position);
        self
    }
    pub fn position(&self) -> u64 {
        self.data.position()
    }
    pub fn len(&self) -> u64 {
        self.data.get_ref().len() as u64
    }
    pub fn seek_relative(&mut self, seek: i64) -> Result<&mut Self, Error> {
        self.data.seek_relative(seek)?;
        Ok(self)
    }

    pub fn write<T: FastWriter>(&mut self, value: &T) -> Result<&mut Self, Error> {
        let value = value.write(&self.endian)?;
        self.data.write(&value)?;
        Ok(self)
    }
    pub fn extend_from_slice(&mut self, data: &Vec<u8>) -> &mut Self {
        self.data.get_mut().extend_from_slice(data);
        let position = self.data.position();
        self.data.set_position(position + data.len() as u64);
        self
    }
    pub fn fill_size(&mut self, size: u32) -> &mut Self {
        let data_len = self.data.get_ref().len();
        if data_len < size as usize {
            self.data.get_mut().resize(size as usize, 0);
        }
        self
    }
    pub fn read_exact(&mut self, data: &mut [u8]) -> Result<&mut Self, Error> {
        self.data.read_exact(data)?;
        Ok(self)
    }
    pub fn read_size(&mut self, size: u32) -> Result<Vec<u8>, Error> {
        let mut size_data = vec![0_u8; size as usize];
        self.data.read_exact(&mut size_data)?;
        Ok(size_data)
    }
    pub fn read_until(&mut self, byte: u8, data: &mut Vec<u8>) -> Result<&mut Self, Error> {
        self.data.read_until(byte, data)?;
        if data.len() > 0 {
            data.pop();
        }
        Ok(self)
    }
    pub fn read_string_until(&mut self, byte: u8) -> Result<String, Error> {
        let mut data = vec![];
        self.read_until(byte, &mut data)?;
        Ok(String::from_utf8_lossy(&data).to_string())
    }
    pub fn read<'a, T: FastReader>(&mut self) -> Result<T, Error> {
        T::read(self)
    }
    pub fn take<'a, T: FastReader>(&mut self) -> Result<T, Error> {
        let position = self.position() as usize;
        let value = self.read()?;
        let size = self.position() as usize - position;
        self.data.get_mut().drain(position..position + size);
        Ok(value)
    }
    pub fn has_data(&self) -> bool {
        self.data.position() < self.data().len() as u64
    }
    pub fn take_data(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.data_mut())
    }
}
#[cfg(test)]
mod test_stream {
    use crate::bytes::stream::Stream;

    #[test]
    fn test_reader() {
        let data = vec![];
        let mut stream = Stream::from(data);
        stream.write(&10_u32).unwrap();
        stream.restore();
        let first = stream.read::<u8>();
        assert_eq!(first.ok(), Some(10));
    }
    #[test]
    fn test_pin() {
        let data = vec![1, 2, 3, 4];
        let mut stream = Stream::from(data);
        stream.pin();
        let value = stream.read::<u8>().unwrap();
        assert_eq!(value, 1);
        let value2 = stream.read::<u8>().unwrap();
        assert_eq!(value2, 2);
        stream.un_pin();
        let position = stream.position();
        assert_eq!(position, 0);
    }
    #[test]
    fn test_error() {
        let data = vec![];
        let mut stream = Stream::from(data);
        let value = stream.read::<u8>();
        assert_eq!(value.is_err(), true)
    }
}
