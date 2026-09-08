pub struct Sweep {
    enabled: bool,
    negate: bool,
    counter_reload: u8,
    timer: u16,
    counter: u8,
    shift_amount: u8,
    reload_flag: bool,
    twos_complement: bool
}

impl Sweep {
    pub fn new() -> Self {
        Self {
            enabled: false,
            negate: false,
            counter: 0,
            counter_reload: 0,
            timer: 0,
            shift_amount: 0,
            reload_flag: false,
            twos_complement: true
        }
    }

    fn target_period(&self) -> u16 {
        let current = self.timer;
        let change = current >> self.shift_amount;
        if self.negate {
            if self.twos_complement {
                current.saturating_sub(change)
            } else {
                current.saturating_sub(change - 1)
            }
        } else {
            current + change
        }
    }

    pub fn unmuted(&self) -> bool {
        self.timer >= 8 && self.target_period() <= 0x7ff
    }

    pub fn tick(&mut self) {
        if self.enabled && self.counter == 0 && self.shift_amount > 0 {
            if self.unmuted() {
                self.timer = self.target_period();
            }
        }
        if self.counter == 0 || self.reload_flag {
            self.counter = self.counter_reload;
            self.reload_flag = false;
        } else {
            self.counter = self.counter.saturating_sub(1);
        }
    }

    pub fn set_reg(&mut self, val: u8) {
        self.enabled = val & 0x80 != 0; // Enables the sweep unit
        self.negate = val & 0x08 != 0; // Negates the change applied to the timer
        self.counter_reload = (val & 0x70) >> 4; // value to reload counter with
        self.shift_amount = val & 0x07; // Amount to shift the timer in the new target calculation
        self.reload_flag = true; // Generates a reload of the counter
    }

    pub fn get_timer(&self) -> u16 {
        self.timer
    }

    pub fn set_timer_upper(&mut self, val: u8) {
        self.timer &= !0xf00;
        self.timer |= ((val as u16) & 0x07) << 8;
    }

    pub fn set_timer_lower(&mut self, val: u8) {
        self.timer &= !0x0ff;
        self.timer |= val as u16;
    }
}