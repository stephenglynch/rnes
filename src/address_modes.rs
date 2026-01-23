use crate::system::{*};

pub trait AddrMode {
    fn get_addr(sys: &Cpu) -> u16;
    fn read_addr(sys: &Cpu) -> u8 {
        let addr = Self::get_addr(sys);
        sys.mmu_load(addr)
    }
    fn write_addr(sys: &mut Cpu, val: u8) {
        let addr = Self::get_addr(sys);
        sys.mmu_store(addr, val);
    }
}

pub struct Immediate;
impl AddrMode for Immediate {
    fn get_addr(_sys: &Cpu) -> u16 {
        panic!("Immediate cannot get address")
    }

    fn read_addr(sys: &Cpu) -> u8 {
        let pc = sys.registers.pc;
        sys.mmu_load(pc + 1)
    }

    fn write_addr(_sys: &mut Cpu, _val: u8) {
        panic!("Immediate cannot write to address");
    }
}

pub struct Absolute;
impl AddrMode for Absolute {
    fn get_addr(sys: &Cpu) -> u16 {
        let pc  = sys.registers.pc;
        let addr_lo = sys.mmu_load(pc + 1) as u16;
        let addr_hi = sys.mmu_load(pc + 2) as u16;
        addr_lo | (addr_hi >> 8)
    }
}

pub struct Indirect;
impl AddrMode for Indirect {
    fn get_addr(sys: &Cpu) -> u16 {
        let pc  = sys.registers.pc;
        let indirect_addr_lo = sys.mmu_load(pc + 1) as u16;
        let indirect_addr_hi = sys.mmu_load(pc + 2) as u16;
        let indirect_addr = indirect_addr_lo | (indirect_addr_hi >> 8);
        let addr_lo = sys.mmu_load(indirect_addr) as u16;
        let addr_hi = sys.mmu_load(indirect_addr + 1) as u16;
        addr_lo | (addr_hi >> 8)
    }
}
