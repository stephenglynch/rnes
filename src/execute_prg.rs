use crate::system::{*};
use crate::address_modes::{*};
use crate::parse_ines::INes;

pub fn execute_rom(ines: INes) {
    // Note: suppport CHR-RAM
    let mut system = Cpu::new(ines.prg_rom, ines.chr_rom.unwrap());
    loop {
        system.run();
    }
}