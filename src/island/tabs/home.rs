use crate::island::*;

impl App {
    pub(in crate::island) fn open_dev_menu(&mut self) {
        let items = volume::devices();
        self.ui.home.dev_menu = (!items.is_empty()).then_some((items, 0));
        self.repaint_expanded();
    }

    pub(in crate::island) fn click_dev_menu(&mut self, x: i32, y: i32) {
        let pill = self.pill_rect(true);
        let Some((items, scroll)) = self.ui.home.dev_menu.take() else { return };
        if let Some(Some(row)) = view::hit_menu(pill, self.scale, x as f32, y as f32, items.len()) {
            if let Some(d) = items.get(scroll + row) {
                volume::set_default(&d.id);
                self.refresh_home();
                self.ui.mic_muted = volume::mic_muted();
            }
        } else if view::hit_menu(pill, self.scale, x as f32, y as f32, items.len()).is_some() {
            self.ui.home.dev_menu = Some((items, scroll));
            return;
        }
        self.repaint_expanded();
    }

    pub(in crate::island) fn click_home(&mut self, x: i32, y: i32) {
        if self.ui.home.dev_menu.is_some() {
            return self.click_dev_menu(x, y);
        }
        let pill = self.pill_rect(true);
        let has = !self.ui.media.title.is_empty();
        match view::hit_home(
            pill,
            self.scale,
            x as f32,
            y as f32,
            has,
            view::spotify_active(&self.ui),
            view::discord_strip(&self.ui),
            self.ui.home.dock.len(),
        ) {
            Some(view::HomeHit::Media(i)) => media::control(match i {
                0 => media::Control::Previous,
                1 => media::Control::PlayPause,
                _ => media::Control::Next,
            }),
            Some(view::HomeHit::Action(i)) => self.home_action(i),
            Some(view::HomeHit::Dock(i)) => self.open_dock_item(i),
            Some(view::HomeHit::DockAdd) => self.want_dock_dialog = true,
            Some(view::HomeHit::DiscordOverlay) => self.toggle_overlay(),
            Some(view::HomeHit::DiscordMute) => discord::set_mute(!self.ui.discord.self_mute),
            Some(view::HomeHit::DiscordDeaf) => discord::set_deaf(!self.ui.discord.self_deaf),
            Some(view::HomeHit::Seek(f)) => {
                self.pointer.seek_drag = true;
                self.ui.home.seek = Some(f);
                self.repaint_expanded();
            }
            Some(view::HomeHit::VolumeIcon) => {
                if self.ui.home.vol_track {
                    self.ui.home.vol_track = false;
                } else if let Some(v) =
                    volume::app_volume(&self.ui.media.source).filter(|_| !self.ui.media.source.is_empty())
                {
                    self.ui.home.app_vol = v;
                    self.ui.home.vol_track = true;
                }
                self.repaint_expanded();
            }
            Some(view::HomeHit::Volume) => {
                self.pointer.vol_drag = true;
                self.drag_volume(x);
            }
            Some(view::HomeHit::Device) => self.open_dev_menu(),
            Some(view::HomeHit::Extra(i)) => match i {
                0 => spotify::toggle_shuffle(),
                1 => spotify::cycle_repeat(),
                _ => spotify::toggle_like(),
            },
            None => {}
        }
    }

    pub(in crate::island) fn drag_volume(&mut self, x: i32) {
        if !self.pointer.vol_drag {
            return;
        }
        let (tl, tr, _) = view::volume_track(self.pill_rect(true), self.scale);
        let level = ((x as f32 - tl) / (tr - tl).max(1.0)).clamp(0.0, 1.0);
        let current = if self.ui.home.vol_track { self.ui.home.app_vol } else { self.ui.home.vol };
        if (level - current).abs() > 0.004 {
            if self.ui.home.vol_track {
                self.ui.home.app_vol = level;
                volume::set_app_volume(&self.ui.media.source, level);
            } else {
                self.ui.home.vol = level;
                volume::set(level);
            }
            self.repaint_expanded();
        }
    }

    pub(in crate::island) fn drag_seek(&mut self, x: i32) {
        if !self.pointer.seek_drag {
            return;
        }
        let (l, r, _) = view::seek_track(self.pill_rect(true), self.scale);
        self.ui.home.seek = Some(((x as f32 - l) / (r - l).max(1.0)).clamp(0.0, 1.0));
        self.repaint_expanded();
    }

    pub(in crate::island) fn finish_seek(&mut self) {
        if !self.pointer.seek_drag {
            return;
        }
        self.pointer.seek_drag = false;
        if let (Some(f), Some((_, len))) = (self.ui.home.seek.take(), media::progress()) {
            media::seek(f * len);
        }
        self.repaint_expanded();
    }

    pub(in crate::island) fn home_action(&mut self, i: usize) {
        match i {
            0 | 1 | 2 | 3 | 4 | 6 => {
                if i == 6 && self.ui.recording_secs.is_some() {
                    self.collapse();
                    return self.toggle_recording();
                }
                self.collapse();
                match i {
                    0 => self.capture(capture::Mode::Area),
                    1 => capture::fullscreen(self.hwnd),
                    2 => self.capture(capture::Mode::Ocr),
                    3 => self.capture(capture::Mode::Qr),
                    4 => self.capture(capture::Mode::Picker),
                    _ => self.toggle_recording(),
                }
            }
            5 => {
                self.collapse();
                self.plain_paste();
            }
            _ => self.toggle_command(),
        }
    }

    pub(in crate::island) fn refresh_home(&mut self) {
        if let Some(v) = volume::get() {
            self.ui.home.vol = v;
        }
        self.ui.home.device = volume::device_name().unwrap_or_default();
        if self.ui.home.vol_track {
            match volume::app_volume(&self.ui.media.source) {
                Some(v) => self.ui.home.app_vol = v,
                None => self.ui.home.vol_track = false,
            }
        }
        self.start_progress_timer();
    }

    pub(in crate::island) fn start_progress_timer(&self) {
        if self.state == State::Expanded && self.ui.tab == view::Tab::Home && self.ui.media.playing {
            self.set_timer(Timer::Progress, 1000);
        }
    }

    pub(in crate::island) fn reload_dock(&mut self) {
        self.ui.home.dock = self.store.as_ref().map(Store::dock).unwrap_or_default();
    }

    pub(in crate::island) fn open_dock_item(&mut self, i: usize) {
        let Some(path) = view::dock_path(&self.ui, i).map(str::to_string) else { return };
        if std::path::Path::new(&path).exists() {
            cmd::shell_open(&path, None);
            self.collapse();
        } else {
            if let (Some(item), Some(store)) = (self.ui.home.dock.get(i), &self.store) {
                store.dock_remove(item.id);
            }
            self.reload_dock();
            self.show_notice('\u{E783}', "File not found");
            self.repaint_expanded();
        }
    }

    pub(in crate::island) fn finish_dock_dialog(&mut self, picked: Option<String>) {
        self.popup_open = false;
        self.pointer.tracking = false;
        if let (Some(path), Some(store)) = (picked, &self.store) {
            store.dock_add(&path);
            self.reload_dock();
        }
        self.repaint_expanded();
        if !self.cursor_inside() {
            self.set_timer(Timer::Collapse, self.settings.close_delay_ms.max(1));
        }
    }

    pub(in crate::island) fn dock_target_at(&self, x: i32, y: i32) -> Option<(usize, String)> {
        if self.state != State::Expanded || self.ui.tab != view::Tab::Home || self.ui.home.dev_menu.is_some() {
            return None;
        }
        let pill = self.pill_rect(true);
        match view::hit_home(
            pill,
            self.scale,
            x as f32,
            y as f32,
            false,
            false,
            view::discord_strip(&self.ui),
            self.ui.home.dock.len(),
        ) {
            Some(view::HomeHit::Dock(i)) => view::dock_path(&self.ui, i).map(|p| (i, p.to_string())),
            _ => None,
        }
    }

    pub(in crate::island) fn apply_dock_choice(&mut self, choice: Option<usize>, index: usize, path: &str) {
        self.pointer.tracking = false;
        match choice {
            Some(1) => cmd::reveal(std::path::Path::new(path)),
            Some(2) => cmd::shell_open(path, Some("runas")),
            Some(3) => {
                if let (Some(item), Some(store)) = (self.ui.home.dock.get(index), &self.store) {
                    store.dock_remove(item.id);
                }
                self.reload_dock();
            }
            _ => {}
        }
        self.repaint_expanded();
        if !self.cursor_inside() {
            self.set_timer(Timer::Collapse, self.settings.close_delay_ms.max(1));
        }
    }
}
