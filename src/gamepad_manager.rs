use gilrs::{Gilrs, Event, EventType};
use std::thread;
use std::collections::BTreeMap;
use std::sync::Mutex;

#[derive(Default, Clone, Copy, Debug)]
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

pub struct GamepadManager {
    gilrs: Gilrs,
    gamepads: Mutex<BTreeMap<usize, GamepadState>>
}

impl GamepadManager {
    pub fn new() -> Self {
        let gilrs = Gilrs::new().unwrap();
        let gamepads = Mutex::new(gilrs.gamepads().map(|(id, _)| {
            (id.into(), Default::default())
        }).collect());
        GamepadManager {
            gilrs: gilrs,
            gamepads: gamepads,
        }
    }

    pub fn start(mut self) {
        thread::spawn(move || {
            println!("Starting gamepad manager thread");
            loop {
                // Examine new events
                // TODO: This is currently MacOS + PiHut SNES controller specific, need to make this generic somehow
                // TODO: Make this async instead of using blocking calls, would be nice to make this work on wasm
                while let Some(Event { id, event, .. }) = self.gilrs.next_event_blocking(None) {
                    let mut gamepads = self.gamepads.lock().unwrap();
                    match event {
                        EventType::Connected => {
                            gamepads.insert(id.into(), Default::default());
                        },
                        EventType::ButtonChanged(_, val, code) => {
                            if let Some(state) = gamepads.get_mut(&id.into()) {
                                match code.into_u32() {
                                    0x90002 => state.a = val.round() == 1.0,
                                    0x90003 => state.b = val.round() == 1.0,
                                    0x9000a => state.select = val.round() == 1.0,
                                    0x90009 => state.start = val.round() == 1.0,
                                    _ => ()
                                }
                                println!("state: {:?}", state);
                            }
                        },
                        EventType::AxisChanged(_, val,code) => {
                            if let Some(state) = gamepads.get_mut(&id.into()) {
                                match code.into_u32() {
                                    0x10030 => if val.round() > 0.0 {
                                        state.right = true;
                                        state.left = false;
                                    } else if val.round() < 0.0 {
                                        state.right = false;
                                        state.left = true;
                                    } else {
                                        state.right = false;
                                        state.left = false;
                                    },
                                    0x10031 => if val.round() > 0.0 {
                                        state.up = true;
                                        state.down = false;
                                    } else if val.round() < 0.0 {
                                        state.up = false;
                                        state.down = true;
                                    } else {
                                        state.up = false;
                                        state.down = false;
                                    },
                                    _ => ()
                                }
                            }
                        },
                        _ => ()
                    }
                }
            }
        });
    }
}

// 90009 = Start
// 9000a = Select
// 90002 = A
// 90003 = B
// 10030 = Right/Left
// 10031 = Up/Down