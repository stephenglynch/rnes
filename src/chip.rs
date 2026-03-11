use std::cell::RefCell;
use std::rc::Rc;
use crate::audio::{Audio, AudioInterface, Sound};
use crate::input::ActiveGamepads;
use crate::clock::{Clock, CycleDelay};

// Awaits a certain number of APU clock cycles (2x CPU cycles)
macro_rules! cycles {
    ($chip:expr, $n:expr) => {
        let clock = $chip.borrow_mut().clock.clone();
        CycleDelay::new(clock, $n * 6, false).await
    }
}

struct Envelope {
    volume: u8,
    divider: u8,
    decay: u8,
    start: bool,
    constant_vol: bool,
    loop_flag: bool,
}

struct Pulse {
    id: usize,
    interface: AudioInterface,
    enabled: bool,
    duty: u8,
    counter_halt: bool,
    envelope: Envelope,
    timer: u16,
    length: u8
}

struct Noise {
    interface: AudioInterface,
    enabled: bool,
    mode: bool,
    period: u8,
    length: u8
}

struct Triangle {
    interface: AudioInterface,
    control_flag: bool,
    counter_reload_flag: bool,
    counter_reload: u8,
    counter: u8,
    timer: u16,
    length: u8
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

const CPU_HZ: f32 = 1.789773e6;
const LENGTH_TABLE: [u8; 32] = [
    10, 254, 20,  2, 40,  4, 80,  6, 160,  8, 60, 10, 14, 12, 26, 14,
    12,  16, 24, 18, 48, 20, 96, 22, 192, 24, 72, 26, 16, 28, 32, 30
];
const DUTY_TABLE: [f32; 4] = [0.125, 0.250, 0.500, 0.750];

impl Envelope {
    fn new() -> Self {
        Self {
            volume: 0,
            divider: 0,
            decay: 0,
            start: false,
            constant_vol: false,
            loop_flag: false,
        }
    }

    fn set_constant_vol(&mut self, val: bool) {
        self.constant_vol = val;
    }

    fn set_start(&mut self) {
        self.start = true;
    }

    fn set_volume(&mut self, volume: u8) {
        self.volume = volume & 0x0f;
    }

    fn tick(&mut self) {
        if self.start {
            self.start = false;
            self.decay = 15;
            self.divider = self.volume;
        } else if self.divider == 0{
            self.divider = self.volume;
            self.tick_decay();
        } else {
            self.divider -= 1;
        }
    }

    fn tick_decay(&mut self) {
        if self.decay > 0 {
            self.decay -= 1;
        } else if self.decay == 0 && self.loop_flag {
            self.decay = 15;
        }
    }

    fn output_volume(&self) -> f32 {
        (if self.constant_vol {
            self.volume as f32
        } else {
            self.decay as f32
        }) / 15.0
    }
}

impl Pulse {
    fn new(id: usize, interface: AudioInterface) -> Self {
        Self {
            id: id,
            interface: interface,
            enabled: false,
            duty: 0,
            counter_halt: false,
            envelope: Envelope::new(),
            timer: 0,
            length: 0
        }
    }

    fn tick(&mut self) {
        if !self.counter_halt {
            self.length = self.length.saturating_sub(1);
        }
        if self.length > 0 && self.timer >= 8 {
            let period = (((self.timer + 1) * 16) as f32) / CPU_HZ;
            // println!("Generating tone of {} Hz", 1.0/period);
            let duty = DUTY_TABLE[(self.duty & 0x03) as usize];
            let volume = self.envelope.output_volume();
            let _ = self.interface.tx.send(Sound::SquareWave { period: period, duty: duty, volume: volume});
        } else {
            let _ = self.interface.tx.send(Sound::None);
        }
    }

    fn set_reg(&mut self, loc: usize, val: u8) {
        match loc {
            0 => {
                self.duty = (val & 0xc0) >> 6;
                self.counter_halt = (val & 0x20) != 0;
                self.envelope.set_constant_vol((val & 0x10) != 0);
                self.envelope.set_volume(val & 0x0f);
            },
            1 => {
                // Do nothing
            },
            2 => {
                self.timer &= 0x00ff;
                self.timer |= val as u16;
            },
            3 => {
                self.timer &= 0x0f00;
                self.timer |= (val as u16 & 0x07) << 8;
                self.length = LENGTH_TABLE[(val >> 3) as usize];
                self.envelope.set_start();
            }
            _ => unreachable!("Should not get here")
        }
    }
}

impl Triangle {
    fn new(interface: AudioInterface) -> Self {
        Self {
            interface: interface,
            control_flag: false,
            counter_reload_flag: false,
            counter_reload: 0,
            counter: 0,
            timer: 0,
            length: 0
        }
    }

    fn set_reg(&mut self, loc: usize, val: u8) {
        match loc {
            0 => {
                self.control_flag = val & 0x80 != 0;
                self.counter_reload = val & 0x7f;
            },
            1 => {
                // Do nothing
            },
            2 => {
                self.timer &= 0x00ff;
                self.timer |= val as u16;
            },
            3 => {
                self.timer &= 0x0f00;
                self.timer |= (val as u16 & 0x07) << 8;
                self.length = LENGTH_TABLE[(val >> 3) as usize];
                self.counter_reload_flag = true;
            }
            _ => unreachable!("Should not get here")
        }
    }

    fn tick_linear_counter(&mut self) {
        // Tick linear counter
        if self.counter_reload_flag {
            self.counter = self.counter_reload;
        } else {
            if self.counter > 0 {
                self.counter -= 1;
            }
        }
        if !self.control_flag {
            self.counter_reload_flag = false;
        }

        // Check if we generate a triangle wave
        if self.length > 0 && self.counter > 0 {
            let period = (((self.timer + 1) * 16) as f32) / CPU_HZ;
            let _ = self.interface.tx.send(Sound::TriangleWave { period: period });
        } else {
            let _ = self.interface.tx.send(Sound::None);
        }
    }

    fn tick_length_counter(&mut self) {
        if !self.control_flag {
            self.length = self.length.saturating_sub(1);
        }
    }
}

impl Chip {
    pub fn new(clock: Rc<RefCell<Clock>>, audio: Audio, active_gamepads: ActiveGamepads) -> Self {
        let pulse1 = Pulse::new(1, audio.create_interface().unwrap());
        let pulse2 = Pulse::new(2, audio.create_interface().unwrap());
        let triangle = Triangle::new(audio.create_interface().unwrap());
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
                if val & 0x01 != 0 {
                    self.pulse1.enabled = true;
                } else {
                    self.pulse1.enabled = false;
                    self.pulse1.length = 0;
                }

                if val & 0x02 != 0 {
                    self.pulse2.enabled = true;
                } else {
                    self.pulse2.enabled = false;
                    self.pulse2.length = 0;
                }
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