//! Rate-limits work driven by every drag frame without losing the last change.
use std::time::Duration;
use zgui::{prelude::*, view::IntervalHandle};

/// Runs work at once, then at most once per interval while changes keep
/// arriving; the final change always runs.
#[derive(Clone, Copy)]
pub struct Throttle {
    interval: Duration,
    active: StoredValue<Option<IntervalHandle>, LocalStorage>,
    dirty: StoredValue<bool, LocalStorage>,
}
impl Throttle {
    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            active: StoredValue::new_local(None),
            dirty: StoredValue::new_local(false),
        }
    }
    pub fn run(self, work: impl Fn() + Copy + 'static) {
        if self.active.with_value(Option::is_some) {
            self.dirty.set_value(true);
            return;
        }
        work();
        // Without a window clock, run every change.
        let Some(timers) = Timers::current() else {
            return;
        };
        // A repeating timer that cancels itself once idle: a fired one-shot
        // handle would leave its id behind in the timer heap's cancel set.
        let interval = timers.set_interval(self.interval, move || {
            if self.dirty.with_value(|dirty| *dirty) {
                self.dirty.set_value(false);
                work();
            } else {
                self.active.set_value(None);
            }
        });
        self.active.set_value(Some(interval));
    }
}
