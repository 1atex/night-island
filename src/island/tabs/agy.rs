use crate::island::*;

impl App {
    pub(in crate::island) fn reload_agy(&mut self) {
        self.ui.agy.chats = agy::chats(40);
        self.ui.agy.jobs = agy::jobs();
        self.ui.agy.quota = agy::quota_reset_at();
        if self.ui.agy.open {
            self.load_history(true);
        }
        self.start_agy_timer();
        let known = self.ui.agy.sel.as_deref().is_some_and(|id| self.ui.agy.chats.iter().any(|c| c.id == id));
        if !known {
            self.ui.agy.sel = self.ui.agy.chats.first().map(|c| c.id.clone());
        }
        let rows = view::agy_rows();
        self.ui.agy.scroll = self.ui.agy.scroll.min(self.ui.agy.chats.len().saturating_sub(rows));
    }

    pub(in crate::island) fn send_agy(&mut self) {
        let Some(chat) =
            self.ui.agy.sel.as_deref().and_then(|id| self.ui.agy.chats.iter().find(|c| c.id == id)).cloned()
        else {
            return;
        };
        let text = self.ui.agy.input.text();
        if agy::send(self.hwnd, &chat, &text, self.settings.agy_auto) {
            self.ui.agy.input.set_text("");
            self.notify(Notice::Agy, '\u{E943}', "AGY: sent");
            self.ui.agy.jobs = agy::jobs();
            self.load_history(false);
            self.start_agy_timer();
        }
        self.repaint_expanded();
    }

    pub(in crate::island) fn load_history(&mut self, keep_scroll: bool) {
        let Some(id) = self.ui.agy.sel.clone() else { return };
        self.ui.agy.msgs = agy::history(&id);
        if !keep_scroll {
            self.ui.agy.hist = 0.0;
        }
    }

    pub(in crate::island) fn start_agy_timer(&self) {
        let running = self.ui.agy.sel.as_deref().is_some_and(agy::running);
        if self.state == State::Expanded && self.ui.tab == view::Tab::Agy && self.ui.agy.open && running {
            self.set_timer(Timer::Agy, 2000);
        }
    }

    pub(in crate::island) fn click_agy(&mut self, x: i32, y: i32) {
        let pill = self.pill_rect(true);
        let hit = view::hit_agy(
            pill,
            self.scale,
            x as f32,
            y as f32,
            self.ui.agy.chats.len(),
            self.ui.agy.scroll,
            self.ui.agy.open,
            self.ui.agy.input_h.get(),
        );
        match hit {
            Some(view::AgyHit::Row(i)) => {
                if let Some(chat) = self.ui.agy.chats.get(self.ui.agy.scroll + i) {
                    self.ui.agy.sel = Some(chat.id.clone());
                    self.ui.agy.open = true;
                    self.load_history(false);
                    self.start_agy_timer();
                }
            }
            Some(view::AgyHit::Back) => {
                self.ui.agy.open = false;
                if self.ui.editing {
                    self.end_edit(false);
                }
            }
            Some(view::AgyHit::Input) => {
                self.begin_edit();
                if let Some(layout) = self.active_layout() {
                    let (_, inner) = view::agy_input(pill, self.scale, self.ui.agy.input_h.get());
                    let shift = unsafe { GetKeyState(VK_SHIFT.0 as i32) } < 0;
                    let scroll = self.ui.agy.input.scroll;
                    self.ui.agy.input.click(&layout, x as f32 - inner.0, y as f32 - inner.1 + scroll, shift);
                }
            }
            Some(view::AgyHit::Send) => {
                let selected = self.ui.agy.sel.clone();
                match selected {
                    Some(id) if agy::running(&id) => agy::stop(&id),
                    Some(_) => self.send_agy(),
                    None => {}
                }
            }
            Some(view::AgyHit::History) | None => {
                if self.ui.editing {
                    self.end_edit(false);
                }
            }
        }
        self.repaint_expanded();
    }

    pub(in crate::island) fn on_agy(&mut self) {
        let jobs = agy::jobs();
        let mut told = false;
        for (id, job) in &jobs {
            let was = self.ui.agy.jobs.get(id);
            if let agy::Job::Done { ok, reply, .. } = job
                && was != Some(job)
                && matches!(was, Some(agy::Job::Running))
            {
                let title: String = agy::job_title(id).chars().take(26).collect();
                let text = if *ok {
                    let first: String =
                        reply.lines().find(|l| !l.trim().is_empty()).unwrap_or("").chars().take(48).collect();
                    if first.is_empty() { format!("AGY · {title}") } else { format!("AGY · {title}: {first}") }
                } else {
                    format!("AGY · {title}: error")
                };
                self.notify_for(Notice::Agy, if *ok { '\u{E73E}' } else { '\u{E783}' }, &text, 7000);
                told = true;
            }
        }
        let _ = told;
        self.ui.agy.jobs = jobs;
        if self.ui.tab == view::Tab::Agy {
            let keep = self.ui.agy.sel.clone();
            self.ui.agy.chats = agy::chats(40);
            self.ui.agy.sel = keep;
            self.ui.agy.quota = agy::quota_reset_at();
            if self.ui.agy.open {
                self.load_history(false);
            }
            self.start_agy_timer();
            self.repaint_expanded();
        }
    }
}
