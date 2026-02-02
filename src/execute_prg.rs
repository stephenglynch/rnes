
use std::cell::RefCell;
use std::rc::Rc;
use futures::executor::LocalPool;
use futures::task::LocalSpawnExt;
use crate::ppu::Ppu;
use crate::system::Cpu;
use crate::parse_ines::INes;
use crate::clock::Clock;

pub fn execute_rom(ines: INes) -> ! {
    // Note: suppport CHR-RAM
    let clock = Rc::new(RefCell::new(Clock::new()));
    let ppu = Rc::new(Ppu::new(clock.clone(), ines.chr_rom.unwrap()));
    let cpu = Cpu::new(clock.clone(), ines.prg_rom, ppu.clone());
    let mut pool = LocalPool::new();
    let spawner = pool.spawner();

    spawner.spawn_local(cpu.run()).unwrap();
    spawner.spawn_local(async move { ppu.clone().run().await }).unwrap();

    loop {
        pool.run_until_stalled();
        clock.borrow_mut().tick();
    }
}