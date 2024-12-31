use crate::bytes::stream::Stream;
use crate::chunk::Chunk;
use crate::ios_png::IosPng;
use miniz_oxide::deflate::core::{
    compress_to_output, create_comp_flags_from_zip_params, CompressionStrategy, CompressorOxide,
    TDEFLFlush,
};
use miniz_oxide::deflate::{compress_to_vec, compress_to_vec_zlib};
use std::fs;
use std::io::Read;

pub mod bytes;
mod chunk;
mod error;
pub mod ios_png;
fn main() {
    // let slice = vec![1, 2, 3, 4, 5, 6, 7, 8];
    // let mut encoded = vec![];
    // let flags = create_comp_flags_from_zip_params(0, 1, 0);
    // let mut d = CompressorOxide::new(0x01000);
    // let (status, in_consumed) =
    //     compress_to_output(&mut d, &slice, TDEFLFlush::Finish, |out: &[u8]| {
    //         encoded.extend_from_slice(out);
    //         true
    //     });
    // let encoded = compress_to_vec_zlib(&vec![1, 2, 3, 4], 0);
    // dbg!(&encoded);
    // fs::write("./all_idat.bin", &encoded).unwrap();

    let data = fs::read("./data/ios.png").unwrap();
    let data = IosPng::restore(data).unwrap();
    println!("{:?}", data);
    // let compress_data =
    //     fs::read("/Users/lake/dounine/github/rust/rust-pngdefry/pngdefry/output.bin").unwrap();
    // let mut d = DeflateDecoder::new(&compress_data[..]);
    // let mut s = vec![];
    // let out_lenth = d.read_to_end(&mut s).unwrap();
    // let len = s.len();
    // dbg!(len);
    // let mut decoder = deflate::
    // println!("Hello, world!");
}
