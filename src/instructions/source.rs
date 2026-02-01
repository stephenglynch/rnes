use crate::system::Cpu;
use super::address_modes::*;

pub trait Source<A: AddrMode=Immediate> {
    fn get(sys: &mut Cpu, addr_mode: &mut A) -> u8;
}

pub struct Accumulator;
impl <A: AddrMode> Source<A> for Accumulator {
    fn get(sys: &mut Cpu, _addr_mode: &mut A) -> u8 {
        sys.registers.ac
    }
}

pub struct IndexX;
impl <A: AddrMode> Source<A> for IndexX {
    fn get(sys: &mut Cpu, _addr_mode: &mut A) -> u8 {
        sys.registers.x
    }
}

pub struct IndexY;
impl <A: AddrMode> Source<A> for IndexY {
    fn get(sys: &mut Cpu, _addr_mode: &mut A) -> u8 {
        sys.registers.y
    }
}

pub struct StackPointer;
impl <A: AddrMode> Source<A> for StackPointer {
    fn get(sys: &mut Cpu, _addr_mode: &mut A) -> u8 {
        sys.registers.sp
    }
}

pub struct StatusRegister;
impl <A: AddrMode> Source<A> for StatusRegister {
    fn get(sys: &mut Cpu, _addr_mode: &mut A) -> u8 {
        sys.registers.sr.bits()
    }
}

pub struct Memory;
impl <A: AddrMode + Default> Source<A> for Memory {
    fn  get(sys: &mut Cpu, addr_mode: &mut A) -> u8 {
        let addr = addr_mode.get_addr(sys);
        sys.mmu_load(addr)
    }
}
