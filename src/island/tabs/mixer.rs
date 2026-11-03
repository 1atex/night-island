use crate::island::*;

impl App {
    pub(in crate::island) fn sync_mixer(&self) {
        crate::platform::audio_mixer::visible(
            self.hwnd,
            self.state == State::Expanded && self.ui.tab == view::Tab::Mixer && !self.gaming,
        );
    }

    pub(in crate::island) fn on_mixer(&mut self) {
        let mut next = crate::platform::audio_mixer::snapshot();
        if let Some(key) = &self.ui.mixer.dragging {
            if let (Some(old), Some(new)) = (
                self.ui.mixer.state.apps.iter().find(|app| &app.key == key),
                next.apps.iter_mut().find(|app| &app.key == key),
            ) {
                new.level = old.level;
                new.muted = false;
            } else {
                self.ui.mixer.dragging = None;
            }
            let order: Vec<_> = self.ui.mixer.state.apps.iter().map(|app| app.key.clone()).collect();
            next.apps.sort_by_key(|app| order.iter().position(|key| key == &app.key).unwrap_or(usize::MAX));
        }
        self.ui.mixer.state = next;
        let rows = view::mixer_visible_rows(self.pill_rect(true), self.scale);
        self.ui.mixer.scroll = self.ui.mixer.scroll.min(self.ui.mixer.state.apps.len().saturating_sub(rows));
        if self.state == State::Expanded && self.ui.tab == view::Tab::Mixer {
            self.repaint_expanded();
        }
    }

    pub(in crate::island) fn click_mixer(&mut self, x: i32, y: i32) {
        match view::hit_mixer(self.pill_rect(true), self.scale, &self.ui.mixer, x as f32, y as f32) {
            Some(view::MixerHit::Volume(index)) => {
                if let Some(app) = self.ui.mixer.state.apps.get(index) {
                    self.ui.mixer.dragging = Some(app.key.clone());
                    unsafe {
                        windows::Win32::UI::Input::KeyboardAndMouse::SetCapture(self.hwnd);
                    }
                    self.drag_mixer(x);
                }
            }
            Some(view::MixerHit::Mute(index)) => {
                if let Some(app) = self.ui.mixer.state.apps.get_mut(index) {
                    app.muted = !app.muted && app.level > 0.001;
                    crate::platform::audio_mixer::set_muted(&app.key, app.muted);
                    self.repaint_expanded();
                }
            }
            Some(view::MixerHit::Refresh) => self.sync_mixer(),
            None => {}
        }
    }

    pub(in crate::island) fn drag_mixer(&mut self, x: i32) {
        let Some(key) = &self.ui.mixer.dragging else { return };
        let level = view::mixer_level_at(self.pill_rect(true), self.scale, x as f32);
        if let Some(app) = self.ui.mixer.state.apps.iter_mut().find(|app| &app.key == key) {
            app.level = level;
            app.muted = false;
            crate::platform::audio_mixer::set_level(key, level);
            self.repaint_expanded();
        }
    }

    pub(in crate::island) fn finish_mixer(&mut self) {
        if self.ui.mixer.dragging.take().is_some() {
            unsafe {
                let _ = windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture();
            }
            self.sync_mixer();
        }
    }
}
