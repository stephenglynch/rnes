use crate::system::{*};

pub trait AddrMode {
    fn get_addr(sys: &mut Cpu) -> u16;
    fn size() -> u16;
}

pub struct Implied;
impl AddrMode for Implied {
    fn get_addr(_sys: &mut Cpu) -> u16 {
        panic!()
    }

    fn size() -> u16 {
        1
    }
}

pub struct Immediate;
impl AddrMode for Immediate {
    fn get_addr(sys: &mut Cpu) -> u16 {
        let pc = sys.registers.pc;
        pc + 1
    }

    fn size() -> u16 {
        2
    }
}

pub struct Absolute;
impl AddrMode for Absolute {
    fn get_addr(sys: &mut Cpu) -> u16 {
        let pc  = sys.registers.pc;
        let addr_lo = sys.mmu_load(pc + 1) as u16;
        let addr_hi = sys.mmu_load(pc + 2) as u16;
        addr_lo | (addr_hi << 8)
    }

    fn size() -> u16 {
        3
    }
}

pub struct ZeroPage;
impl AddrMode for ZeroPage {
    fn get_addr(sys: &mut Cpu) -> u16 {
        let pc  = sys.registers.pc;
        sys.mmu_load(pc + 1) as u16
    }

    fn size() -> u16 {
        2
    }
}

pub struct IndexedX;
impl AddrMode for IndexedX {
    fn get_addr(sys: &mut Cpu) -> u16 {
        let pc  = sys.registers.pc;
        let x = sys.registers.x as u16;
        let addr_lo = sys.mmu_load(pc + 1) as u16;
        let addr_hi = sys.mmu_load(pc + 2) as u16;
        (addr_lo | (addr_hi << 8)) + x
    }

    fn size() -> u16 {
        3
    }
}

pub struct IndexedY;
impl AddrMode for IndexedY {
    fn get_addr(sys: &mut Cpu) -> u16 {
        let pc  = sys.registers.pc;
        let y = sys.registers.y as u16;
        let addr_lo = sys.mmu_load(pc + 1) as u16;
        let addr_hi = sys.mmu_load(pc + 2) as u16;
        (addr_lo | (addr_hi << 8)).wrapping_add(y)
    }

    fn size() -> u16 {
        3
    }
}

pub struct ZPIndexedX;
impl AddrMode for ZPIndexedX {
    fn get_addr(sys: &mut Cpu) -> u16 {
        let pc  = sys.registers.pc;
        let x = sys.registers.x;
        let mut addr_base = sys.mmu_load(pc + 1);
        addr_base = addr_base.wrapping_add(x);
        addr_base as u16
    }

    fn size() -> u16 {
        2
    }
}

pub struct ZPIndexedY;
impl AddrMode for ZPIndexedY {
    fn get_addr(sys: &mut Cpu) -> u16 {
        let pc  = sys.registers.pc;
        let y = sys.registers.y;
        let mut addr_base = sys.mmu_load(pc + 1);
        addr_base = addr_base.wrapping_add(y);
        addr_base as u16
    }

    fn size() -> u16 {
        2
    }
}

pub struct Indirect;
impl AddrMode for Indirect {
    fn get_addr(sys: &mut Cpu) -> u16 {
        let pc  = sys.registers.pc;
        let mut indirect_addr_lo = sys.mmu_load(pc + 1) as u8;
        let indirect_addr_hi = sys.mmu_load(pc + 2) as u16;
        // Note we have to simulate a bug when incrementing the low byte past
        // 0xff does not carry into the high byte
        let addr_lo = sys.mmu_load(indirect_addr_lo as u16 | (indirect_addr_hi << 8)) as u16;
        indirect_addr_lo = indirect_addr_lo.wrapping_add(1);
        let addr_hi = sys.mmu_load(indirect_addr_lo as u16 | (indirect_addr_hi << 8)) as u16;
        addr_lo | (addr_hi << 8)
    }

    fn size() -> u16 {
        3
    }
}

pub struct PreIndexed;
impl AddrMode for PreIndexed {
    fn get_addr(sys: &mut Cpu) -> u16 {
        let pc  = sys.registers.pc;
        let x = sys.registers.x;
        let indirect_addr_lo = sys.mmu_load(pc + 1);
        let indirect_addr = (indirect_addr_lo + x) as u16;
        let addr_lo = sys.mmu_load(indirect_addr) as u16;
        let addr_hi = sys.mmu_load(indirect_addr + 1) as u16;
        addr_lo | (addr_hi << 8)
    }

    fn size() -> u16 {
        2
    }
}

pub struct PostIndexed;
impl AddrMode for PostIndexed {
    fn get_addr(sys: &mut Cpu) -> u16 {
        let pc  = sys.registers.pc;
        let y = sys.registers.y;
        let indirect_addr = sys.mmu_load(pc + 1) as u16;
        let addr_lo: u8 = sys.mmu_load(indirect_addr);
        let addr_hi = sys.mmu_load(indirect_addr + 1);
        (addr_lo as u16 | ((addr_hi as u16) << 8)).wrapping_add(y as u16)
    }

    fn size() -> u16 {
        2
    }
}

pub struct Relative;
impl AddrMode for Relative {
    fn get_addr(sys: &mut Cpu) -> u16 {
        let pc  = sys.registers.pc;
        pc + sys.mmu_load(pc + 1) as u16
    }

    fn size() -> u16 {
        2
    }
}