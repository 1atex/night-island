use crate::island::*;

pub(in crate::island) const NOTE_SAVE_MS: u32 = 800;

impl App {
    pub(in crate::island) fn remind_active(&self) -> bool {
        self.ui.tab == view::Tab::Note && self.ui.remind.focus
    }

    pub(in crate::island) fn note_inner(&self) -> view::Rect {
        self.field_frame().1
    }

    pub(in crate::island) fn note_layout(&self) -> Option<windows::Win32::Graphics::DirectWrite::IDWriteTextLayout> {
        let inner = self.note_inner();
        crate::ui::gfx::make_layout(
            self.gpu.dwrite(),
            &self.multi().text(),
            if self.remind_active() { 100_000.0 } else { inner.2 - inner.0 },
            if self.remind_active() {
                13.5
            } else if self.ui.tab == view::Tab::Agy {
                view::AGY_FONT
            } else {
                view::NOTE_FONT
            } * self.scale,
        )
        .ok()
    }

    pub(in crate::island) fn save_note(&self) {
        self.kill_timer(Timer::NoteSave);
        if let Some(store) = &self.store {
            store.note_save(&self.ui.note.text());
        }
    }

    pub(in crate::island) fn note_changed(&mut self) {
        if self.extra_field().is_some() {
            self.set_timer(Timer::Geo, 450);
        }
        if self.int_field().is_some() || self.extra_field().is_some() {
            self.ui.caret_on = true;
            self.set_timer(Timer::Caret, CARET_BLINK_MS);
            return self.repaint_expanded();
        }
        if self.clip_search() {
            self.ui.caret_on = true;
            self.set_timer(Timer::Caret, CARET_BLINK_MS);
            self.ui.clip.scroll = 0;
            self.apply_clip_filter();
            return self.repaint_expanded();
        }
        if self.ui.cmd.open {
            self.ui.caret_on = true;
            self.set_timer(Timer::Caret, CARET_BLINK_MS);
            self.refresh_cmd();
            return self.repaint_expanded();
        }
        if self.ui.tab == view::Tab::Note && !self.ui.remind.focus {
            self.set_timer(Timer::NoteSave, NOTE_SAVE_MS);
        }
        self.ui.caret_on = true;
        if self.ui.editing {
            self.set_timer(Timer::Caret, CARET_BLINK_MS);
        }
        self.ensure_caret_visible();
        self.repaint_expanded();
    }

    pub(in crate::island) fn click_note(&mut self, x: i32, y: i32) {
        if self.ui.tab == view::Tab::Note {
            let pill = self.pill_rect(true);
            let (fx, fy) = (x as f32, y as f32);
            if let Some(hit) = view::hit_remind(pill, self.scale, fx, fy, self.ui.remind.list.len()) {
                if let view::RemHit::Done(i) = hit {
                    self.remove_reminder_at(i);
                }
                return;
            }
            let inside = |r: view::Rect| fx >= r.0 && fx < r.2 && fy >= r.1 && fy < r.3;
            if inside(view::remind_frame(pill, self.scale).0) {
                self.ui.remind.focus = true;
            } else if inside(view::note_frame(pill, self.scale).0) {
                self.ui.remind.focus = false;
            }
        }
        let (frame, inner) = self.field_frame();
        let (fx, fy) = (x as f32, y as f32);
        if fx < frame.0 || fx >= frame.2 || fy < frame.1 || fy >= frame.3 {
            self.end_edit(true);
            return;
        }
        self.begin_edit();
        let shift = unsafe { GetKeyState(VK_SHIFT.0 as i32) } < 0;
        if let Some(layout) = self.note_layout() {
            let scroll = self.multi().scroll;
            self.multi_mut().click(&layout, fx - inner.0, fy - inner.1 + scroll, shift);
        }
        self.pointer.selecting = true;
        self.note_changed();
    }

    pub(in crate::island) fn cursor_over_note(&self) -> bool {
        if self.state != State::Expanded || self.ui.tab != view::Tab::Note {
            return false;
        }
        let mut pt = windows::Win32::Foundation::POINT::default();
        unsafe {
            if GetCursorPos(&mut pt).is_err()
                || !windows::Win32::Graphics::Gdi::ScreenToClient(self.hwnd, &mut pt).as_bool()
            {
                return false;
            }
        }
        let frame = view::note_frame(self.pill_rect(true), self.scale).0;
        let (x, y) = (pt.x as f32, pt.y as f32);
        x >= frame.0 && x < frame.2 && y >= frame.1 && y < frame.3
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
