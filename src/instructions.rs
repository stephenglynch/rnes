use std::cmp::Ordering;

use crate::{system::*};

mod address_modes;
mod dest;
mod source;
use address_modes::*;
use bitflags::Flags;
use dest::Dest;
use source::Source;

fn bump_pc<A: AddrMode>(sys: &mut Cpu) {
    sys.registers.pc += A::size();
}

// Nop

fn nop(sys: &mut Cpu) {
    bump_pc::<Immediate>(sys);
}

// Transfer operations

fn load<A: AddrMode, D: Dest<A>, S: Source<A>>(sys: &mut Cpu) {
    D::set(sys, S::get(sys));
    bump_pc::<A>(sys);
}

fn store<A: AddrMode, D: Dest<A>, S: Source>(sys: &mut Cpu) {
    D::set(sys, S::get(sys));
    bump_pc::<A>(sys);
}

fn trans<D: Dest, S: Source>(sys: &mut Cpu) {
    D::set(sys, S::get(sys));
    bump_pc::<Immediate>(sys);
}

// Stack operations

fn push_raw(sys: &mut Cpu, val: u8) {
    let sp = sys.registers.sp as u16;
    sys.mmu_store(0x0100 + sp, val);
    sys.registers.sp -= 1;
}

fn push<S: Source>(sys: &mut Cpu) {
    let val = S::get(sys);
    push_raw(sys, val);
    bump_pc::<Immediate>(sys);
}

fn pull_raw(sys: &mut Cpu) -> u8 {
    let sp = sys.registers.sp as u16;
    sys.registers.sp += 1;
    sys.mmu_load(0x0100 + sp)
}

fn pull<D: Dest>(sys: &mut Cpu) {
    let val = pull_raw(sys);
    D::set(sys, val);
    bump_pc::<Immediate>(sys);
}

// Increment/Decrements

fn incr<A: AddrMode, D: Dest, S: Source>(sys: &mut Cpu) {
    let (val, overflow) = S::get(sys).overflowing_add(1);
    sys.registers.sr.set(StatusRegister::NEGATIVE, val & 0x80 != 0);
    sys.registers.sr.set(StatusRegister::ZERO, val == 0);
    D::set(sys, val);
    bump_pc::<A>(sys);
}

fn decr<A: AddrMode, D: Dest, S: Source>(sys: &mut Cpu) {
    let (val, overflow) = S::get(sys).overflowing_sub(1);
    sys.registers.sr.set(StatusRegister::NEGATIVE, val & 0x80 != 0);
    sys.registers.sr.set(StatusRegister::ZERO, val == 0);
    D::set(sys, val);
    bump_pc::<A>(sys);
}

// Math

fn adc<A: AddrMode, D: Dest<A>, S: Source<A>>(sys: &mut Cpu) {
    let val = S::get(sys) as u16;
    let carry = sys.registers.sr.contains(StatusRegister::CARRY) as u16;
    let ac = sys.registers.ac as u16;
    let total = ac + val + carry;
    let overflow = (total ^ ac) & (total ^ val) & 0x80 != 0;
    sys.registers.sr.set(StatusRegister::CARRY, total & 0x100 != 0);
    sys.registers.sr.set(StatusRegister::ZERO, total == 0);
    sys.registers.sr.set(StatusRegister::NEGATIVE, total & 0x80 != 0);
    sys.registers.sr.set(StatusRegister::OVERFLOW, overflow);
    D::set(sys, val as u8);
    bump_pc::<A>(sys);
}

fn sbc<A: AddrMode, D: Dest<A>, S: Source<A>>(sys: &mut Cpu) {
    let val = S::get(sys) as i16;
    let carry = sys.registers.sr.contains(StatusRegister::CARRY) as i16;
    let ac = sys.registers.ac as i16;
    let total = ac + val + carry;
    let overflow = (total ^ ac) & (total ^ !val) & 0x80 != 0;
    sys.registers.sr.set(StatusRegister::CARRY, total & 0x100 != 0);
    sys.registers.sr.set(StatusRegister::ZERO, total == 0);
    sys.registers.sr.set(StatusRegister::NEGATIVE, total & 0x80 != 0);
    sys.registers.sr.set(StatusRegister::OVERFLOW, overflow);
    D::set(sys, val as u8);
    bump_pc::<A>(sys);
}

// Logical

fn and<A: AddrMode, D: Dest<A>, S: Source<A>>(sys: &mut Cpu) {
    let val = S::get(sys);
    let result = val & sys.registers.ac;
    sys.registers.sr.set(StatusRegister::ZERO, result == 0);
    sys.registers.sr.set(StatusRegister::NEGATIVE, result & 0x80 != 0);
    D::set(sys, result);
    bump_pc::<A>(sys);
}

fn eor<A: AddrMode, D: Dest<A>, S: Source<A>>(sys: &mut Cpu) {
    let val = S::get(sys);
    let result = val ^ sys.registers.ac;
    sys.registers.sr.set(StatusRegister::ZERO, result == 0);
    sys.registers.sr.set(StatusRegister::NEGATIVE, result & 0x80 != 0);
    D::set(sys, result);
    bump_pc::<A>(sys);
}

fn or<A: AddrMode, D: Dest<A>, S: Source<A>>(sys: &mut Cpu) {
    let val = S::get(sys);
    let result = val | sys.registers.ac;
    sys.registers.sr.set(StatusRegister::ZERO, result == 0);
    sys.registers.sr.set(StatusRegister::NEGATIVE, result & 0x80 != 0);
    D::set(sys, result);
    bump_pc::<A>(sys);
}

// Shift

fn asl<A: AddrMode, D: Dest<A>, S: Source<A>>(sys: &mut Cpu) {
    let val = S::get(sys);
    let result = val << 1;
    let carry = val & 0x80 != 0;
    sys.registers.sr.set(StatusRegister::ZERO, result == 0);
    sys.registers.sr.set(StatusRegister::NEGATIVE, result & 0x80 != 0);
    sys.registers.sr.set(StatusRegister::CARRY, carry);
    D::set(sys, result as u8);
    bump_pc::<A>(sys);
}

fn lsr<A: AddrMode, D: Dest<A>, S: Source<A>>(sys: &mut Cpu) {
    let val = S::get(sys);
    let result = val >> 1;
    let carry = val & 0x01 != 0;
    sys.registers.sr.set(StatusRegister::ZERO, result == 0);
    sys.registers.sr.set(StatusRegister::NEGATIVE, result & 0x80 != 0);
    sys.registers.sr.set(StatusRegister::CARRY, carry);
    D::set(sys, result as u8);
    bump_pc::<A>(sys);
}

fn rol<A: AddrMode, D: Dest<A>, S: Source<A>>(sys: &mut Cpu) {
    let val = S::get(sys);
    let old_carry = sys.registers.sr.contains(StatusRegister::CARRY) as u8;
    let new_carry = val & 0x80 != 0;
    let mut result = val << 1;
    result |= old_carry;
    sys.registers.sr.set(StatusRegister::ZERO, result == 0);
    sys.registers.sr.set(StatusRegister::NEGATIVE, result & 0x80 != 0);
    sys.registers.sr.set(StatusRegister::CARRY, new_carry);
    D::set(sys, result as u8);
    bump_pc::<A>(sys);
}

fn ror<A: AddrMode, D: Dest<A>, S: Source<A>>(sys: &mut Cpu) {
    let val = S::get(sys);
    let old_carry = if sys.registers.sr.contains(StatusRegister::CARRY) {0x80} else {0x00};
    let new_carry = val & 0x01 != 0;
    let mut result = val >> 1;
    result |= old_carry;
    sys.registers.sr.set(StatusRegister::ZERO, result == 0);
    sys.registers.sr.set(StatusRegister::NEGATIVE, result & 0x80 != 0);
    sys.registers.sr.set(StatusRegister::CARRY, new_carry);
    D::set(sys, result as u8);
    bump_pc::<A>(sys);
}

// Flags

fn clc(sys: &mut Cpu) {
    sys.registers.sr.set(StatusRegister::CARRY, false);
    bump_pc::<Immediate>(sys);
}

fn cld(sys: &mut Cpu) {
    sys.registers.sr.set(StatusRegister::DECIMAL, false);
    bump_pc::<Immediate>(sys);
}

fn cli(sys: &mut Cpu) {
    sys.registers.sr.set(StatusRegister::INTERRUPT, false);
    bump_pc::<Immediate>(sys);
}

fn clv(sys: &mut Cpu) {
    sys.registers.sr.set(StatusRegister::OVERFLOW, false);
    bump_pc::<Immediate>(sys);
}

fn sec(sys: &mut Cpu) {
    sys.registers.sr.set(StatusRegister::CARRY, true);
    bump_pc::<Immediate>(sys);
}

fn sed(sys: &mut Cpu) {
    sys.registers.sr.set(StatusRegister::DECIMAL, true);
    bump_pc::<Immediate>(sys);
}

fn sei(sys: &mut Cpu) {
    sys.registers.sr.set(StatusRegister::INTERRUPT, true);
    bump_pc::<Immediate>(sys);
}

// Compare

fn cp<A: AddrMode, R: Source<A>, S: Source<A>>(sys: &mut Cpu) {
    let reg = R::get(sys);
    let operand = S::get(sys);
    match reg.cmp(&operand) {
        Ordering::Equal => {
            sys.registers.sr.set(StatusRegister::ZERO, false);
            sys.registers.sr.set(StatusRegister::CARRY, false);
        },
        Ordering::Less => {
            sys.registers.sr.set(StatusRegister::ZERO, true);
            sys.registers.sr.set(StatusRegister::CARRY, true);
        },
        Ordering::Greater => {
            sys.registers.sr.set(StatusRegister::ZERO, false);
            sys.registers.sr.set(StatusRegister::CARRY, true);
        }
    }
    bump_pc::<Immediate>(sys);
}

// Bit Test

fn bit<A: AddrMode, R: Source<A>, S: Source<A>>(sys: &mut Cpu) {
    let reg = R::get(sys);
    let operand = S::get(sys);
    let result = reg & operand;
    sys.registers.sr.set(StatusRegister::ZERO, result == 0);
    sys.registers.sr.set(StatusRegister::OVERFLOW, operand & (1 << 6) != 0);
    sys.registers.sr.set(StatusRegister::NEGATIVE, operand & (1 << 7) != 0);
}

// Conditional branching

fn bcc<A: AddrMode>(sys: &mut Cpu) {
    let test_bit = !sys.registers.sr.contains(StatusRegister::CARRY);
    let dest = A::get_addr(sys);
    if test_bit {
        sys.registers.pc = dest;
    }
    bump_pc::<A>(sys);
}
fn bcs<A: AddrMode>(sys: &mut Cpu) {
    let test_bit = sys.registers.sr.contains(StatusRegister::CARRY);
    let dest = A::get_addr(sys);
    if test_bit {
        sys.registers.pc = dest;
    }
    bump_pc::<A>(sys);
}
fn beq<A: AddrMode>(sys: &mut Cpu) {
    let test_bit = sys.registers.sr.contains(StatusRegister::ZERO);
    let dest = A::get_addr(sys);
    if test_bit {
        sys.registers.pc = dest;
    }
    bump_pc::<A>(sys);
}
fn bmi<A: AddrMode>(sys: &mut Cpu) {
    let test_bit = sys.registers.sr.contains(StatusRegister::NEGATIVE);
    let dest = A::get_addr(sys);
    if test_bit {
        sys.registers.pc = dest;
    }
    bump_pc::<A>(sys);
}
fn bne<A: AddrMode>(sys: &mut Cpu) {
    let test_bit = !sys.registers.sr.contains(StatusRegister::ZERO);
    let dest = A::get_addr(sys);
    if test_bit {
        sys.registers.pc = dest;
    }
    bump_pc::<A>(sys);
}
fn bpl<A: AddrMode>(sys: &mut Cpu) {
    let test_bit = !sys.registers.sr.contains(StatusRegister::NEGATIVE);
    let dest = A::get_addr(sys);
    if test_bit {
        sys.registers.pc = dest;
    }
    bump_pc::<A>(sys);
}
fn bvc<A: AddrMode>(sys: &mut Cpu) {
    let test_bit = !sys.registers.sr.contains(StatusRegister::OVERFLOW);
    let dest = A::get_addr(sys);
    if test_bit {
        sys.registers.pc = dest;
    }
    bump_pc::<A>(sys);
}
fn bvs<A: AddrMode>(sys: &mut Cpu) {
    let test_bit = sys.registers.sr.contains(StatusRegister::OVERFLOW);
    let dest = A::get_addr(sys);
    if test_bit {
        sys.registers.pc = dest;
    }
    bump_pc::<A>(sys);
}

// Jumps and subroutines

fn jmp<A: AddrMode>(sys: &mut Cpu) {
    let dest = A::get_addr(sys);
    sys.registers.pc = dest;
    bump_pc::<A>(sys);
}

fn jsr<A: AddrMode>(sys: &mut Cpu) {
    let dest = A::get_addr(sys);
    let ret_addr = sys.registers.pc + 2;
    let ret_addr_hi = ((ret_addr & 0xff00) >> 8) as u8;
    let ret_addr_lo = ret_addr as u8;
    push_raw(sys, ret_addr_hi);
    push_raw(sys, ret_addr_lo);
    sys.registers.pc = dest;
    bump_pc::<A>(sys);
}

fn rts(sys: &mut Cpu) {
    let ret_addr_lo = pull_raw(sys) as u16;
    let ret_addr_hi = pull_raw(sys) as u16;
    let ret_addr = ret_addr_lo | (ret_addr_hi << 8);
    sys.registers.pc = ret_addr;
    bump_pc::<Implied>(sys);
}

// Interrupts

pub fn execute(sys: &mut Cpu, instruction: u8) {
    (match instruction {
        0x00 => nop,
        0x4c => jmp::<Absolute>,
        0x6c => adc::<Immediate, dest::Accumulator, source::Memory>,
        0x6d => adc::<Absolute, dest::Accumulator, source::Memory>,
        0x78 => sei,
        _ => panic!()
    })(sys);
}