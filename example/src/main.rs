use iospng::Png;
use std::fs;
use std::time::Instant;

fn main() {
    let time = Instant::now();
    let data = fs::read("./data/ios.png").unwrap();
    let data = Png::restore(data).unwrap();
    fs::write("./data/origin.png", data).unwrap();
    dbg!(time.elapsed());
}
