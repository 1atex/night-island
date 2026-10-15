use crate::island::*;

impl App {
    pub(in crate::island) fn click_remind(&mut self, x: i32, y: i32) {
        let pill = self.pill_rect(true);
        let (fx, fy) = (x as f32, y as f32);
        if let Some(hit) = view::hit_remind(pill, self.scale, fx, fy, self.ui.remind.list.len()) {
            if let view::RemHit::Done(i) = hit {
                self.remove_reminder_at(i);
            }
            return;
        }
        let (frame, _) = view::remind_frame(pill, self.scale);
        if fx < frame.0 || fx >= frame.2 || fy < frame.1 || fy >= frame.3 {
            self.end_edit(true);
            return;
        }
        self.click_in_field(fx, fy);
    }

    pub(in crate::island) fn reload_reminders(&mut self) {
        self.reminders = self.store.as_ref().map(Store::reminders).unwrap_or_default();
        self.refresh_reminder_labels();
        self.schedule_reminder();
    }

    pub(in crate::island) fn refresh_reminder_labels(&mut self) {
        use chrono::{Local, TimeZone};
        let now = Local::now();
        self.ui.remind.list = self
            .reminders
            .iter()
            .map(|r| {
                let due = Local.timestamp_opt(r.due, 0).single().unwrap_or(now);
                (r.id, r.text.clone(), remind::describe(due, now))
            })
            .collect();
    }

    pub(in crate::island) fn schedule_reminder(&self) {
        let now = chrono::Local::now().timestamp();
        match self.reminders.iter().map(|r| r.due).min() {
            Some(due) => {
                const DAY_MS: i64 = 24 * 3600 * 1000;
                let ms = ((due - now).max(0) * 1000).clamp(50, DAY_MS);
                self.set_timer(Timer::Remind, ms as u32);
            }
            None => self.kill_timer(Timer::Remind),
        }
    }

    pub(in crate::island) fn add_reminder(&mut self) {
        let text = self.ui.remind.input.text();
        match remind::parse(&text, chrono::Local::now()) {
            Some(parsed) => {
                let ok = self.store.as_ref().is_some_and(|s| s.reminder_add(parsed.due.timestamp(), &parsed.text));
                self.ui.remind.status = if ok {
                    self.ui.remind.input.set_text("");
                    format!("Set: {}", remind::describe(parsed.due, chrono::Local::now()))
                } else {
                    "Too many reminders".to_string()
                };
                self.reload_reminders();
            }
            None => self.ui.remind.status = "Start with a time: in 30 min · 18:30 · tomorrow 9:00".to_string(),
        }
        self.ui.caret_on = true;
        self.repaint_expanded();
    }

    pub(in crate::island) fn remove_reminder_at(&mut self, index: usize) {
        if let (Some(r), Some(store)) = (self.reminders.get(index), &self.store) {
            store.reminder_remove(r.id);
        }
        self.reload_reminders();
        self.repaint_expanded();
    }

    pub(in crate::island) fn on_remind_timer(&mut self) {
        if self.ui.remind.alert.is_some() {
            return;
        }
        let now = chrono::Local::now().timestamp();
        let due = self.reminders.iter().filter(|r| r.due <= now + 1).min_by_key(|r| r.due);
        let Some(r) = due else { return self.schedule_reminder() };
        self.ui.remind.alert = Some((r.id, r.text.clone()));
        self.ui.cmd.open = false;
        unsafe {
            let _ = windows::Win32::System::Diagnostics::Debug::MessageBeep(MB_ICONASTERISK);
        }
        self.kill_timer(Timer::Collapse);
        self.expand();
        self.repaint_expanded();
    }

    pub(in crate::island) fn click_alert(&mut self, x: i32, y: i32) {
        let pill = self.pill_rect(true);
        let Some(choice) = view::hit_alert(pill, self.scale, x as f32, y as f32) else { return };
        let Some((id, _)) = self.ui.remind.alert.take() else { return };
        if let Some(store) = &self.store {
            match choice {
                0 => store.reminder_remove(id),
                1 => store.reminder_set_due(id, chrono::Local::now().timestamp() + 600),
                _ => store.reminder_set_due(id, chrono::Local::now().timestamp() + 3600),
            }
        }
        self.reload_reminders();
        self.repaint_expanded();
        self.set_timer(Timer::Collapse, self.settings.close_delay_ms.max(300));
    }
}
