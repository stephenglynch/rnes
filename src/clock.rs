use std::collections::BTreeMap;
use std::rc::Rc;
use std::cell::RefCell;
use std::task::{Waker, Context, Poll};
use std::future::Future;
use std::pin::Pin;
use std::time::{Instant, Duration};
use std::thread::sleep;
use std::sync::Arc;
use crate::system_control::SystemControl;

const CYCLE_PERIOD: Duration = Duration::from_nanos(186);

type Sleeper = (Waker, bool, bool);

pub struct Clock {
    pub current_cycle: u64,
    // Tracking cycles against time to determine how long to wait when we want
    // to let "real time" catchup to NES system time
    last_catchup_time: Instant,
    last_catchup_cycle: u64,
    // Tasks waiting for a specific cycle to pass
    sleepers: BTreeMap<u64, Vec<Sleeper>>,
    system_control: Arc<SystemControl>
}

impl Clock {
    pub fn new(system_control: Arc<SystemControl>) -> Self {
        Clock {
            current_cycle: 3 * 7, // The starting cycle number for the CPU
            last_catchup_time: Instant::now(),
            last_catchup_cycle: 0,
            sleepers: BTreeMap::new(), // TODO: This generates expensive heap allocations and is bottle necking performance
            system_control: system_control
        }
    }

    pub fn tick(&mut self) {
        self.current_cycle += 1;
        // Wake up everyone waiting for this specific cycle
        if let Some(wakers) = self.sleepers.remove(&self.current_cycle) {
            for (waker, catchup, frame_done) in wakers {
                // Wait if there's a request to catchup
                if catchup {
                    let duration_cycles = self.current_cycle - self.last_catchup_cycle;
                    let duration = CYCLE_PERIOD * duration_cycles as u32;
                    let now = Instant::now();
                    let wakeup_time = self.last_catchup_time + duration;
                    if wakeup_time > now {
                        let wait_duration = wakeup_time - now;
                        sleep(wait_duration);
                        self.last_catchup_time = wakeup_time;
                    } else {
                        self.last_catchup_time = now;
                    }
                    self.last_catchup_cycle = self.current_cycle;
                }

                // Check if there's a pause request blocking if necessary
                if frame_done {
                    self.system_control.wait_on_pause();
                }

                waker.wake();
            }
        }
    }
}

pub struct CycleDelay {
    clock: Rc<RefCell<Clock>>,
    until: u64,
    catchup: bool, // Do we try and catch up to current real time
    frame_done: bool // Indicates frame has been completed
}

impl CycleDelay {
    pub fn new(clock: Rc<RefCell<Clock>>, until: u64, catchup: bool) -> Self {
        let current_cycle = clock.borrow().current_cycle;
        CycleDelay {
            clock: clock,
            until: current_cycle + until,
            catchup: catchup,
            frame_done: false
        }
    }

    pub fn frame_done(clock: Rc<RefCell<Clock>>, frame_done: bool) -> Self {
        let current_cycle = clock.borrow().current_cycle;
        CycleDelay {
            clock: clock,
            until: current_cycle + 1,
            catchup: false,
            frame_done: frame_done
        }
    }
}

impl Future for CycleDelay {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut clock = self.clock.borrow_mut();
        if clock.current_cycle >= self.until {
            Poll::Ready(())
        } else {
            // Register the current task's waker to be called later
            clock.sleepers
                .entry(self.until)
                .or_default()
                .push((cx.waker().clone(), self.catchup, self.frame_done));
            Poll::Pending
        }
    }
}