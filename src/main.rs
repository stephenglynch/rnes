mod parse_ines;

fn main() {
    println!("Hello, world!");
    parse_ines::read_ines("nestest.nes").unwrap();
}
