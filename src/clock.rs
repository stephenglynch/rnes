use std::collections::BTreeMap;
use std::rc::Rc;
use std::cell::RefCell;
use std::task::{Waker, Context, Poll};
use std::future::Future;
use std::pin::Pin;

pub struct Clock {
    pub current_cycle: u64,
    // Tasks waiting for a specific cycle to pass
    sleepers: BTreeMap<u64, Vec<Waker>>,
}

impl Clock {
    pub fn new() -> Self {
        Clock {
            current_cycle: 3 * 7, // The starting cycle number for thge CPU
            sleepers: BTreeMap::new() // TODO: This generates expensive heap allocations and is bottle necking performance
        }
    }

    pub fn tick(&mut self) {
        self.current_cycle += 1;
        // Wake up everyone waiting for this specific cycle
        if let Some(wakers) = self.sleepers.remove(&self.current_cycle) {
            for waker in wakers {
                waker.wake();
            }
        }
    }
}

pub struct CycleDelay {
    clock: Rc<RefCell<Clock>>,
    until: u64,
}

impl CycleDelay {
    pub fn new(clock: Rc<RefCell<Clock>>, until: u64) -> Self {
        let current_cycle = clock.borrow().current_cycle;
        CycleDelay {
            clock: clock,
            until: current_cycle + until,
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
                .push(cx.waker().clone());
            Poll::Pending
        }
    }
}