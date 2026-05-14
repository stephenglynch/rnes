use crate::audio::{AudioInterface, Sound};
use super::length_counter::LengthCounter;
use super::envelope::Envelope;

struct Noise {
    interface: AudioInterface,
    mode: bool,
    timer: u8,
    envelope: Envelope,
    length_counter: LengthCounter,
    muted: bool
}

impl Noise {
    fn new(id: usize, interface: AudioInterface, muted: bool) -> Self {
        Self {
            interface: interface,
            length_counter: LengthCounter::new(),
            envelope: Envelope::new(),
            timer: 0,
            muted: muted
        }
    }

    fn tick(&mut self) {
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

    fn set_reg(&mut self, loc: usize, val: u8) {
        match loc {
            0 => {
                self.length_counter.halt = (val & 0x20) != 0;
                self.envelope.set_constant_vol((val & 0x10) != 0);
                self.envelope.set_volume(val & 0x0f);
            },
            1 => {
                // Unused
            },
            2 => {
                self.timer &= !0x0f;
                self.timer |= val & 0x0f;
                self.mode = val & 0x80 != 0;
            },
            3 => {
                self.length_counter.set(val >> 3);
                self.envelope.set_start();
            }
            _ => unreachable!("Should not get here")
        }
    }
}