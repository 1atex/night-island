use crate::island::*;

impl App {
    pub(in crate::island) fn reload_reminders(&mut self) {
        self.reminders = self.store.as_ref().map(Store::reminders).unwrap_or_default();
        self.refresh_reminder_labels();
        self.schedule_reminder();
        self.refresh_note_remind();
        self.repaint_collapsed();
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

    pub(in crate::island) fn store_reminder(&mut self, due: chrono::DateTime<chrono::Local>, message: &str) {
        if let Some(store) = &self.store {
            store.reminder_add(due.timestamp(), message);
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
        if self.foreground_hidden() && !self.settings.notification_priority {
            self.kill_timer(Timer::Remind);
            return;
        }
        self.ui.remind.alert = Some((r.id, r.text.clone()));
        self.ui.cmd.open = false;
        crate::platform::sounds::play(crate::platform::sounds::Cue::Reminder, 100);
        if self.foreground_hidden() {
            self.hidden_timer_card(self.ui.remind.alert.as_ref().unwrap().1.clone());
            return;
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
