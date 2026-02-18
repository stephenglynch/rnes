// TODO: Support command line args
use std::error::Error;

mod parse_ines;
mod execute_prg;
mod instructions;
mod system;
mod clock;
mod chip;
mod ppu;
mod renderer;
mod gamepad_manager;

fn main() -> Result<(), Box<dyn Error>> {
    let ines = parse_ines::read_ines("nestest.nes").unwrap();
    execute_prg::execute_rom(ines);
    Ok(())
}
