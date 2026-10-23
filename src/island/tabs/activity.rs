use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Instant;

use crate::island::*;

const SAMPLE_MS: u32 = 5000;
const AWAY_SECS: u64 = 120;
const MAX_STEP_SECS: i64 = 15;
const FLUSH_SECS: i64 = 60;

#[derive(Default)]
struct Pending {
    secs: HashMap<String, i64>,
    day: i64,
    last_look: Option<Instant>,
    last_flush: Option<Instant>,
}

thread_local! {
    static PENDING: RefCell<Pending> = RefCell::new(Pending::default());
}

impl App {
    pub(in crate::island) fn start_activity(&mut self) {
        if self.settings.activity_track {
            PENDING.with(|p| p.borrow_mut().last_look = Some(Instant::now()));
            self.set_timer(Timer::Activity, SAMPLE_MS);
        } else {
            self.kill_timer(Timer::Activity);
            self.flush_activity();
        }
    }

    pub(in crate::island) fn on_activity_timer(&mut self) {
        if !self.settings.activity_track {
            return;
        }
        let today = activity::today();
        let app = (activity::idle_secs() < AWAY_SECS).then(activity::foreground_app).flatten();
        let due = PENDING.with(|p| {
            let mut p = p.borrow_mut();
            let now = Instant::now();
            let step = p.last_look.map_or(0, |t| now.duration_since(t).as_secs() as i64).min(MAX_STEP_SECS);
            p.last_look = Some(now);
            if p.day != today {
                p.day = today;
            }
            if let Some(app) = app.filter(|a| !a.eq_ignore_ascii_case("night-island")) {
                *p.secs.entry(app).or_insert(0) += step;
            }
            let last = *p.last_flush.get_or_insert(now);
            now.duration_since(last).as_secs() as i64 >= FLUSH_SECS
        });
        if due {
            self.flush_activity();
        }
        self.set_timer(Timer::Activity, SAMPLE_MS);
    }

    pub(in crate::island) fn flush_activity(&mut self) {
        let (day, secs) = PENDING.with(|p| {
            let mut p = p.borrow_mut();
            p.last_flush = Some(Instant::now());
            (p.day, std::mem::take(&mut p.secs))
        });
        if let Some(store) = &self.store {
            for (app, s) in secs {
                store.activity_add(day, &app, s);
            }
        }
    }

    pub(in crate::island) fn reload_activity(&mut self) {
        self.flush_activity();
        let first = activity::today() - if self.ui.activity.week { 6 } else { 0 };
        self.ui.activity.rows = self.store.as_ref().map(|s| s.activity_since(first, 12)).unwrap_or_default();
    }

    pub(in crate::island) fn click_activity(&mut self, x: i32, y: i32) {
        let pill = self.pill_rect(true);
        match view::hit_activity(pill, self.scale, x as f32, y as f32, self.settings.activity_track) {
            Some(view::ActivityHit::TurnOn) => {
                self.settings.activity_track = true;
                self.settings.save();
                self.ui.cfg = self.settings.clone();
                self.start_activity();
            }
            Some(view::ActivityHit::Today) => {
                self.ui.activity.week = false;
                self.reload_activity();
            }
            Some(view::ActivityHit::Week) => {
                self.ui.activity.week = true;
                self.reload_activity();
            }
            Some(view::ActivityHit::Clear) => {
                if self.ui.activity.clear_armed {
                    self.ui.activity.clear_armed = false;
                    self.kill_timer(Timer::Disarm);
                    PENDING.with(|p| p.borrow_mut().secs.clear());
                    if let Some(store) = &self.store {
                        store.activity_clear();
                    }
                    self.reload_activity();
                } else {
                    self.ui.activity.clear_armed = true;
                    self.set_timer(Timer::Disarm, 3000);
                }
            }
            None => {
                if self.ui.activity.clear_armed {
                    self.ui.activity.clear_armed = false;
                }
            }
        }
        self.repaint_expanded();
    }
}
