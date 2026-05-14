pub struct LengthCounter {
    enabled: bool,
    pub halt: bool,
    length: u8
}

const LENGTH_TABLE: [u8; 32] = [
    10, 254, 20,  2, 40,  4, 80,  6, 160,  8, 60, 10, 14, 12, 26, 14,
    12,  16, 24, 18, 48, 20, 96, 22, 192, 24, 72, 26, 16, 28, 32, 30
];

impl LengthCounter {
    pub fn new() -> Self {
        Self {
            enabled: false,
            halt: false,
            length: 0
        }
    }

    pub fn tick(&mut self) {
        if self.enabled && !self.halt {
            self.length = self.length.saturating_sub(1);
        }
    }

    pub fn unmuted(&self) -> bool {
        self.length > 0
    }

    pub fn set(&mut self, ind: u8) {
        if self.enabled {
            self.length = LENGTH_TABLE[(ind & 0x1f) as usize];
        }
    }

    pub fn set_enabled(&mut self, val: bool) {
        self.enabled = val;
        if val {
            self.length = 0;
        }
    }

    pub fn get_enabled(&self) -> bool {
        self.enabled
    }
}