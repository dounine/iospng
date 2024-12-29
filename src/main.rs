use crate::chunk::Chunk;
use std::fs;
use std::io::Read;
use flate2::read::{DeflateDecoder, ZlibDecoder};

mod chunk;
mod error;

fn main() {
    let data = fs::read("./data/ios.png").unwrap();
    let chunk = Chunk::init(data).unwrap();
    let compress_data =
        fs::read("/Users/lake/dounine/github/rust/rust-pngdefry/pngdefry/output.bin").unwrap();
    let mut d = DeflateDecoder::new(&compress_data[..]);
    let mut s = vec![];
    let out_lenth = d.read_to_end(&mut s).unwrap();
    let len = s.len();
    dbg!(len);
    // let mut decoder = deflate::
    // println!("Hello, world!");
}
