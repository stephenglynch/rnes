mod parse_ines;
mod execute_prg;
mod instructions;
mod system;

fn main() {
    println!("Hello, world!");
    let ines = parse_ines::read_ines("nestest.nes").unwrap();
    execute_prg::execute_rom(ines);
}
