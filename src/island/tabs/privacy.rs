use crate::island::*;

const LOG_ROWS: i64 = 300;

impl App {
    pub(in crate::island) fn reload_uses(&mut self) {
        self.ui.privacy_log.rows = self.store.as_ref().map(|s| s.recent_uses(LOG_ROWS)).unwrap_or_default();
        let shown = view::privacy_rows(self.pill_rect(true), self.scale);
        self.ui.privacy_log.scroll =
            self.ui.privacy_log.scroll.min(self.ui.privacy_log.rows.len().saturating_sub(shown));
    }

    pub(in crate::island) fn log_privacy_events(&mut self) {
        let events = privacy::take_events();
        if events.is_empty() {
            return;
        }
        if let Some(store) = &self.store {
            for e in &events {
                if e.started {
                    store.use_started(e.kind, &e.app, e.at);
                } else {
                    store.use_ended(e.kind, &e.app, e.at);
                }
            }
        }
        for e in events.iter().filter(|e| e.started && e.kind == UseKind::Screen) {
            let text = format!("{} {}", e.app, crate::ui::i18n::tr("is capturing the screen"));
            self.notify_for(Notice::Privacy, '\u{E7F4}', &text, 4500);
        }
        if self.ui.tab == view::Tab::Privacy {
            self.reload_uses();
            if self.state == State::Expanded {
                self.repaint_expanded();
            }
        }
    }

    pub(in crate::island) fn click_privacy(&mut self, x: i32, y: i32) {
        let button = view::usage_clear_button(self.pill_rect(true), self.scale);
        let (fx, fy) = (x as f32, y as f32);
        if fx >= button.0 && fx < button.2 && fy >= button.1 && fy < button.3 {
            if self.ui.privacy_log.clear_armed {
                self.ui.privacy_log.clear_armed = false;
                self.kill_timer(Timer::Disarm);
                if let Some(store) = &self.store {
                    store.clear_uses();
                }
                self.ui.privacy_log.scroll = 0;
                self.reload_uses();
            } else {
                self.ui.privacy_log.clear_armed = true;
                self.set_timer(Timer::Disarm, 3000);
            }
            self.repaint_expanded();
        } else if self.ui.privacy_log.clear_armed {
            self.ui.privacy_log.clear_armed = false;
            self.repaint_expanded();
        }
    }
}
