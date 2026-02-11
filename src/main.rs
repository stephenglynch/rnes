use std::error::Error;

mod parse_ines;
mod execute_prg;
mod instructions;
mod system;
mod clock;
mod ppu;
mod apu;
mod renderer;
mod gamepad_manager;

fn main() -> Result<(), Box<dyn Error>> {
    // let delay = CycleDelay::new(&mut clock, 123);
    // renderer::render()?;
    let gamepad_manager = gamepad_manager::GamepadManager::new();
    gamepad_manager.start();
    let ines = parse_ines::read_ines("nestest.nes").unwrap();
    execute_prg::execute_rom(ines);
    Ok(())
}
