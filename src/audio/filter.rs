use std::f32::consts::PI;

pub struct LowPassFilter {
    sample_rate: f32,
    freq_hz: f32,
    last_y: f32
}

pub struct HighPassFilter {
    sample_rate: f32,
    freq_hz: f32,
    last_y: f32,
    last_x: f32
}

impl LowPassFilter {
    pub fn new(sample_rate: f32, freq_hz: f32) -> Self {
        Self {
            sample_rate: sample_rate,
            freq_hz: freq_hz,
            last_y: 0.0
        }
    }

    pub fn apply(&mut self, x: f32) -> f32 {
        let angular_term = 2.*PI*self.freq_hz/self.sample_rate;
        let alpha = angular_term / (1. + angular_term);
        let y = alpha*x + (1. - alpha)*self.last_y;
        self.last_y = y;
        y
    }
}

impl HighPassFilter {
    pub fn new(sample_rate: f32, freq_hz: f32) -> Self {
        Self {
            sample_rate: sample_rate,
            freq_hz: freq_hz,
            last_y: 0.0,
            last_x: 0.0
        }
    }

    pub fn apply(&mut self, x: f32) -> f32 {
        let angular_term = 2.*PI*self.freq_hz/self.sample_rate;
        let alpha_inv = 1. + angular_term;
        let y = (self.last_y + x - self.last_x) / alpha_inv;
        self.last_y = y;
        self.last_x = x;
        y
    }
}