// TODO: Support command line args
use std::error::Error;
use std::path::PathBuf;
use clap::Parser;

use crate::audio::Audio;
use crate::parse_ines::INes;

mod parse_ines;
mod execute_prg;
mod mapper;
mod instructions;
mod system;
mod clock;
mod chip;
mod ppu;
mod renderer;
mod input;
mod audio;
mod system_control;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Path to a .nes file to run
    #[arg(value_name = "FILE")]
    nes: Option<PathBuf>,

    /// Provide audio device
    #[arg(short, long)]
    audio_device: Option<String>,

    /// Turn debugging information on
    #[arg(short, long, action = clap::ArgAction::Count)]
    debug: u8,
}

pub struct Config {
    ines: INes,
    audio: Audio
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    let audio = audio::Audio::new(cli.audio_device)?;

    if let Some(nes_path) = cli.nes.as_deref() {
        let ines = parse_ines::read_ines(nes_path).unwrap();
        execute_prg::execute_rom(Config {
            ines: ines,
            audio: audio
        })?;
    }
    Ok(())
}
