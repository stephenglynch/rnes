use std::cmp::Ordering;

use crate::system::{Cpu, StatusRegister};

mod address_modes;
mod dest;
mod source;
use address_modes::*;
use dest::Dest;
use source::Source;

fn bump_pc<A: AddrMode>(sys: &mut Cpu) {
    sys.registers.pc += A::size();
}

fn update_zero_status(sys: &mut Cpu, val: u8) {
    sys.registers.sr.set(StatusRegister::ZERO, val == 0);
}

fn update_negative_status(sys: &mut Cpu, val: u8) {
    sys.registers.sr.set(StatusRegister::NEGATIVE, val & 0x80 != 0);
}

// Nop

fn nop(sys: &mut Cpu) {
    bump_pc::<Implied>(sys);
}

// Transfer operations

fn load<A: AddrMode, D: Dest<A>, S: Source<A>>(sys: &mut Cpu) {
    let val = S::get(sys);
    D::set(sys, val);
    update_zero_status(sys, val);
    update_negative_status(sys, val);
    bump_pc::<A>(sys);
}

fn store<A: AddrMode, D: Dest<A>, S: Source>(sys: &mut Cpu) {
    D::set(sys, S::get(sys));
    bump_pc::<A>(sys);
}

fn trans<D: Dest, S: Source>(sys: &mut Cpu) {
    let val = S::get(sys);
    D::set(sys, val);
    update_zero_status(sys, val);
    update_negative_status(sys, val);
    bump_pc::<Implied>(sys);
}

// Stack operations

fn push_raw(sys: &mut Cpu, val: u8) {
    let sp = sys.registers.sp as u16;
    sys.mmu_store(0x0100 + sp, val);
    sys.registers.sp -= 1;
}

fn php(sys: &mut Cpu) {
    let p = sys.registers.sr | StatusRegister::BREAK | StatusRegister::IGNORED;
    push_raw(sys, p.bits());
    bump_pc::<Implied>(sys);
}

fn pha(sys: &mut Cpu) {
    push_raw(sys, sys.registers.ac);
    bump_pc::<Implied>(sys);
}

fn pull_raw(sys: &mut Cpu) -> u8 {
    sys.registers.sp += 1;
    sys.mmu_load(0x0100 + sys.registers.sp as u16)
}

fn pla(sys: &mut Cpu) {
    let val = pull_raw(sys);
    update_zero_status(sys, val);
    update_negative_status(sys, val);
    sys.registers.ac = val;
    bump_pc::<Implied>(sys);
}

fn plp(sys: &mut Cpu) {
    let val = pull_raw(sys);
    let sr = sys.registers.sr.bits();
    sys.registers.sr = StatusRegister::from_bits_retain(
        val & StatusRegister::STANDARD_FLAGS.bits() |
        sr & StatusRegister::UNUSED_FLAGS.bits()
    );
    bump_pc::<Implied>(sys);
}

// Increment/Decrements

fn incr<A: AddrMode, D: Dest, S: Source>(sys: &mut Cpu) {
    let result= S::get(sys).wrapping_add(1);
    update_zero_status(sys, result);
    update_negative_status(sys, result);
    D::set(sys, result);
    bump_pc::<A>(sys);
}

fn decr<A: AddrMode, D: Dest, S: Source>(sys: &mut Cpu) {
    let result = S::get(sys).wrapping_sub(1);
    update_zero_status(sys, result);
    update_negative_status(sys, result);
    D::set(sys, result);
    bump_pc::<A>(sys);
}

// Math

fn adc<A: AddrMode, S: Source<A>>(sys: &mut Cpu) {
    let val = S::get(sys) as u16;
    let carry = sys.registers.sr.contains(StatusRegister::CARRY) as u16;
    let ac = sys.registers.ac as u16;
    let total = ac + val + carry;
    let overflow = (total ^ ac) & (total ^ val) & 0x80 != 0;
    sys.registers.sr.set(StatusRegister::CARRY, total & 0x100 != 0);
    sys.registers.sr.set(StatusRegister::OVERFLOW, overflow);
    update_zero_status(sys, total as u8);
    update_negative_status(sys, total as u8);
    sys.registers.ac = total as u8;
    bump_pc::<A>(sys);
}

fn sbc<A: AddrMode, S: Source<A>>(sys: &mut Cpu) {
    let val = S::get(sys) as i16;
    let carry = sys.registers.sr.contains(StatusRegister::CARRY) as i16;
    let ac = sys.registers.ac as i16;
    let total = ac - val - carry;
    let overflow = (total ^ ac) & (total ^ !val) & 0x80 != 0;
    sys.registers.sr.set(StatusRegister::CARRY, total & 0x100 != 0);
    sys.registers.sr.set(StatusRegister::OVERFLOW, overflow);
    update_zero_status(sys, total as u8);
    update_negative_status(sys, total as u8);
    sys.registers.ac = total as u8;
    bump_pc::<A>(sys);
}

// Logical

fn and<A: AddrMode, D: Dest<A>, S: Source<A>>(sys: &mut Cpu) {
    let val = S::get(sys);
    let result = val & sys.registers.ac;
    update_zero_status(sys, result);
    update_negative_status(sys, result);
    D::set(sys, result);
    bump_pc::<A>(sys);
}

fn eor<A: AddrMode, D: Dest<A>, S: Source<A>>(sys: &mut Cpu) {
    let val = S::get(sys);
    let result = val ^ sys.registers.ac;
    update_zero_status(sys, result);
    update_negative_status(sys, result);
    D::set(sys, result);
    bump_pc::<A>(sys);
}

fn or<A: AddrMode, D: Dest<A>, S: Source<A>>(sys: &mut Cpu) {
    let val = S::get(sys);
    let result = val | sys.registers.ac;
    update_zero_status(sys, result);
    update_negative_status(sys, result);
    D::set(sys, result);
    bump_pc::<A>(sys);
}

// Shift

fn asl<A: AddrMode, D: Dest<A>, S: Source<A>>(sys: &mut Cpu) {
    let val = S::get(sys);
    let result = val << 1;
    let carry = val & 0x80 != 0;
    update_zero_status(sys, result);
    update_negative_status(sys, result);
    sys.registers.sr.set(StatusRegister::CARRY, carry);
    D::set(sys, result as u8);
    bump_pc::<A>(sys);
}

fn lsr<A: AddrMode, D: Dest<A>, S: Source<A>>(sys: &mut Cpu) {
    let val = S::get(sys);
    let result = val >> 1;
    let carry = val & 0x01 != 0;
    update_zero_status(sys, result);
    update_negative_status(sys, result);
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
    update_zero_status(sys, result);
    update_negative_status(sys, result);
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
    update_zero_status(sys, result);
    update_negative_status(sys, result);
    sys.registers.sr.set(StatusRegister::CARRY, new_carry);
    D::set(sys, result as u8);
    bump_pc::<A>(sys);
}

// Flags

fn clc(sys: &mut Cpu) {
    sys.registers.sr.set(StatusRegister::CARRY, false);
    bump_pc::<Implied>(sys);
}

fn cld(sys: &mut Cpu) {
    sys.registers.sr.set(StatusRegister::DECIMAL, false);
    bump_pc::<Implied>(sys);
}

fn cli(sys: &mut Cpu) {
    sys.registers.sr.set(StatusRegister::INTERRUPT, false);
    bump_pc::<Implied>(sys);
}

fn clv(sys: &mut Cpu) {
    sys.registers.sr.set(StatusRegister::OVERFLOW, false);
    bump_pc::<Implied>(sys);
}

fn sec(sys: &mut Cpu) {
    sys.registers.sr.set(StatusRegister::CARRY, true);
    bump_pc::<Implied>(sys);
}

fn sed(sys: &mut Cpu) {
    sys.registers.sr.set(StatusRegister::DECIMAL, true);
    bump_pc::<Implied>(sys);
}

fn sei(sys: &mut Cpu) {
    sys.registers.sr.set(StatusRegister::INTERRUPT, true);
    bump_pc::<Implied>(sys);
}

// Compare

fn cp<A: AddrMode, R: Source<A>, S: Source<A>>(sys: &mut Cpu) {
    let reg = R::get(sys);
    let operand = S::get(sys);
    match reg.cmp(&operand) {
        Ordering::Less => {
            sys.registers.sr.set(StatusRegister::ZERO, false);
            sys.registers.sr.set(StatusRegister::CARRY, false);
        },
        Ordering::Equal => {
            sys.registers.sr.set(StatusRegister::ZERO, true);
            sys.registers.sr.set(StatusRegister::CARRY, true);
        },
        Ordering::Greater => {
            sys.registers.sr.set(StatusRegister::ZERO, false);
            sys.registers.sr.set(StatusRegister::CARRY, true);
        }
    }
    update_negative_status(sys, reg.wrapping_sub(operand));
    bump_pc::<A>(sys);
}

// Bit Test

fn bit<A: AddrMode, S: Source<A>>(sys: &mut Cpu) {
    let reg = sys.registers.ac;
    let operand = S::get(sys);
    let result = reg & operand;
    sys.registers.sr.set(StatusRegister::ZERO, result == 0);
    sys.registers.sr.set(StatusRegister::OVERFLOW, operand & (1 << 6) != 0);
    sys.registers.sr.set(StatusRegister::NEGATIVE, operand & (1 << 7) != 0);
    bump_pc::<A>(sys);
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
}

fn jsr<A: AddrMode>(sys: &mut Cpu) {
    let dest = A::get_addr(sys);
    let ret_addr = sys.registers.pc + 2;
    let ret_addr_hi = ((ret_addr & 0xff00) >> 8) as u8;
    let ret_addr_lo = ret_addr as u8;
    push_raw(sys, ret_addr_hi);
    push_raw(sys, ret_addr_lo);
    sys.registers.pc = dest;
}

fn rts(sys: &mut Cpu) {
    let ret_addr_lo = pull_raw(sys) as u16;
    let ret_addr_hi = pull_raw(sys) as u16;
    let ret_addr = ret_addr_lo | (ret_addr_hi << 8);
    sys.registers.pc = ret_addr;
    bump_pc::<Implied>(sys);
}

// Interrupts

fn brk(_sys: &mut Cpu) {
    unimplemented!();
}

fn rti(sys: &mut Cpu) {
    let mut sr = sys.registers.sr.bits();
    let flags = pull_raw(sys);
    sr |= flags & StatusRegister::STANDARD_FLAGS.bits();
    sr &= !(!flags & StatusRegister::STANDARD_FLAGS.bits());
    let ret_addr_lo = pull_raw(sys) as u16;
    let ret_addr_hi = pull_raw(sys) as u16;
    let ret_addr = ret_addr_lo | (ret_addr_hi << 8);
    sys.registers.sr = StatusRegister::from_bits_retain(sr);
    sys.registers.pc = ret_addr;
}

pub fn execute(sys: &mut Cpu, instruction: u8) {
    (match instruction {
        0x00 => brk,
        0x01 => or::<PreIndexed, dest::Accumulator, source::Memory>,
        0x05 => or::<ZeroPage, dest::Accumulator, source::Memory>,
        0x06 => asl::<ZeroPage, dest::Memory, source::Memory>,
        0x08 => php,
        0x09 => or::<Immediate, dest::Accumulator, source::Memory>,
        0x0A => asl::<Implied, dest::Accumulator, source::Accumulator>,
        0x0D => or::<Absolute, dest::Accumulator, source::Memory>,
        0x0E => asl::<Absolute, dest::Memory, source::Memory>,
        0x10 => bpl::<Relative>,
        0x11 => or::<PostIndexed, dest::Accumulator, source::Memory>,
        0x15 => or::<ZPIndexedX, dest::Accumulator, source::Memory>,
        0x16 => asl::<ZPIndexedX, dest::Memory, source::Memory>,
        0x18 => clc,
        0x19 => or::<IndexedY, dest::Accumulator, source::Memory>,
        0x1D => or::<IndexedX, dest::Accumulator, source::Memory>,
        0x1E => asl::<IndexedX, dest::Memory, source::Memory>,
        0x20 => jsr::<Absolute>,
        0x21 => and::<PreIndexed, dest::Accumulator, source::Memory>,
        0x24 => bit::<ZeroPage, source::Memory>,
        0x25 => and::<ZeroPage, dest::Accumulator, source::Memory>,
        0x26 => rol::<ZeroPage, dest::Memory, source::Memory>,
        0x28 => plp,
        0x29 => and::<Immediate, dest::Accumulator, source::Memory>,
        0x2A => rol::<Implied, dest::Accumulator, source::Accumulator>,
        0x2C => bit::<Absolute, source::Memory>,
        0x2D => and::<Absolute, dest::Accumulator, source::Memory>,
        0x2E => rol::<Absolute, dest::Memory, source::Memory>,
        0x30 => bmi::<Relative>,
        0x31 => and::<PostIndexed, dest::Accumulator, source::Memory>,
        0x35 => and::<ZPIndexedX, dest::Accumulator, source::Memory>,
        0x36 => rol::<ZPIndexedX, dest::Memory, source::Memory>,
        0x38 => sec,
        0x39 => and::<IndexedY, dest::Accumulator, source::Memory>,
        0x3D => and::<IndexedX, dest::Accumulator, source::Memory>,
        0x3E => rol::<IndexedX, dest::Memory, source::Memory>,
        0x40 => rti,
        0x41 => eor::<PreIndexed, dest::Accumulator, source::Memory>,
        0x45 => eor::<ZeroPage, dest::Accumulator, source::Memory>,
        0x46 => lsr::<ZeroPage, dest::Memory, source::Memory>,
        0x48 => pha,
        0x49 => eor::<Immediate, dest::Accumulator, source::Memory>,
        0x4A => lsr::<Implied, dest::Accumulator, source::Accumulator>,
        0x4C => jmp::<Absolute>,
        0x4D => eor::<Absolute, dest::Accumulator, source::Memory>,
        0x4E => lsr::<Absolute, dest::Memory, source::Memory>,
        0x50 => bvc::<Relative>,
        0x51 => eor::<PostIndexed, dest::Accumulator, source::Memory>,
        0x55 => eor::<ZPIndexedX, dest::Accumulator, source::Memory>,
        0x56 => lsr::<ZPIndexedX, dest::Memory, source::Memory>,
        0x58 => cli,
        0x59 => eor::<IndexedY, dest::Accumulator, source::Memory>,
        0x5D => eor::<IndexedX, dest::Accumulator, source::Memory>,
        0x5E => lsr::<IndexedX, dest::Memory, source::Memory>,
        0x60 => rts,
        0x61 => adc::<PreIndexed, source::Memory>,
        0x65 => adc::<ZeroPage, source::Memory>,
        0x66 => ror::<ZeroPage, dest::Memory, source::Memory>,
        0x68 => pla,
        0x69 => adc::<Immediate, source::Memory>,
        0x6A => ror::<Implied, dest::Accumulator, source::Accumulator>,
        0x6C => jmp::<Indirect>,
        0x6D => adc::<Absolute, source::Memory>,
        0x6E => ror::<Absolute, dest::Memory, source::Memory>,
        0x70 => bvs::<Relative>,
        0x71 => adc::<PostIndexed, source::Memory>,
        0x75 => adc::<ZPIndexedX, source::Memory>,
        0x76 => ror::<ZPIndexedX, dest::Memory, source::Memory>,
        0x78 => sei,
        0x79 => adc::<IndexedY, source::Memory>,
        0x7D => adc::<IndexedX, source::Memory>,
        0x7E => ror::<IndexedX, dest::Memory, source::Memory>,
        0x81 => store::<PreIndexed, dest::Memory, source::Accumulator>,
        0x84 => store::<ZeroPage, dest::Memory, source::IndexY>,
        0x85 => store::<ZeroPage, dest::Memory, source::Accumulator>,
        0x86 => store::<ZeroPage, dest::Memory, source::IndexX>,
        0x88 => decr::<Implied, dest::IndexY, source::IndexY>,
        0x8A => trans::<dest::Accumulator, source::IndexX>,
        0x8C => store::<Absolute, dest::Memory, source::IndexY>,
        0x8D => store::<Absolute, dest::Memory, source::Accumulator>,
        0x8E => store::<Absolute, dest::Memory, source::IndexX>,
        0x90 => bcc::<Relative>,
        0x91 => store::<PostIndexed, dest::Memory, source::Accumulator>,
        0x94 => store::<ZPIndexedX, dest::Memory, source::IndexY>,
        0x95 => store::<ZPIndexedX, dest::Memory, source::Accumulator>,
        0x96 => store::<ZPIndexedY, dest::Memory, source::IndexX>,
        0x98 => trans::<dest::Accumulator, source::IndexY>,
        0x99 => store::<IndexedY, dest::Memory, source::Accumulator>,
        0x9A => trans::<dest::StackPointer, source::IndexX>,
        0x9D => store::<IndexedX, dest::Memory, source::Accumulator>,
        0xA0 => load::<Immediate, dest::IndexY, source::Memory>,
        0xA1 => load::<PreIndexed, dest::Accumulator, source::Memory>,
        0xA2 => load::<Immediate, dest::IndexX, source::Memory>,
        0xA4 => load::<ZeroPage, dest::IndexY, source::Memory>,
        0xA5 => load::<ZeroPage, dest::Accumulator, source::Memory>,
        0xA6 => load::<ZeroPage, dest::IndexX, source::Memory>,
        0xA8 => trans::<dest::IndexY, source::Accumulator>,
        0xA9 => load::<Immediate, dest::Accumulator, source::Memory>,
        0xAA => trans::<dest::IndexX, source::Accumulator>,
        0xAC => load::<Absolute, dest::IndexY, source::Memory>,
        0xAD => load::<Absolute, dest::Accumulator, source::Memory>,
        0xAE => load::<Absolute, dest::IndexX, source::Memory>,
        0xB0 => bcs::<Relative>,
        0xB1 => load::<PostIndexed, dest::Accumulator, source::Memory>,
        0xB4 => load::<ZPIndexedX, dest::IndexY, source::Memory>,
        0xB5 => load::<ZPIndexedX, dest::Accumulator, source::Memory>,
        0xB6 => load::<ZPIndexedY, dest::IndexX, source::Memory>,
        0xB8 => clv,
        0xB9 => load::<IndexedY, dest::Accumulator, source::Memory>,
        0xBA => trans::<dest::IndexX, source::StackPointer>,
        0xBC => load::<IndexedX, dest::IndexY, source::Memory>,
        0xBD => load::<IndexedX, dest::Accumulator, source::Memory>,
        0xBE => load::<IndexedY, dest::IndexX, source::Memory>,
        0xC0 => cp::<Immediate, source::IndexY, source::Memory>,
        0xC1 => cp::<PreIndexed, source::Accumulator, source::Memory>,
        0xC4 => cp::<ZeroPage, source::IndexY, source::Memory>,
        0xC5 => cp::<ZeroPage, source::Accumulator, source::Memory>,
        0xC6 => decr::<ZeroPage, dest::Memory, source::Memory>,
        0xC8 => incr::<Implied, dest::IndexY, source::IndexY>,
        0xC9 => cp::<Immediate, source::Accumulator, source::Memory>,
        0xCA => decr::<Implied, dest::IndexX, source::IndexX>,
        0xCC => cp::<Absolute, source::IndexY, source::Memory>,
        0xCD => cp::<Absolute, source::Accumulator, source::Memory>,
        0xCE => decr::<Absolute, dest::Memory, source::Memory>,
        0xD0 => bne::<Relative>,
        0xD1 => cp::<PostIndexed, source::Accumulator, source::Memory>,
        0xD5 => cp::<ZPIndexedX, source::Accumulator, source::Memory>,
        0xD6 => decr::<ZPIndexedX, dest::Memory, source::Memory>,
        0xD8 => cld,
        0xD9 => cp::<IndexedY, source::Accumulator, source::Memory>,
        0xDD => cp::<IndexedX, source::Accumulator, source::Memory>,
        0xDE => decr::<IndexedX, dest::Memory, source::Memory>,
        0xE0 => cp::<Immediate, source::IndexX, source::Memory>,
        0xE1 => sbc::<PreIndexed, source::Memory>,
        0xE4 => cp::<ZeroPage, source::IndexX, source::Memory>,
        0xE5 => sbc::<ZeroPage, source::Memory>,
        0xE6 => incr::<ZeroPage, dest::Memory, source::Memory>,
        0xE8 => incr::<Implied, dest::IndexX, source::IndexX>,
        0xE9 => sbc::<Immediate, source::Memory>,
        0xEA => nop,
        0xEC => cp::<Absolute, source::IndexX, source::Memory>,
        0xED => sbc::<Absolute, source::Memory>,
        0xEE => incr::<Absolute, dest::Memory, source::Memory>,
        0xF0 => beq::<Relative>,
        0xF1 => sbc::<PostIndexed, source::Memory>,
        0xF5 => sbc::<ZPIndexedX, source::Memory>,
        0xF6 => incr::<ZPIndexedX, dest::Memory, source::Memory>,
        0xF8 => sed,
        0xF9 => sbc::<IndexedY, source::Memory>,
        0xFD => sbc::<IndexedX, source::Memory>,
        0xFE => incr::<IndexedX, dest::Memory, source::Memory>,
        _ => unimplemented!("Instruction 0x{:x} no implemented", instruction)
    })(sys);
}