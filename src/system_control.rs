use std::sync::{Mutex, Condvar};

pub struct SystemControl {
    is_paused: Mutex<bool>,
    condvar: Condvar,
}

impl SystemControl {
    pub fn new() -> Self {
        Self {
            is_paused: Mutex::new(false),
            condvar: Condvar::new(),
        }
    }

    // Called by the Emulator Thread
    pub fn wait_on_pause(&self) {
        let mut paused = self.is_paused.lock().unwrap();
        while *paused {
            // This blocks the thread and releases the lock automatically.
            // It wakes up when the UI thread calls .notify_all()
            paused = self.condvar.wait(paused).unwrap();
        }
    }

    // Called by the UI/Winit Thread
    pub fn toggle_pause(&self) {
        let mut paused = self.is_paused.lock().unwrap();
        *paused = !*paused;
        if !*paused {
            // Wake up the emulator thread
            self.condvar.notify_all();
        }
    }
}