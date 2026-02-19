// TODO: Support command line args
use std::error::Error;
use std::path::PathBuf;
use clap::Parser;

mod parse_ines;
mod execute_prg;
mod mapper;
mod instructions;
mod system;
mod clock;
mod chip;
mod ppu;
mod renderer;
mod gamepad_manager;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Path to a .nes file to run
    #[arg(value_name = "FILE")]
    nes: Option<PathBuf>,

    /// Turn debugging information on
    #[arg(short, long, action = clap::ArgAction::Count)]
    debug: u8,
}


fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    if let Some(nes_path) = cli.nes.as_deref() {
        let ines = parse_ines::read_ines(nes_path).unwrap();
        execute_prg::execute_rom(ines);
    }
    Ok(())
}
