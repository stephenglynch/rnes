use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::SystemTime;
use std::thread;
use futures::executor::LocalPool;

use crate::Config;
use crate::chip::Chip;
use crate::ppu::Ppu;
use crate::system::Cpu;
use crate::clock::Clock;
use crate::renderer::Renderer;
use crate::input::InputManager;
use crate::mapper::generate_mapper;
use crate::system_control::SystemControl;

const CYCLES_TO_RUN: usize = 100000000;

pub fn execute_rom(config: Config) -> Result<(), Box<dyn std::error::Error>> {
    // Create renderer
    let input_manager= InputManager::new(true);

    let system_control = Arc::new(SystemControl::new());
    let renderer = Renderer::new(|key| input_manager.handle_key_event(key), system_control.clone());
    let frame_buffer = renderer.get_frame_buffer();

    let gamepads = input_manager.get_gamepads();

    thread::spawn(move || {
        // Create "async" pool to handle clock cycles
        let mut pool = LocalPool::new();

        // Build NES components
        let clock  = Rc::new(RefCell::new(Clock::new(system_control, pool.spawner())));
        let mapper  = generate_mapper(config.ines);
        let chip = Rc::new(Chip::new(clock.clone(), config.audio, gamepads));
        let ppu    = Rc::new(Ppu::new(clock.clone(), mapper.clone(), frame_buffer));
        let cpu    = Cpu::new(clock.clone(), mapper, chip.clone(), ppu.clone());

        // Start Async coroutines
        cpu.start();
        ppu.start();
        chip.start();

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