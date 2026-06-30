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
        let clock = $chip.clock.clone();
        CycleDelay::new(clock, $n * 6, false).await
    }
}

struct ChipState {
    gamepad_fifos: [Vec<u8>; 2],
    seq_mode: bool,
    int_flag: bool,
    int_set: bool
}

pub struct Chip {
    clock: Rc<RefCell<Clock>>,
    pulse1: Rc<Pulse>,
    pulse2: Rc<Pulse>,
    triangle: Rc<Triangle>,
    active_gamepads: ActiveGamepads,
    chip_state: RefCell<ChipState>
}

impl Chip {
    pub fn new(clock: Rc<RefCell<Clock>>, audio: Audio, active_gamepads: ActiveGamepads) -> Self {
        let pulse1 = Pulse::new(1, audio.create_interface(1).unwrap(), clock.clone(), false);
        let pulse2 = Pulse::new(2, audio.create_interface(2).unwrap(), clock.clone(), false);
        let triangle = Triangle::new(audio.create_interface(3).unwrap(), clock.clone(), false);
        Self {
            clock: clock,
            pulse1: Rc::new(pulse1),
            pulse2: Rc::new(pulse2),
            triangle: Rc::new(triangle),
            active_gamepads: active_gamepads,
            chip_state: RefCell::new(ChipState {
                gamepad_fifos: Default::default(),
                seq_mode: false,
                int_flag: false,
                int_set: false
            })
        }
    }

    pub fn start(self: Rc<Self>) {
        self.pulse1.clone().start();
        self.pulse2.clone().start();
        self.triangle.clone().start();
        let clock = self.clock.clone();
        let chip = self.clone();
        clock.borrow().spawn(async move {
            let chip_state = &chip.chip_state;
            loop {
                // Step 1
                cycles!(chip, 3728);
                chip.pulse1.tick_envelope();
                chip.pulse2.tick_envelope();
                chip.triangle.tick_linear_counter();

                // Step 2
                cycles!(chip, 3728);
                chip.pulse1.tick_envelope();
                chip.pulse2.tick_envelope();
                chip.triangle.tick_linear_counter();
                chip.pulse1.tick_length();
                chip.pulse2.tick_length();
                chip.triangle.tick_length();

                // Step 3
                chip.pulse1.tick_envelope();
                chip.pulse2.tick_envelope();
                chip.triangle.tick_linear_counter();
                cycles!(chip, 3729);

                // Step 4
                cycles!(chip, 3729);
                if !chip_state.borrow().seq_mode {
                    chip_state.borrow_mut().int_set = true;
                }

                // Step 4/5
                if chip_state.borrow().seq_mode {
                    cycles!(chip, 3726);
                }
                chip.pulse1.tick_envelope();
                chip.pulse2.tick_envelope();
                chip.triangle.tick_linear_counter();
                chip.pulse1.tick_length();
                chip.pulse2.tick_length();
                chip.triangle.tick_length();
            }
        });
    }

    fn read_game_pad(&self, index: usize) -> u8 {
        self.chip_state.borrow_mut().gamepad_fifos[index].pop().unwrap_or(0)
    }

    pub fn int_request(&self) -> bool {
        self.chip_state.borrow().int_flag
    }

    pub fn get_reg(&self, addr: usize) -> u8 {
        match addr {
            0x15 => {
                ((self.pulse1.get_enabled() as u8) << 0) |
                ((self.pulse2.get_enabled() as u8) << 1) |
                ((self.triangle.get_enabled() as u8) << 1)
                // TODO: ((chip_state.noise.length_counter.get_enabled() as u8) << 1)
            }
            0x16 => self.read_game_pad(0),
            0x17 => self.read_game_pad(1),
            _ => 0 // Do nothing
        }
    }

    pub fn set_reg(&self, addr: usize, val: u8) {
        let mut chip_state = self.chip_state.borrow_mut();
        match addr {
            0x00..0x04 => {
                self.pulse1.set_reg(addr & 0x3, val);
            },
            0x04..0x08 => {
                self.pulse2.set_reg(addr & 0x3, val);
            },
            0x15 => {
                self.pulse1.set_enabled(val & 0x01 != 0);
                self.pulse2.set_enabled(val & 0x02 != 0);
                self.triangle.set_enabled(val & 0x04 != 0);
            },
            0x08..0x0c => {
                self.triangle.set_reg(addr & 0x3, val);
            },
            0x16 => {
                if val & 0x01 != 0 {
                    let sampled = self.active_gamepads.lock().unwrap();
                    for i in 0..chip_state.gamepad_fifos.len() {
                        if let Some((_, state)) = sampled.get(i) {
                            let fifo = &mut chip_state.gamepad_fifos[i];
                            fifo.clear();
                            fifo.extend_from_slice(&state.serialise());
                        }
                    }
                }
            }, // Start strobe
            0x17 => {
                chip_state.seq_mode = 0x80 & val != 0;
                chip_state.int_flag = 0x40 & val != 0;
                // Clear interrupt if interrupt inhibit is set
                if chip_state.int_flag {
                    chip_state.int_set = false;
                }
            }
            _ => () // Do nothing
        }
    }
}
