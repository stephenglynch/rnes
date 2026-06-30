use std::cell::{RefCell, Cell};
use std::rc::Rc;
use crate::audio::{AudioInterface, NesSample};
use super::length_counter::LengthCounter;
use super::constants::CPU_HZ;
use crate::clock::{Clock, CycleDelay};

macro_rules! cycles_cpu {
    ($chip:expr, $n:expr) => {
        let clock = $chip.clock.clone();
        CycleDelay::new(clock, $n * 3, false).await
    }
}

pub struct Triangle {
    clock: Rc<RefCell<Clock>>,
    interface: AudioInterface,
    counter_reload_flag: Cell<bool>,
    counter_reload: Cell<u8>,
    counter: Cell<u8>,
    length_counter: RefCell<LengthCounter>,
    timer: Cell<u16>,
    muted: bool,
    output_i: Cell<usize>,
}

const fn generate_triangle_lut() -> [f32; 32] {
    let volume = 0.00851;
    let mut lut = [0.0; 32];
    let mut i = 0;
    let raw = [
        15, 14, 13, 12, 11, 10,  9,  8,  7,  6,  5,  4,  3,  2,  1,  0,
        0,  1,  2,  3,  4,  5,  6,  7,  8,  9, 10, 11, 12, 13, 14, 15
    ];
    while i < 32 {
        lut[i] = (raw[i] as f32) * volume;
        i += 1;
    }
    lut
}

impl Triangle {
    pub fn new(interface: AudioInterface, clock: Rc<RefCell<Clock>>, muted: bool) -> Self {
        Self {
            clock: clock,
            interface: interface,
            counter_reload_flag: Cell::new(false),
            counter_reload: Cell::new(0),
            counter: Cell::new(0),
            length_counter: RefCell::new(LengthCounter::new()),
            timer: Cell::new(0),
            muted: muted,
            output_i: Cell::new(0)
        }
    }

    pub fn start(self: Rc<Self>) {
        let clock = self.clock.clone();
        clock.borrow().spawn(async move {
            loop {
                self.next_output().await;
            }
         });
    }

    pub fn get_enabled(&self) -> bool {
        self.length_counter.borrow().get_enabled()
    }

    pub fn set_enabled(&self, val: bool) {
        self.length_counter.borrow_mut().set_enabled(val)
    }

    fn increment_output_i(&self) {
        let output_i = self.output_i.get();
        if output_i == 31 {
            self.output_i.set(0);
        } else {
            self.output_i.set(output_i + 1);
        }
    }

    async fn next_output(&self) {
        let timer_cycles = self.timer.get() + 1;
        let output_table = generate_triangle_lut();
        if !self.muted && self.length_counter.borrow().unmuted() && self.counter.get() > 0 && self.timer.get() >= 8 {
            let duty_val = output_table[self.output_i.get()] as f32;
            let _ = self.interface.tx.send(NesSample {
                volume: duty_val,
                duration: timer_cycles as f32 / CPU_HZ,
            });
        }
        self.increment_output_i();
        cycles_cpu!(self, timer_cycles as u64);
    }

    pub fn set_reg(&self, loc: usize, val: u8) {
        match loc {
            0 => {
                self.length_counter.borrow_mut().halt = (val & 0x80) != 0;
                self.counter_reload.set(val & 0x7f);
            },
            1 => {
                // Do nothing
            },
            2 => {
                self.timer.update(|t| {
                    let t = t & !0x00ff;
                    t | val as u16
                });
            },
            3 => {
                self.timer.update(|t| {
                    let t = t & !0x0f00;
                    t | (val as u16 & 0x07) << 8
                });
                self.length_counter.borrow_mut().set(val >> 3);
                self.counter_reload_flag.set(true);
            }
            _ => unreachable!("Should not get here")
        }
    }

    pub fn tick_linear_counter(&self) {
        // Tick linear counter
        if self.counter_reload_flag.get() {
            self.counter.set(self.counter_reload.get());
        } else {
            if self.counter.get() > 0 {
                self.counter.update(|c| c - 1);
            }
        }
        // Linear counter shares the length counter halt
        if !self.length_counter.borrow().halt {
            self.counter_reload_flag.set(false);
        }
    }

    pub fn tick_length(&self) {
        self.length_counter.borrow_mut().tick();
    }
}