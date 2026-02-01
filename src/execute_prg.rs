
use std::cell::RefCell;
use std::rc::Rc;
use futures::executor::LocalPool;
use futures::task::LocalSpawnExt;
use crate::system::{*};
use crate::parse_ines::INes;
use crate::clock::Clock;

pub fn execute_rom(ines: INes) {
    // Note: suppport CHR-RAM
    let clock = Rc::new(RefCell::new(Clock::new()));
    let cpu = Cpu::new(clock.clone(), ines.prg_rom, ines.chr_rom.unwrap());
    let mut pool = LocalPool::new();
    let spawner = pool.spawner();

    spawner.spawn_local(cpu.run()).unwrap();

    loop {
        pool.run_until_stalled();
        clock.borrow_mut().tick();
    }
}