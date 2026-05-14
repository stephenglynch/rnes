use crate::audio::{AudioInterface, Sound};
use super::length_counter::LengthCounter;
use super::constants::CPU_HZ;

pub struct Triangle {
    interface: AudioInterface,
    counter_reload_flag: bool,
    counter_reload: u8,
    counter: u8,
    pub length_counter: LengthCounter,
    timer: u16,
    muted: bool
}

impl Triangle {
    pub fn new(interface: AudioInterface, muted: bool) -> Self {
        Self {
            interface: interface,
            counter_reload_flag: false,
            counter_reload: 0,
            counter: 0,
            length_counter: LengthCounter::new(),
            timer: 0,
            muted: muted
        }
    }

    pub fn set_reg(&mut self, loc: usize, val: u8) {
        match loc {
            0 => {
                self.length_counter.halt = val & 0x80 != 0;
                self.counter_reload = val & 0x7f;
            },
            1 => {
                // Do nothing
            },
            2 => {
                self.timer &= !0x00ff;
                self.timer |= val as u16;
            },
            3 => {
                self.timer &= !0x0f00;
                self.timer |= (val as u16 & 0x07) << 8;
                self.length_counter.set(val >> 3);
                self.counter_reload_flag = true;
            }
            _ => unreachable!("Should not get here")
        }
    }

    pub fn tick_linear_counter(&mut self) {
        // Tick linear counter
        if self.counter_reload_flag {
            self.counter = self.counter_reload;
        } else {
            if self.counter > 0 {
                self.counter -= 1;
            }
        }
        if !self.length_counter.halt {
            self.counter_reload_flag = false;
        }

        // Check if we generate a triangle wave
        if !self.muted && self.length_counter.unmuted() && self.counter > 0 {
            let period = (((self.timer + 1) * 32) as f32) / CPU_HZ;
            let _ = self.interface.tx.send(Sound::TriangleWave { period: period });
        } else {
            let _ = self.interface.tx.send(Sound::None);
        }
    }

    pub fn tick_length_counter(&mut self) {
        self.length_counter.tick();
    }
}