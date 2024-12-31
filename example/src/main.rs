use std::fs;
use iospng::png::Png;

fn main() {
    let data = fs::read("./data/ios.png").unwrap();
    let data = Png::restore(data).unwrap();
    fs::write("./origin.png", data).unwrap();
}
