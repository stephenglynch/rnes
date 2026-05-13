use std::f32::consts::PI;

pub struct LowPassFilter {
    sample_period: f32,
    freq_hz: f32,
    last_y: f32
}

pub struct HighPassFilter {
    sample_period: f32,
    freq_hz: f32,
    last_y: f32,
    last_x: f32
}

impl LowPassFilter {
    pub fn new(sample_period: f32, freq_hz: f32) -> Self {
        Self {
            sample_period: sample_period,
            freq_hz: freq_hz,
            last_y: 0.0
        }
    }

    pub fn apply(&mut self, x: f32) -> f32 {
        let angular_term = 2.*PI*self.freq_hz*self.sample_period;
        let alpha = angular_term / (1. + angular_term);
        let y = alpha*x + (1. - alpha)*self.last_y;
        self.last_y = y;
        y
    }
}

impl HighPassFilter {
    pub fn new(sample_period: f32, freq_hz: f32) -> Self {
        Self {
            sample_period: sample_period,
            freq_hz: freq_hz,
            last_y: 0.0,
            last_x: 0.0
        }
    }

    pub fn apply(&mut self, x: f32) -> f32 {
        let angular_term = 2.*PI*self.freq_hz*self.sample_period;
        let alpha = 1. / (1. + angular_term);
        let y = alpha*self.last_y + alpha*(x - self.last_x);
        self.last_y = y;
        self.last_x = x;
        y
    }
}