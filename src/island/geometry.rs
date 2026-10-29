use crate::island::*;

pub(in crate::island) const COLLAPSED: (f32, f32) = (200.0, 34.0);
pub(in crate::island) const EXPANDED: (f32, f32) = (640.0, 384.0);
pub(in crate::island) const TOP_MARGIN: f32 = 8.0;
pub(in crate::island) const PAD: f32 = 20.0;
pub(in crate::island) const EXPANDED_RADIUS: f32 = 28.0;
pub(in crate::island) const EAR: f32 = 14.0;
pub(in crate::island) const DOCK_RADIUS: f32 = 16.0;
pub(in crate::island) const STRIP_H: i32 = 3;

pub(in crate::island) static SNAP_FIT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(in crate::island) const RING_ROOM: f32 = 40.0;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::island) enum OpenEq {
    Off,
    Under,
    Around,
}

#[derive(Clone, Copy)]
pub(in crate::island) struct Room {
    pub bottom: f32,
    pub ring: bool,
    pub open: OpenEq,
}

pub(in crate::island) fn window_room(settings: &Settings) -> Room {
    let open = match (settings.eq_when_open && settings.edge_spikes, settings.eq_style.around()) {
        (false, _) => OpenEq::Off,
        (true, false) => OpenEq::Under,
        (true, true) => OpenEq::Around,
    };
    Room { bottom: bump_room_for(settings), ring: settings.ring_wanted(), open }
}

pub(in crate::island) fn big_pad(open: OpenEq, docked: bool) -> (f32, f32, f32) {
    let top = if docked { 0.0 } else { TOP_MARGIN };
    match open {
        OpenEq::Off => (PAD, top, PAD),
        OpenEq::Under => (PAD, top, PAD.max(RING_ROOM)),
        OpenEq::Around => (PAD.max(RING_ROOM), top.max(RING_ROOM), PAD.max(RING_ROOM)),
    }
}

pub(in crate::island) fn window_size(scale: f32, win: Win, docked: bool, cw: f32, room: &Room) -> (i32, i32) {
    let (w, h) = if win == Win::Big { EXPANDED } else { (cw, COLLAPSED.1) };
    let base_top = if docked { 0.0 } else { TOP_MARGIN };
    let (pad, top) = match win {
        Win::Small if room.ring => (PAD.max(RING_ROOM), base_top.max(RING_ROOM)),
        Win::Big => {
            let (x, t, _) = big_pad(room.open, docked);
            (x, t)
        }
        _ => (PAD, base_top),
    };
    let width = ((w + 2.0 * pad) * scale).round() as i32;
    match win {
        Win::Strip => (width, STRIP_H),
        Win::Small => (width, ((h + top + room.bottom) * scale).round() as i32),
        Win::Big => (width, ((h + top + big_pad(room.open, docked).2) * scale).round() as i32),
    }
}

impl App {
    pub(in crate::island) fn edge(&self) -> Edge {
        self.settings.dock_edge
    }

    pub(in crate::island) fn autohide_ok(&self) -> bool {
        self.settings.auto_hide
            && !(self.docked() && self.edge() != Edge::Top)
            && (self.docked() || self.settings.float_pos.is_none())
    }

    pub(in crate::island) fn work_area() -> (f32, f32, f32, f32) {
        use windows::Win32::UI::WindowsAndMessaging::{
            SPI_GETWORKAREA, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
        };
        let mut rc = windows::Win32::Foundation::RECT::default();
        let ok = unsafe {
            SystemParametersInfoW(
                SPI_GETWORKAREA,
                0,
                Some((&raw mut rc).cast()),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            )
            .is_ok()
        };
        if ok && rc.right > rc.left && rc.bottom > rc.top {
            (rc.left as f32, rc.top as f32, rc.right as f32, rc.bottom as f32)
        } else {
            let (w, h) = Self::screen_size();
            (0.0, 0.0, w, h)
        }
    }

    pub(in crate::island) fn screen_size() -> (f32, f32) {
        unsafe { (GetSystemMetrics(SM_CXSCREEN) as f32, GetSystemMetrics(SM_CYSCREEN) as f32) }
    }

    pub(in crate::island) fn pill_screen(&self, expanded: bool) -> (f32, f32, f32, f32) {
        let s = self.scale;
        let (sw, sh) = Self::screen_size();
        let (wl, wt, wr, wb) = Self::work_area();
        let (cw, ch) = (self.cw * s, COLLAPSED.1 * s);
        let (ew, eh) = (EXPANDED.0 * s, EXPANDED.1 * s);
        let clamp = |v: f32, lo: f32, hi: f32| v.max(lo).min(hi.max(lo));
        let (cx, cy) = if self.docked() {
            match self.edge() {
                Edge::Top => (clamp(self.settings.dock_offset * sw - cw / 2.0, 0.0, sw - cw), wt),
                Edge::Bottom => (clamp(self.settings.dock_offset * sw - cw / 2.0, 0.0, sw - cw), wb - ch),
                Edge::Left => (wl, clamp(self.settings.dock_offset * sh - ch / 2.0, wt, wb - ch)),
                Edge::Right => (wr - cw, clamp(self.settings.dock_offset * sh - ch / 2.0, wt, wb - ch)),
            }
        } else {
            match self.settings.float_pos {
                Some([x, y]) => (clamp(x as f32 - cw / 2.0, 0.0, sw - cw), clamp(y as f32, 0.0, sh - ch)),
                None => ((sw - cw) / 2.0, TOP_MARGIN * s),
            }
        };
        if !expanded {
            return (cx, cy, cw, ch);
        }
        let (mid_x, mid_y) = (cx + cw / 2.0, cy + ch / 2.0);
        let (ex, ey) = if self.docked() {
            match self.edge() {
                Edge::Top => (clamp(mid_x - ew / 2.0, 0.0, sw - ew), wt),
                Edge::Bottom => (clamp(mid_x - ew / 2.0, 0.0, sw - ew), wb - eh),
                Edge::Left => (wl, clamp(mid_y - eh / 2.0, wt, wb - eh)),
                Edge::Right => (wr - ew, clamp(mid_y - eh / 2.0, wt, wb - eh)),
            }
        } else {
            (clamp(mid_x - ew / 2.0, 0.0, sw - ew), clamp(cy, 0.0, sh - eh))
        };
        (ex, ey, ew, eh)
    }

    pub(in crate::island) fn window_origin(&self, win: Win) -> (f32, f32) {
        let s = self.scale;
        let pill = self.pill_screen(win == Win::Big);
        match win {
            Win::Strip => (pill.0 - PAD * s, 0.0),
            Win::Small => {
                let (px, py) = self.small_pad();
                (pill.0 - px * s, pill.1 - py * s)
            }
            Win::Big => {
                let (px, py, _) = self.big_pad();
                (pill.0 - px * s, pill.1 - py * s)
            }
        }
    }

    pub(in crate::island) fn docked(&self) -> bool {
        self.settings.placement == Placement::Docked
    }

    pub(in crate::island) fn top_margin(&self) -> f32 {
        if self.docked() { 0.0 } else { TOP_MARGIN }
    }

    pub(in crate::island) fn small_pad(&self) -> (f32, f32) {
        if self.settings.ring_wanted() {
            (PAD.max(RING_ROOM), self.top_margin().max(RING_ROOM))
        } else {
            (PAD, self.top_margin())
        }
    }

    pub(in crate::island) fn win_pad(&self) -> (f32, f32) {
        if self.win == Win::Big {
            let (x, t, _) = self.big_pad();
            (x, t)
        } else {
            self.small_pad()
        }
    }

    pub(in crate::island) fn big_pad(&self) -> (f32, f32, f32) {
        big_pad(window_room(&self.settings).open, self.docked())
    }

    pub(in crate::island) fn open_ms(&self) -> u64 {
        (OPEN_BASE_MS / self.settings.open_speed) as u64
    }

    pub(in crate::island) fn close_ms(&self) -> u64 {
        (CLOSE_BASE_MS / self.settings.close_speed) as u64
    }

    pub(in crate::island) fn window_size(&self, win: Win) -> (i32, i32) {
        window_size(self.scale, win, self.docked(), self.cw, &window_room(&self.settings))
    }

    pub(in crate::island) fn pill_full(&self) -> view::Rect {
        let (w, h) = EXPANDED;
        let (px, py, _) = self.big_pad();
        (px * self.scale, py * self.scale, w * self.scale, h * self.scale)
    }

    pub(in crate::island) fn pill_rect(&self, expanded: bool) -> view::Rect {
        let (w, h) = if expanded { EXPANDED } else { (self.cw, COLLAPSED.1) };
        let (px, py) = if expanded {
            let (x, t, _) = self.big_pad();
            (x, t)
        } else {
            self.small_pad()
        };
        let full = (px * self.scale, py * self.scale, w * self.scale, h * self.scale);
        if expanded { view::body_rect(full, self.scale, self.ui.tab) } else { full }
    }

    pub(in crate::island) fn update_offsets(&self) {
        let (win_w, _) = self.window_size(self.win);
        let small = window_size(self.scale, Win::Small, self.docked(), self.cw, &window_room(&self.settings)).0;
        let big = window_size(self.scale, Win::Big, self.docked(), self.cw, &window_room(&self.settings)).0;
        for (layer, own_w) in [(&self.collapsed, small), (&self.expanded, big), (&self.liquid.ring, small)] {
            let x = (win_w - own_w) as f32 / 2.0;
            let _ = layer.sprite.SetOffset(Vector3 { X: x, Y: 0.0, Z: 0.0 });
        }
        self.place_bump();
        self.place_spectrum();
        self.place_char();
    }

    pub(in crate::island) fn fit_width(&mut self) {
        if self.state != State::Collapsed {
            return;
        }
        let short_lived = self.ui.notice.is_some() || self.ui.volume.is_some() || self.ui.downloading.is_some();
        let animate = short_lived && !SNAP_FIT.load(std::sync::atomic::Ordering::Relaxed);
        let want = view::collapsed_width(&self.ui, COLLAPSED.0);
        if (want - self.cw).abs() < 0.5 {
            return;
        }
        let old_origin = self.window_origin(Win::Small);
        self.cw = want;
        let new_origin = self.window_origin(Win::Small);
        if let Ok(cur) = self.geometry.Offset() {
            let _ = self
                .geometry
                .SetOffset(Vector2 { X: cur.X + old_origin.0 - new_origin.0, Y: cur.Y + old_origin.1 - new_origin.1 });
        }
        self.place(Win::Small);
        let docked = self.docked();
        let small = window_size(self.scale, Win::Small, docked, self.cw, &window_room(&self.settings));
        let _ = self.collapsed.resize(small);
        let _ = self.liquid.ring.resize(self.window_size(self.win));
        self.fit_bump();
        self.update_offsets();
        if animate {
            let _ = self.animate_to(false, false);
        } else {
            for name in [windows::core::h!("Offset"), windows::core::h!("Size"), windows::core::h!("CornerRadius")] {
                let _ = self.geometry.StopAnimation(name);
            }
            let _ = self.set_geometry_now(Win::Small, false, false);
        }
    }

    pub(in crate::island) fn pill(&self, win: Win, expanded: bool, hidden: bool) -> (Vector2, Vector2, Vector2) {
        let (w, h) = if expanded { EXPANDED } else { (self.cw, COLLAPSED.1) };
        let docked = self.docked();
        let radius = if expanded {
            EXPANDED_RADIUS
        } else if docked {
            DOCK_RADIUS.min(h / 2.0)
        } else {
            h / 2.0
        } * self.scale;
        let (w, h) = (w * self.scale, h * self.scale);
        let origin = self.window_origin(win);
        let at = self.pill_screen(expanded);
        let (mut x, mut y) = (at.0 - origin.0, at.1 - origin.1);
        let (mut width, mut height) = (w, h);
        if docked {
            match self.edge() {
                Edge::Top => {
                    y -= radius;
                    height += radius;
                }
                Edge::Bottom => height += radius,
                Edge::Left => {
                    x -= radius;
                    width += radius;
                }
                Edge::Right => width += radius,
            }
        }
        if hidden {
            y = if docked { -h - radius - 2.0 } else { -h - 2.0 };
        }
        (Vector2 { X: x, Y: y }, Vector2 { X: width, Y: height }, Vector2 { X: radius, Y: radius })
    }

    pub(in crate::island) fn ear_targets(&self, win: Win, expanded: bool, hidden: bool) -> (f32, f32, f32) {
        let (off, size, _) = self.pill(win, expanded, hidden);
        let r = EAR * self.scale;
        let shown = self.docked() && self.edge() == Edge::Top && !expanded && !hidden;
        (off.X - r, off.X + size.X, if shown { 1.0 } else { 0.0 })
    }

    pub(in crate::island) fn place(&mut self, win: Win) {
        self.win = win;
        let (w, h) = self.window_size(win);
        let (x, y) = self.window_origin(win);
        unsafe {
            let _ =
                SetWindowPos(self.hwnd, Some(HWND_TOPMOST), x.round() as i32, y.round() as i32, w, h, SWP_NOACTIVATE);
        }
        let _ = self.liquid.ring.resize(self.window_size(win));
        self.update_offsets();
    }

    pub(in crate::island) fn set_geometry_now(&self, win: Win, expanded: bool, hidden: bool) -> Result<()> {
        let (offset, size, radius) = self.pill(win, expanded, hidden);
        self.geometry.SetOffset(offset)?;
        self.geometry.SetSize(size)?;
        self.geometry.SetCornerRadius(radius)?;
        let (left, right, opacity) = self.ear_targets(win, expanded, hidden);
        for (ear, x) in self.ears.iter().zip([left, right]) {
            ear.sprite.SetOffset(Vector3 { X: x, Y: 0.0, Z: 0.0 })?;
            ear.sprite.SetOpacity(opacity * self.settings.pill_opacity)?;
        }
        Ok(())
    }

    pub(in crate::island) fn cursor_inside(&self) -> bool {
        unsafe {
            let mut pt = windows::Win32::Foundation::POINT::default();
            let mut rc = windows::Win32::Foundation::RECT::default();
            GetCursorPos(&mut pt).is_ok()
                && GetWindowRect(self.hwnd, &mut rc).is_ok()
                && windows::Win32::Graphics::Gdi::PtInRect(&rc, pt).as_bool()
        }
    }

    pub(in crate::island) fn reset_position(&mut self) {
        let fresh = Settings::default();
        self.settings.placement = fresh.placement;
        self.settings.dock_edge = fresh.dock_edge;
        self.settings.dock_offset = fresh.dock_offset;
        self.settings.float_pos = fresh.float_pos;
        self.settings.save();
        self.ui.cfg = self.settings.clone();
        if matches!(self.state, State::Hiding | State::Hidden) {
            self.reveal();
        }
        self.apply_layout();
        self.render_settings_win();
        self.set_timer(Timer::Settle, 700);
    }

    pub(in crate::island) fn apply_layout(&mut self) {
        self.scale = self.dpi_scale * self.settings.scale_percent as f32 / 100.0;
        self.stop_idle();
        self.liquid.bump_amp = 0.0;
        self.liquid.bump_vel = 0.0;
        let docked = self.docked();
        let win = self.win;
        self.place(win);
        let expanded = self.state == State::Expanded;
        let hidden = matches!(self.state, State::Hidden | State::Hiding);
        let _ = self.set_geometry_now(win, expanded, hidden);
        let small = window_size(self.scale, Win::Small, docked, self.cw, &window_room(&self.settings));
        let _ = self.collapsed.resize(small);
        let _ = self.liquid.ring.resize(self.window_size(self.win));
        let _ = self.expanded.resize(window_size(self.scale, Win::Big, docked, self.cw, &window_room(&self.settings)));
        let _ = self.liquid.bump.resize(bump_size(self.scale, self.cw));
        let _ = self
            .pet
            .char_layer
            .resize(((characters::BOX_W * self.scale).round() as i32, (characters::BOX_H * self.scale).round() as i32));
        self.pet.char_key = None;
        let _ = self.liquid.bump.sprite.SetCenterPoint(Vector3 {
            X: self.liquid.bump.size().0 as f32 / 2.0,
            Y: 0.0,
            Z: 0.0,
        });
        let ear_px = (EAR * self.scale).round() as i32;
        for ear in &mut self.ears {
            let _ = ear.resize((ear_px, ear_px));
        }
        self.paint_ears();
        self.update_offsets();
        self.repaint();
        self.start_liquid();
    }
}
