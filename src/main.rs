use crate::png::Png;
use std::fs;

pub mod bytes;
mod chunk;
mod error;
pub mod png;
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
    let data = Png::restore(data).unwrap();
    fs::write("./origin.png", data).unwrap();
}
