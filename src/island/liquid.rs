use std::cell::Cell;

use crate::island::*;

thread_local! {
    static TICKS: Cell<u32> = const { Cell::new(0) };
}

pub(in crate::island) const BUMP_ROOM: f32 = 52.0;
pub(in crate::island) const BUMP_DEPTH_MAX: f32 = 40.0;
pub(in crate::island) const BUMP_HALF: f32 = 44.0;
pub(in crate::island) const IDLE_AMP: f32 = 5.0;
pub(in crate::island) const BUMP_SEAM: f32 = 3.0;
pub(in crate::island) const SPEC_W: f32 = 3.0;
pub(in crate::island) const SPEC_GAP: f32 = 3.0;
pub(in crate::island) const SPEC_H: f32 = 24.0;

pub(in crate::island) fn bump_room_for(settings: &Settings) -> f32 {
    let liquid = if settings.style == Style::Liquid || (settings.edge_spikes && !settings.eq_style.around()) {
        BUMP_ROOM
    } else {
        0.0
    };
    if settings.ring_wanted() { liquid.max(RING_ROOM) } else { liquid }
}

pub(in crate::island) fn bump_size(scale: f32, (w, h): (f32, f32)) -> (i32, i32) {
    let straight = (w.max(h) - w.min(h)) * scale;
    (straight.round() as i32, ((BUMP_DEPTH_MAX + BUMP_SEAM + 4.0) * scale).round() as i32)
}

#[derive(Clone, Copy, PartialEq)]
pub(in crate::island) struct EqClipKey {
    body: crate::ui::gfx::ElasticBody,
    edge: Hang,
    size: (i32, i32),
}

pub(in crate::island) struct Liquid {
    pub(in crate::island) bump: Layer,
    pub(in crate::island) ring: Layer,
    pub(in crate::island) highlight: Layer,
    pub(in crate::island) eq_shape: CompositionPathGeometry,
    pub(in crate::island) eq_clip: CompositionGeometricClip,
    pub(in crate::island) eq_key: Cell<Option<EqClipKey>>,
    pub(in crate::island) pull: crate::ui::gfx::ElasticPull,
    pub(in crate::island) pull_ready: bool,
    pub(in crate::island) bump_fill: SpriteVisual,
    pub(in crate::island) bump_blur: SpriteVisual,
    pub(in crate::island) bump_shape: CompositionPathGeometry,
    pub(in crate::island) body_shape: CompositionPathGeometry,
    pub(in crate::island) body_clip: CompositionGeometricClip,
    pub(in crate::island) round_clip: CompositionGeometricClip,
    pub(in crate::island) body_active: Cell<bool>,
    pub(in crate::island) bump_amp: f32,
    pub(in crate::island) bump_vel: f32,
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
    fn elastic_body_spec(&self, depth: f32) -> crate::ui::gfx::ElasticBody {
        let (at, size, radius) = self.pill(self.win, false, false);
        let legacy = crate::ui::gfx::LiquidBody {
            rect: (at.X, at.Y, size.X, size.Y),
            radius: radius.X,
            edge: self.settings.hang(),
            center: 0.0,
            half: self.bump_half(depth),
            depth,
        }
        .elastic();
        let mut body = legacy;
        if self.liquid.pull_ready {
            body.pull = crate::ui::gfx::ElasticPull { depth, ..self.liquid.pull };
        }
        body.pull.depth = body.pull.depth.clamp(0.0, BUMP_DEPTH_MAX * self.scale);
        body
    }

    fn clip_eq_to_body(&self) {
        if !self.spikes_active() {
            return;
        }
        let depth = if self.liquid_on() && self.liquid.body_active.get() { self.liquid.bump_amp } else { 0.0 };
        let body = self.elastic_body_spec(depth);
        let size = self.liquid.bump.size();
        let edge = self.settings.hang();
        let key = EqClipKey { body, edge, size };
        if self.liquid.eq_key.get() == Some(key) {
            return;
        }
        let (px, py) = self.bump_pivot();
        let half = size.0 as f32 / 2.0;
        let transform = match edge {
            Hang::Down => {
                windows_numerics::Matrix3x2 { M11: 1.0, M22: 1.0, M31: half - px, M32: -py, ..Default::default() }
            }
            Hang::Up => {
                windows_numerics::Matrix3x2 { M11: -1.0, M22: -1.0, M31: half + px, M32: py, ..Default::default() }
            }
            Hang::Right => {
                windows_numerics::Matrix3x2 { M12: 1.0, M21: -1.0, M31: half + py, M32: -px, ..Default::default() }
            }
            Hang::Left => {
                windows_numerics::Matrix3x2 { M12: -1.0, M21: 1.0, M31: half - py, M32: px, ..Default::default() }
            }
        };
        if let Ok(visible) = self.gpu.outside_body(size, &body, &transform)
            && let Ok(path) = crate::ui::gfx::composition_path(&visible)
        {
            let _ = self.liquid.eq_shape.SetPath(&path);
            let _ = self.liquid.bump.sprite.SetClip(&self.liquid.eq_clip);
            self.liquid.eq_key.set(Some(key));
        }
    }

    pub(in crate::island) fn restore_liquid_body(&self) {
        let _ = self.liquid.highlight.sprite.SetIsVisible(false);
        if self.liquid.body_active.replace(false) {
            let _ = self.liquid.body_shape.StopAnimation(h!("Path"));
            let _ = self._visual.SetClip(&self.liquid.round_clip);
        }
    }

    fn liquid_body_path(&self, depth: f32, center: f32) -> Result<windows::UI::Composition::CompositionPath> {
        let (at, size, radius) = self.pill(self.win, false, false);
        let shape = self.gpu.liquid_body(&crate::ui::gfx::LiquidBody {
            rect: (at.X, at.Y, size.X, size.Y),
            radius: radius.X,
            edge: self.settings.hang(),
            center,
            half: self.bump_half(depth),
            depth: depth.clamp(0.0, BUMP_DEPTH_MAX * self.scale),
        })?;
        crate::ui::gfx::composition_path(&shape)
    }

    pub(in crate::island) fn liquid_on(&self) -> bool {
        self.settings.style == Style::Liquid
    }

    pub(in crate::island) fn place_spectrum(&self) {
        let s = self.scale;
        let (win_w, _) = self.window_size(self.win);
        if self.settings.vertical() {
            return self.place_spectrum_upright(win_w as f32);
        }
        let pill_left = (win_w as f32 - self.cw * s) / 2.0;
        let Some(bars) = view::music_slots(&self.ui, s, self.cw * s).bars else { return };
        let x0 = pill_left + bars.0;
        let y = self.win_pad().1 * s + (COLLAPSED.1 - SPEC_H) * s / 2.0;
        for (i, bar) in self.music.spectrum.iter().enumerate() {
            let _ = bar.SetSize(Vector2 { X: SPEC_W * s, Y: SPEC_H * s });
            let _ = bar.SetCenterPoint(Vector3 { X: 0.0, Y: SPEC_H * s / 2.0, Z: 0.0 });
            let _ = bar.SetRotationAngleInDegrees(0.0);
            let _ = bar.SetOffset(Vector3 { X: x0 + (i / 2) as f32 * (SPEC_W + SPEC_GAP) * s, Y: y, Z: 0.0 });
        }
    }

    fn place_spectrum_upright(&self, win_w: f32) {
        let s = self.scale;
        let Some(bars) = view::vertical_slots(&self.ui, s, self.cw * s).bars else { return };
        let cx = win_w / 2.0;
        let top = self.win_pad().1 * s + bars.0;
        for (i, bar) in self.music.spectrum.iter().enumerate() {
            let _ = bar.SetSize(Vector2 { X: SPEC_W * s, Y: SPEC_H * s });
            let _ = bar.SetCenterPoint(Vector3 { X: 0.0, Y: SPEC_H * s / 2.0, Z: 0.0 });
            let _ = bar.SetRotationAngleInDegrees(90.0);
            let y = top + (i / 2) as f32 * (SPEC_W + SPEC_GAP) * s - SPEC_H * s / 2.0;
            let _ = bar.SetOffset(Vector3 { X: cx, Y: y, Z: 0.0 });
        }
    }

    pub(in crate::island) fn spikes_active(&self) -> bool {
        self.settings.edge_spikes
            && !self.settings.eq_style.around()
            && self.music.spectrum_running
            && self.state == State::Collapsed
    }

    pub(in crate::island) fn ring_active(&self) -> bool {
        self.settings.edge_spikes
            && self.settings.eq_style.around()
            && self.music.spectrum_running
            && self.state == State::Collapsed
    }

    fn eq_palette(&self) -> crate::ui::gfx::Palette {
        use crate::data::settings::EqGradient;
        let base = if self.settings.spike_color {
            self.ui.media.accent.unwrap_or_else(crate::ui::theme::accent_u8)
        } else {
            crate::ui::theme::accent_u8()
        };
        let tone = match self.settings.eq_gradient {
            EqGradient::Single => crate::ui::gfx::Tone::Single,
            EqGradient::Vertical => crate::ui::gfx::Tone::Vertical,
            EqGradient::Horizontal => crate::ui::gfx::Tone::Horizontal,
        };
        crate::ui::gfx::Palette { base, tone, flip: false }
    }

    fn flipped(&self, palette: crate::ui::gfx::Palette) -> crate::ui::gfx::Palette {
        crate::ui::gfx::Palette { flip: self.settings.hang() == Hang::Up, ..palette }
    }

    pub(in crate::island) fn refresh_eq_shape(&mut self) {
        let _ = self.liquid.ring.sprite.SetIsVisible(false);
        self.set_bump_visible(false);
        self.update_eq();
        if self.ring_active() {
            self.paint_ring();
        } else if self.spikes_active() {
            self.paint_spikes();
        } else if self.open_eq_active() {
            self.paint_open_eq();
        }
        self.repaint_collapsed();
    }

    pub(in crate::island) fn paint_spikes(&self) {
        self.clip_eq_to_body();
        let _ = self.liquid.bump_blur.SetIsVisible(self.settings.pill_blur);
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
        let depth = self.settings.liquid_strength * s * 0.9 * self.settings.eq_strength;
        let palette = self.flipped(self.eq_palette());
        let shape = match self.settings.eq_style {
            crate::data::settings::EqStyle::Wave => crate::ui::gfx::EdgeShape::Wave,
            crate::data::settings::EqStyle::Bars => crate::ui::gfx::EdgeShape::Bars,
            _ => crate::ui::gfx::EdgeShape::Spikes,
        };
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
            if let Ok(shape) = p.fill_spikes(w as f32, self.seam_px(), &levels, depth, base, tip, shape, palette) {
                self.clip_bump_blur(&shape);
            }
        });
        self.set_bump_visible(levels.iter().any(|l| *l * depth > 0.8));
    }

    fn eq_tip(&self) -> crate::ui::gfx::D2D1Colour {
        let (r, g, b) = if self.settings.spike_color {
            self.ui.media.accent.unwrap_or_else(crate::ui::theme::accent_u8)
        } else {
            crate::ui::theme::accent_u8()
        };
        let (r, g, b) = (f32::from(r) / 255.0, f32::from(g) / 255.0, f32::from(b) / 255.0);
        let k = 1.0 / r.max(g).max(b).max(0.05);
        crate::ui::gfx::rgba((r * k).min(1.0), (g * k).min(1.0), (b * k).min(1.0), 1.0)
    }

    fn edge_levels(&self, count: usize) -> Vec<f32> {
        let mid = count / 2;
        (0..count)
            .map(|i| {
                let t = i.abs_diff(mid) as f32 / mid.max(1) as f32 * (audio::BANDS - 1) as f32;
                let lo = (t as usize).min(audio::BANDS - 2);
                let f = t - lo as f32;
                audio::level(lo) * (1.0 - f) + audio::level(lo + 1) * f
            })
            .collect()
    }

    pub(in crate::island) fn open_eq_active(&self) -> bool {
        self.music.spectrum_running
            && self.state == State::Expanded
            && self.settings.eq_when_open
            && self.settings.edge_spikes
    }

    pub(in crate::island) fn paint_open_eq(&self) {
        if TICKS.get().is_multiple_of(150) {
            crate::log_info!(
                "open equalizer: style {:?}, window {:?}, ring {:?}, levels {:.2}/{:.2}",
                self.settings.eq_style,
                self.window_size(self.win),
                self.liquid.ring.size(),
                audio::level(0),
                audio::level(audio::BANDS / 2)
            );
        }
        if self.settings.eq_style.around() {
            self.paint_ring();
        } else {
            self.paint_open_under();
        }
    }

    fn paint_open_under(&self) {
        let s = self.scale;
        let (px, py, _) = self.big_pad();
        let (l, t) = (px * s, py * s);
        let (w, h) = (EXPANDED.0 * s, EXPANDED.1 * s);
        let r = EXPANDED_RADIUS * s;
        let width = (w - 2.0 * r).max(40.0 * s);
        let count = (((width / s / 20.0) as usize) | 1).clamp(9, 41);
        let levels = self.edge_levels(count);
        let depth = (self.settings.liquid_strength * s * 0.9 * self.settings.eq_strength).min(RING_ROOM * s - 2.0);
        let shape = match self.settings.eq_style {
            crate::data::settings::EqStyle::Wave => crate::ui::gfx::EdgeShape::Wave,
            crate::data::settings::EqStyle::Bars => crate::ui::gfx::EdgeShape::Bars,
            _ => crate::ui::gfx::EdgeShape::Spikes,
        };
        let (tip, palette) = (self.eq_tip(), self.eq_palette());
        let seam = 3.0 * s;
        let _ = self.liquid.ring.paint(&self.gpu, |p| {
            let clear = crate::ui::gfx::rgba(0.0, 0.0, 0.0, 0.0);
            let old = p.push_offset(l + r, t + h - seam);
            let _ = p.fill_spikes(width, seam, &levels, depth, clear, tip, shape, palette);
            p.set_transform(&old);
        });
        let _ = self.liquid.ring.sprite.SetIsVisible(levels.iter().any(|v| *v * depth > 0.8));
    }

    pub(in crate::island) fn paint_ring(&self) {
        use crate::ui::gfx::RingPt;
        let s = self.scale;
        let (frame, radius) = if self.state == State::Expanded {
            let (px, py, _) = self.big_pad();
            ((px * s, py * s, EXPANDED.0 * s, EXPANDED.1 * s), EXPANDED_RADIUS * s)
        } else {
            let (px, py) = self.small_pad();
            let (pw, ph) = self.collapsed();
            ((px * s, py * s, pw * s, ph * s), pw.min(ph) * s / 2.0)
        };
        let hang = if self.state == State::Expanded { Hang::Down } else { self.settings.hang() };
        let (l0, t0, w0, h0) = frame;
        let (ccx, ccy) = (l0 + w0 / 2.0, t0 + h0 / 2.0);
        let (w, h) = if matches!(hang, Hang::Left | Hang::Right) { (h0, w0) } else { (w0, h0) };
        let (l, t) = (ccx - w / 2.0, ccy - h / 2.0);
        let turn = |dx: f32, dy: f32| match hang {
            Hang::Down => (dx, dy),
            Hang::Up => (-dx, -dy),
            Hang::Right => (dy, -dx),
            Hang::Left => (-dy, dx),
        };
        let r = radius.min(h / 2.0).min(w / 2.0);
        let (sw, sh) = (w - 2.0 * r, h - 2.0 * r);
        let quarter = std::f32::consts::FRAC_PI_2 * r;
        let perimeter = 2.0 * sw + 2.0 * sh + 4.0 * quarter;
        let n = ((perimeter / (9.0 * s)) as usize).clamp(24, 220) & !1;
        let free = RING_ROOM * s - 2.0;
        let depth = self.settings.liquid_strength * s * 0.9 * self.settings.eq_strength;
        let (cx, cy) = (l + w / 2.0, t + h / 2.0);
        let hx = sw / 2.0;
        let (vy, pi) = (sh / 2.0, std::f32::consts::PI);
        let outline = |at: f32| -> (f32, f32, f32, f32) {
            let corner = |centre: (f32, f32), from: f32, k: f32| {
                let a = from + k / r;
                (centre.0 + r * a.sin(), centre.1 + r * a.cos(), a.sin(), a.cos())
            };
            let mut k = at;
            if k < hx {
                return (cx + k, cy + h / 2.0, 0.0, 1.0);
            }
            k -= hx;
            if k < quarter {
                return corner((cx + hx, cy + vy), 0.0, k);
            }
            k -= quarter;
            if k < sh {
                return (cx + w / 2.0, cy + vy - k, 1.0, 0.0);
            }
            k -= sh;
            if k < quarter {
                return corner((cx + hx, cy - vy), pi / 2.0, k);
            }
            k -= quarter;
            if k < sw {
                return (cx + hx - k, cy - h / 2.0, 0.0, -1.0);
            }
            k -= sw;
            if k < quarter {
                return corner((cx - hx, cy - vy), pi, k);
            }
            k -= quarter;
            if k < sh {
                return (cx - w / 2.0, cy - vy + k, -1.0, 0.0);
            }
            k -= sh;
            if k < quarter {
                return corner((cx - hx, cy + vy), 1.5 * pi, k);
            }
            k -= quarter;
            (cx - hx + k, cy + h / 2.0, 0.0, 1.0)
        };
        let levels: Vec<f32> = (0..n)
            .map(|i| {
                let at = perimeter * (i as f32 + 0.5) / n as f32;
                let u = at.min(perimeter - at) / (perimeter / 2.0);
                let t = u * (audio::BANDS - 1) as f32;
                let lo = (t as usize).min(audio::BANDS - 2);
                let f = t - lo as f32;
                audio::level(lo) * (1.0 - f) + audio::level(lo + 1) * f
            })
            .collect();
        let smooth: Vec<f32> =
            (0..n).map(|i| levels[(i + n - 1) % n] * 0.25 + levels[i] * 0.5 + levels[(i + 1) % n] * 0.25).collect();
        let points: Vec<RingPt> = (0..n)
            .map(|i| {
                let (x, y, nx, ny) = outline(perimeter * (i as f32 + 0.5) / n as f32);
                let d = (smooth[i].clamp(0.0, 1.0) * depth).min(free.max(0.0));
                let (px, py) = turn(x - ccx, y - ccy);
                let (nx, ny) = turn(nx, ny);
                RingPt { x: ccx + px, y: ccy + py, nx, ny, d }
            })
            .collect();
        let tip = self.eq_tip();
        let aura = self.settings.eq_style == crate::data::settings::EqStyle::Aura;
        let palette = self.eq_palette();
        let _ = self.liquid.ring.paint(&self.gpu, |p| {
            let _ = p.fill_ring(&points, aura, tip, 3.0 * s, palette);
        });
        let _ = self.liquid.ring.sprite.SetIsVisible(points.iter().any(|p| p.d > 0.8));
    }

    pub(in crate::island) fn spectrum_tick(&mut self) {
        if !self.music.spectrum_running {
            return;
        }
        if self.ring_active() {
            self.paint_ring();
        } else if self.open_eq_active() {
            self.paint_open_eq();
        }
        if self.settings.music_eq
            && self.state == State::Collapsed
            && TICKS.replace(TICKS.get().wrapping_add(1)).is_multiple_of(20)
        {
            self.place_spectrum();
        }
        if self.spikes_active() {
            self.paint_spikes();
        }
        if !self.settings.music_eq {
            self.set_timer(Timer::Spectrum, self.frame_ms());
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
        self.set_timer(Timer::Spectrum, self.frame_ms());
    }

    pub(in crate::island) fn update_spectrum(&mut self, show: bool) {
        let bars_visible = show && self.settings.music_eq && self.state == State::Collapsed;
        if show == self.music.spectrum_running {
            for (i, bar) in self.music.spectrum.iter().enumerate() {
                let _ = bar.SetIsVisible(bars_visible && i % 2 == 0);
            }
            if bars_visible {
                self.place_spectrum();
            }
            if show && (self.spikes_active() || self.ring_active()) {
                self.stop_idle();
            }
            if self.state != State::Collapsed {
                self.set_bump_visible(false);
            }
            if show {
                self.set_timer(Timer::Spectrum, self.frame_ms());
            }
            return;
        }
        self.music.spectrum_running = show;
        if !show {
            let _ = self.liquid.ring.sprite.SetIsVisible(false);
        }
        for (i, bar) in self.music.spectrum.iter().enumerate() {
            let _ = bar.SetIsVisible(bars_visible && i % 2 == 0);
            let _ = bar.SetScale(Vector3 { X: 1.0, Y: 0.14, Z: 1.0 });
        }
        if show && (self.spikes_active() || self.ring_active()) {
            self.stop_idle();
        } else if !show {
            self.set_bump_visible(false);
        }
        self.music.spectrum_scale = [0.14; audio::BANDS];
        if show {
            self.place_spectrum();
            self.set_timer(Timer::Spectrum, self.frame_ms());
        } else {
            self.kill_timer(Timer::Spectrum);
        }
    }

    pub(in crate::island) fn update_eq(&mut self) {
        let visible = self.state == State::Collapsed
            && !self.gaming
            && (!self.pill_hidden || self.ghosted)
            && self.ui.media.playing
            && self.ui.volume.is_none()
            && self.ui.downloading.is_none()
            && self.ui.notice.is_none();
        let spikes = self.settings.edge_spikes;
        let open = self.state == State::Expanded
            && self.settings.eq_when_open
            && self.settings.edge_spikes
            && !self.gaming
            && (!self.pill_hidden || self.ghosted)
            && self.ui.media.playing;
        let analysing = self.ui.media.playing
            && !self.gaming
            && ((self.pet.desktop.is_some())
                || ((!self.pill_hidden || self.ghosted) && (self.settings.music_eq || spikes || open)));
        if analysing {
            audio::start();
        } else {
            audio::stop();
        }
        self.update_spectrum((visible && (self.settings.music_eq || spikes)) || open);
        self.update_lyrics(visible);
        self.start_progress_timer();
    }

    pub(in crate::island) fn fit_bump(&mut self) {
        let _ = self.liquid.highlight.resize(self.collapsed.size());
        let size = bump_size(self.scale, self.collapsed());
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
            self.paint_liquid_body(self.liquid.bump_amp);
        }
    }

    pub(in crate::island) fn place_bump(&self) {
        let width = bump_size(self.scale, self.collapsed()).0 as f32;
        let (px, py) = self.bump_pivot();
        let angle = match self.settings.hang() {
            Hang::Down => 0.0,
            Hang::Up => 180.0,
            Hang::Right => -90.0,
            Hang::Left => 90.0,
        };
        let _ = self.liquid.bump.sprite.SetRotationAngleInDegrees(angle);
        let _ = self.liquid.bump.sprite.SetOffset(Vector3 { X: px - width / 2.0, Y: py, Z: 0.0 });
    }

    pub(in crate::island) fn bump_pivot(&self) -> (f32, f32) {
        let s = self.scale;
        let (win_w, _) = self.window_size(self.win);
        let (_, top) = self.win_pad();
        let (pw, ph) = self.collapsed();
        let seam = BUMP_SEAM * s;
        let cx = win_w as f32 / 2.0;
        match self.settings.hang() {
            Hang::Down => (cx, (top + ph) * s - seam),
            Hang::Up => (cx, top * s + seam),
            Hang::Right => (cx + pw * s / 2.0 - seam, (top + ph / 2.0) * s),
            Hang::Left => (cx - pw * s / 2.0 + seam, (top + ph / 2.0) * s),
        }
    }

    pub(in crate::island) fn seam_px(&self) -> f32 {
        BUMP_SEAM * self.scale + if self.settings.pill_opacity < 0.98 { 1.0 } else { 0.0 }
    }

    pub(in crate::island) fn bump_half(&self, amp: f32) -> f32 {
        let s = self.scale;
        let max = (self.settings.liquid_strength * s).max(1.0);
        BUMP_HALF * s * (0.6 + 0.4 * (amp / max).min(1.0))
    }

    pub(in crate::island) fn bump_look(&self) -> Option<(crate::ui::gfx::Palette, crate::ui::gfx::D2D1Colour)> {
        use crate::data::settings::BumpTint;
        use crate::ui::gfx::{Palette, Tone};
        if !self.ui.media.playing {
            return None;
        }
        match self.settings.bump_tint {
            BumpTint::Off => None,
            BumpTint::Progress if view::progress_strip_on(&self.ui) => {
                let base = self.ui.media.accent.unwrap_or_else(crate::ui::theme::accent_u8);
                let palette = Palette { base, tone: Tone::Horizontal, flip: self.settings.hang() == Hang::Up };
                Some((palette, palette.at(0.5, 1.0)))
            }
            _ => Some((self.flipped(self.eq_palette()), self.eq_tip())),
        }
    }

    pub(in crate::island) fn paint_liquid_body(&self, depth: f32) {
        if !self.liquid_on() || self.state != State::Collapsed {
            self.restore_liquid_body();
            return;
        }
        if self.liquid.idle_running && self.liquid.body_active.get() {
            return;
        }
        let body = self.elastic_body_spec(depth);
        if let Ok(shape) = self.gpu.elastic_body(&body)
            && let Ok(path) = crate::ui::gfx::composition_path(&shape)
        {
            let _ = self.liquid.body_shape.SetPath(&path);
            let _ = self._visual.SetClip(&self.liquid.body_clip);
            self.liquid.body_active.set(true);
        }
        if !self.spikes_active() {
            self.set_bump_visible(false);
        }
        if let Some((palette, tip)) = self.bump_look().filter(|_| depth > 0.2) {
            let color = if palette.tone == crate::ui::gfx::Tone::Horizontal {
                palette.at(((body.pull.point.0 - body.rect.0) / body.rect.2.max(1.0)).clamp(0.0, 1.0), 1.0)
            } else {
                tip
            };
            let _ = self.liquid.highlight.paint(&self.gpu, |p| {
                let _ = p.liquid_highlight(self.liquid.highlight.size(), &body.pull, color);
            });
            let _ = self.liquid.highlight.sprite.SetIsVisible(true);
        } else {
            let _ = self.liquid.highlight.sprite.SetIsVisible(false);
        }
        self.clip_eq_to_body();
    }

    pub(in crate::island) fn clip_bump_blur(&self, shape: &windows::Win32::Graphics::Direct2D::ID2D1PathGeometry) {
        if !self.settings.pill_blur {
            return;
        }
        match crate::ui::gfx::composition_path(shape) {
            Ok(path) => {
                if let Err(e) = self.liquid.bump_shape.SetPath(&path) {
                    crate::log_warn!("bump blur: SetPath failed: {e}");
                }
            }
            Err(e) => crate::log_warn!("bump blur: no path: {e}"),
        }
    }

    pub(in crate::island) fn liquid_target(&self) -> crate::ui::gfx::ElasticPull {
        let mut cursor = windows::Win32::Foundation::POINT::default();
        let mut window = windows::Win32::Foundation::RECT::default();
        unsafe {
            if GetCursorPos(&mut cursor).is_err() || GetWindowRect(self.hwnd, &mut window).is_err() {
                return Default::default();
            }
        }
        let (at, size, radius) = self.pill(self.win, false, false);
        let blocked = if self.docked() {
            Some(match self.settings.hang() {
                Hang::Down => Hang::Up,
                Hang::Up => Hang::Down,
                Hang::Left => Hang::Right,
                Hang::Right => Hang::Left,
            })
        } else {
            None
        };
        let mut pull = crate::ui::gfx::cursor_pull(
            (at.X, at.Y, size.X, size.Y),
            radius.X,
            ((cursor.x - window.left) as f32, (cursor.y - window.top) as f32),
            self.settings.liquid_reach * self.scale,
            self.settings.liquid_strength * self.scale,
            blocked,
        );
        pull.spread = BUMP_HALF * self.scale;
        pull
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
        if self.liquid.idle_running && self.liquid.body_active.get() {
            return;
        }
        self.liquid.idle_running = true;
        self.set_bump_visible(false);
        let result = (|| -> Result<()> {
            let ease = self
                .compositor
                .CreateCubicBezierEasingFunction(Vector2 { X: 0.45, Y: 0.0 }, Vector2 { X: 0.55, Y: 1.0 })?;
            let animation = self.compositor.CreatePathKeyFrameAnimation()?;
            for (key, depth, center) in [(0.0, 2.0, -6.0), (0.5, IDLE_AMP, 6.0), (1.0, 2.0, -6.0)] {
                let shape = self.liquid_body_path(depth * self.scale, center * self.scale)?;
                if key == 0.0 {
                    self.liquid.body_shape.SetPath(&shape)?;
                }
                animation.InsertKeyFrameWithEasingFunction(key, &shape, &ease)?;
            }
            animation.SetDuration(Duration::from_millis(5200).into())?;
            animation.SetIterationBehavior(AnimationIterationBehavior::Forever)?;
            self.liquid.body_shape.StartAnimation(h!("Path"), &animation.cast::<CompositionAnimation>()?)?;
            self._visual.SetClip(&self.liquid.body_clip)?;
            self.liquid.body_active.set(true);
            Ok(())
        })();
        if let Err(error) = result {
            self.liquid.idle_running = false;
            crate::log_warn!("liquid body animation failed: {error}");
        }
    }

    pub(in crate::island) fn stop_idle(&mut self) {
        if !self.liquid.idle_running {
            return;
        }
        self.liquid.idle_running = false;
        self.restore_liquid_body();
        self.place_bump();
    }

    pub(in crate::island) fn liquid_tick(&mut self) {
        if !self.liquid_on() || self.state != State::Collapsed {
            self.liquid.bump_amp = 0.0;
            self.liquid.bump_vel = 0.0;
            self.stop_idle();
            self.restore_liquid_body();
            self.set_bump_visible(false);
            return;
        }
        let now = Instant::now();
        let dt = now.duration_since(self.liquid.last_tick).as_secs_f32().clamp(0.001, 0.05);
        self.liquid.last_tick = now;

        let target = self.liquid_target();
        let active = target.depth > 0.0 || self.liquid.bump_amp > 0.3 || self.liquid.bump_vel.abs() > 1.0;
        if !active {
            self.liquid.bump_amp = 0.0;
            self.liquid.bump_vel = 0.0;
            if !self.spikes_active() && !self.ring_active() {
                self.start_idle();
            } else {
                self.restore_liquid_body();
            }
            self.set_timer(Timer::Liquid, 90);
            return;
        }
        if self.liquid.idle_running {
            self.stop_idle();
            self.liquid.pull_ready = false;
        }
        let (k, c) = self.liquid_spring();
        let acc = k * (target.depth - self.liquid.bump_amp) - c * self.liquid.bump_vel;
        self.liquid.bump_vel += acc * dt;
        self.liquid.bump_amp = (self.liquid.bump_amp + self.liquid.bump_vel * dt).max(0.0);
        if target.depth > 0.0 {
            if !self.liquid.pull_ready {
                self.liquid.pull = target;
                self.liquid.pull_ready = true;
            } else {
                let a = 1.0 - (-dt * 16.0).exp();
                self.liquid.pull.point.0 += (target.point.0 - self.liquid.pull.point.0) * a;
                self.liquid.pull.point.1 += (target.point.1 - self.liquid.pull.point.1) * a;
                let normal = (
                    self.liquid.pull.normal.0 + (target.normal.0 - self.liquid.pull.normal.0) * a,
                    self.liquid.pull.normal.1 + (target.normal.1 - self.liquid.pull.normal.1) * a,
                );
                let length = normal.0.hypot(normal.1).max(0.001);
                self.liquid.pull.normal = (normal.0 / length, normal.1 / length);
                self.liquid.pull.spread = target.spread;
            }
        }
        if target.depth == 0.0 && self.liquid.bump_amp < 0.3 && self.liquid.bump_vel.abs() < 1.0 {
            self.liquid.bump_amp = 0.0;
            self.liquid.bump_vel = 0.0;
        }
        self.paint_liquid_body(self.liquid.bump_amp);
        self.set_timer(Timer::Liquid, self.frame_ms());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_shape_reserves_room_for_every_edge_equalizer_and_keeps_internal_bars() {
        let mut settings = Settings { style: Style::Flat, edge_spikes: true, ..Settings::default() };
        for shape in crate::data::settings::EqStyle::ALL {
            settings.eq_style = shape;
            assert!(bump_room_for(&settings) >= if shape.around() { RING_ROOM } else { BUMP_ROOM });
        }
        settings.edge_spikes = false;
        assert_eq!(bump_room_for(&settings), 0.0);
        let mut ui = view::UiState::new();
        ui.cfg = settings;
        ui.cfg.music_eq = true;
        ui.media.playing = true;
        ui.media.title = "Test track".into();
        assert!(view::music_slots(&ui, 1.0, 400.0).bars.is_some());
        ui.cfg.music_eq = false;
        assert!(view::music_slots(&ui, 1.0, 400.0).bars.is_none());
    }
}
