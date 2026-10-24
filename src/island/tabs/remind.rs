use crate::island::*;

impl App {
    pub(in crate::island) fn click_remind(&mut self, x: i32, y: i32) {
        let pill = self.pill_rect(true);
        let (fx, fy) = (x as f32, y as f32);
        let (count, scroll) = (self.ui.remind.list.len(), self.ui.remind.scroll);
        if let Some(hit) = view::hit_remind(pill, self.scale, fx, fy, count, scroll) {
            return self.remind_hit(hit, scroll);
        }
        let (frame, _) = view::remind_frame(pill, self.scale);
        if fx < frame.0 || fx >= frame.2 || fy < frame.1 || fy >= frame.3 {
            self.end_edit(true);
            return;
        }
        self.click_in_field(fx, fy);
    }

    fn remind_hit(&mut self, hit: view::RemHit, scroll: usize) {
        use view::RemHit::*;
        match hit {
            Done(i) => self.remove_reminder_at(scroll + i),
            Row(_) | HourBox | MinBox => {}
            Chip(i) => {
                let due = remind::QUICK.get(i).and_then(|(_, q)| remind::quick_due(*q, chrono::Local::now()));
                self.add_reminder_due(due);
            }
            Day(tomorrow) => self.ui.remind.picker.tomorrow = tomorrow,
            HourDec => self.step_picker(-1, 0),
            HourInc => self.step_picker(1, 0),
            MinDec => self.step_picker(0, -1),
            MinInc => self.step_picker(0, 1),
            Set => {
                let pick = self.ui.remind.picker;
                self.add_reminder_due(remind::picker_due(chrono::Local::now(), pick.hour, pick.minute, pick.tomorrow));
            }
        }
        self.repaint_expanded();
    }

    pub(in crate::island) fn step_picker(&mut self, hours: i32, minutes: i32) {
        let pick = &mut self.ui.remind.picker;
        pick.hour = (pick.hour as i32 + hours).rem_euclid(24) as u32;
        pick.minute = (pick.minute as i32 + minutes * 5).rem_euclid(60) as u32;
    }

    pub(in crate::island) fn reset_picker(&mut self) {
        let (hour, minute, tomorrow) = remind::default_picker(chrono::Local::now());
        self.ui.remind.picker = view::Picker { hour, minute, tomorrow };
    }

    pub(in crate::island) fn wheel_remind(&mut self, delta: i32) {
        let step = if delta > 0 { 1 } else { -1 };
        let pill = self.pill_rect(true);
        let (count, scroll) = (self.ui.remind.list.len(), self.ui.remind.scroll);
        let over = self.cursor_client().and_then(|(x, y)| view::hit_remind(pill, self.scale, x, y, count, scroll));
        match over {
            Some(view::RemHit::HourBox | view::RemHit::HourInc | view::RemHit::HourDec) => {
                self.step_picker(step, 0);
            }
            Some(view::RemHit::MinBox | view::RemHit::MinInc | view::RemHit::MinDec) => {
                let pick = &mut self.ui.remind.picker;
                pick.minute = (pick.minute as i32 + step).rem_euclid(60) as u32;
            }
            _ => {
                let max = count.saturating_sub(view::remind_rows(pill, self.scale));
                let next = if delta > 0 { scroll.saturating_sub(1) } else { (scroll + 1).min(max) };
                if next == scroll {
                    return;
                }
                self.ui.remind.scroll = next;
                self.ui.remind.hover = None;
            }
        }
        self.repaint_expanded();
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
            Some(parsed) => self.store_reminder(parsed.due, &parsed.text),
            None => {
                self.ui.remind.status =
                    "No time in the text: pick one above, or start with in 30 min · 18:30".to_string();
                self.ui.caret_on = true;
                self.repaint_expanded();
            }
        }
    }

    fn add_reminder_due(&mut self, due: Option<chrono::DateTime<chrono::Local>>) {
        let Some(due) = due else {
            self.ui.remind.status = "That time does not exist".to_string();
            return;
        };
        let typed = self.ui.remind.input.text();
        let message = match remind::parse(&typed, chrono::Local::now()) {
            Some(parsed) => parsed.text,
            None => typed.trim().chars().take(300).collect(),
        };
        let message = if message.is_empty() { crate::ui::i18n::tr("Reminder").into_owned() } else { message };
        self.store_reminder(due, &message);
    }

    pub(in crate::island) fn store_reminder(&mut self, due: chrono::DateTime<chrono::Local>, message: &str) {
        let ok = self.store.as_ref().is_some_and(|s| s.reminder_add(due.timestamp(), message));
        self.ui.remind.status = if ok {
            self.ui.remind.input.set_text("");
            format!("Set: {}", remind::describe(due, chrono::Local::now()))
        } else {
            "Too many reminders".to_string()
        };
        if ok {
            self.reset_picker();
        }
        self.reload_reminders();
        self.ui.caret_on = true;
        self.repaint_expanded();
    }

    pub(in crate::island) fn remove_reminder_at(&mut self, index: usize) {
        if let (Some(r), Some(store)) = (self.reminders.get(index), &self.store) {
            store.reminder_remove(r.id);
        }
        self.reload_reminders();
        let max = self.ui.remind.list.len().saturating_sub(view::remind_rows(self.pill_rect(true), self.scale));
        self.ui.remind.scroll = self.ui.remind.scroll.min(max);
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
