use crate::island::*;

impl App {
    pub(in crate::island) fn sync_live_activities(&mut self) {
        let mut items = Vec::new();
        let now = chrono::Local::now().timestamp();
        if self.settings.live_activities {
            if self.settings.live_reminders {
                let mut reminders: Vec<_> = self
                    .reminders
                    .iter()
                    .filter(|r| {
                        r.due > now
                            && (r.due - now <= 86400
                                || r.text.to_lowercase().starts_with("таймер")
                                || r.text.to_lowercase().starts_with("timer"))
                    })
                    .collect();
                reminders.sort_by_key(|r| r.due);
                for reminder in reminders.into_iter().take(2) {
                    items.push(view::LiveActivity {
                        kind: view::LiveKind::Reminder(reminder.id),
                        title: reminder.text.clone(),
                        seconds: Some((reminder.due - now) as u64),
                    });
                }
            }
            if self.settings.live_recording
                && let Some(seconds) = self.ui.recording_secs
            {
                items.push(view::LiveActivity {
                    kind: view::LiveKind::Recording,
                    title: crate::ui::i18n::tr("Recording").into_owned(),
                    seconds: Some(u64::from(seconds)),
                });
            }
            if self.settings.live_downloads
                && let Some(name) = &self.ui.downloading
            {
                items.push(view::LiveActivity { kind: view::LiveKind::Download, title: name.clone(), seconds: None });
            }
        }
        items.sort_by_key(|item| match item.kind {
            view::LiveKind::Recording => 0,
            view::LiveKind::Reminder(_) => 1,
            view::LiveKind::Download => 2,
        });
        let changed = !items.iter().map(|v| v.kind).eq(self.ui.live.iter().map(|v| v.kind));
        self.ui.live = items;
        if changed {
            self.ui.live_changed =
                (self.settings.island_animations && crate::ui::feedback::motion_allowed()).then(Instant::now);
            if self.ui.live_changed.is_some() && self.state == State::Expanded {
                self.set_timer(Timer::UiAnim, self.frame_ms());
            }
        }
        let visible = (!self.pill_hidden || self.ghosted)
            && !self.foreground_hidden()
            && (self.state == State::Collapsed || (self.state == State::Expanded && self.ui.tab == view::Tab::Home));
        if visible && self.ui.live.iter().any(|v| matches!(v.kind, view::LiveKind::Reminder(_))) {
            self.set_timer(Timer::LiveActivity, 1000 - (chrono::Local::now().timestamp_subsec_millis() % 1000));
        } else {
            self.kill_timer(Timer::LiveActivity);
        }
    }

    pub(in crate::island) fn live_activity_tick(&mut self) {
        self.repaint_collapsed();
        if self.state == State::Expanded && self.ui.tab == view::Tab::Home {
            self.repaint_expanded();
        }
    }

    pub(in crate::island) fn open_live_activity(&mut self, index: usize) {
        let Some(activity) = self.ui.live.get(index).cloned() else { return };
        match activity.kind {
            view::LiveKind::Reminder(id) => {
                let note = self.store.as_ref().and_then(|store| store.note_for_reminder(id));
                self.reload_notes();
                self.open_tab(view::Tab::Note);
                if let Some(note) = note {
                    self.open_note(note);
                }
            }
            view::LiveKind::Recording => self.open_tab(view::Tab::Record),
            view::LiveKind::Download => self.open_tab(view::Tab::Home),
        }
    }
}
