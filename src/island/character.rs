use crate::island::*;

pub(in crate::island) const CHAR_INSET: f32 = 30.0;

pub(in crate::island) struct Pet {
    pub(in crate::island) char_layer: Layer,
    pub(in crate::island) born: Instant,
    pub(in crate::island) next_blink: Instant,
    pub(in crate::island) blink_until: Instant,
    pub(in crate::island) rng: u32,
    pub(in crate::island) reaction_until: Option<(Reaction, Instant)>,
    pub(in crate::island) char_key: Option<(Reaction, i8, i8, bool, i32)>,
}

impl App {
    pub(in crate::island) fn char_size(&self) -> (f32, f32) {
        (characters::BOX_W * self.scale, characters::BOX_H * self.scale)
    }

    pub(in crate::island) fn place_char(&self) {
        let s = self.scale;
        let (win_w, _) = self.window_size(self.win);
        let x = (win_w as f32 - self.cw * s) / 2.0 + 6.0 * s;
        let y = self.win_pad().1 * s + (COLLAPSED.1 - characters::BOX_H) * s / 2.0;
        let _ = self.pet.char_layer.sprite.SetOffset(Vector3 { X: x, Y: y, Z: 0.0 });
    }

    pub(in crate::island) fn apply_character(&mut self) {
        let on = self.settings.character != Character::None;
        self.ui.char_inset = if on { CHAR_INSET } else { 0.0 };
        let _ = self.pet.char_layer.sprite.SetIsVisible(on);
        self.pet.char_key = None;
        self.place_char();
        self.place_spectrum();
        self.repaint_collapsed();
        if on {
            self.set_timer(Timer::Char, 1);
        } else {
            self.kill_timer(Timer::Char);
        }
    }

    pub(in crate::island) fn start_char(&self) {
        if self.settings.character != Character::None && self.state == State::Collapsed {
            self.set_timer(Timer::Char, 60);
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
        if character == Character::None || self.state != State::Collapsed {
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
                let cx = rc.left as f32 + (win_w as f32 - self.cw * s) / 2.0 + 6.0 * s + cw / 2.0;
                let cy = rc.top as f32 + self.win_pad().1 * s + COLLAPSED.1 * s / 2.0 + ch * 0.0;
                (((pt.x as f32 - cx) / 260.0).clamp(-1.0, 1.0), ((pt.y as f32 - cy) / 200.0).clamp(-1.0, 1.0))
            } else {
                (0.0, 0.0)
            }
        };

        let t = now.duration_since(self.pet.born).as_secs_f32();
        let beat = if self.music.spectrum_running {
            (0..3).map(audio::level).sum::<f32>() / 3.0
        } else {
            0.5 + 0.5 * (t * 7.5).sin()
        };
        let accent =
            self.ui.media.accent.map_or((0.65, 0.7, 1.0), |(r, g, b)| {
                (f32::from(r) / 255.0, f32::from(g) / 255.0, f32::from(b) / 255.0)
            });
        let pose = characters::Pose { t, reaction, look, blink, accent, beat };

        let key = (
            reaction,
            (look.0 * 3.0).round() as i8,
            (look.1 * 2.0).round() as i8,
            blink,
            if pose.lively() { (t * 30.0) as i32 } else { 0 },
        );
        if self.pet.char_key != Some(key) {
            self.pet.char_key = Some(key);
            let w = cw;
            let _ = self.pet.char_layer.paint(&self.gpu, |p| {
                let _ = characters::draw(p, character, w, &pose);
            });
        }
        self.set_timer(Timer::Char, if pose.lively() { 33 } else { 80 });
    }
}
