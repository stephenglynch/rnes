use super::Mapper;
use crate::parse_ines::INes;

pub struct Mapper0 {
    ppu_ram: Vec<u8>,
    chr_rom: Vec<u8>,
    prg_rom: Vec<u8>,
    prg_ram: Vec<u8>
}

impl Mapper0 {
    pub fn new(ines: INes) -> Self {
        let prg_rom = ines.prg_rom;
        let chr_rom = ines.chr_rom.unwrap();
        // Mapper 0 must be 16k or 32k
        if prg_rom.len() != 16*1024 && prg_rom.len() != 32*1024 {
            panic!("Incompatible PRG ROM size for mapper 0 ({})", prg_rom.len());
        }
        if chr_rom.len() != 8*1024 {
            panic!("Incompatible CHR ROM size for mapper 0 ({})", chr_rom.len());
        }
        Self {
            ppu_ram: vec![0; 2048],
            prg_rom: prg_rom,
            prg_ram: vec![0; 2048],
            chr_rom: chr_rom
        }
    }

    fn prg_rom_mask(&self) -> usize {
        if self.prg_rom.len() == 32*1024 {
            0x7fff
        } else {
            0x3fff
        }
    }
}

impl Mapper for Mapper0 {
    fn get(&mut self, loc: usize) -> u8 {
        let mask = self.prg_rom_mask();
        match loc {
            0x6000.. 0x8000 => self.prg_ram[loc & 0x7ff],
            0x8000.. 0xc000 => self.prg_rom[loc & 0x3fff],
            0x8000..=0xffff => self.prg_rom[loc & mask],
            _ => 0
        }
    }

    fn set(&mut self, loc: usize, val: u8) {
        let mask = self.prg_rom_mask();
        match loc {
            0x6000.. 0x8000 => self.prg_ram[loc & 0x7ff] = val,
            0x8000.. 0xc000 => self.prg_rom[loc & 0x3fff] = val,
            0x8000..=0xffff => self.prg_rom[loc & mask] = val,
            _ => ()
        }
    }

    fn ppu_get(&mut self, addr: usize) -> u8 {
        match addr {
            0x0000..0x2000 => self.chr_rom[addr],
            0x2000..0x3f00 => self.ppu_ram[addr & 0x0fff],
            _ => 0
        }
    }

    fn ppu_set(&mut self, addr: usize, val: u8) {
        match addr {
            0x0000..0x2000 => self.chr_rom[addr] = val,
            0x2000..0x3f00 => self.ppu_ram[addr & 0x0fff] = val,
            _ => ()
        }
    }
}
