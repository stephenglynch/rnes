use winit::{
    event::{ElementState, KeyEvent},
    keyboard::{KeyCode, PhysicalKey}
};
use super::ActiveGamepads;

pub struct KeyboardManager {
    gamepads: ActiveGamepads
}

impl KeyboardManager {
    pub fn new(gamepads: ActiveGamepads) -> Self {
        gamepads.lock().unwrap()
            .push_front((0, Default::default()));
        Self {
            gamepads: gamepads
        }
    }

    pub fn handle_key_event(&self, key_event: KeyEvent) {
        let key = key_event.physical_key;
        let pressed = key_event.state == ElementState::Pressed;
        if let PhysicalKey::Code(key) = key {
            let (_, gamepad) = &mut self.gamepads.lock().unwrap()[0];
            let mut unused = false;
            *(match key {
                KeyCode::KeyW => &mut gamepad.up,
                KeyCode::KeyA => &mut gamepad.left,
                KeyCode::KeyS => &mut gamepad.down,
                KeyCode::KeyD => &mut gamepad.right,
                KeyCode::KeyU => &mut gamepad.select,
                KeyCode::KeyI => &mut gamepad.start,
                KeyCode::KeyJ => &mut gamepad.a,
                KeyCode::KeyK => &mut gamepad.b,
                _ => &mut unused
            }) = pressed;
        }
    }
}