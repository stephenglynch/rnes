use std::collections::BTreeMap;
use std::rc::Rc;
use std::cell::RefCell;
use std::task::{Waker, Context, Poll};
use std::future::Future;
use std::pin::Pin;
use std::time::{Instant, Duration};
use std::thread::sleep;

const CYCLE_PERIOD: Duration = Duration::from_nanos(186);

pub struct Clock {
    pub current_cycle: u64,
    // Tracking cycles against time to determine how long to wait when we want
    // to let "real time" catchup to NES system time
    last_catchup_time: Instant,
    last_catchup_cycle: u64,
    // Tasks waiting for a specific cycle to pass
    sleepers: BTreeMap<u64, Vec<(Waker, bool)>>,
}

impl Clock {
    pub fn new() -> Self {
        Clock {
            current_cycle: 3 * 7, // The starting cycle number for thge CPU
            last_catchup_time: Instant::now(),
            last_catchup_cycle: 0,
            sleepers: BTreeMap::new() // TODO: This generates expensive heap allocations and is bottle necking performance
        }
    }

    pub fn tick(&mut self) {
        self.current_cycle += 1;
        // Wake up everyone waiting for this specific cycle
        if let Some(wakers) = self.sleepers.remove(&self.current_cycle) {
            for (waker, catchup) in wakers {
                // Wait if there's a request to catchup
                if catchup {
                    let duration_cycles = self.current_cycle - self.last_catchup_cycle;
                    let duration = CYCLE_PERIOD * duration_cycles as u32;
                    let now = Instant::now();
                    let wakeup_time = self.last_catchup_time + duration;
                    if wakeup_time > now {
                        let wait_duration = wakeup_time - now;
                        sleep(wait_duration);
                    }
                    self.last_catchup_cycle = self.current_cycle;
                    self.last_catchup_time = now;
                }
                waker.wake();
            }
        }
    }
}

pub struct CycleDelay {
    clock: Rc<RefCell<Clock>>,
    until: u64,
    catchup: bool // Do we try and catch up to current real time
}

impl CycleDelay {
    pub fn new(clock: Rc<RefCell<Clock>>, until: u64, catchup: bool) -> Self {
        let current_cycle = clock.borrow().current_cycle;
        CycleDelay {
            clock: clock,
            until: current_cycle + until,
            catchup: catchup
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
                .push((cx.waker().clone(), self.catchup));
            Poll::Pending
        }
    }
}