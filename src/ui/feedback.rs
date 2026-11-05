use std::cell::{Cell, RefCell};
use std::time::{Duration, Instant};

struct Value {
    key: u64,
    from: f32,
    target: f32,
    start: Instant,
    duration: Duration,
}

impl Value {
    fn at(&self, now: Instant) -> f32 {
        let t = (now.saturating_duration_since(self.start).as_secs_f32() / self.duration.as_secs_f32()).clamp(0.0, 1.0);
        self.from + (self.target - self.from) * (1.0 - (1.0 - t).powi(3))
    }
}

pub struct Feedback {
    enabled: Cell<bool>,
    values: RefCell<Vec<Value>>,
    limit: usize,
}

impl Default for Feedback {
    fn default() -> Self {
        Self::with_capacity(256)
    }
}

impl Feedback {
    pub fn with_capacity(limit: usize) -> Self {
        Self { enabled: Cell::new(true), values: RefCell::new(Vec::new()), limit: limit.clamp(1, 1024) }
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.set(enabled);
        if !enabled {
            self.values.borrow_mut().clear();
        }
    }

    pub fn sample(&self, key: u64, target: f32, initial: f32, ms: u64) -> f32 {
        self.sample_at(key, target, initial, ms, Instant::now())
    }

    fn sample_at(&self, key: u64, target: f32, initial: f32, ms: u64, now: Instant) -> f32 {
        if !self.enabled.get() {
            return target;
        }
        let mut values = self.values.borrow_mut();
        if let Some(value) = values.iter_mut().find(|v| v.key == key) {
            if value.target != target {
                value.from = value.at(now);
                value.target = target;
                value.start = now;
                value.duration = Duration::from_millis(ms.max(1));
            }
            value.at(now)
        } else {
            if values.len() >= self.limit {
                values.retain(|v| now.saturating_duration_since(v.start) < Duration::from_secs(2));
                if values.len() >= self.limit {
                    values.remove(0);
                }
            }
            values.push(Value { key, from: initial, target, start: now, duration: Duration::from_millis(ms.max(1)) });
            initial
        }
    }

    pub fn active(&self) -> bool {
        let now = Instant::now();
        self.enabled.get()
            && self
                .values
                .borrow()
                .iter()
                .any(|v| v.from != v.target && now.saturating_duration_since(v.start) < v.duration)
    }
}

pub fn motion_allowed() -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{
        SPI_GETCLIENTAREAANIMATION, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
    };
    let mut enabled = windows::core::BOOL(1);
    unsafe {
        let _ = SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            Some((&raw mut enabled).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );
    }
    enabled.as_bool()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dense_dock_animation_channels_settle_without_restarting() {
        let feedback = Feedback::with_capacity(512);
        let now = Instant::now();
        for key in 0..384 {
            assert_eq!(feedback.sample_at(key, 1.0, 0.0, 180, now), 0.0);
        }
        for key in 0..384 {
            assert_eq!(feedback.sample_at(key, 1.0, 0.0, 180, now + Duration::from_secs(3)), 1.0);
        }
        assert_eq!(feedback.values.borrow().len(), 384);
    }

    #[test]
    fn reversing_midway_continues_from_the_visible_value_and_stops() {
        let values = Feedback::default();
        let start = Instant::now();
        assert_eq!(values.sample_at(1, 1.0, 0.0, 160, start), 0.0);
        let halfway = start + Duration::from_millis(80);
        let before = values.sample_at(1, 1.0, 0.0, 160, halfway);
        assert_eq!(values.sample_at(1, 0.0, 0.0, 160, halfway), before);
        assert_eq!(values.sample_at(1, 0.0, 0.0, 160, halfway + Duration::from_millis(160)), 0.0);
        values.set_enabled(false);
        assert_eq!(values.sample(1, 1.0, 0.0, 160), 1.0);
        assert!(!values.active());
    }
}
