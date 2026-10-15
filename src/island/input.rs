use crate::island::*;

#[derive(Default)]
pub(in crate::island) struct Pointer {
    pub(in crate::island) tracking: bool,
    pub(in crate::island) press: Option<((i32, i32), (i32, i32))>,
    pub(in crate::island) moving: Option<((i32, i32), (i32, i32))>,
    pub(in crate::island) drag_from: Option<(usize, i32, i32)>,
    pub(in crate::island) selecting: bool,
    pub(in crate::island) slider: Option<view::Sid>,
    pub(in crate::island) vol_drag: bool,
    pub(in crate::island) seek_drag: bool,
}

impl App {
    pub(in crate::island) fn mouse_down(&mut self, x: i32, y: i32) {
        if self.state == State::Collapsed && self.ui.notice.is_none() && self.on_pill(x, y) {
            let mut pt = windows::Win32::Foundation::POINT::default();
            unsafe {
                let _ = GetCursorPos(&mut pt);
            }
            self.pointer.press = Some(((pt.x, pt.y), (x, y)));
            return;
        }
        if self.state == State::Expanded && !self.ui.cmd.open && self.ui.remind.alert.is_none() && !self.ui.editing {
            let pill = self.pill_rect(true);
            let (fx, fy) = (x as f32, y as f32);
            let in_header = fy >= pill.1 + 6.0 * self.scale
                && fy < pill.1 + 52.0 * self.scale
                && fx >= pill.0
                && fx < pill.0 + pill.2;
            if in_header
                && view::hit_tab(pill, self.scale, fx, fy, &self.ui).is_none()
                && !view::hit_pin(pill, self.scale, fx, fy)
            {
                let mut pt = windows::Win32::Foundation::POINT::default();
                unsafe {
                    let _ = GetCursorPos(&mut pt);
                }
                self.pointer.press = Some(((pt.x, pt.y), (x, y)));
                return;
            }
        }
        self.click_at(x, y);
    }

    pub(in crate::island) fn drag_island(&mut self) {
        let Some((start_cursor, _)) = self.pointer.press else { return };
        let mut pt = windows::Win32::Foundation::POINT::default();
        unsafe {
            let _ = GetCursorPos(&mut pt);
        }
        if self.pointer.moving.is_none() {
            let far = (pt.x - start_cursor.0).abs().max((pt.y - start_cursor.1).abs()) > (6.0 * self.scale) as i32;
            if !far {
                return;
            }
            let origin = self.window_origin(Win::Small);
            self.pointer.moving = Some(((origin.0.round() as i32, origin.1.round() as i32), start_cursor));
            self.kill_timer(Timer::Expand);
            self.kill_timer(Timer::Liquid);
            self.set_bump_visible(false);
            unsafe {
                windows::Win32::UI::Input::KeyboardAndMouse::SetCapture(self.hwnd);
            }
        }
        if let Some((origin, from)) = self.pointer.moving {
            unsafe {
                let _ = SetWindowPos(
                    self.hwnd,
                    None,
                    origin.0 + pt.x - from.0,
                    origin.1 + pt.y - from.1,
                    0,
                    0,
                    SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOZORDER,
                );
            }
        }
    }

    pub(in crate::island) fn mouse_up(&mut self) {
        let press = self.pointer.press.take();
        if self.pointer.moving.take().is_none() {
            if let Some((_, (x, y))) = press {
                self.click_at(x, y);
            }
            return;
        }
        unsafe {
            let _ = windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture();
        }
        let mut rc = windows::Win32::Foundation::RECT::default();
        unsafe {
            let _ = GetWindowRect(self.hwnd, &mut rc);
        }
        let s = self.scale;
        let (sw, sh) = Self::screen_size();
        let (wl, wt, wr, wb) = Self::work_area();
        let (cw, ch) = if self.state == State::Expanded {
            (EXPANDED.0 * s, EXPANDED.1 * s)
        } else {
            (self.cw * s, COLLAPSED.1 * s)
        };
        let (px, py) = (rc.left as f32 + PAD * s, rc.top as f32 + self.top_margin() * s);
        let (mid_x, mid_y) = (px + cw / 2.0, py + ch / 2.0);
        if self.docked() {
            let near = [
                (py - wt, Edge::Top),
                (wb - (py + ch), Edge::Bottom),
                (px - wl, Edge::Left),
                (wr - (px + cw), Edge::Right),
            ]
            .into_iter()
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map_or(Edge::Top, |e| e.1);
            self.settings.dock_edge = near;
            self.settings.dock_offset = match near {
                Edge::Top | Edge::Bottom => (mid_x / sw).clamp(0.0, 1.0),
                Edge::Left | Edge::Right => (mid_y / sh).clamp(0.0, 1.0),
            };
        } else {
            let near = [
                (py - wt, Edge::Top),
                (wb - (py + ch), Edge::Bottom),
                (px - wl, Edge::Left),
                (wr - (px + cw), Edge::Right),
            ]
            .into_iter()
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .filter(|e| e.0 < 36.0 * s);
            if let Some((_, edge)) = near {
                self.settings.placement = Placement::Docked;
                self.settings.dock_edge = edge;
                self.settings.dock_offset = match edge {
                    Edge::Top | Edge::Bottom => (mid_x / sw).clamp(0.0, 1.0),
                    Edge::Left | Edge::Right => (mid_y / sh).clamp(0.0, 1.0),
                };
            } else {
                self.settings.float_pos = Some([mid_x.round() as i32, py.round() as i32]);
            }
        }
        self.ui.cfg = self.settings.clone();
        self.settings.save();
        self.apply_layout();
        if self.state == State::Collapsed {
            self.start_liquid();
        }
    }

    pub(in crate::island) fn hover_at(&mut self, x: i32, y: i32) {
        if self.state != State::Expanded {
            return;
        }
        let pill = self.pill_rect(true);
        let (fx, fy) = (x as f32, y as f32);
        if self.ui.cmd.open {
            if let Some(i) = view::hit_cmd(pill, self.scale, fx, fy, self.ui.cmd.items.len())
                && i != self.ui.cmd.sel
            {
                self.ui.cmd.sel = i;
                self.repaint_expanded();
            }
            return;
        }
        let hover_tab = view::hit_tab(pill, self.scale, fx, fy, &self.ui);
        let hover_clip = match self.ui.tab {
            view::Tab::Clipboard => {
                let visible = self.clipboard.clips.len().saturating_sub(self.ui.clip.scroll);
                view::hit_clip(pill, self.scale, fx, fy, visible)
            }
            view::Tab::Shelf => {
                let visible = self.shelf.len().saturating_sub(self.ui.shelf_scroll);
                view::hit_clip(pill, self.scale, fx, fy, visible)
            }
            _ => None,
        };
        let hover_ctl = if self.ui.tab == view::Tab::Controls { view::hit_ctl(pill, self.scale, fx, fy) } else { None };
        let hover_rem = if self.ui.tab == view::Tab::Note {
            view::hit_remind(pill, self.scale, fx, fy, self.ui.remind.list.len())
        } else {
            None
        };
        let hover_text = if self.ui.tab == view::Tab::Text { view::hit_text(pill, self.scale, fx, fy) } else { None };
        let hover_act = if self.ui.tab == view::Tab::Home && self.ui.home.dev_menu.is_some() {
            let count = self.ui.home.dev_menu.as_ref().map_or(0, |m| m.0.len());
            match view::hit_menu(pill, self.scale, fx, fy, count) {
                Some(Some(i)) => Some(300 + i),
                _ => None,
            }
        } else if self.ui.tab == view::Tab::Agy {
            match view::hit_agy(
                pill,
                self.scale,
                fx,
                fy,
                self.ui.agy.chats.len(),
                self.ui.agy.scroll,
                self.ui.agy.open,
                self.ui.agy.input_h.get(),
            ) {
                Some(view::AgyHit::Row(i)) => Some(1000 + i),
                Some(view::AgyHit::Send) => Some(1100),
                Some(view::AgyHit::Back) => Some(1101),
                _ => None,
            }
        } else if self.ui.tab == view::Tab::Weather {
            match view::hit_city_match(pill, self.scale, fx, fy, self.ui.city.matches.len(), self.ui.weather.is_some())
            {
                Some(i) => Some(910 + i),
                None => view::hit_city(pill, self.scale, fx, fy, self.ui.weather.is_some()).then_some(900),
            }
        } else if self.ui.tab == view::Tab::Theme {
            view::hit_theme(pill, self.scale, fx, fy).map(|h| match h {
                view::ThemeHit::Preset(i) => 800 + i,
                view::ThemeHit::Panel(i) => 820 + i,
                view::ThemeHit::Accent(i) => 840 + i,
            })
        } else if self.ui.tab == view::Tab::Integrations {
            match view::hit_int(pill, self.scale, fx, fy, self.ui.discord.status == discord::Status::Connected) {
                Some(view::IntHit::Discord) => Some(400),
                Some(view::IntHit::Spotify) => Some(401),
                Some(view::IntHit::DiscordMute) => Some(402),
                Some(view::IntHit::DiscordDeaf) => Some(403),
                Some(view::IntHit::DiscordOverlay) => Some(404),
                None => None,
            }
        } else if self.ui.tab == view::Tab::Home {
            let has_media = !self.ui.media.title.is_empty();
            match view::hit_home(
                pill,
                self.scale,
                fx,
                fy,
                has_media,
                view::spotify_active(&self.ui),
                view::discord_strip(&self.ui),
                self.ui.home.dock.len(),
            ) {
                Some(view::HomeHit::Action(i)) => Some(i),
                Some(view::HomeHit::Device) => Some(100),
                Some(view::HomeHit::Media(i)) => Some(500 + i),
                Some(view::HomeHit::Extra(i)) => Some(510 + i),
                Some(view::HomeHit::Seek(_)) => Some(520),
                Some(view::HomeHit::DiscordDeaf) => Some(540),
                Some(view::HomeHit::DiscordMute) => Some(541),
                Some(view::HomeHit::DiscordOverlay) => Some(542),
                Some(view::HomeHit::Dock(i)) => Some(600 + i),
                Some(view::HomeHit::DockAdd) => Some(700),
                Some(view::HomeHit::VolumeIcon) => Some(530),
                _ => None,
            }
        } else {
            None
        };
        let hover_pin = view::hit_pin(pill, self.scale, fx, fy);
        if hover_act != self.ui.home.hover_act || hover_pin != self.ui.hover_pin {
            self.ui.home.hover_act = hover_act;
            self.ui.hover_pin = hover_pin;
            self.repaint_expanded();
        }
        if hover_tab != self.ui.hover_tab
            || hover_clip != self.ui.clip.hover
            || hover_ctl != self.ui.hover_ctl
            || hover_text != self.ui.text.hover
            || hover_rem != self.ui.remind.hover
        {
            self.ui.remind.hover = hover_rem;
            self.ui.text.hover = hover_text;
            self.ui.hover_tab = hover_tab;
            self.ui.clip.hover = hover_clip;
            self.ui.hover_ctl = hover_ctl;
            self.repaint_expanded();
        }
    }

    pub(in crate::island) fn click_at(&mut self, x: i32, y: i32) {
        if self.state == State::Collapsed {
            return self.click_collapsed(x, y);
        }
        if self.state != State::Expanded {
            return;
        }
        if self.ui.remind.alert.is_some() {
            return self.click_alert(x, y);
        }
        if self.ui.cmd.open {
            return self.click_cmd(x, y);
        }
        let pill = self.pill_rect(true);
        if view::hit_pin(pill, self.scale, x as f32, y as f32) {
            self.ui.pinned = !self.ui.pinned;
            if self.ui.pinned {
                self.kill_timer(Timer::Collapse);
            }
            self.repaint_expanded();
            return;
        }
        if let Some(i) = view::hit_tab(pill, self.scale, x as f32, y as f32, &self.ui) {
            if view::TABS[i].0 == view::Tab::Settings {
                return self.open_settings_window();
            }
            if self.ui.tab != view::TABS[i].0 {
                self.ui.clip.hover = None;
                self.ui.hover_ctl = None;
                self.disarm();
                self.select_tab(view::TABS[i].0);
            }
            return;
        }
        match self.ui.tab {
            view::Tab::Clipboard => self.click_clip(x, y),
            view::Tab::Controls => self.click_ctl(x, y),
            view::Tab::Shelf => self.click_shelf(x, y),
            view::Tab::Note => self.click_note(x, y),
            view::Tab::Text => self.click_text(x, y),
            view::Tab::Record => self.click_record(x, y),
            view::Tab::Home => self.click_home(x, y),
            view::Tab::Weather => self.click_weather(x, y),
            view::Tab::Agy => self.click_agy(x, y),
            view::Tab::Integrations => self.click_integrations(x, y),
            view::Tab::Theme => self.click_theme(x, y),
            view::Tab::Settings => {}
        }
    }

    pub(in crate::island) fn wheel(&mut self, delta: i32) {
        if self.state == State::Collapsed {
            return self.adjust_volume(delta);
        }
        if self.state != State::Expanded || self.ui.cmd.open {
            return;
        }
        if let Some((items, scroll)) = &mut self.ui.home.dev_menu {
            let max = items.len().saturating_sub(view::menu_visible(items.len()));
            *scroll = if delta > 0 { scroll.saturating_sub(1) } else { (*scroll + 1).min(max) };
            return self.repaint_expanded();
        }
        if self.ui.tab == view::Tab::Agy && self.ui.agy.open {
            let step = 54.0 * self.scale * if delta > 0 { 1.0 } else { -1.0 };
            self.ui.agy.hist = (self.ui.agy.hist + step).clamp(0.0, self.ui.agy.max.get());
            return self.repaint_expanded();
        }
        let rows = if self.ui.tab == view::Tab::Agy {
            view::agy_rows()
        } else {
            view::clip_rows(self.pill_rect(true), self.scale)
        };
        if matches!(self.ui.tab, view::Tab::Note | view::Tab::Text) {
            let Some(layout) = self.note_layout() else { return };
            let inner = self.note_inner();
            let max = (edit::content_height(&layout) - (inner.3 - inner.1)).max(0.0);
            let step = 42.0 * self.scale * if delta > 0 { -1.0 } else { 1.0 };
            let next = (self.multi().scroll + step).clamp(0.0, max);
            if next != self.multi().scroll {
                self.multi_mut().scroll = next;
                self.repaint_expanded();
            }
            return;
        }
        let (len, scroll) = match self.ui.tab {
            view::Tab::Clipboard => (self.clipboard.clips.len(), &mut self.ui.clip.scroll),
            view::Tab::Shelf => (self.shelf.len(), &mut self.ui.shelf_scroll),
            view::Tab::Agy => (self.ui.agy.chats.len(), &mut self.ui.agy.scroll),
            _ => return,
        };
        let max = len.saturating_sub(rows);
        let next = if delta > 0 { scroll.saturating_sub(1) } else { (*scroll + 1).min(max) };
        if next != *scroll {
            *scroll = next;
            self.ui.clip.hover = None;
            self.repaint_expanded();
        }
    }

    pub(in crate::island) fn on_pill(&self, x: i32, y: i32) -> bool {
        let s = self.scale;
        let (win_w, _) = self.window_size(self.win);
        let (w, h) = (self.cw * s, COLLAPSED.1 * s);
        let (left, top) = ((win_w as f32 - w) / 2.0, self.top_margin() * s);
        let tol = 3.0 * s;
        let (fx, fy) = (x as f32, y as f32);
        fx >= left - tol && fx < left + w + tol && fy >= top - tol - 40.0 && fy < top + h + tol
    }

    pub(in crate::island) fn on_mouse_move(&mut self, x: i32, y: i32) {
        unsafe {
            if !self.pointer.tracking {
                self.pointer.tracking = true;
                let mut tme = TRACKMOUSEEVENT {
                    cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: self.hwnd,
                    dwHoverTime: 0,
                };
                let _ = TrackMouseEvent(&mut tme);
            }
        }
        self.kill_timer(Timer::Collapse);
        self.kill_timer(Timer::Autohide);
        if self.state != State::Expanded {
            let hidden = matches!(self.state, State::Hidden | State::Hiding);
            if hidden || self.on_pill(x, y) {
                self.set_timer(Timer::Expand, self.settings.open_delay_ms.max(1));
            } else {
                self.kill_timer(Timer::Expand);
            }
        }
    }

    pub(in crate::island) fn on_mouse_leave(&mut self) {
        self.pointer.tracking = false;
        let had_hover = self.ui.hover_tab.take().is_some()
            | self.ui.clip.hover.take().is_some()
            | self.ui.hover_ctl.take().is_some()
            | self.ui.text.hover.take().is_some()
            | self.ui.remind.hover.take().is_some()
            | self.ui.home.hover_act.take().is_some()
            | std::mem::take(&mut self.ui.hover_pin);
        if had_hover {
            self.repaint_expanded();
        }
        self.kill_timer(Timer::Expand);
        match self.state {
            State::Expanded
                if self.ui.editing
                    || self.ui.remind.alert.is_some()
                    || self.ui.pinned
                    || self.popup_open
                    || self.pointer.moving.is_some() => {}
            State::Expanded => self.set_timer(Timer::Collapse, self.settings.close_delay_ms.max(1)),
            State::Collapsed => self.schedule_autohide(),
            _ => {}
        }
    }
}
