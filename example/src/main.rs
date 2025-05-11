// use iospng::fast_stream::stream::Stream;
use iospng::Png;
use std::fs;
use std::fs::OpenOptions;
use std::time::Instant;
use fast_stream::stream::Stream;

fn main() {
    let time = Instant::now();
    let data = fs::read("./data/hi.png").unwrap();
    let input = Stream::new(data.into());
    let output_file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open("./data/origin2.png")
        .unwrap();
    let mut output = Stream::new(output_file.into());
    Png::restore(input, &mut output).unwrap();
    dbg!(time.elapsed());
}
