
use std::cell::RefCell;
use std::rc::Rc;
use std::time::SystemTime;
use std::thread;
use futures::executor::LocalPool;
use futures::task::LocalSpawnExt;
use crate::ppu::Ppu;
use crate::system::Cpu;
use crate::parse_ines::INes;
use crate::clock::Clock;
use crate::renderer::Renderer;

const CYCLES_TO_RUN: usize = 1000000;

pub fn execute_rom(ines: INes) {
    // Create renderer
    let renderer = Renderer::new();
    let frame_buffer = renderer.get_frame_buffer();

    thread::spawn(move || {
        // Build NES components
        let clock = Rc::new(RefCell::new(Clock::new()));
        let ppu = Rc::new(Ppu::new(clock.clone(), ines.chr_rom.unwrap(), frame_buffer));
        let cpu = Cpu::new(clock.clone(), ines.prg_rom, ppu.clone());

        // Create "async" pool to handle clock cycles
        let mut pool = LocalPool::new();
        let spawner = pool.spawner();
        spawner.spawn_local(cpu.run()).unwrap();
        spawner.spawn_local(async move { ppu.clone().run().await }).unwrap();

        let now = SystemTime::now();
        // for _ in 0..CYCLES_TO_RUN {
        loop {
            pool.run_until_stalled();
            clock.borrow_mut().tick();
        }

        println!("{} Instructions per us", (CYCLES_TO_RUN as f64) / now.elapsed().unwrap().as_secs_f64() / 1e6);
    });

    renderer.run();
}