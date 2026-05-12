use bitflags::bitflags;
use super::Mapper;
use crate::parse_ines::INes;

bitflags! {
    /// Represents a set of flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    struct ControlBits: u8 {
        const NAME_TABLE_ARRANGEMENT_0 = (1 << 0);
        const NAME_TABLE_ARRANGEMENT_1 = (1 << 1);
        const PRG_BANK_MODE_0 = (1 << 2);
        const PRG_BANK_MODE_1 = (1 << 3);
        const CHR_BANK_MODE = (1 << 4);
    }
}

pub struct Mapper1 {
    // Registers
    control: ControlBits,
    chr_bank_0: u8,
    chr_bank_1: u8,
    prg_bank: u8,
    // Shift register
    shift_reg: u8,
    shift_reg_count: u8,
    // Memories
    ppu_ram: Vec<u8>,
    chr: Vec<u8>,
    prg_rom: Vec<u8>,
    prg_ram: Vec<u8>,

}

impl Mapper1 {
    pub fn new(ines: INes) -> Self {
        // TODO: Handle CHR configuration from iNES
        let prg_rom = ines.prg_rom;
        // let chr_rom = ines.chr_rom.unwrap_or(vec![0; 8*1024]);
        Self {
            control: ControlBits::from_bits_truncate(0x0c),
            chr_bank_0: 0,
            chr_bank_1: 0,
            prg_bank: 0,
            shift_reg: 0,
            shift_reg_count: 0,
            ppu_ram: vec![0; 2048],
            chr: vec![0; 128*1024],
            prg_rom: prg_rom,
            prg_ram: vec![0; 32*1024],
        }
    }

    fn resolve_ram_addr(&self, addr: usize) -> usize {
        let nt_bits = (
            self.control.contains(ControlBits::NAME_TABLE_ARRANGEMENT_1),
            self.control.contains(ControlBits::NAME_TABLE_ARRANGEMENT_0)
        );
        match nt_bits {
            // One Screen, lower bank
            (false, false) => addr & 0x03ff, // TODO: Need to double check
            // One screen, upper bank
            (false, true) => (addr & 0x03ff) | 0x400 , // TODO: Need to double check
            // Vertical mirroring
            (true, false) => addr & 0x07ff,
            // Horizontal mirroring
            (true, true) => (addr & 0x03ff) | ((addr & 0x800) >> 1)
        }
    }

    fn resolve_chr_addr(&self, addr: usize) -> usize {
        let low_part;
        let high_part;
        if self.control.contains(ControlBits::CHR_BANK_MODE) {
            // 4kB mode
            low_part = addr & 0xfff;
            high_part = if addr & 0x1000 == 0 {
                self.chr_bank_0
            } else {
                self.chr_bank_1
            } as usize;
        } else {
            // 8kB mode
            low_part = addr & 0x1fff;
            high_part = (self.chr_bank_0 & 0x1e) as usize;
        }
        (high_part << 12) | low_part
    }

    fn resolve_rom_addr(&self, addr: usize) -> usize {
        // addr 0x0000 - 0x7fff
        let addr = addr & 0x7fff;
        let rom_bits = (
            self.control.contains(ControlBits::PRG_BANK_MODE_1),
            self.control.contains(ControlBits::PRG_BANK_MODE_0)
        );
        match rom_bits {
            // Switch 32 kB
            (false, _) => {
                let low_part = addr;
                let high_part = (self.prg_bank & 0x0e) as usize;
                (high_part << 14) | low_part
            }
            // Fix first bank at 0x8000, switch 16 kB at 0xc000
            (true, false) => {
                if addr < 0x4000 {
                    addr
                } else {
                    let low_part = 0x3fff & addr;
                    let high_part = (self.prg_bank & 0x0f) as usize;
                    (high_part << 14) | low_part
                }
            }
            // Switch 16 kB at 0x8000, fix last bank at 0xc000
            (true, true) => {
                if addr >= 0x4000 {
                    let addr = addr & 0x3fff;
                    addr | (self.prg_rom.len() - 16*1024)
                } else {
                    let low_part = 0x3fff & addr;
                    let high_part = (self.prg_bank & 0x0f) as usize;
                    (high_part << 14) | low_part
                }
            }
        }
    }

    fn set_reg(&mut self, addr: usize, val: u8) {
        let addr = addr & 0x6000;
        match addr {
            0x0000 => self.control = ControlBits::from_bits_truncate(val),
            0x2000 => self.chr_bank_0 = val,
            0x4000 => self.chr_bank_1 = val,
            0x6000 => self.prg_bank = val,
            _ => unreachable!()
        }
    }

    fn set_shift_reg(&mut self, addr: usize, val: u8) {
        let addr = addr & 0xffff;
        if 0x80 & val != 0 {
            self.shift_reg = 0;
            self.shift_reg_count = 0;
        } else {
            self.shift_reg >>= 1;
            self.shift_reg_count += 1;
            self.shift_reg |= (val & 0x01) << 4;
            if self.shift_reg_count == 5 {
                self.set_reg(addr, self.shift_reg);
                self.shift_reg_count = 0;
                self.shift_reg = 0;
            }
        }
    }
}

impl Mapper for Mapper1 {
    fn get(&mut self, loc: usize) -> u8 {
        match loc {
            0x6000.. 0x8000 => self.prg_ram[loc & 0x1fff],
            0x8000..=0xffff => {
                let addr = self.resolve_rom_addr(loc) & 0x3ffff;
                self.prg_rom[addr]
            }
            _ => 0
        }
    }

    fn set(&mut self, loc: usize, val: u8) {
        match loc {
            0x6000.. 0x8000 => self.prg_ram[loc & 0x1fff] = val,
            0x8000..=0xffff => self.set_shift_reg(loc & 0x6000, val),
            _ => ()
        }
    }

    fn ppu_get(&mut self, addr: usize) -> u8 {
        match addr {
            0x0000..0x2000 => self.chr[self.resolve_chr_addr(addr)],
            0x2000..0x3f00 => self.ppu_ram[self.resolve_ram_addr(addr)],
            _ => 0
        }
    }

    fn ppu_set(&mut self, addr: usize, val: u8) {
        match addr {
            0x0000..0x2000 => {
                let addr = self.resolve_chr_addr(addr);
                self.chr[addr] = val;
            },
            0x2000..0x3f00 => {
                let addr = self.resolve_ram_addr(addr);
                self.ppu_ram[addr] = val;
            }
            _ => ()
        }
    }
}
