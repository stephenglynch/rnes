use std::cell::RefCell;
use std::rc::Rc;
use crate::audio::Audio;
use crate::input::ActiveGamepads;
use crate::clock::{Clock, CycleDelay};
use pulse::Pulse;
use triangle::Triangle;

mod constants;
mod length_counter;
mod envelope;
mod pulse;
mod triangle;

// Awaits a certain number of APU clock cycles (2x CPU cycles)
macro_rules! cycles {
    ($chip:expr, $n:expr) => {
        let clock = $chip.borrow_mut().clock.clone();
        CycleDelay::new(clock, $n * 6, false).await
    }
}

pub struct Chip {
    clock: Rc<RefCell<Clock>>,
    active_gamepads: ActiveGamepads,
    gamepad_fifos: [Vec<u8>; 2],
    pulse1: Pulse,
    pulse2: Pulse,
    triangle: Triangle,
    seq_mode: bool,
    int_flag: bool,
    int_set: bool
}

impl Chip {
    pub fn new(clock: Rc<RefCell<Clock>>, audio: Audio, active_gamepads: ActiveGamepads) -> Self {
        let pulse1 = Pulse::new(1, audio.create_interface().unwrap(), false);
        let pulse2 = Pulse::new(2, audio.create_interface().unwrap(), false);
        let triangle = Triangle::new(audio.create_interface().unwrap(), false);
        Self {
            clock: clock,
            active_gamepads: active_gamepads,
            gamepad_fifos: Default::default(),
            pulse1: pulse1,
            pulse2: pulse2,
            triangle: triangle,
            seq_mode: false,
            int_flag: false,
            int_set: false
        }
    }

    fn read_game_pad(&mut self, index: usize) -> u8 {
        self.gamepad_fifos[index].pop().unwrap_or(0)
    }

    pub fn int_request(&self) -> bool {
        self.int_flag
    }

    pub fn get_reg(&mut self, addr: usize) -> u8 {
        match addr {
            0x16 => self.read_game_pad(0),
            0x17 => self.read_game_pad(1),
            _ => 0 // Do nothing
        }
    }

    pub fn set_reg(&mut self, addr: usize, val: u8) {
        match addr {
            0x00..0x04 => {
                self.pulse1.set_reg(addr & 0x3, val);
            },
            0x04..0x08 => {
                self.pulse2.set_reg(addr & 0x3, val);
            },
            0x15 => {
                self.pulse1.length_counter.set_enabled(val & 0x01 != 0);
                self.pulse2.length_counter.set_enabled(val & 0x02 != 0);
                self.triangle.length_counter.set_enabled(val & 0x04 != 0);
            },
            0x08..0x0c => {
                self.triangle.set_reg(addr & 0x3, val);
            },
            0x16 => {
                if val & 0x01 != 0 {
                    let sampled = self.active_gamepads.lock().unwrap();
                    for i in 0..self.gamepad_fifos.len() {
                        if let Some((_, state)) = sampled.get(i) {
                            let fifo = &mut self.gamepad_fifos[i];
                            fifo.clear();
                            fifo.extend_from_slice(&state.serialise());
                        }
                    }
                }
            }, // Start strobe
            0x17 => {
                self.seq_mode = 0x80 & val != 0;
                self.int_flag = 0x40 & val != 0;
                // Clear interrupt if interrupt inhibit is set
                if self.int_flag {
                    self.int_set = false;
                }
            }
            _ => () // Do nothing
        }
    }
}

pub async fn run_chip(chip: Rc<RefCell<Chip>>) {
    loop {
        // Step 1
        cycles!(chip, 3728);
        chip.borrow_mut().pulse1.envelope.tick();
        chip.borrow_mut().pulse2.envelope.tick();
        chip.borrow_mut().triangle.tick_linear_counter();

        // Step 2
        cycles!(chip, 3728);
        chip.borrow_mut().pulse1.envelope.tick();
        chip.borrow_mut().pulse2.envelope.tick();
        chip.borrow_mut().triangle.tick_linear_counter();
        chip.borrow_mut().pulse1.tick();
        chip.borrow_mut().pulse2.tick();
        chip.borrow_mut().triangle.tick_length_counter();

        // Step 3
        chip.borrow_mut().pulse1.envelope.tick();
        chip.borrow_mut().pulse2.envelope.tick();
        chip.borrow_mut().triangle.tick_linear_counter();
        cycles!(chip, 3729);

        // Step 4
        cycles!(chip, 3729);
        if !chip.borrow().seq_mode {
            chip.borrow_mut().int_set = true;
        }

        // Step 4/5
        if chip.borrow().seq_mode {
            cycles!(chip, 3726);
        }
        chip.borrow_mut().pulse1.envelope.tick();
        chip.borrow_mut().pulse2.envelope.tick();
        chip.borrow_mut().triangle.tick_linear_counter();
        chip.borrow_mut().pulse1.tick();
        chip.borrow_mut().pulse2.tick();
        chip.borrow_mut().triangle.tick_length_counter();
    }
}