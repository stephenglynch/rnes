use bitflags::bitflags;
use crate::instructions::execute;

bitflags! {
    /// Represents a set of flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct StatusRegister: u8 {
        const CARRY = (1 << 0);
        const ZERO = (1 << 1);
        const INTERRUPT = (1 << 2);
        const DECIMAL = (1 << 3);
        const BREAK = (1 << 4);
        const IGNORED = (1 << 5);
        const OVERFLOW = (1 << 6);
        const NEGATIVE = (1 << 7);
    }
}

pub struct Registers {
    pub pc: u16,
    pub ac: u8,
    pub x: u8,
    pub y: u8,
    pub sp: u8,
    pub sr: StatusRegister
}

impl Registers {
    fn new() -> Self {
        Registers { pc: 0, ac: 0, x: 0, y: 0, sp: 0, sr: StatusRegister::INTERRUPT }
    }
}

enum Memory {
    Ram,
    PrgRom,
    PrgRam,
    ChrRom,
}

pub struct Cpu {
    pub registers: Registers,
    ram: Vec<u8>,
    prg_rom: Vec<u8>,
    chr_rom: Vec<u8>,
    prg_ram: Vec<u8>
}

impl Cpu {
    pub fn new(prg_rom: Vec<u8>, chr_rom: Vec<u8>) -> Self {
        let mut cpu = Cpu {
            registers: Registers::new(),
            ram: vec![0; 2048],
            prg_rom: prg_rom,
            chr_rom: chr_rom,
            prg_ram: vec![0; 2048]
        };
        cpu.reset();
        cpu
    }

    fn reset(&mut self) {
        // Resolve reset vector
        let pc_lo = self.mmu_load(0xfffc) as u16;
        let pc_hi = self.mmu_load(0xfffd) as u16;
        self.registers.pc = pc_lo | (pc_hi << 8);
        self.registers.sp = self.registers.sp.wrapping_sub(3);
    }

    fn mmu_resolve(&self, addr: u16) -> (Memory, usize) {
        let prog_rom_len = self.prg_rom.len();
        match addr {
            0x0000..0x2000 => (Memory::Ram, addr as usize & 0xfff),
            0x2000..0x4000 => unimplemented!("Not implemented PPU!"),
            0x4000..0x401f => unimplemented!("Not implemented APU!"),
            0x401f..0x6000 => panic!("Unused - what behaviour should occur here?"),
            0x6000..0x8000 => (Memory::PrgRam, addr as usize & 0xfff),
            0x8000..0xc000 => (Memory::PrgRom, addr as usize & 0x3fff),
            0xc000..=0xffff=> (Memory::PrgRom, prog_rom_len - 0x4000 + addr as usize & 0x3fff)
        }
    }

    pub fn mmu_load(&self, addr: u16) -> u8 {
        let (mem, loc) = self.mmu_resolve(addr);
        match mem {
            Memory::Ram => self.ram[loc],
            Memory::PrgRam => self.prg_ram[loc],
            Memory::PrgRom => self.prg_rom[loc],
            _ => unreachable!("Unexpected memory")
        }
    }

    pub fn mmu_store(&mut self, addr: u16, val: u8) {
        let (mem, loc) = self.mmu_resolve(addr);
        match mem {
            Memory::Ram => self.ram[loc] = val,
            Memory::PrgRam => self.prg_ram[loc] = val,
            Memory::PrgRom => self.prg_rom[loc] = val,
            _ => unreachable!("Unexpected memory")
        }
    }

    pub fn run(&mut self) {
        let pc = self.registers.pc;
        let next_instruction = self.mmu_load(pc);
        execute(self, next_instruction);
    }
}