use crate::system::{*};

pub enum AccessType {
    Read,
    Write,
    ReadModifyWrite,
    Jump
}

pub trait AddrMode {
    fn get_addr(&mut self, sys: &mut Cpu) -> u16;
    fn size() -> u16;
    fn cycles(&self, atype: AccessType)-> u64; // Extra cycles produced by the address mode
    fn page_crossed(&self) -> bool {
        false
    }
}

#[derive(Default)]
pub struct Implied;
impl AddrMode for Implied {
    fn get_addr(&mut self, _sys: &mut Cpu) -> u16 {
        panic!()
    }

    fn size() -> u16 {
        1
    }

    fn cycles(&self, _atype: AccessType)-> u64 {
        2
    }
}

#[derive(Default)]
pub struct Immediate;
impl AddrMode for Immediate {
    fn get_addr(&mut self, sys: &mut Cpu) -> u16 {
        let pc = sys.registers.pc;
        pc + 1
    }

    fn size() -> u16 {
        2
    }

    fn cycles(&self, _atype: AccessType)-> u64 {
        2
    }
}

#[derive(Default)]
pub struct Absolute;
impl AddrMode for Absolute {
    fn get_addr(&mut self, sys: &mut Cpu) -> u16 {
        let pc  = sys.registers.pc;
        let addr_lo = sys.mmu_load(pc + 1) as u16;
        let addr_hi = sys.mmu_load(pc + 2) as u16;
        addr_lo | (addr_hi << 8)
    }

    fn size() -> u16 {
        3
    }

    fn cycles(&self, atype: AccessType)-> u64 {
        match atype {
            AccessType::Read => 4,
            AccessType::Write => 4,
            AccessType::ReadModifyWrite => 6,
            AccessType::Jump => 3
        }
    }
}

#[derive(Default)]
pub struct ZeroPage;
impl AddrMode for ZeroPage {
    fn get_addr(&mut self, sys: &mut Cpu) -> u16 {
        let pc  = sys.registers.pc;
        sys.mmu_load(pc + 1) as u16
    }

    fn size() -> u16 {
        2
    }

    fn cycles(&self, atype: AccessType)-> u64 {
        match atype {
            AccessType::Read => 3,
            AccessType::Write => 3,
            AccessType::ReadModifyWrite => 5,
            _ => panic!()
        }
    }
}

#[derive(Default)]
pub struct IndexedX {
    page_crossed: bool
}
impl AddrMode for IndexedX {
    fn get_addr(&mut self, sys: &mut Cpu) -> u16 {
        let pc  = sys.registers.pc;
        let x = sys.registers.x as u16;
        let addr_lo = sys.mmu_load(pc + 1) as u16;
        let addr_hi = sys.mmu_load(pc + 2) as u16;
        let addr = addr_lo | (addr_hi << 8);
        let indexed_addr = addr.wrapping_add(x);
        self.page_crossed = (addr & 0xff00) ^ (indexed_addr & 0xff00) != 0;
        indexed_addr
    }

    fn size() -> u16 {
        3
    }

    fn cycles(&self,  atype: AccessType)-> u64 {
        match atype {
            AccessType::Read => 4 + self.page_crossed as u64,
            AccessType::Write => 5,
            AccessType::ReadModifyWrite => 6,
            _ => panic!()
        }
    }

    fn page_crossed(&self) -> bool {
        self.page_crossed
    }
}

#[derive(Default)]
pub struct IndexedY {
    page_crossed: bool
}
impl AddrMode for IndexedY {
    fn get_addr(&mut self, sys: &mut Cpu) -> u16 {
        let pc  = sys.registers.pc;
        let y = sys.registers.y as u16;
        let addr_lo = sys.mmu_load(pc + 1) as u16;
        let addr_hi = sys.mmu_load(pc + 2) as u16;
        let addr = addr_lo | (addr_hi << 8);
        let indexed_addr = addr.wrapping_add(y);
        self.page_crossed = (addr & 0xff00) ^ (indexed_addr & 0xff00) != 0;
        indexed_addr
    }

    fn size() -> u16 {
        3
    }

    fn cycles(&self, atype: AccessType)-> u64 {
        match atype {
            AccessType::Read => 4 + self.page_crossed as u64,
            AccessType::Write => 5,
            AccessType::ReadModifyWrite => 6,
            _ => panic!()
        }
    }

    fn page_crossed(&self) -> bool {
        self.page_crossed
    }
}

#[derive(Default)]
pub struct ZPIndexedX;
impl AddrMode for ZPIndexedX {
    fn get_addr(&mut self, sys: &mut Cpu) -> u16 {
        let pc  = sys.registers.pc;
        let x = sys.registers.x;
        let mut addr_base = sys.mmu_load(pc + 1);
        addr_base = addr_base.wrapping_add(x);
        addr_base as u16
    }

    fn size() -> u16 {
        2
    }

    fn cycles(&self, atype: AccessType)-> u64 {
        match atype {
            AccessType::Read => 4,
            AccessType::Write => 4,
            AccessType::ReadModifyWrite => 6,
            _ => panic!()
        }
    }
}

#[derive(Default)]
pub struct ZPIndexedY;
impl AddrMode for ZPIndexedY {
    fn get_addr(&mut self, sys: &mut Cpu) -> u16 {
        let pc  = sys.registers.pc;
        let y = sys.registers.y;
        let mut addr_base = sys.mmu_load(pc + 1);
        addr_base = addr_base.wrapping_add(y);
        addr_base as u16
    }

    fn size() -> u16 {
        2
    }

    fn cycles(&self, atype: AccessType)-> u64 {
        match atype {
            AccessType::Read => 4,
            AccessType::Write => 4,
            AccessType::ReadModifyWrite => 6,
            _ => panic!()
        }
    }
}

#[derive(Default)]
pub struct Indirect;
impl AddrMode for Indirect {
    fn get_addr(&mut self, sys: &mut Cpu) -> u16 {
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

    fn cycles(&self, _atype: AccessType)-> u64 {
        5
    }
}

#[derive(Default)]
pub struct PreIndexed;
impl AddrMode for PreIndexed {
    fn get_addr(&mut self, sys: &mut Cpu) -> u16 {
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

    fn cycles(&self, atype: AccessType)-> u64 {
        match atype {
            AccessType::Read => 6,
            AccessType::Write => 6,
            AccessType::ReadModifyWrite => panic!(),
            _ => panic!()
        }
    }
}

#[derive(Default)]
pub struct PostIndexed {
    page_crossed: bool
}
impl AddrMode for PostIndexed {
    fn get_addr(&mut self, sys: &mut Cpu) -> u16 {
        let pc  = sys.registers.pc;
        let y = sys.registers.y;
        let indirect_addr = sys.mmu_load(pc + 1) as u16;
        let addr_lo: u8 = sys.mmu_load(indirect_addr);
        let addr_hi = sys.mmu_load(indirect_addr + 1);
        let addr = addr_lo as u16 | ((addr_hi as u16) << 8);
        let indexed_addr = (addr).wrapping_add(y as u16);
        self.page_crossed = (addr & 0xff00) ^ (indexed_addr & 0xff00) != 0;
        indexed_addr
    }

    fn size() -> u16 {
        2
    }

    fn cycles(&self, atype: AccessType)-> u64 {
        match atype {
            AccessType::Read => 5 + self.page_crossed as u64,
            AccessType::Write => 6,
            AccessType::ReadModifyWrite => panic!(),
            _ => panic!()
        }
    }

    fn page_crossed(&self) -> bool {
        self.page_crossed
    }
}

#[derive(Default)]
pub struct Relative {
    page_crossed: bool
}
impl AddrMode for Relative {
    fn get_addr(&mut self, sys: &mut Cpu) -> u16 {
        let pc  = sys.registers.pc;
        let dest = pc.wrapping_add(sys.mmu_load(pc + 1) as u16);
        self.page_crossed = (pc & 0xff00) ^ (dest & 0xff00) != 0;
        dest
    }

    fn size() -> u16 {
        2
    }

    fn cycles(&self, _atype: AccessType)-> u64 {
        2
    }

    fn page_crossed(&self) -> bool {
        self.page_crossed
    }
}