use iospng::Png;
use std::fs;
use std::fs::{File, OpenOptions};
use std::io::{Cursor, Read, Seek, SeekFrom, Write};
use std::time::Instant;
#[derive(Debug)]
pub enum MyData {
    File(File),
    Mem(Cursor<Vec<u8>>),
}
impl Clone for MyData {
    fn clone(&self) -> Self {
        match self {
            MyData::File(f) => MyData::File(f.try_clone().unwrap()),
            MyData::Mem(v) => MyData::Mem(Cursor::new(v.get_ref().clone())),
        }
    }
}
impl Default for MyData {
    fn default() -> Self {
        Self::Mem(Cursor::new(vec![]))
    }
}
impl Read for MyData {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            MyData::File(v) => v.read(buf),
            MyData::Mem(v) => v.read(buf),
        }
    }
}
impl Write for MyData {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            MyData::File(v) => v.write(buf),
            MyData::Mem(v) => v.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            MyData::File(v) => v.flush(),
            MyData::Mem(v) => v.flush(),
        }
    }
}
impl Seek for MyData {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        match self {
            MyData::File(v) => v.seek(pos),
            MyData::Mem(v) => v.seek(pos),
        }
    }
}
fn main() {
    let time = Instant::now();
    let input = MyData::Mem(Cursor::new(fs::read("./data/AppIcon160x60@2x.png".to_string()).unwrap()));
    let output = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open("./data/origin.png".to_string())
        .unwrap();
    let mut output = MyData::File(output); //Stream::new(output_file.into());
    Png::<MyData>::restore(input, &mut output).unwrap();
    dbg!(time.elapsed());
}
