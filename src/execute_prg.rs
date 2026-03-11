use std::cell::RefCell;
use std::rc::Rc;
use std::time::SystemTime;
use std::thread;
use futures::executor::LocalPool;
use futures::task::LocalSpawnExt;

use crate::audio::Audio;
use crate::chip::{Chip, run_chip};
use crate::ppu::Ppu;
use crate::system::Cpu;
use crate::parse_ines::INes;
use crate::clock::Clock;
use crate::renderer::Renderer;
use crate::input::InputManager;
use crate::mapper::generate_mapper;

const CYCLES_TO_RUN: usize = 100000000;

pub fn execute_rom(ines: INes) -> Result<(), Box<dyn std::error::Error>> {
    // Create renderer
    let input_manager= InputManager::new(true);

    let renderer = Renderer::new(|key| input_manager.handle_key_event(key));
    let frame_buffer = renderer.get_frame_buffer();

    let audio = Audio::new()?;

    let gamepads = input_manager.get_gamepads();

    thread::spawn(move || {
        // Build NES components
        let clock  = Rc::new(RefCell::new(Clock::new()));
        let mapper  = generate_mapper(ines);
        let chip = Rc::new(RefCell::new(Chip::new(clock.clone(), audio, gamepads)));
        let ppu    = Rc::new(Ppu::new(clock.clone(), mapper.clone(), frame_buffer));
        let cpu    = Cpu::new(clock.clone(), mapper, chip.clone(), ppu.clone());

        // Create "async" pool to handle clock cycles
        let mut pool = LocalPool::new();
        let spawner = pool.spawner();
        spawner.spawn_local(cpu.run()).unwrap();
        spawner.spawn_local(async move { ppu.clone().run().await }).unwrap();
        spawner.spawn_local(async move { run_chip(chip).await }).unwrap();

        let now = SystemTime::now();
        // for _ in 0..CYCLES_TO_RUN {
        loop {
            pool.run_until_stalled();
            clock.borrow_mut().tick();
        }

        println!("{} Instructions per us", (CYCLES_TO_RUN as f64) / now.elapsed().unwrap().as_secs_f64() / 1e6);
    });

    renderer.run().map_err(Into::into)
}