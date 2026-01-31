use std::error::Error;

mod parse_ines;
mod execute_prg;
mod instructions;
mod system;
mod clock;
mod ppu;
mod apu;
mod renderer;

fn main() -> Result<(), Box<dyn Error>> {
    // let delay = CycleDelay::new(&mut clock, 123);
    // renderer::render()?;
    let ines = parse_ines::read_ines("nestest.nes").unwrap();
    execute_prg::execute_rom(ines);
    Ok(())
}
