use crate::island::*;

pub(in crate::island) const BUMP_ROOM: f32 = 52.0;
pub(in crate::island) const BUMP_DEPTH_MAX: f32 = 40.0;
pub(in crate::island) const BUMP_HALF: f32 = 44.0;
pub(in crate::island) const IDLE_AMP: f32 = 5.0;
pub(in crate::island) const BUMP_SEAM: f32 = 3.0;
pub(in crate::island) const SPECTRUM_MS: u32 = 33;
pub(in crate::island) const SPEC_W: f32 = 3.0;
pub(in crate::island) const SPEC_GAP: f32 = 3.0;
pub(in crate::island) const SPEC_H: f32 = 24.0;

pub(in crate::island) fn bump_room_for(settings: &Settings) -> f32 {
    let docked_elsewhere = settings.placement == Placement::Docked && settings.dock_edge != Edge::Top;
    if settings.style == Style::Liquid && !docked_elsewhere { BUMP_ROOM } else { 0.0 }
}

pub(in crate::island) fn bump_size(scale: f32, width: f32) -> (i32, i32) {
    let straight = (width - COLLAPSED.1) * scale;
    (straight.round() as i32, ((BUMP_DEPTH_MAX + BUMP_SEAM + 4.0) * scale).round() as i32)
}

pub(in crate::island) struct Liquid {
    pub(in crate::island) bump: Layer,
    pub(in crate::island) bump_fill: SpriteVisual,
    pub(in crate::island) bump_blur: SpriteVisual,
    pub(in crate::island) bump_shape: CompositionPathGeometry,
    pub(in crate::island) bump_amp: f32,
    pub(in crate::island) bump_cx: f32,
    pub(in crate::island) bump_vel: f32,
    pub(in crate::island) bump_cx_vel: f32,
    pub(in crate::island) last_tick: Instant,
    pub(in crate::island) idle_running: bool,
}

pub(in crate::island) struct MusicBars {
    pub(in crate::island) spectrum: Vec<SpriteVisual>,
    pub(in crate::island) spectrum_brushes: Vec<CompositionColorBrush>,
    pub(in crate::island) spectrum_running: bool,
    pub(in crate::island) spectrum_scale: [f32; audio::BANDS],
}

impl App {
    pub(in crate::island) fn bump_room(&self) -> f32 {
        bump_room_for(&self.settings)
    }

    pub(in crate::island) fn liquid_on(&self) -> bool {
        self.settings.style == Style::Liquid && !(self.docked() && self.edge() != Edge::Top)
    }

    pub(in crate::island) fn place_spectrum(&self) {
        let s = self.scale;
        let (win_w, _) = self.window_size(self.win);
        let pill_left = (win_w as f32 - self.cw * s) / 2.0;
        let x0 = pill_left + view::music_slots(&self.ui, s, self.cw * s).bars.map_or(0.0, |b| b.0);
        let y = self.top_margin() * s + (COLLAPSED.1 - SPEC_H) * s / 2.0;
        for (i, bar) in self.music.spectrum.iter().enumerate() {
            let _ = bar.SetSize(Vector2 { X: SPEC_W * s, Y: SPEC_H * s });
            let _ = bar.SetCenterPoint(Vector3 { X: 0.0, Y: SPEC_H * s / 2.0, Z: 0.0 });
            let _ = bar.SetOffset(Vector3 { X: x0 + (i / 2) as f32 * (SPEC_W + SPEC_GAP) * s, Y: y, Z: 0.0 });
        }
    }

    pub(in crate::island) fn spikes_active(&self) -> bool {
        self.liquid_on() && self.settings.edge_spikes && self.music.spectrum_running && self.state == State::Collapsed
    }

    pub(in crate::island) fn paint_spikes(&self) {
        let s = self.scale;
        let (w, _) = self.liquid.bump.size();
        let count = (((w as f32 / s / 20.0) as usize) | 1).clamp(9, 31);
        let mid = count / 2;
        let levels: Vec<f32> = (0..count)
            .map(|i| {
                let t = i.abs_diff(mid) as f32 / mid.max(1) as f32 * (audio::BANDS - 1) as f32;
                let lo = (t as usize).min(audio::BANDS - 2);
                let f = t - lo as f32;
                audio::level(lo) * (1.0 - f) + audio::level(lo + 1) * f
            })
            .collect();
        let depth = self.settings.liquid_strength * s * 0.9;
        let _ = self.liquid.bump.paint(&self.gpu, |p| {
            let (pr, pg, pb) = crate::ui::theme::panel();
            let base = crate::ui::gfx::rgba(pr, pg, pb, 1.0);
            let tip = if self.settings.spike_color {
                let (r, g, b) = self.ui.media.accent.unwrap_or_else(crate::ui::theme::accent_u8);
                let (r, g, b) = (f32::from(r) / 255.0, f32::from(g) / 255.0, f32::from(b) / 255.0);
                let k = 1.0 / r.max(g).max(b).max(0.05);
                crate::ui::gfx::rgba((r * k).min(1.0), (g * k).min(1.0), (b * k).min(1.0), 1.0)
            } else {
                base
            };
            if let Ok(shape) = p.fill_spikes(w as f32, BUMP_SEAM * s, &levels, depth, base, tip) {
                self.clip_bump_blur(&shape);
            }
        });
        self.set_bump_visible(levels.iter().any(|l| *l * depth > 0.8));
    }

    pub(in crate::island) fn spectrum_tick(&mut self) {
        if !self.music.spectrum_running {
            return;
        }
        if self.spikes_active()
            && self.liquid.bump_amp < 0.3
            && self.liquid.bump_vel.abs() < 1.0
            && self.liquid_target().0 == 0.0
        {
            self.paint_spikes();
        }
        if !self.settings.music_eq {
            self.set_timer(Timer::Spectrum, SPECTRUM_MS);
            return;
        }
        for (i, bar) in self.music.spectrum.iter().enumerate() {
            if i % 2 == 1 {
                continue;
            }
            let target = 0.14 + 0.86 * audio::level(i);
            if (target - self.music.spectrum_scale[i]).abs() > 0.012 {
                self.music.spectrum_scale[i] = target;
                let _ = bar.SetScale(Vector3 { X: 1.0, Y: target, Z: 1.0 });
            }
        }
        self.set_timer(Timer::Spectrum, SPECTRUM_MS);
    }

    pub(in crate::island) fn update_spectrum(&mut self, show: bool) {
        if show == self.music.spectrum_running {
            return;
        }
        self.music.spectrum_running = show;
        let bars_visible = show && self.settings.music_eq;
        for (i, bar) in self.music.spectrum.iter().enumerate() {
            let _ = bar.SetIsVisible(bars_visible && i % 2 == 0);
            let _ = bar.SetScale(Vector3 { X: 1.0, Y: 0.14, Z: 1.0 });
        }
        if show && self.spikes_active() {
            self.stop_idle();
        } else if !show {
            self.set_bump_visible(false);
        }
        self.music.spectrum_scale = [0.14; audio::BANDS];
        if show {
            self.place_spectrum();
            audio::start();
            self.set_timer(Timer::Spectrum, SPECTRUM_MS);
        } else {
            audio::stop();
            self.kill_timer(Timer::Spectrum);
        }
    }

    pub(in crate::island) fn update_eq(&mut self) {
        let visible = self.state == State::Collapsed
            && !self.gaming
            && self.ui.media.playing
            && self.ui.volume.is_none()
            && self.ui.downloading.is_none()
            && self.ui.notice.is_none();
        let spikes = self.liquid_on() && self.settings.edge_spikes;
        self.update_spectrum(visible && (self.settings.music_eq || spikes));
        self.update_lyrics(visible);
    }

    pub(in crate::island) fn fit_bump(&mut self) {
        let size = bump_size(self.scale, self.cw);
        if size == self.liquid.bump.size() {
            return;
        }
        let idle = self.liquid.idle_running;
        self.stop_idle();
        let _ = self.liquid.bump.resize(size);
        let _ = self.liquid.bump.sprite.SetCenterPoint(Vector3 { X: size.0 as f32 / 2.0, Y: 0.0, Z: 0.0 });
        self.place_bump();
        if self.spikes_active() {
            self.paint_spikes();
        } else if idle {
            self.start_idle();
        } else {
            self.paint_bump_with(self.liquid.bump_amp, self.liquid.bump_cx);
        }
    }

    pub(in crate::island) fn place_bump(&self) {
        let (win_w, _) = self.window_size(self.win);
        let x = (win_w - self.liquid.bump.size().0) as f32 / 2.0;
        let y = (self.top_margin() + COLLAPSED.1) * self.scale - BUMP_SEAM * self.scale;
        let _ = self.liquid.bump.sprite.SetOffset(Vector3 { X: x, Y: y, Z: 0.0 });
    }

    pub(in crate::island) fn bump_half(&self, amp: f32) -> f32 {
        let s = self.scale;
        let max = (self.settings.liquid_strength * s).max(1.0);
        BUMP_HALF * s * (0.6 + 0.4 * (amp / max).min(1.0))
    }

    pub(in crate::island) fn paint_bump_with(&self, amp: f32, cx: f32) {
        let s = self.scale;
        let (w, _) = self.liquid.bump.size();
        let half = self.bump_half(amp);
        let _ = self.liquid.bump.paint(&self.gpu, |p| {
            let (r, g, b) = crate::ui::theme::panel();
            if let Ok(shape) =
                p.fill_bump(w as f32, BUMP_SEAM * s, w as f32 / 2.0 + cx, half, amp, crate::ui::gfx::rgba(r, g, b, 1.0))
            {
                self.clip_bump_blur(&shape);
            }
        });
    }

    pub(in crate::island) fn clip_bump_blur(&self, shape: &windows::Win32::Graphics::Direct2D::ID2D1PathGeometry) {
        if self.settings.pill_blur
            && let Ok(path) = crate::ui::gfx::composition_path(shape)
        {
            let _ = self.liquid.bump_shape.SetPath(&path);
        }
    }

    pub(in crate::island) fn liquid_target(&self) -> (f32, f32) {
        let s = self.scale;
        let mut pt = windows::Win32::Foundation::POINT::default();
        let mut rc = windows::Win32::Foundation::RECT::default();
        unsafe {
            if GetCursorPos(&mut pt).is_err() || GetWindowRect(self.hwnd, &mut rc).is_err() {
                return (0.0, 0.0);
            }
        }
        let reach = self.settings.liquid_reach * s;
        let strength = self.settings.liquid_strength * s;
        let (win_w, _) = self.window_size(self.win);
        let dx = (pt.x - rc.left) as f32 - win_w as f32 / 2.0;
        let dy = (pt.y - rc.top) as f32 - (self.top_margin() + COLLAPSED.1) * s;
        let straight = (self.cw - COLLAPSED.1) / 2.0 * s;
        let slack = 120.0 * s;
        if dy > reach || dx.abs() > straight + slack {
            return (0.0, 0.0);
        }
        let t = if dy <= 0.0 { 1.0 } else { 1.0 - dy / reach };
        let eased = t * t * (3.0 - 2.0 * t);
        let horizontal = if dx.abs() <= straight { 1.0 } else { 1.0 - (dx.abs() - straight) / slack };
        let limit = (straight - BUMP_HALF * s * 0.6).max(0.0);
        (strength * eased * horizontal, dx.clamp(-limit, limit))
    }

    pub(in crate::island) fn set_bump_visible(&self, visible: bool) {
        let _ = self.liquid.bump.sprite.SetIsVisible(visible);
    }

    pub(in crate::island) fn start_liquid(&self) {
        self.start_char();
        if self.liquid_on() && self.state == State::Collapsed {
            self.set_timer(Timer::Liquid, 60);
        }
    }

    pub(in crate::island) fn liquid_spring(&self) -> (f32, f32) {
        let period = 0.16 + 0.55 * self.settings.liquid_softness;
        let omega = std::f32::consts::TAU / period;
        (omega * omega, 2.0 * 0.42 * omega)
    }

    pub(in crate::island) fn start_idle(&mut self) {
        if self.liquid.idle_running {
            return;
        }
        self.liquid.idle_running = true;
        let s = self.scale;
        self.paint_bump_with(IDLE_AMP * s, 0.0);
        self.set_bump_visible(true);
        let result = (|| -> Result<()> {
            let ease = self
                .compositor
                .CreateCubicBezierEasingFunction(Vector2 { X: 0.45, Y: 0.0 }, Vector2 { X: 0.55, Y: 1.0 })?;
            let scale_anim = self.compositor.CreateVector3KeyFrameAnimation()?;
            let v = |y: f32| Vector3 { X: 1.0, Y: y, Z: 1.0 };
            scale_anim.InsertKeyFrameWithEasingFunction(0.0, v(0.45), &ease)?;
            scale_anim.InsertKeyFrameWithEasingFunction(0.5, v(1.25), &ease)?;
            scale_anim.InsertKeyFrameWithEasingFunction(1.0, v(0.45), &ease)?;
            scale_anim.SetDuration(Duration::from_millis(2600).into())?;
            scale_anim.SetIterationBehavior(AnimationIterationBehavior::Forever)?;
            self.liquid.bump.sprite.StartAnimation(h!("Scale"), &scale_anim.cast::<CompositionAnimation>()?)?;

            let (win_w, _) = self.window_size(self.win);
            let base_x = (win_w - self.liquid.bump.size().0) as f32 / 2.0;
            let base_y = (self.top_margin() + COLLAPSED.1) * s - BUMP_SEAM * s;
            let drift = ((self.liquid.bump.size().0 as f32 / 2.0) - self.bump_half(IDLE_AMP * s)) * 0.8;
            let drift_anim = self.compositor.CreateVector3KeyFrameAnimation()?;
            let at = |dx: f32| Vector3 { X: base_x + dx, Y: base_y, Z: 0.0 };
            drift_anim.InsertKeyFrameWithEasingFunction(0.0, at(-drift), &ease)?;
            drift_anim.InsertKeyFrameWithEasingFunction(0.5, at(drift), &ease)?;
            drift_anim.InsertKeyFrameWithEasingFunction(1.0, at(-drift), &ease)?;
            drift_anim.SetDuration(Duration::from_millis(7200).into())?;
            drift_anim.SetIterationBehavior(AnimationIterationBehavior::Forever)?;
            self.liquid.bump.sprite.StartAnimation(h!("Offset"), &drift_anim.cast::<CompositionAnimation>()?)
        })();
        let _ = result;
    }

    pub(in crate::island) fn stop_idle(&mut self) {
        if !self.liquid.idle_running {
            return;
        }
        self.liquid.idle_running = false;
        let _ = self.liquid.bump.sprite.StopAnimation(h!("Scale"));
        let _ = self.liquid.bump.sprite.StopAnimation(h!("Offset"));
        let _ = self.liquid.bump.sprite.SetScale(Vector3 { X: 1.0, Y: 1.0, Z: 1.0 });
        self.place_bump();
    }

    pub(in crate::island) fn liquid_tick(&mut self) {
        if !self.liquid_on() || self.state != State::Collapsed {
            self.liquid.bump_amp = 0.0;
            self.liquid.bump_vel = 0.0;
            self.stop_idle();
            self.set_bump_visible(false);
            return;
        }
        let now = Instant::now();
        let dt = now.duration_since(self.liquid.last_tick).as_secs_f32().clamp(0.001, 0.05);
        self.liquid.last_tick = now;

        let (target, target_cx) = self.liquid_target();
        let active = target > 0.0 || self.liquid.bump_amp > 0.3 || self.liquid.bump_vel.abs() > 1.0;
        if !active {
            self.liquid.bump_amp = 0.0;
            self.liquid.bump_vel = 0.0;
            if !self.spikes_active() {
                self.start_idle();
            }
            self.set_timer(Timer::Liquid, 90);
            return;
        }
        if self.liquid.idle_running {
            self.stop_idle();
            self.liquid.bump_cx = target_cx;
            self.liquid.bump_cx_vel = 0.0;
        }
        let (k, c) = self.liquid_spring();
        let acc = k * (target - self.liquid.bump_amp) - c * self.liquid.bump_vel;
        self.liquid.bump_vel += acc * dt;
        self.liquid.bump_amp = (self.liquid.bump_amp + self.liquid.bump_vel * dt).max(0.0);
        let acc_x = k * (target_cx - self.liquid.bump_cx) - c * self.liquid.bump_cx_vel;
        self.liquid.bump_cx_vel += acc_x * dt;
        self.liquid.bump_cx += self.liquid.bump_cx_vel * dt;
        if target == 0.0 && self.liquid.bump_amp < 0.3 && self.liquid.bump_vel.abs() < 1.0 {
            self.liquid.bump_amp = 0.0;
            self.liquid.bump_vel = 0.0;
        }
        self.paint_bump_with(self.liquid.bump_amp, self.liquid.bump_cx);
        self.set_bump_visible(self.liquid.bump_amp > 0.2);
        self.set_timer(Timer::Liquid, 16);
    }
}
