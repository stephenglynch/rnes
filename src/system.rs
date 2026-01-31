use std::rc::Rc;
use std::cell::RefCell;
use bitflags::bitflags;
use crate::instructions::execute;
use crate::ppu::Ppu;
use crate::apu::Apu;
use crate::clock::Clock;

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
        const STANDARD_FLAGS = 0b11001111;
        const UNUSED_FLAGS = !Self::STANDARD_FLAGS.bits();
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
        let sr = StatusRegister::INTERRUPT | StatusRegister::IGNORED;
        Registers { pc: 0, ac: 0, x: 0, y: 0, sp: 0, sr: sr }
    }
}

enum Memory {
    Ram,
    PrgRom,
    PrgRam,
    Oam,
    ApuRegs,
    PpuRegs,
}

pub struct Cpu {
    pub registers: Registers,
    pub clock: Rc<RefCell<Clock>>,
    ram: Vec<u8>,
    prg_rom: Vec<u8>,
    prg_ram: Vec<u8>,
    apu: Apu,
    ppu: Ppu
}

impl Cpu {
    pub fn new(clock: Rc<RefCell<Clock>>, prg_rom: Vec<u8>, chr_rom: Vec<u8>) -> Self {
        let mut cpu = Cpu {
            clock: clock,
            registers: Registers::new(),
            ram: vec![0; 2048],
            prg_rom: prg_rom,
            prg_ram: vec![0; 2048],
            apu: Apu::new(),
            ppu: Ppu::new(chr_rom)
        };
        cpu.reset();
        cpu
    }

    fn reset(&mut self) {
        // Resolve reset vector
        // let pc_lo = self.mmu_load(0xfffc) as u16;
        // let pc_hi = self.mmu_load(0xfffd) as u16;
        // self.registers.pc = pc_lo | (pc_hi << 8);
        self.registers.pc = 0xc000;
        self.registers.sp = self.registers.sp.wrapping_sub(3);
    }

    fn mmu_resolve(&self, addr: u16) -> (Memory, usize) {
        let prog_rom_len = self.prg_rom.len();
        let (mem, loc) = match addr {
            0x0000..0x2000 => (Memory::Ram, addr & 0xfff),
            0x2000..0x4000 => (Memory::PpuRegs, addr & 0x0007),
            0x4014         => (Memory::Oam, 0),
            0x4000..0x401f => (Memory::ApuRegs, addr & 0x00ff),
            0x401f..0x6000 => panic!("Unused - what behaviour should occur here?"),
            0x6000..0x8000 => (Memory::PrgRam, addr & 0xfff),
            0x8000..0xc000 => (Memory::PrgRom, addr & 0x3fff),
            0xc000..=0xffff=> (Memory::PrgRom, prog_rom_len as u16 - 0x4000 + addr & 0x3fff)
        };
        (mem, loc as usize)
    }

    pub fn mmu_load(&mut self, addr: u16) -> u8 {
        let (mem, loc) = self.mmu_resolve(addr);
        match mem {
            Memory::Ram => self.ram[loc],
            Memory::PrgRam => self.prg_ram[loc],
            Memory::PrgRom => self.prg_rom[loc],
            Memory::ApuRegs => 0,
            Memory::PpuRegs => self.ppu.get_reg(loc),
            Memory::Oam => 0,
        }
    }

    pub fn mmu_store(&mut self, addr: u16, val: u8) {
        let (mem, loc) = self.mmu_resolve(addr);
        match mem {
            Memory::Ram => self.ram[loc] = val,
            Memory::PrgRam => self.prg_ram[loc] = val,
            Memory::PrgRom => self.prg_rom[loc] = val,
            Memory::ApuRegs => (),
            Memory::PpuRegs => self.ppu.set_reg(loc, val),
            Memory::Oam => self.oam_transfer(val)
        }
    }

    fn oam_transfer(&mut self, hi_addr: u8) {
        // TODO: this consumes some number of cycles
        let hi_addr = hi_addr as u16;
        for lo_addr in 0..256 {
            let addr = (hi_addr << 8) | lo_addr;
            let val = self.mmu_load(addr);
            self.ppu.write_oam(val);
        }
    }

    pub async fn run(mut self) {
        loop {
            let pc = self.registers.pc;
            let a = self.registers.ac;
            let x = self.registers.x;
            let y = self.registers.y;
            let p = self.registers.sr.bits();
            let sp = self.registers.sp;
            println!("{:04x}     A: {:02x} X: {:02x} Y: {:02x} P: {:02x} SP = {:02x}",
                pc, a, x, y, p, sp);
            let next_instruction = self.mmu_load(pc);
            execute(&mut self, next_instruction).await;
        }
    }
}