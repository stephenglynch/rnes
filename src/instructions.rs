use std::cmp::Ordering;

use crate::system::{Cpu, StatusRegister};
use crate::clock::CycleDelay;

mod address_modes;
mod dest;
mod source;
use address_modes::*;
use dest::Dest;
use source::Source;

macro_rules! cycles {
    ($sys:expr, $n:expr) => {
        CycleDelay::new($sys.clock.clone(), $n).await;
    }
}

fn bump_pc<A: AddrMode + Default>(sys: &mut Cpu) {
    sys.registers.pc += A::size();
}

fn update_zero_status(sys: &mut Cpu, val: u8) {
    sys.registers.sr.set(StatusRegister::ZERO, val == 0);
}

fn update_negative_status(sys: &mut Cpu, val: u8) {
    sys.registers.sr.set(StatusRegister::NEGATIVE, val & 0x80 != 0);
}

// Nop

async fn nop(sys: &mut Cpu) {
    bump_pc::<Implied>(sys);
    cycles!(sys, 2);
}

// Transfer operations

async fn load<A: AddrMode + Default, D: Dest<A>, S: Source<A>>(sys: &mut Cpu) {
    let mut addr_mode = A::default();
    let val = S::get(sys, &mut addr_mode);
    D::set(sys, &mut addr_mode, val);
    update_zero_status(sys, val);
    update_negative_status(sys, val);
    bump_pc::<A>(sys);
    cycles!(sys, addr_mode.cycles(AccessType::Read));
}

async fn store<A: AddrMode + Default, D: Dest<A>, S: Source<A>>(sys: &mut Cpu) {
    let mut addr_mode = A::default();
    let val = S::get(sys, &mut addr_mode);
    D::set(sys, &mut addr_mode, val);
    bump_pc::<A>(sys);
    cycles!(sys, addr_mode.cycles(AccessType::Write));
}

async fn trans<D: Dest, S: Source>(sys: &mut Cpu) {
    let mut addr_mode = Immediate::default();
    let val = S::get(sys, &mut addr_mode);
    D::set(sys, &mut addr_mode, val);
    update_zero_status(sys, val);
    update_negative_status(sys, val);
    bump_pc::<Implied>(sys);
    cycles!(sys, 2);
}

async fn txs(sys: &mut Cpu) {
    sys.registers.sp = sys.registers.x;
    bump_pc::<Implied>(sys);
    cycles!(sys, 2);
}

// Stack operations

fn push_raw(sys: &mut Cpu, val: u8) {
    let sp = sys.registers.sp as u16;
    sys.mmu_store(0x0100 + sp, val);
    sys.registers.sp -= 1;
}

async fn php(sys: &mut Cpu) {
    let p = sys.registers.sr | StatusRegister::BREAK | StatusRegister::IGNORED;
    push_raw(sys, p.bits());
    bump_pc::<Implied>(sys);
    cycles!(sys, 3);
}

async fn pha(sys: &mut Cpu) {
    push_raw(sys, sys.registers.ac);
    bump_pc::<Implied>(sys);
    cycles!(sys, 3);
}

fn pull_raw(sys: &mut Cpu) -> u8 {
    sys.registers.sp += 1;
    sys.mmu_load(0x0100 + sys.registers.sp as u16)
}

async fn pla(sys: &mut Cpu) {
    let val = pull_raw(sys);
    update_zero_status(sys, val);
    update_negative_status(sys, val);
    sys.registers.ac = val;
    bump_pc::<Implied>(sys);
    cycles!(sys, 4);
}

async fn plp(sys: &mut Cpu) {
    let val = pull_raw(sys);
    let sr = sys.registers.sr.bits();
    sys.registers.sr = StatusRegister::from_bits_retain(
        val & StatusRegister::STANDARD_FLAGS.bits() |
        sr & StatusRegister::UNUSED_FLAGS.bits()
    );
    bump_pc::<Implied>(sys);
    cycles!(sys, 4);
}

// Increment/Decrements

async fn incr<A: AddrMode + Default, D: Dest<A>, S: Source<A>>(sys: &mut Cpu) {
    let mut addr_mode = A::default();
    let result= S::get(sys, &mut addr_mode).wrapping_add(1);
    update_zero_status(sys, result);
    update_negative_status(sys, result);
    D::set(sys, &mut addr_mode, result);
    bump_pc::<A>(sys);
    cycles!(sys, addr_mode.cycles(AccessType::ReadModifyWrite));
}

async fn decr<A: AddrMode + Default, D: Dest<A>, S: Source<A>>(sys: &mut Cpu) {
    let mut addr_mode = A::default();
    let result = S::get(sys, &mut addr_mode).wrapping_sub(1);
    update_zero_status(sys, result);
    update_negative_status(sys, result);
    D::set(sys, &mut addr_mode, result);
    bump_pc::<A>(sys);
    cycles!(sys, addr_mode.cycles(AccessType::ReadModifyWrite));
}

// Math

async fn adc<A: AddrMode + Default, S: Source<A>>(sys: &mut Cpu) {
    let mut addr_mode = A::default();
    let val = S::get(sys, &mut addr_mode) as u16;
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
    cycles!(sys, addr_mode.cycles(AccessType::Read));
}

async fn sbc<A: AddrMode + Default, S: Source<A>>(sys: &mut Cpu) {
    let mut addr_mode = A::default();
    let val = S::get(sys, &mut addr_mode) as i16;
    let carry = !sys.registers.sr.contains(StatusRegister::CARRY) as i16;
    let ac = sys.registers.ac as i16;
    let total = ac - val - carry;
    let overflow = (total ^ ac) & (total ^ !val) & 0x80 != 0;
    sys.registers.sr.set(StatusRegister::CARRY, total & 0x100 == 0);
    sys.registers.sr.set(StatusRegister::OVERFLOW, overflow);
    update_zero_status(sys, total as u8);
    update_negative_status(sys, total as u8);
    sys.registers.ac = total as u8;
    bump_pc::<A>(sys);
    cycles!(sys, addr_mode.cycles(AccessType::Read));
}

// Logical

async fn and<A: AddrMode + Default, D: Dest<A>, S: Source<A>>(sys: &mut Cpu) {
    let mut addr_mode = A::default();
    let val = S::get(sys, &mut addr_mode);
    let result = val & sys.registers.ac;
    update_zero_status(sys, result);
    update_negative_status(sys, result);
    D::set(sys, &mut addr_mode, result);
    bump_pc::<A>(sys);
    cycles!(sys, addr_mode.cycles(AccessType::Read));
}

async fn eor<A: AddrMode + Default, D: Dest<A>, S: Source<A>>(sys: &mut Cpu) {
    let mut addr_mode = A::default();
    let val = S::get(sys, &mut addr_mode);
    let result = val ^ sys.registers.ac;
    update_zero_status(sys, result);
    update_negative_status(sys, result);
    D::set(sys, &mut addr_mode, result);
    bump_pc::<A>(sys);
    cycles!(sys, addr_mode.cycles(AccessType::Read));
}

async fn or<A: AddrMode + Default, D: Dest<A>, S: Source<A>>(sys: &mut Cpu) {
    let mut addr_mode = A::default();
    let val = S::get(sys, &mut addr_mode);
    let result = val | sys.registers.ac;
    update_zero_status(sys, result);
    update_negative_status(sys, result);
    D::set(sys, &mut addr_mode, result);
    bump_pc::<A>(sys);
    cycles!(sys, addr_mode.cycles(AccessType::Read));
}

// Shift

async fn asl<A: AddrMode + Default, D: Dest<A>, S: Source<A>>(sys: &mut Cpu) {
    let mut addr_mode = A::default();
    let val = S::get(sys, &mut addr_mode);
    let result = val << 1;
    let carry = val & 0x80 != 0;
    update_zero_status(sys, result);
    update_negative_status(sys, result);
    sys.registers.sr.set(StatusRegister::CARRY, carry);
    D::set(sys, &mut addr_mode, result as u8);
    bump_pc::<A>(sys);
    cycles!(sys, addr_mode.cycles(AccessType::ReadModifyWrite));
}

async fn lsr<A: AddrMode + Default, D: Dest<A>, S: Source<A>>(sys: &mut Cpu) {
    let mut addr_mode = A::default();
    let val = S::get(sys, &mut addr_mode);
    let result = val >> 1;
    let carry = val & 0x01 != 0;
    update_zero_status(sys, result);
    update_negative_status(sys, result);
    sys.registers.sr.set(StatusRegister::CARRY, carry);
    D::set(sys, &mut addr_mode, result as u8);
    bump_pc::<A>(sys);
    cycles!(sys, addr_mode.cycles(AccessType::ReadModifyWrite));
}

async fn rol<A: AddrMode + Default, D: Dest<A>, S: Source<A>>(sys: &mut Cpu) {
    let mut addr_mode = A::default();
    let val = S::get(sys, &mut addr_mode);
    let old_carry = sys.registers.sr.contains(StatusRegister::CARRY) as u8;
    let new_carry = val & 0x80 != 0;
    let mut result = val << 1;
    result |= old_carry;
    update_zero_status(sys, result);
    update_negative_status(sys, result);
    sys.registers.sr.set(StatusRegister::CARRY, new_carry);
    D::set(sys, &mut addr_mode, result as u8);
    bump_pc::<A>(sys);
    cycles!(sys, addr_mode.cycles(AccessType::ReadModifyWrite));
}

async fn ror<A: AddrMode + Default, D: Dest<A>, S: Source<A>>(sys: &mut Cpu) {
    let mut addr_mode = A::default();
    let val = S::get(sys, &mut addr_mode);
    let old_carry = if sys.registers.sr.contains(StatusRegister::CARRY) {0x80} else {0x00};
    let new_carry = val & 0x01 != 0;
    let mut result = val >> 1;
    result |= old_carry;
    update_zero_status(sys, result);
    update_negative_status(sys, result);
    sys.registers.sr.set(StatusRegister::CARRY, new_carry);
    D::set(sys, &mut addr_mode, result as u8);
    bump_pc::<A>(sys);
    cycles!(sys, addr_mode.cycles(AccessType::ReadModifyWrite));
}

// Flags

async fn clc(sys: &mut Cpu) {
    sys.registers.sr.set(StatusRegister::CARRY, false);
    bump_pc::<Implied>(sys);
    cycles!(sys, 2);
}

async fn cld(sys: &mut Cpu) {
    sys.registers.sr.set(StatusRegister::DECIMAL, false);
    bump_pc::<Implied>(sys);
    cycles!(sys, 2);
}

async fn cli(sys: &mut Cpu) {
    sys.registers.sr.set(StatusRegister::INTERRUPT, false);
    bump_pc::<Implied>(sys);
    cycles!(sys, 2);
}

async fn clv(sys: &mut Cpu) {
    sys.registers.sr.set(StatusRegister::OVERFLOW, false);
    bump_pc::<Implied>(sys);
    cycles!(sys, 2);
}

async fn sec(sys: &mut Cpu) {
    sys.registers.sr.set(StatusRegister::CARRY, true);
    bump_pc::<Implied>(sys);
    cycles!(sys, 2);
}

async fn sed(sys: &mut Cpu) {
    sys.registers.sr.set(StatusRegister::DECIMAL, true);
    bump_pc::<Implied>(sys);
    cycles!(sys, 2);
}

async fn sei(sys: &mut Cpu) {
    sys.registers.sr.set(StatusRegister::INTERRUPT, true);
    bump_pc::<Implied>(sys);
    cycles!(sys, 2);
}

// Compare

async fn cp<A: AddrMode + Default, R: Source<A>, S: Source<A>>(sys: &mut Cpu) {
    let mut addr_mode = A::default();
    let reg = R::get(sys, &mut addr_mode);
    let operand = S::get(sys, &mut addr_mode);
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
    cycles!(sys, addr_mode.cycles(AccessType::Read));
}

// Bit Test

async fn bit<A: AddrMode + Default, S: Source<A>>(sys: &mut Cpu) {
    let mut addr_mode = A::default();
    let reg = sys.registers.ac;
    let operand = S::get(sys, &mut addr_mode);
    let result = reg & operand;
    sys.registers.sr.set(StatusRegister::ZERO, result == 0);
    sys.registers.sr.set(StatusRegister::OVERFLOW, operand & (1 << 6) != 0);
    sys.registers.sr.set(StatusRegister::NEGATIVE, operand & (1 << 7) != 0);
    bump_pc::<A>(sys);
    cycles!(sys, addr_mode.cycles(AccessType::Read));
}

// Conditional branching

async fn bcc<A: AddrMode + Default>(sys: &mut Cpu) {
    let test_bit = !sys.registers.sr.contains(StatusRegister::CARRY);
    let mut addr_mode = A::default();
    let dest = addr_mode.get_addr(sys);
    if test_bit {
        sys.registers.pc = dest;
    }
    bump_pc::<A>(sys);
    cycles!(sys, addr_mode.cycles(AccessType::Read) + test_bit as u64);
}

async fn bcs<A: AddrMode + Default>(sys: &mut Cpu) {
    let test_bit = sys.registers.sr.contains(StatusRegister::CARRY);
    let mut addr_mode = A::default();
    let dest = addr_mode.get_addr(sys);
    if test_bit {
        sys.registers.pc = dest;
    }
    bump_pc::<A>(sys);
    cycles!(sys, addr_mode.cycles(AccessType::Read) + test_bit as u64);
}

async fn beq<A: AddrMode + Default>(sys: &mut Cpu) {
    let test_bit = sys.registers.sr.contains(StatusRegister::ZERO);
    let mut addr_mode = A::default();
    let dest = addr_mode.get_addr(sys);
    if test_bit {
        sys.registers.pc = dest;
    }
    bump_pc::<A>(sys);
    cycles!(sys, addr_mode.cycles(AccessType::Read) + test_bit as u64);
}

async fn bmi<A: AddrMode + Default>(sys: &mut Cpu) {
    let test_bit = sys.registers.sr.contains(StatusRegister::NEGATIVE);
    let mut addr_mode = A::default();
    let dest = addr_mode.get_addr(sys);
    if test_bit {
        sys.registers.pc = dest;
    }
    bump_pc::<A>(sys);
    cycles!(sys, addr_mode.cycles(AccessType::Read) + test_bit as u64);
}

async fn bne<A: AddrMode + Default>(sys: &mut Cpu) {
    let test_bit = !sys.registers.sr.contains(StatusRegister::ZERO);
    let mut addr_mode = A::default();
    let dest = addr_mode.get_addr(sys);
    let mut extra_cycles = 0;
    if test_bit {
        sys.registers.pc = dest;
        extra_cycles = 1 + addr_mode.page_crossed() as u64;
    }
    bump_pc::<A>(sys);
    cycles!(sys, addr_mode.cycles(AccessType::Read) + extra_cycles);
}

async fn bpl<A: AddrMode + Default>(sys: &mut Cpu) {
    let test_bit = !sys.registers.sr.contains(StatusRegister::NEGATIVE);
    let mut addr_mode = A::default();
    let dest = addr_mode.get_addr(sys);
    let mut extra_cycles = 0;
    if test_bit {
        sys.registers.pc = dest;
        extra_cycles = 1 + addr_mode.page_crossed() as u64;
    }
    bump_pc::<A>(sys);
    cycles!(sys, addr_mode.cycles(AccessType::Read) + extra_cycles);
}

async fn bvc<A: AddrMode + Default>(sys: &mut Cpu) {
    let test_bit = !sys.registers.sr.contains(StatusRegister::OVERFLOW);
    let mut addr_mode = A::default();
    let dest = addr_mode.get_addr(sys);
    let mut extra_cycles = 0;
    if test_bit {
        sys.registers.pc = dest;
        extra_cycles = 1 + addr_mode.page_crossed() as u64;
    }
    bump_pc::<A>(sys);
    cycles!(sys, addr_mode.cycles(AccessType::Read) + extra_cycles);
}

async fn bvs<A: AddrMode + Default>(sys: &mut Cpu) {
    let test_bit = sys.registers.sr.contains(StatusRegister::OVERFLOW);
    let mut addr_mode = A::default();
    let dest = addr_mode.get_addr(sys);
    let mut extra_cycles = 0;
    if test_bit {
        sys.registers.pc = dest;
        extra_cycles = 1 + addr_mode.page_crossed() as u64;
    }
    bump_pc::<A>(sys);
    cycles!(sys, addr_mode.cycles(AccessType::Read) + extra_cycles);
}

// Jumps and subroutines

async fn jmp<A: AddrMode + Default>(sys: &mut Cpu) {
    let mut addr_mode = A::default();
    let dest = addr_mode.get_addr(sys);
    sys.registers.pc = dest;
    cycles!(sys, addr_mode.cycles(AccessType::Jump));
}

async fn jsr<A: AddrMode + Default>(sys: &mut Cpu) {
    let mut addr_mode = A::default();
    let dest = addr_mode.get_addr(sys);
    let ret_addr = sys.registers.pc + 2;
    let ret_addr_hi = ((ret_addr & 0xff00) >> 8) as u8;
    let ret_addr_lo = ret_addr as u8;
    push_raw(sys, ret_addr_hi);
    push_raw(sys, ret_addr_lo);
    sys.registers.pc = dest;
    cycles!(sys, 6);
}

async fn rts(sys: &mut Cpu) {
    let ret_addr_lo = pull_raw(sys) as u16;
    let ret_addr_hi = pull_raw(sys) as u16;
    let ret_addr = ret_addr_lo | (ret_addr_hi << 8);
    sys.registers.pc = ret_addr;
    bump_pc::<Implied>(sys);
    cycles!(sys, 6);
}

// Interrupts

async fn brk(_sys: &mut Cpu) {
    unimplemented!();
}

async fn rti(sys: &mut Cpu) {
    let mut sr = sys.registers.sr.bits();
    let flags = pull_raw(sys);
    sr |= flags & StatusRegister::STANDARD_FLAGS.bits();
    sr &= !(!flags & StatusRegister::STANDARD_FLAGS.bits());
    let ret_addr_lo = pull_raw(sys) as u16;
    let ret_addr_hi = pull_raw(sys) as u16;
    let ret_addr = ret_addr_lo | (ret_addr_hi << 8);
    sys.registers.sr = StatusRegister::from_bits_retain(sr);
    sys.registers.pc = ret_addr;
    cycles!(sys, 6);
}

pub async fn execute(sys: &mut Cpu, instruction: u8) {
    match instruction {
        0x00 => brk(sys).await,
        0x01 => or::<PreIndexed, dest::Accumulator, source::Memory>(sys).await,
        0x05 => or::<ZeroPage, dest::Accumulator, source::Memory>(sys).await,
        0x06 => asl::<ZeroPage, dest::Memory, source::Memory>(sys).await,
        0x08 => php(sys).await,
        0x09 => or::<Immediate, dest::Accumulator, source::Memory>(sys).await,
        0x0A => asl::<Implied, dest::Accumulator, source::Accumulator>(sys).await,
        0x0D => or::<Absolute, dest::Accumulator, source::Memory>(sys).await,
        0x0E => asl::<Absolute, dest::Memory, source::Memory>(sys).await,
        0x10 => bpl::<Relative>(sys).await,
        0x11 => or::<PostIndexed, dest::Accumulator, source::Memory>(sys).await,
        0x15 => or::<ZPIndexedX, dest::Accumulator, source::Memory>(sys).await,
        0x16 => asl::<ZPIndexedX, dest::Memory, source::Memory>(sys).await,
        0x18 => clc(sys).await,
        0x19 => or::<IndexedY, dest::Accumulator, source::Memory>(sys).await,
        0x1D => or::<IndexedX, dest::Accumulator, source::Memory>(sys).await,
        0x1E => asl::<IndexedX, dest::Memory, source::Memory>(sys).await,
        0x20 => jsr::<Absolute>(sys).await,
        0x21 => and::<PreIndexed, dest::Accumulator, source::Memory>(sys).await,
        0x24 => bit::<ZeroPage, source::Memory>(sys).await,
        0x25 => and::<ZeroPage, dest::Accumulator, source::Memory>(sys).await,
        0x26 => rol::<ZeroPage, dest::Memory, source::Memory>(sys).await,
        0x28 => plp(sys).await,
        0x29 => and::<Immediate, dest::Accumulator, source::Memory>(sys).await,
        0x2A => rol::<Implied, dest::Accumulator, source::Accumulator>(sys).await,
        0x2C => bit::<Absolute, source::Memory>(sys).await,
        0x2D => and::<Absolute, dest::Accumulator, source::Memory>(sys).await,
        0x2E => rol::<Absolute, dest::Memory, source::Memory>(sys).await,
        0x30 => bmi::<Relative>(sys).await,
        0x31 => and::<PostIndexed, dest::Accumulator, source::Memory>(sys).await,
        0x35 => and::<ZPIndexedX, dest::Accumulator, source::Memory>(sys).await,
        0x36 => rol::<ZPIndexedX, dest::Memory, source::Memory>(sys).await,
        0x38 => sec(sys).await,
        0x39 => and::<IndexedY, dest::Accumulator, source::Memory>(sys).await,
        0x3D => and::<IndexedX, dest::Accumulator, source::Memory>(sys).await,
        0x3E => rol::<IndexedX, dest::Memory, source::Memory>(sys).await,
        0x40 => rti(sys).await,
        0x41 => eor::<PreIndexed, dest::Accumulator, source::Memory>(sys).await,
        0x45 => eor::<ZeroPage, dest::Accumulator, source::Memory>(sys).await,
        0x46 => lsr::<ZeroPage, dest::Memory, source::Memory>(sys).await,
        0x48 => pha(sys).await,
        0x49 => eor::<Immediate, dest::Accumulator, source::Memory>(sys).await,
        0x4A => lsr::<Implied, dest::Accumulator, source::Accumulator>(sys).await,
        0x4C => jmp::<Absolute>(sys).await,
        0x4D => eor::<Absolute, dest::Accumulator, source::Memory>(sys).await,
        0x4E => lsr::<Absolute, dest::Memory, source::Memory>(sys).await,
        0x50 => bvc::<Relative>(sys).await,
        0x51 => eor::<PostIndexed, dest::Accumulator, source::Memory>(sys).await,
        0x55 => eor::<ZPIndexedX, dest::Accumulator, source::Memory>(sys).await,
        0x56 => lsr::<ZPIndexedX, dest::Memory, source::Memory>(sys).await,
        0x58 => cli(sys).await,
        0x59 => eor::<IndexedY, dest::Accumulator, source::Memory>(sys).await,
        0x5D => eor::<IndexedX, dest::Accumulator, source::Memory>(sys).await,
        0x5E => lsr::<IndexedX, dest::Memory, source::Memory>(sys).await,
        0x60 => rts(sys).await,
        0x61 => adc::<PreIndexed, source::Memory>(sys).await,
        0x65 => adc::<ZeroPage, source::Memory>(sys).await,
        0x66 => ror::<ZeroPage, dest::Memory, source::Memory>(sys).await,
        0x68 => pla(sys).await,
        0x69 => adc::<Immediate, source::Memory>(sys).await,
        0x6A => ror::<Implied, dest::Accumulator, source::Accumulator>(sys).await,
        0x6C => jmp::<Indirect>(sys).await,
        0x6D => adc::<Absolute, source::Memory>(sys).await,
        0x6E => ror::<Absolute, dest::Memory, source::Memory>(sys).await,
        0x70 => bvs::<Relative>(sys).await,
        0x71 => adc::<PostIndexed, source::Memory>(sys).await,
        0x75 => adc::<ZPIndexedX, source::Memory>(sys).await,
        0x76 => ror::<ZPIndexedX, dest::Memory, source::Memory>(sys).await,
        0x78 => sei(sys).await,
        0x79 => adc::<IndexedY, source::Memory>(sys).await,
        0x7D => adc::<IndexedX, source::Memory>(sys).await,
        0x7E => ror::<IndexedX, dest::Memory, source::Memory>(sys).await,
        0x81 => store::<PreIndexed, dest::Memory, source::Accumulator>(sys).await,
        0x84 => store::<ZeroPage, dest::Memory, source::IndexY>(sys).await,
        0x85 => store::<ZeroPage, dest::Memory, source::Accumulator>(sys).await,
        0x86 => store::<ZeroPage, dest::Memory, source::IndexX>(sys).await,
        0x88 => decr::<Implied, dest::IndexY, source::IndexY>(sys).await,
        0x8A => trans::<dest::Accumulator, source::IndexX>(sys).await,
        0x8C => store::<Absolute, dest::Memory, source::IndexY>(sys).await,
        0x8D => store::<Absolute, dest::Memory, source::Accumulator>(sys).await,
        0x8E => store::<Absolute, dest::Memory, source::IndexX>(sys).await,
        0x90 => bcc::<Relative>(sys).await,
        0x91 => store::<PostIndexed, dest::Memory, source::Accumulator>(sys).await,
        0x94 => store::<ZPIndexedX, dest::Memory, source::IndexY>(sys).await,
        0x95 => store::<ZPIndexedX, dest::Memory, source::Accumulator>(sys).await,
        0x96 => store::<ZPIndexedY, dest::Memory, source::IndexX>(sys).await,
        0x98 => trans::<dest::Accumulator, source::IndexY>(sys).await,
        0x99 => store::<IndexedY, dest::Memory, source::Accumulator>(sys).await,
        0x9A => txs(sys).await,
        0x9D => store::<IndexedX, dest::Memory, source::Accumulator>(sys).await,
        0xA0 => load::<Immediate, dest::IndexY, source::Memory>(sys).await,
        0xA1 => load::<PreIndexed, dest::Accumulator, source::Memory>(sys).await,
        0xA2 => load::<Immediate, dest::IndexX, source::Memory>(sys).await,
        0xA4 => load::<ZeroPage, dest::IndexY, source::Memory>(sys).await,
        0xA5 => load::<ZeroPage, dest::Accumulator, source::Memory>(sys).await,
        0xA6 => load::<ZeroPage, dest::IndexX, source::Memory>(sys).await,
        0xA8 => trans::<dest::IndexY, source::Accumulator>(sys).await,
        0xA9 => load::<Immediate, dest::Accumulator, source::Memory>(sys).await,
        0xAA => trans::<dest::IndexX, source::Accumulator>(sys).await,
        0xAC => load::<Absolute, dest::IndexY, source::Memory>(sys).await,
        0xAD => load::<Absolute, dest::Accumulator, source::Memory>(sys).await,
        0xAE => load::<Absolute, dest::IndexX, source::Memory>(sys).await,
        0xB0 => bcs::<Relative>(sys).await,
        0xB1 => load::<PostIndexed, dest::Accumulator, source::Memory>(sys).await,
        0xB4 => load::<ZPIndexedX, dest::IndexY, source::Memory>(sys).await,
        0xB5 => load::<ZPIndexedX, dest::Accumulator, source::Memory>(sys).await,
        0xB6 => load::<ZPIndexedY, dest::IndexX, source::Memory>(sys).await,
        0xB8 => clv(sys).await,
        0xB9 => load::<IndexedY, dest::Accumulator, source::Memory>(sys).await,
        0xBA => trans::<dest::IndexX, source::StackPointer>(sys).await,
        0xBC => load::<IndexedX, dest::IndexY, source::Memory>(sys).await,
        0xBD => load::<IndexedX, dest::Accumulator, source::Memory>(sys).await,
        0xBE => load::<IndexedY, dest::IndexX, source::Memory>(sys).await,
        0xC0 => cp::<Immediate, source::IndexY, source::Memory>(sys).await,
        0xC1 => cp::<PreIndexed, source::Accumulator, source::Memory>(sys).await,
        0xC4 => cp::<ZeroPage, source::IndexY, source::Memory>(sys).await,
        0xC5 => cp::<ZeroPage, source::Accumulator, source::Memory>(sys).await,
        0xC6 => decr::<ZeroPage, dest::Memory, source::Memory>(sys).await,
        0xC8 => incr::<Implied, dest::IndexY, source::IndexY>(sys).await,
        0xC9 => cp::<Immediate, source::Accumulator, source::Memory>(sys).await,
        0xCA => decr::<Implied, dest::IndexX, source::IndexX>(sys).await,
        0xCC => cp::<Absolute, source::IndexY, source::Memory>(sys).await,
        0xCD => cp::<Absolute, source::Accumulator, source::Memory>(sys).await,
        0xCE => decr::<Absolute, dest::Memory, source::Memory>(sys).await,
        0xD0 => bne::<Relative>(sys).await,
        0xD1 => cp::<PostIndexed, source::Accumulator, source::Memory>(sys).await,
        0xD5 => cp::<ZPIndexedX, source::Accumulator, source::Memory>(sys).await,
        0xD6 => decr::<ZPIndexedX, dest::Memory, source::Memory>(sys).await,
        0xD8 => cld(sys).await,
        0xD9 => cp::<IndexedY, source::Accumulator, source::Memory>(sys).await,
        0xDD => cp::<IndexedX, source::Accumulator, source::Memory>(sys).await,
        0xDE => decr::<IndexedX, dest::Memory, source::Memory>(sys).await,
        0xE0 => cp::<Immediate, source::IndexX, source::Memory>(sys).await,
        0xE1 => sbc::<PreIndexed, source::Memory>(sys).await,
        0xE4 => cp::<ZeroPage, source::IndexX, source::Memory>(sys).await,
        0xE5 => sbc::<ZeroPage, source::Memory>(sys).await,
        0xE6 => incr::<ZeroPage, dest::Memory, source::Memory>(sys).await,
        0xE8 => incr::<Implied, dest::IndexX, source::IndexX>(sys).await,
        0xE9 => sbc::<Immediate, source::Memory>(sys).await,
        0xEA => nop(sys).await,
        0xEC => cp::<Absolute, source::IndexX, source::Memory>(sys).await,
        0xED => sbc::<Absolute, source::Memory>(sys).await,
        0xEE => incr::<Absolute, dest::Memory, source::Memory>(sys).await,
        0xF0 => beq::<Relative>(sys).await,
        0xF1 => sbc::<PostIndexed, source::Memory>(sys).await,
        0xF5 => sbc::<ZPIndexedX, source::Memory>(sys).await,
        0xF6 => incr::<ZPIndexedX, dest::Memory, source::Memory>(sys).await,
        0xF8 => sed(sys).await,
        0xF9 => sbc::<IndexedY, source::Memory>(sys).await,
        0xFD => sbc::<IndexedX, source::Memory>(sys).await,
        0xFE => incr::<IndexedX, dest::Memory, source::Memory>(sys).await,
        _ => unimplemented!("Instruction 0x{:x} no implemented", instruction)
    };
}