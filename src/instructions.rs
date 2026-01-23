use crate::address_modes::{*};
use crate::system::{*};


pub fn sei(sys: &mut Cpu) {
    sys.registers.sr |= StatusRegister::INTERRUPT;
}

pub fn add<A: AddrMode>(sys: &mut Cpu) {
    let val = A::read_addr(sys);
    sys.registers.ac = val + sys.registers.ac;
}

pub fn jmp<A: AddrMode>(sys: &mut Cpu) {
    let dest = A::get_addr(sys);
    sys.registers.pc = dest;
}