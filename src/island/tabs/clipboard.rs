use crate::island::*;

#[derive(Default)]
pub(in crate::island) struct ClipboardState {
    pub(in crate::island) clips: Vec<Clip>,
    pub(in crate::island) clips_all: Vec<Clip>,
    pub(in crate::island) own_clip_seq: u32,
    pub(in crate::island) paste_tries: u32,
}

impl App {
    pub(in crate::island) fn clip_search(&self) -> bool {
        self.ui.tab == view::Tab::Clipboard && !self.ui.cmd.open && self.ui.editing
    }

    pub(in crate::island) fn plain_paste(&mut self) {
        self.clipboard.paste_tries = 0;
        self.set_timer(Timer::Paste, 30);
    }

    pub(in crate::island) fn on_paste_timer(&mut self) {
        let held =
            |vk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY| unsafe { GetAsyncKeyState(vk.0 as i32) } < 0;
        if (held(VK_MENU) || held(VK_SHIFT) || held(VK_CONTROL)) && self.clipboard.paste_tries < 40 {
            self.clipboard.paste_tries += 1;
            self.set_timer(Timer::Paste, 30);
            return;
        }
        if let Some(text) = clips::sys::read_text(self.hwnd) {
            self.clipboard.own_clip_seq = clips::sys::write_and_paste(self.hwnd, &text);
        }
    }

    pub(in crate::island) fn click_clip(&mut self, x: i32, y: i32) {
        let pill = self.pill_rect(true);
        let (frame, inner) = view::clip_search_frame(pill, self.scale);
        let (fx, fy) = (x as f32, y as f32);
        if fx >= frame.0 && fx < frame.2 && fy >= frame.1 && fy < frame.3 {
            self.begin_edit();
            if let Some(layout) = self.active_layout() {
                let shift = unsafe { GetKeyState(VK_SHIFT.0 as i32) } < 0;
                self.ui.clip.query.click(&layout, fx - inner.0, 4.0, shift);
            }
            return self.repaint_expanded();
        }
        if self.ui.editing {
            self.end_edit(true);
        }
        let visible = self.clipboard.clips.len().saturating_sub(self.ui.clip.scroll);
        let Some(hit) = view::hit_clip(pill, self.scale, x as f32, y as f32, visible) else {
            return;
        };
        let at = |i: usize| self.clipboard.clips.get(self.ui.clip.scroll + i);
        match hit {
            view::ClipHit::Row(i) => {
                if let Some(text) = at(i).map(|c| c.text.clone()) {
                    self.clipboard.own_clip_seq = clips::sys::write_and_paste(self.hwnd, &text);
                }
            }
            view::ClipHit::Pin(i) => {
                if let (Some(id), Some(store)) = (at(i).map(|c| c.id), &self.store) {
                    store.toggle_pin(id);
                }
                self.reload_clips();
                self.repaint_expanded();
            }
            view::ClipHit::Delete(i) => {
                if let (Some(id), Some(store)) = (at(i).map(|c| c.id), &self.store) {
                    store.delete(id);
                }
                self.reload_clips();
                self.repaint_expanded();
            }
        }
    }

    pub(in crate::island) fn reload_clips(&mut self) {
        self.clipboard.clips_all = self.store.as_ref().map(Store::recent).unwrap_or_default();
        self.apply_clip_filter();
    }

    pub(in crate::island) fn apply_clip_filter(&mut self) {
        let q = self.ui.clip.query.text().trim().to_lowercase();
        self.clipboard.clips = if q.is_empty() {
            self.clipboard.clips_all.clone()
        } else {
            self.clipboard.clips_all.iter().filter(|c| c.text.to_lowercase().contains(&q)).cloned().collect()
        };
        let rows = view::clip_rows(self.pill_rect(true), self.scale);
        self.ui.clip.scroll = self.ui.clip.scroll.min(self.clipboard.clips.len().saturating_sub(rows));
    }

    pub(in crate::island) fn on_clipboard(&mut self) {
        let seq = clips::sys::sequence_number();
        if seq == self.clipboard.own_clip_seq {
            return;
        }
        let Some(text) = clips::sys::read_text(self.hwnd) else { return };
        if let Some(store) = &self.store {
            store.add(&text);
        }
        self.reload_clips();
        if self.ui.tab == view::Tab::Clipboard {
            self.repaint_expanded();
        }
    }
}
