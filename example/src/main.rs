use binrw::io::{Read, Seek, Write};
use iospng::Png;
use std::fs;
use std::fs::{File, OpenOptions};
use std::io::{Cursor, SeekFrom};
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
    async fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            MyData::File(v) => std::io::Read::read(v, buf),
            MyData::Mem(v) => std::io::Read::read(v, buf),
        }
    }

    async fn flush(&mut self) -> std::io::Result<()> {
        match self {
            MyData::File(v) => std::io::Write::flush(v),
            MyData::Mem(v) => std::io::Write::flush(v),
        }
    }
}
impl Write for MyData {
    async fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            MyData::File(v) => std::io::Write::write(v, buf),
            MyData::Mem(v) => std::io::Write::write(v, buf),
        }
    }

    async fn flush(&mut self) -> std::io::Result<()> {
        match self {
            MyData::File(v) => std::io::Write::flush(v),
            MyData::Mem(v) => std::io::Write::flush(v),
        }
    }
}
impl Seek for MyData {
    async fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        match self {
            MyData::File(v) => std::io::Seek::seek(v, pos),
            MyData::Mem(v) => std::io::Seek::seek(v, pos),
        }
    }
}
#[tokio::main]
async fn main() {
    let time = Instant::now();
    let input = MyData::Mem(Cursor::new(
        fs::read("./data/AppIcon160x60@2x.png".to_string()).unwrap(),
    ));
    let output = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open("./data/origin.png".to_string())
        .unwrap();
    let mut output = MyData::File(output); //Stream::new(output_file.into());
    Png::<MyData>::restore(input, &mut output).await.unwrap();
    dbg!(time.elapsed());
}
