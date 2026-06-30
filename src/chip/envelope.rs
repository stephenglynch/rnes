pub struct Envelope {
    volume: u8,
    divider: u8,
    decay: u8,
    start: bool,
    constant_vol: bool,
    loop_flag: bool,
}

impl Envelope {
    pub fn new() -> Self {
        Self {
            volume: 0,
            divider: 0,
            decay: 0,
            start: false,
            constant_vol: false,
            loop_flag: false,
        }
    }

    pub fn set_constant_vol(&mut self, val: bool) {
        self.constant_vol = val;
    }

    pub fn set_start(&mut self) {
        self.start = true;
    }

    pub fn set_volume(&mut self, volume: u8) {
        self.volume = volume & 0x0f;
    }

    pub fn tick(&mut self) {
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

    pub fn output_volume(&self) -> f32 {
        (if self.constant_vol {
            self.volume as f32
        } else {
            self.decay as f32
        }) * 0.00752
    }
}