use crate::island::*;

pub(in crate::island) const CHAR_INSET: f32 = 30.0;

pub(in crate::island) struct Pet {
    pub(in crate::island) char_layer: Layer,
    pub(in crate::island) desktop: Option<crate::ui::companion::CompanionWindow>,
    pub(in crate::island) born: Instant,
    pub(in crate::island) beat: f32,
    pub(in crate::island) beat_at: Instant,
    pub(in crate::island) chat: Option<super::companion_chat::CompanionChat>,
    pub(in crate::island) next_blink: Instant,
    pub(in crate::island) blink_until: Instant,
    pub(in crate::island) rng: u32,
    pub(in crate::island) reaction_until: Option<(Reaction, Instant)>,
    pub(in crate::island) char_key: Option<(Reaction, i8, i8, bool, i32)>,
}

impl App {
    pub(in crate::island) fn drag_character(&mut self) {
        let Some((x, y)) = self.pointer.character_press else { return };
        let mut point = windows::Win32::Foundation::POINT::default();
        if unsafe { GetCursorPos(&mut point) }.is_err()
            || (point.x - x).abs().max((point.y - y).abs()) < (8.0 * self.scale).round() as i32
        {
            return;
        }
        self.pointer.character_press = None;
        self.pointer.press = None;
        self.kill_timer(Timer::Expand);
        self.set_character_detached(true);
        if let Some(desktop) = &mut self.pet.desktop {
            desktop.begin_drag(point);
        } else {
            unsafe {
                let _ = windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture();
            }
        }
        self.char_tick();
    }

    fn companion_over_island(&self) -> bool {
        if self.gaming
            || (self.pill_hidden && !self.ghosted)
            || !matches!(self.state, State::Collapsed | State::Expanded)
        {
            return false;
        }
        let Some(desktop) = &self.pet.desktop else { return false };
        let mut rect = windows::Win32::Foundation::RECT::default();
        unsafe {
            if !IsWindowVisible(self.hwnd).as_bool() || GetWindowRect(self.hwnd, &mut rect).is_err() {
                return false;
            }
        }
        let (width, _) = self.window_size(self.win);
        let pill = if self.state == State::Expanded {
            self.pill_full()
        } else {
            let (w, h) = self.collapsed();
            ((width as f32 - w * self.scale) / 2.0, self.win_pad().1 * self.scale, w * self.scale, h * self.scale)
        };
        let (cx, cy) = desktop.center();
        crate::ui::companion::dock_hit(
            (rect.left as f32 + pill.0, rect.top as f32 + pill.1, pill.2, pill.3),
            (cx, cy),
            self.scale,
        )
    }

    pub(in crate::island) fn attached_character_on(&self) -> bool {
        self.settings.character != Character::None && self.pet.desktop.is_none()
    }

    pub(in crate::island) fn char_size(&self) -> (f32, f32) {
        (characters::BOX_W * self.scale, characters::BOX_H * self.scale)
    }

    fn char_origin(&self) -> (f32, f32) {
        let s = self.scale;
        let (win_w, _) = self.window_size(self.win);
        if self.settings.vertical() {
            ((win_w as f32 - characters::BOX_W * s) / 2.0, self.win_pad().1 * s + 6.0 * s)
        } else {
            (
                (win_w as f32 - self.cw * s) / 2.0 + 6.0 * s,
                self.win_pad().1 * s + (COLLAPSED.1 - characters::BOX_H) * s / 2.0,
            )
        }
    }

    pub(in crate::island) fn place_char(&self) {
        let (x, y) = self.char_origin();
        let _ = self.pet.char_layer.sprite.SetOffset(Vector3 { X: x, Y: y, Z: 0.0 });
    }

    pub(in crate::island) fn apply_character(&mut self) {
        if self.settings.character == Character::None {
            self.settings.character_detached = false;
            self.ui.cfg.character_detached = false;
        }
        let detached = self.settings.character_detached && self.settings.character != Character::None;
        if detached && self.pet.desktop.is_none() {
            match crate::ui::companion::CompanionWindow::new(self.hwnd, self.settings.character_pos, self.scale) {
                Ok(window) => {
                    self.settings.character_pos = Some(window.position());
                    self.pet.desktop = Some(window);
                }
                Err(error) => {
                    self.settings.character_detached = false;
                    self.ui.cfg.character_detached = false;
                    crate::log_warn!("could not release character: {error}");
                }
            }
        } else if !detached && let Some(window) = self.pet.desktop.take() {
            self.settings.character_pos = Some(window.position());
            drop(window);
            self.restore_pill();
            self.reveal();
        }
        let on = self.attached_character_on();
        self.ui.char_inset = if on { CHAR_INSET } else { 0.0 };
        let _ = self.pet.char_layer.sprite.SetIsVisible(on && self.state != State::Expanded);
        if let Some(desktop) = &self.pet.desktop {
            desktop.set_capture_hidden(self.settings.capture_hidden);
        }
        self.update_eq();
        self.pet.char_key = None;
        self.place_char();
        self.place_spectrum();
        self.repaint_collapsed();
        if on || self.pet.desktop.is_some() {
            self.set_timer(Timer::Char, 1);
        } else {
            self.kill_timer(Timer::Char);
        }
    }

    pub(in crate::island) fn start_char(&self) {
        if !self.gaming
            && self.settings.character != Character::None
            && (self.state == State::Collapsed || self.pet.desktop.is_some())
        {
            self.set_timer(Timer::Char, 60);
        }
    }

    pub(in crate::island) fn can_release_character(&self, x: i32, y: i32) -> bool {
        if self.state != State::Collapsed || self.settings.character == Character::None || self.pet.desktop.is_some() {
            return false;
        }
        let (left, top) = self.char_origin();
        let (w, h) = self.char_size();
        let dx = (x as f32 - left - w / 2.0) / (w * 0.32);
        let dy = (y as f32 - top - h / 2.0) / (h * 0.43);
        dx * dx + dy * dy <= 1.0
    }

    pub(in crate::island) fn set_character_detached(&mut self, detached: bool) {
        self.settings.character_detached = detached;
        if detached && self.settings.character == Character::None {
            self.settings.character = Character::Spark;
        } else if !detached {
            self.restore_pill();
            self.reveal();
        }
        self.apply_character();
        self.ui.cfg = self.settings.clone();
        self.settings.save();
    }

    pub(in crate::island) fn on_companion(&mut self, id: usize, source: isize) {
        if self.pet.desktop.as_ref().is_none_or(|window| window.hwnd().0 as isize != source) {
            return;
        }
        match crate::ui::companion::Action::from_id(id) {
            Some(crate::ui::companion::Action::Chat) => {
                self.open_companion_chat();
            }
            Some(crate::ui::companion::Action::ToggleIsland) => self.toggle_island_from_character(),
            Some(action @ (crate::ui::companion::Action::Moved | crate::ui::companion::Action::Dropped)) => {
                if action == crate::ui::companion::Action::Dropped && self.companion_over_island() {
                    self.set_character_detached(false);
                    self.react(Reaction::Done, 650);
                    return;
                }
                if let Some(window) = &self.pet.desktop {
                    window.keep_on_screen();
                    self.settings.character_pos = Some(window.position());
                    self.ui.cfg = self.settings.clone();
                    self.settings.save();
                }
                self.schedule_autohide();
                if action == crate::ui::companion::Action::Dropped && self.state == State::Expanded && !self.ui.pinned {
                    self.set_timer(Timer::Collapse, self.settings.close_delay_ms.max(1));
                }
            }
            Some(crate::ui::companion::Action::Dock) => self.set_character_detached(false),
            Some(crate::ui::companion::Action::Previous) => {
                media::control(media::Control::Previous);
                self.react(Reaction::Music, 600);
            }
            Some(crate::ui::companion::Action::PlayPause) => {
                media::control(media::Control::PlayPause);
                self.react(Reaction::Done, 500);
            }
            Some(crate::ui::companion::Action::Next) => {
                media::control(media::Control::Next);
                self.react(Reaction::Music, 600);
            }
            None => {}
        }
    }

    pub(in crate::island) fn react(&mut self, reaction: Reaction, ms: u64) {
        self.pet.reaction_until = Some((reaction, Instant::now() + Duration::from_millis(ms)));
        self.start_char();
    }

    pub(in crate::island) fn next_random(&mut self) -> u32 {
        let mut x = self.pet.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.pet.rng = x;
        x
    }

    pub(in crate::island) fn idle_ms() -> u32 {
        use windows::Win32::System::SystemInformation::GetTickCount;
        use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
        let mut info = LASTINPUTINFO { cbSize: size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
        unsafe { if GetLastInputInfo(&mut info).as_bool() { GetTickCount().wrapping_sub(info.dwTime) } else { 0 } }
    }

    pub(in crate::island) fn char_tick(&mut self) {
        let character = self.settings.character;
        if self.gaming {
            if let Some(desktop) = &mut self.pet.desktop {
                desktop.hide();
            }
            return;
        }
        let detached = self.pet.desktop.is_some();
        if self.pet.desktop.as_ref().is_some_and(|desktop| desktop.dragging()) {
            self.kill_timer(Timer::Expand);
            self.kill_timer(Timer::Collapse);
            self.kill_timer(Timer::Autohide);
        }
        if character == Character::None || (!detached && self.state != State::Collapsed) {
            return;
        }
        let now = Instant::now();
        if now >= self.pet.next_blink {
            self.pet.blink_until = now + Duration::from_millis(150);
            self.pet.next_blink = now + Duration::from_millis(2500 + u64::from(self.next_random() % 3500));
        }
        let blink = now < self.pet.blink_until;

        let reaction = if self.ui.remind.alert.is_some() {
            Reaction::Attention
        } else if let Some((r, _)) = self.pet.reaction_until.filter(|(_, u)| now < *u) {
            r
        } else if self.ui.media.playing {
            Reaction::Music
        } else if Self::idle_ms() > 90_000 {
            Reaction::Sleep
        } else {
            Reaction::Idle
        };

        let mut pt = windows::Win32::Foundation::POINT::default();
        let mut rc = windows::Win32::Foundation::RECT::default();
        let (cw, ch) = self.char_size();
        let look = unsafe {
            if GetCursorPos(&mut pt).is_ok() && GetWindowRect(self.hwnd, &mut rc).is_ok() {
                let s = self.scale;
                let (win_w, _) = self.window_size(self.win);
                let (cx, cy) = if let Some(desktop) = &self.pet.desktop {
                    desktop.center()
                } else if self.settings.vertical() {
                    (rc.left as f32 + win_w as f32 / 2.0, rc.top as f32 + self.win_pad().1 * s + 6.0 * s + ch / 2.0)
                } else {
                    (
                        rc.left as f32 + (win_w as f32 - self.cw * s) / 2.0 + 6.0 * s + cw / 2.0,
                        rc.top as f32 + self.win_pad().1 * s + COLLAPSED.1 * s / 2.0,
                    )
                };
                (((pt.x as f32 - cx) / 260.0).clamp(-1.0, 1.0), ((pt.y as f32 - cy) / 200.0).clamp(-1.0, 1.0))
            } else {
                (0.0, 0.0)
            }
        };

        let t = now.duration_since(self.pet.born).as_secs_f32();
        let target = if detached || self.music.spectrum_running {
            (0..3).map(audio::level).sum::<f32>() / 3.0
        } else {
            0.5 + 0.5 * (t * 7.5).sin()
        };
        let dt = now.duration_since(self.pet.beat_at).as_secs_f32().min(0.1);
        self.pet.beat_at = now;
        self.pet.beat += (target.clamp(0.0, 1.0) - self.pet.beat) * (1.0 - (-dt * 8.0).exp());
        let beat = self.pet.beat;
        let accent =
            self.ui.media.accent.map_or((0.65, 0.7, 1.0), |(r, g, b)| {
                (f32::from(r) / 255.0, f32::from(g) / 255.0, f32::from(b) / 255.0)
            });
        let pose = characters::Pose { t, reaction, look, blink, accent, beat };

        let docking =
            self.pet.desktop.as_ref().is_some_and(|desktop| desktop.dragging()) && self.companion_over_island();
        if let Some(desktop) = &mut self.pet.desktop {
            if desktop.set_docking(docking) {
                self.pet.char_key = None;
            }
            let available = self.ui.media.playing || !self.ui.media.title.is_empty() || self.ui.media.players > 0;
            if let Err(error) = desktop.update_controls(&self.gpu, available, self.ui.media.playing, self.scale) {
                crate::log_warn!("could not draw companion music controls: {error}");
            }
        }

        let key = (
            reaction,
            (look.0 * 3.0).round() as i8,
            (look.1 * 2.0).round() as i8,
            blink,
            if pose.lively() {
                (t * 30.0) as i32
            } else if detached {
                (t * 12.0) as i32
            } else {
                0
            },
        );
        if self.pet.char_key != Some(key) {
            self.pet.char_key = Some(key);
            if let Some(desktop) = &mut self.pet.desktop {
                if let Err(error) = desktop.render(&self.gpu, character, &pose, self.scale) {
                    crate::log_warn!("could not draw desktop character: {error}");
                }
            } else {
                let _ = self.pet.char_layer.paint(&self.gpu, |p| {
                    let _ = characters::draw(p, character, cw, &pose);
                });
            }
        }
        self.set_timer(Timer::Char, if pose.lively() { self.frame_ms() } else { 80 });
    }
}
