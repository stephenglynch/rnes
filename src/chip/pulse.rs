use std::cell::{RefCell, Cell};
use std::rc::Rc;
use crate::audio::{AudioInterface, NesSample};
use super::envelope::Envelope;
use super::length_counter::LengthCounter;
use super::constants::CPU_HZ;
use crate::clock::{Clock, CycleDelay};

macro_rules! cycles_apu {
    ($chip:expr, $n:expr) => {
        let clock = $chip.clock.clone();
        CycleDelay::new(clock, $n * 2 * 3, false).await
    }
}

pub struct Pulse {
    _id: usize,
    clock: Rc<RefCell<Clock>>,
    interface: AudioInterface,
    duty: Cell<u8>,
    length_counter: RefCell<LengthCounter>,
    envelope: RefCell<Envelope>,
    timer: Cell<u16>,
    muted: bool,
    output_i: Cell<usize>,
}

impl Pulse {
    pub fn new(id: usize, interface: AudioInterface, clock: Rc<RefCell<Clock>>, muted: bool) -> Self {
        Self {
            _id: id,
            clock: clock,
            interface: interface,
            duty: Cell::new(0),
            length_counter: RefCell::new(LengthCounter::new()),
            envelope: RefCell::new(Envelope::new()),
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

    fn increment_output_i(&self) {
        let output_i = self.output_i.get();
        if output_i == 7 {
            self.output_i.set(0);
        } else {
            self.output_i.set(output_i + 1);
        }
    }

    async fn next_output(&self) {
        let timer_cycles = self.timer.get() + 1;
        let output_table = [
            [0, 1, 0, 0, 0, 0, 0, 0],
            [0, 1, 1, 0, 0, 0, 0, 0],
            [0, 1, 1, 1, 1, 0, 0, 0],
            [1, 0, 0, 1, 1, 1, 1, 1],
        ];
        if !self.muted && self.length_counter.borrow().unmuted() && self.timer.get() >= 8 {
            let duty_val = output_table[self.duty.get() as usize][self.output_i.get()] as f32;
            let _ = self.interface.tx.send(NesSample {
                volume: self.envelope.borrow().output_volume() * duty_val,
                duration: (timer_cycles * 2) as f32 / CPU_HZ,
            });
        }
        self.increment_output_i();
        cycles_apu!(self, timer_cycles as u64);
    }

    pub fn tick_length(&self) {
        self.length_counter.borrow_mut().tick();
    }

    pub fn tick_envelope(&self) {
        self.length_counter.borrow_mut().tick();
    }

    pub fn get_enabled(&self) -> bool {
        self.length_counter.borrow().get_enabled()
    }

    pub fn set_enabled(&self, val: bool) {
        self.length_counter.borrow_mut().set_enabled(val)
    }

    pub fn set_reg(&self, loc: usize, val: u8) {
        match loc {
            0 => {
                self.duty.set((val & 0xc0) >> 6);
                self.length_counter.borrow_mut().halt = (val & 0x20) != 0;
                self.envelope.borrow_mut().set_constant_vol((val & 0x10) != 0);
                self.envelope.borrow_mut().set_volume(val & 0x0f);
            },
            1 => {
                // Do nothing
            },
            2 => {
                self.timer.update(|t| t & !0x00ff);
                self.timer.update(|t| t | val as u16);
            },
            3 => {
                self.timer.update(|t| t & !0x0f00);
                self.timer.update(|t| t | (val as u16 & 0x07) << 8);
                self.length_counter.borrow_mut().set(val >> 3);
                self.envelope.borrow_mut().set_start();
            }
            _ => unreachable!("Should not get here")
        }
    }
}