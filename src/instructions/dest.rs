use crate::system;
use crate::system::Cpu;
use super::address_modes::*;

pub trait Dest<A: AddrMode=Immediate> {
    fn set(sys: &mut Cpu, val: u8);
}

pub struct Accumulator;
impl <A: AddrMode> Dest<A> for Accumulator {
    fn set(sys: &mut Cpu, val: u8) {
        sys.registers.ac = val;
    }
}

pub struct IndexX;
impl <A: AddrMode> Dest<A> for IndexX {
    fn set(sys: &mut Cpu, val: u8) {
        sys.registers.x = val;
    }
}

pub struct IndexY;
impl <A: AddrMode> Dest<A> for IndexY {
    fn set(sys: &mut Cpu, val: u8) {
        sys.registers.y = val;
    }
}

pub struct StackPointer;
impl <A: AddrMode> Dest<A> for StackPointer {
    fn set(sys: &mut Cpu, val: u8) {
        sys.registers.sp = val;
    }
}

pub struct StatusRegister;
impl <A: AddrMode> Dest<A> for StatusRegister {
    fn set(sys: &mut Cpu, val: u8) {
        sys.registers.sr = system::StatusRegister::from_bits_retain(val);
    }
}

pub struct Memory;
impl <A: AddrMode> Dest<A> for Memory {
    fn  set(sys: &mut Cpu, val: u8) {
        let addr = A::get_addr(sys);
        sys.mmu_store(addr, val);
    }
}
