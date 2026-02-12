// TODO: Implement sampling of gamepad
// TOOD: Implement APU
use crate::gamepad_manager::ActiveGamepads;

pub struct Chip {
    active_gamepads: ActiveGamepads,
    gamepad_fifos: [Vec<u8>; 2],
}

impl Chip {
    pub fn new(active_gamepads: ActiveGamepads) -> Self {
        Chip {
            active_gamepads: active_gamepads,
            gamepad_fifos: Default::default(),
        }
    }

    fn read_game_pad(&mut self, index: usize) -> u8 {
        self.gamepad_fifos[index].pop().unwrap_or(1)
    }

    pub fn get_reg(&mut self, addr: usize) -> u8 {
        match addr {
            0x16 => self.read_game_pad(0),
            0x17 => self.read_game_pad(1),
            _ => 0 // Do nothing
        }
    }

    pub fn set_reg(&mut self, addr: usize, data: u8) {
        match addr {
            0x16 => {
                if data & 0x01 != 0  {
                    let sampled = self.active_gamepads.lock().unwrap();
                    for i in 0..self.gamepad_fifos.len() {
                        if let Some((_, state)) = sampled.get(i) {
                            let fifo = &mut self.gamepad_fifos[i];
                            fifo.clear();
                            fifo.extend_from_slice(&state.serialise());
                        }
                    }
                }
            }, // Start strobe
            _ => () // Do nothing
        }
    }
}