use std::time::{Duration, Instant};

pub(super) struct Travel {
    from: f32,
    target: f32,
    started: Instant,
    duration: Duration,
}

impl Default for Travel {
    fn default() -> Self {
        Self { from: 0.0, target: 0.0, started: Instant::now(), duration: Duration::ZERO }
    }
}

impl Travel {
    #[cfg(test)]
    pub fn seek(&mut self, milliseconds: u64) {
        self.started = Instant::now() - Duration::from_millis(milliseconds);
    }

    pub fn value(&self, now: Instant) -> f32 {
        if self.duration.is_zero() {
            return self.target;
        }
        let t = (now.saturating_duration_since(self.started).as_secs_f32() / self.duration.as_secs_f32()).min(1.0);
        self.from + (self.target - self.from) * (1.0 - (1.0 - t).powi(3))
    }

    pub fn show(&mut self, show: bool, ms: u32, now: Instant) {
        let target = if show { 1.0 } else { 0.0 };
        if target != self.target || ms == 0 {
            self.from = self.value(now);
            self.target = target;
            self.started = now;
            self.duration = Duration::from_millis((ms as f32 * (target - self.from).abs()) as u64);
        }
    }

    pub fn active(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.started) < self.duration
    }

    pub fn shown(&self) -> bool {
        self.target == 1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reversing_reveal_preserves_position_and_reduced_motion_finishes_immediately() {
        let at = Instant::now();
        let mut travel = Travel::default();
        travel.show(true, 360, at);
        let halfway = at + Duration::from_millis(100);
        let before = travel.value(halfway);
        travel.show(false, 360, halfway);
        assert_eq!(before, travel.value(halfway));
        assert!(travel.value(halfway + Duration::from_millis(80)) < before);
        assert_eq!(travel.value(halfway + Duration::from_secs(1)), 0.0);
        travel.show(true, 0, halfway);
        assert_eq!(travel.value(halfway), 1.0);
        assert!(!travel.active(halfway));
    }
}
