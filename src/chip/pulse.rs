use crate::audio::{AudioInterface, Sound};
use super::envelope::Envelope;
use super::length_counter::LengthCounter;
use super::constants::CPU_HZ;

const DUTY_TABLE: [f32; 4] = [0.125, 0.250, 0.500, 0.750];

pub struct Pulse {
    id: usize,
    interface: AudioInterface,
    duty: u8,
    pub length_counter: LengthCounter,
    pub envelope: Envelope,
    timer: u16,
    muted: bool
}

impl Pulse {
    pub fn new(id: usize, interface: AudioInterface, muted: bool) -> Self {
        Self {
            id: id,
            interface: interface,
            duty: 0,
            length_counter: LengthCounter::new(),
            envelope: Envelope::new(),
            timer: 0,
            muted: muted
        }
    }

    pub fn tick(&mut self) {
        self.length_counter.tick();
        if !self.muted && self.length_counter.unmuted() && self.timer >= 8 {
            let period = (((self.timer + 1) * 16) as f32) / CPU_HZ;
            // println!("Generating tone of {} Hz", 1.0/period);
            let duty = DUTY_TABLE[(self.duty & 0x03) as usize];
            let volume = self.envelope.output_volume();
            let _ = self.interface.tx.send(Sound::SquareWave { period: period, duty: duty, volume: volume});
        } else {
            let _ = self.interface.tx.send(Sound::None);
        }
    }

    pub fn set_reg(&mut self, loc: usize, val: u8) {
        match loc {
            0 => {
                self.duty = (val & 0xc0) >> 6;
                self.length_counter.halt = (val & 0x20) != 0;
                self.envelope.set_constant_vol((val & 0x10) != 0);
                self.envelope.set_volume(val & 0x0f);
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
                self.envelope.set_start();
            }
            _ => unreachable!("Should not get here")
        }
    }
}