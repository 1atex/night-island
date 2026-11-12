use std::cell::Cell;

#[link(name = "winmm")]
unsafe extern "system" {
    fn timeBeginPeriod(period: u32) -> u32;
    fn timeEndPeriod(period: u32) -> u32;
}

#[derive(Default)]
pub(crate) struct FrameTimers {
    short: Cell<u64>,
    dispatching: Cell<bool>,
    high_resolution: Cell<bool>,
}

impl FrameTimers {
    pub fn update(&self, id: usize, short: bool) {
        let flag = 1_u64 << id;
        let pending = self.short.get();
        self.short.set(if short { pending | flag } else { pending & !flag });
        if !self.dispatching.get() {
            self.reconcile();
        }
    }

    pub fn begin_dispatch(&self) -> bool {
        self.dispatching.replace(true)
    }

    pub fn end_dispatch(&self, nested: bool) {
        self.dispatching.set(nested);
        if !nested {
            self.reconcile();
        }
    }

    fn reconcile(&self) {
        let needed = self.short.get() != 0;
        if needed != self.high_resolution.get() {
            let result = unsafe { if needed { timeBeginPeriod(1) } else { timeEndPeriod(1) } };
            if result == 0 {
                self.high_resolution.set(needed);
            }
        }
    }
}

impl Drop for FrameTimers {
    fn drop(&mut self) {
        if self.high_resolution.get() {
            unsafe { timeEndPeriod(1) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolution_follows_fast_timers_across_dispatch_and_idle() {
        let timers = FrameTimers::default();
        timers.update(2, false);
        assert!(!timers.high_resolution.get());
        timers.update(1, true);
        timers.update(2, true);
        let nested = timers.begin_dispatch();
        timers.update(1, false);
        timers.update(2, false);
        assert!(timers.high_resolution.get());
        timers.update(1, true);
        timers.end_dispatch(nested);
        assert!(timers.high_resolution.get());
        timers.update(1, false);
        assert!(!timers.high_resolution.get());
    }
}
