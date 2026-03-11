use std::sync::{Arc, Mutex};
use std::collections::VecDeque;
use winit::event::KeyEvent;

mod gamepad_manager;
mod keyboard_manager;

pub type ActiveGamepads = Arc<Mutex<VecDeque<(usize, GamepadState)>>>;

#[derive(Default, Clone, Debug)]
pub struct GamepadState {
    a: bool,
    b: bool,
    select: bool,
    start: bool,
    up: bool,
    down: bool,
    left: bool,
    right: bool,
}

pub struct InputManager {
    gamepads: ActiveGamepads,
    keyboard_manager: Option<keyboard_manager::KeyboardManager>
}

impl GamepadState {
    pub fn serialise(&self) -> [u8; 8] {
        [
            self.right,
            self.left,
            self.down,
            self.up,
            self.start,
            self.select,
            self.b,
            self.a,
        ].map(|x| x as u8)
    }
}

impl InputManager {
    pub fn new(use_keyboard: bool) -> Self {
        let gamepads = Arc::new(Mutex::new(VecDeque::new()));

        let keyboard_manager = if use_keyboard {
            Some(keyboard_manager::KeyboardManager::new(gamepads.clone()))
        } else {
            gamepad_manager::GamepadManager::new(gamepads.clone()).start();
            None
        };

        Self {
            keyboard_manager: keyboard_manager,
            gamepads: gamepads,
        }
    }

    pub fn handle_key_event(&self, key_event: KeyEvent) {
        if let Some(keyboard_manager) = &self.keyboard_manager {
            keyboard_manager.handle_key_event(key_event);
        }
    }

    pub fn get_gamepads(&self) -> ActiveGamepads {
        self.gamepads.clone()
    }
}