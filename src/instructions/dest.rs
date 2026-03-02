use crate::system;
use crate::system::Cpu;
use super::address_modes::*;

pub trait Dest<A: AddrMode=Immediate> {
    async fn set(sys: &mut Cpu, addr_mode: &mut A, val: u8);
}

pub struct Accumulator;
impl <A: AddrMode> Dest<A> for Accumulator {
    async fn set(sys: &mut Cpu, _addr_mode: &mut A, val: u8) {
        sys.registers.ac = val;
    }
}

pub struct IndexX;
impl <A: AddrMode> Dest<A> for IndexX {
    async fn set(sys: &mut Cpu, _addr_mode: &mut A, val: u8) {
        sys.registers.x = val;
    }
}

pub struct IndexY;
impl <A: AddrMode> Dest<A> for IndexY {
    async fn set(sys: &mut Cpu, _addr_mode: &mut A, val: u8) {
        sys.registers.y = val;
    }
}

#[allow(dead_code)]
pub struct StackPointer;
impl <A: AddrMode> Dest<A> for StackPointer {
    async fn set(sys: &mut Cpu, _addr_mode: &mut A, val: u8) {
        sys.registers.sp = val;
    }
}

#[allow(dead_code)]
pub struct StatusRegister;
impl <A: AddrMode> Dest<A> for StatusRegister {
    async fn set(sys: &mut Cpu, _addr_mode: &mut A, val: u8) {
        sys.registers.sr = system::StatusRegister::from_bits_retain(val);
    }
}

pub struct Memory;
impl <A: AddrMode> Dest<A> for Memory {
    async fn set(sys: &mut Cpu, addr_mode: &mut A, val: u8) {
        let addr = addr_mode.get_addr(sys);
        sys.mmu_store(addr, val).await;
    }
}
