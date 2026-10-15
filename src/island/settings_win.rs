use std::ffi::c_void;
use std::sync::Once;

use windows::Win32::Graphics::Dwm::DwmSetWindowAttribute;
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BeginPaint, BitBlt, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS,
    DeleteDC, DeleteObject, EndPaint, GetDC, PAINTSTRUCT, ReleaseDC, SRCCOPY, SelectObject,
};
use windows::Win32::UI::HiDpi::AdjustWindowRectExForDpi;
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture, SetFocus};

use crate::island::*;

const MAX_SEARCH_CHARS: usize = 40;
const SCROLL_STEP: f32 = 64.0;

impl App {
    pub(in crate::island) fn open_settings_window(&mut self) {
        if let Some(hwnd) = self.settings_win {
            unsafe {
                let _ = ShowWindow(hwnd, SW_RESTORE);
                let _ = SetForegroundWindow(hwnd);
            }
            return;
        }
        static CLASS: Once = Once::new();
        let dpi = self.dpi_scale;
        let (cw, ch) = ((view::SW_SIZE.0 * dpi).round() as i32, (view::SW_SIZE.1 * dpi).round() as i32);
        let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX;
        unsafe {
            let Ok(instance) = GetModuleHandleW(None) else { return };
            CLASS.call_once(|| {
                let class = WNDCLASSW {
                    lpfnWndProc: Some(sw_proc),
                    hInstance: instance.into(),
                    lpszClassName: w!("NightIsland.Settings"),
                    hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                    ..Default::default()
                };
                RegisterClassW(&class);
            });
            let mut rc = windows::Win32::Foundation::RECT { left: 0, top: 0, right: cw, bottom: ch };
            let _ = AdjustWindowRectExForDpi(&mut rc, style, false, WINDOW_EX_STYLE(0), (dpi * 96.0) as u32);
            let (ow, oh) = (rc.right - rc.left, rc.bottom - rc.top);
            let (l, t, r, b) = Self::work_area();
            let x = (l + ((r - l) - ow as f32) / 2.0).max(l) as i32;
            let y = (t + ((b - t) - oh as f32) / 2.0).max(t) as i32;
            let Ok(hwnd) = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("NightIsland.Settings"),
                w!("Night Island"),
                style,
                x,
                y,
                ow,
                oh,
                None,
                None,
                Some(instance.into()),
                None,
            ) else {
                crate::log_error!("could not create the settings window");
                return;
            };
            let dark: i32 = 1;
            let _ = DwmSetWindowAttribute(
                hwnd,
                windows::Win32::Graphics::Dwm::DWMWINDOWATTRIBUTE(20),
                (&raw const dark).cast::<c_void>(),
                4,
            );
            let (pr, pg, pb) = crate::ui::theme::panel_u8();
            let caption = u32::from(pr) | (u32::from(pg) << 8) | (u32::from(pb) << 16);
            let _ = DwmSetWindowAttribute(
                hwnd,
                windows::Win32::Graphics::Dwm::DWMWINDOWATTRIBUTE(35),
                (&raw const caption).cast::<c_void>(),
                4,
            );
            self.settings_win = Some(hwnd);
            self.ui.sw = view::SwUi::default();
            self.ui.int.spotify_id.set_text(&spotify::own_client_id());
            spotify::reload_status();
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = SetForegroundWindow(hwnd);
            let _ = SetFocus(Some(hwnd));
        }
        self.render_settings_win();
    }

    fn sw_client(&self) -> Option<(i32, i32)> {
        let hwnd = self.settings_win?;
        let mut rc = windows::Win32::Foundation::RECT::default();
        unsafe { GetClientRect(hwnd, &mut rc).ok()? };
        Some((rc.right, rc.bottom))
    }

    fn sw_size(&self) -> (f32, f32) {
        self.sw_client().map_or(view::SW_SIZE, |(w, h)| (w as f32, h as f32))
    }

    pub(in crate::island) fn render_settings_win(&self) {
        let (Some(hwnd), Some((w, h))) = (self.settings_win, self.sw_client()) else { return };
        if w <= 0 || h <= 0 {
            return;
        }
        unsafe {
            let screen = GetDC(Some(hwnd));
            let mem = CreateCompatibleDC(Some(screen));
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: w,
                    biHeight: -h,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut bits = std::ptr::null_mut();
            if let Ok(dib) = CreateDIBSection(Some(mem), &info, DIB_RGB_COLORS, &mut bits, None, 0) {
                let old = SelectObject(mem, dib.into());
                let painted = self.gpu.paint_dc(mem, (w, h), |p| {
                    let _ = view::paint_settings_window(p, (w as f32, h as f32), self.dpi_scale, &self.ui);
                });
                if painted.is_ok() {
                    let _ = BitBlt(screen, 0, 0, w, h, Some(mem), 0, 0, SRCCOPY);
                }
                SelectObject(mem, old);
                let _ = DeleteObject(dib.into());
            }
            let _ = DeleteDC(mem);
            ReleaseDC(Some(hwnd), screen);
        }
    }

    pub(in crate::island) fn on_sw_move(&mut self, x: i32, y: i32, left_down: bool) {
        if left_down && self.ui.sw.drag.is_some() {
            return self.sw_drag(x);
        }
        let hit = view::sw_hit(self.sw_size(), self.dpi_scale, &self.ui, x as f32, y as f32);
        if hit != self.ui.sw.hover {
            self.ui.sw.hover = hit;
            self.render_settings_win();
        }
    }

    pub(in crate::island) fn on_sw_down(&mut self, x: i32, y: i32) {
        let hit = view::sw_hit(self.sw_size(), self.dpi_scale, &self.ui, x as f32, y as f32);
        if let Some(view::SwHit::Field(field)) = hit {
            self.sw_focus(Some(field));
            self.sw_click_field(field, x);
            return self.render_settings_win();
        }
        self.sw_focus(None);
        match hit {
            Some(view::SwHit::Group(i)) => {
                self.ui.sw.group = i;
                self.ui.sw.scroll = 0.0;
                self.ui.sw.search.clear();
            }
            Some(view::SwHit::Row(id, hit)) => self.sw_row(id, hit, x),
            Some(view::SwHit::Act(act)) => self.sw_act(act),
            Some(view::SwHit::Field(_)) | None => {}
        }
        self.render_settings_win();
    }

    fn sw_focus(&mut self, field: Option<view::Field>) {
        if self.ui.sw.field == field {
            return;
        }
        self.ui.sw.field = field;
        self.ui.sw.caret_on = true;
        if field.is_some() {
            self.set_timer(Timer::SwCaret, CARET_BLINK_MS);
        } else {
            self.kill_timer(Timer::SwCaret);
        }
    }

    pub(in crate::island) fn sw_caret_tick(&mut self) {
        if self.ui.sw.field.is_none() || self.settings_win.is_none() {
            return;
        }
        self.ui.sw.caret_on = !self.ui.sw.caret_on;
        self.set_timer(Timer::SwCaret, CARET_BLINK_MS);
        self.render_settings_win();
    }

    fn sw_edit_mut(&mut self) -> Option<&mut edit::TextEdit> {
        Some(match self.ui.sw.field? {
            view::Field::DiscordId => &mut self.ui.int.id,
            view::Field::DiscordSecret => &mut self.ui.int.secret,
            view::Field::SpotifyId => &mut self.ui.int.spotify_id,
        })
    }

    fn sw_click_field(&mut self, field: view::Field, x: i32) {
        let Some((_, inner)) = view::sw_field_rects(self.sw_size(), self.dpi_scale, &self.ui, field) else { return };
        let shown = {
            let text = view::field_edit(&self.ui, field).text();
            if field == view::Field::DiscordSecret { "\u{2022}".repeat(text.chars().count()) } else { text }
        };
        let Ok(layout) = crate::ui::gfx::make_layout(self.gpu.dwrite(), &shown, 100_000.0, 13.0 * self.dpi_scale)
        else {
            return;
        };
        let shift = unsafe { GetKeyState(VK_SHIFT.0 as i32) } < 0;
        if let Some(n) = self.sw_edit_mut() {
            n.click(&layout, x as f32 - inner.0, 4.0, shift);
        }
    }

    fn sw_act(&mut self, act: view::Act) {
        match act {
            view::Act::DiscordMain => match self.ui.discord.status {
                discord::Status::NoConfig => self.save_credentials(),
                discord::Status::Idle | discord::Status::Error(_) => discord::connect(),
                discord::Status::NotRunning | discord::Status::Connected => discord::disconnect(),
                discord::Status::Authorizing => {}
            },
            view::Act::DiscordMute => discord::set_mute(!self.ui.discord.self_mute),
            view::Act::DiscordDeaf => discord::set_deaf(!self.ui.discord.self_deaf),
            view::Act::DiscordOverlay => self.toggle_overlay(),
            view::Act::SpotifyMain => match self.ui.spotify.status {
                spotify::Status::Connected => spotify::disconnect(),
                spotify::Status::Connecting => {}
                _ => {
                    spotify::reload_status();
                    spotify::connect();
                }
            },
            view::Act::SpotifySaveId => {
                if spotify::set_client_id(&self.ui.int.spotify_id.text()) {
                    self.notify(Notice::Integrations, '\u{E73E}', "Client ID saved");
                } else {
                    self.notify(Notice::Error, '\u{E783}', "That does not look like a Client ID");
                }
            }
        }
    }

    fn sw_row(&mut self, id: view::Sid, hit: view::SetHit, x: i32) {
        match hit {
            view::SetHit::Option(n) => {
                if matches!(id, view::Sid::Character | view::Sid::Edge | view::Sid::PillBattery) {
                    view::cycle_set(id, &mut self.settings, n == 1);
                } else {
                    view::segment_set(id, &mut self.settings, n);
                }
                self.setting_changed(id, true);
            }
            view::SetHit::Toggle => {
                let on = !view::toggle_get(id, &self.settings);
                view::toggle_set(id, &mut self.settings, on);
                self.setting_changed(id, true);
            }
            view::SetHit::Slider => {
                self.pointer.slider = Some(id);
                self.ui.sw.drag = Some(id);
                self.sw_drag(x);
                unsafe { SetCapture(self.settings_win.unwrap_or_default()) };
            }
            view::SetHit::Button => {
                if id == view::Sid::OpenLogs
                    && let Some(dir) = crate::data::log::dir()
                {
                    let _ = std::fs::create_dir_all(&dir);
                    cmd::shell_open(&dir.to_string_lossy(), None);
                }
            }
        }
    }

    fn sw_drag(&mut self, x: i32) {
        let Some(id) = self.ui.sw.drag else { return };
        let Some((left, right)) = view::sw_slider_track(self.sw_size(), self.dpi_scale, &self.ui, id) else { return };
        let norm = (x as f32 - left) / (right - left).max(1.0);
        view::slider_apply(id, &mut self.settings, norm);
        self.setting_changed(id, id != view::Sid::Size);
        self.render_settings_win();
    }

    pub(in crate::island) fn on_sw_up(&mut self) {
        unsafe {
            let _ = ReleaseCapture();
        }
        if self.ui.sw.drag.take().is_some() {
            self.finish_slider();
            self.render_settings_win();
        }
    }

    pub(in crate::island) fn on_sw_wheel(&mut self, delta: i32) {
        let max = view::max_scroll(self.sw_size(), self.dpi_scale, &self.ui);
        let scroll = (self.ui.sw.scroll - delta as f32 / 120.0 * SCROLL_STEP * self.dpi_scale).clamp(0.0, max);
        if scroll != self.ui.sw.scroll {
            self.ui.sw.scroll = scroll;
            self.render_settings_win();
        }
    }

    pub(in crate::island) fn on_sw_char(&mut self, c: u16) -> bool {
        if self.ui.sw.field.is_some() {
            self.on_sw_field_char(c);
            return false;
        }
        let search = &mut self.ui.sw.search;
        match c {
            0x1B => {
                if search.is_empty() {
                    return true;
                }
                search.clear();
            }
            0x08 => {
                search.pop();
            }
            0x16 => {
                if let Some(text) = crate::data::clips::sys::read_text(self.settings_win.unwrap_or(self.hwnd)) {
                    let line: String = text.lines().next().unwrap_or("").chars().filter(|c| !c.is_control()).collect();
                    search.extend(line.chars().take(MAX_SEARCH_CHARS.saturating_sub(search.chars().count())));
                }
            }
            c if c >= 0x20 && c != 0x7F => {
                if search.chars().count() < MAX_SEARCH_CHARS
                    && let Some(ch) = char::from_u32(u32::from(c))
                {
                    search.push(ch);
                }
            }
            _ => return false,
        }
        self.ui.sw.scroll = 0.0;
        self.ui.sw.hover = None;
        self.render_settings_win();
        false
    }

    fn on_sw_field_char(&mut self, unit: u16) {
        const MAX_FIELD_CHARS: usize = 200;
        let Some(field) = self.ui.sw.field else { return };
        let hwnd = self.settings_win.unwrap_or(self.hwnd);
        let ctrl = unsafe { GetKeyState(VK_CONTROL.0 as i32) } < 0;
        match unit {
            0x1B => self.sw_focus(None),
            0x09 => {
                let next = view::sw_next_field(&self.ui, field);
                self.sw_focus(next);
            }
            0x0D => {
                let act =
                    if field == view::Field::SpotifyId { view::Act::SpotifySaveId } else { view::Act::DiscordMain };
                self.sw_act(act);
            }
            0x08 => {
                if let Some(n) = self.sw_edit_mut() {
                    n.backspace(ctrl);
                }
            }
            0x01 => {
                if let Some(n) = self.sw_edit_mut() {
                    n.select_all();
                }
            }
            0x03 | 0x18 => {
                if field != view::Field::DiscordSecret
                    && let Some(n) = self.sw_edit_mut()
                    && let Some(text) = n.selected_text()
                {
                    crate::data::clips::sys::write_text(hwnd, &text);
                    if unit == 0x18 {
                        n.delete_selection();
                    }
                }
            }
            0x16 => {
                if let Some(text) = crate::data::clips::sys::read_text(hwnd)
                    && let Some(n) = self.sw_edit_mut()
                {
                    let line: String = text.lines().next().unwrap_or("").trim().chars().take(MAX_FIELD_CHARS).collect();
                    n.insert(&line);
                }
            }
            0xD800..=0xDBFF => {
                self.pending_high_surrogate = Some(unit);
                return;
            }
            u if u < 0x20 || u == 0x7F => return,
            u => {
                let c = if (0xDC00..=0xDFFF).contains(&u) {
                    let Some(high) = self.pending_high_surrogate.take() else { return };
                    char::decode_utf16([high, u]).next().and_then(|r| r.ok())
                } else {
                    char::from_u32(u32::from(u))
                };
                if let (Some(c), Some(n)) = (c, self.sw_edit_mut())
                    && n.text().chars().count() < MAX_FIELD_CHARS
                {
                    let mut buf = [0u8; 4];
                    n.insert(c.encode_utf8(&mut buf));
                }
            }
        }
        self.ui.sw.caret_on = true;
        if self.ui.sw.field.is_some() {
            self.set_timer(Timer::SwCaret, CARET_BLINK_MS);
        }
        self.render_settings_win();
    }

    pub(in crate::island) fn on_sw_keydown(&mut self, vk: u16) {
        if self.ui.sw.field.is_none() {
            return;
        }
        let down = |k: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY| unsafe { GetKeyState(k.0 as i32) < 0 };
        let (ctrl, shift) = (down(VK_CONTROL), down(VK_SHIFT));
        let Some(n) = self.sw_edit_mut() else { return };
        match vk {
            v if v == VK_LEFT.0 => n.left(shift, ctrl),
            v if v == VK_RIGHT.0 => n.right(shift, ctrl),
            v if v == VK_HOME.0 => n.caret_to_start(shift),
            v if v == VK_END.0 => n.caret_to_end(shift),
            v if v == VK_DELETE.0 => n.delete(ctrl),
            _ => return,
        }
        self.ui.sw.caret_on = true;
        self.render_settings_win();
    }

    pub(in crate::island) fn on_sw_destroyed(&mut self) {
        self.kill_timer(Timer::SwCaret);
        self.settings_win = None;
        self.pointer.slider = None;
        self.ui.sw = view::SwUi::default();
    }
}

extern "system" fn sw_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_ERASEBKGND => LRESULT(1),
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            unsafe {
                BeginPaint(hwnd, &mut ps);
                let _ = EndPaint(hwnd, &ps);
            }
            with_app(|a| a.render_settings_win());
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            let (x, y) = point_from(lparam);
            let left_down = wparam.0 & 0x0001 != 0;
            with_app(|a| a.on_sw_move(x, y, left_down));
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let (x, y) = point_from(lparam);
            with_app(|a| a.on_sw_down(x, y));
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            with_app(|a| a.on_sw_up());
            LRESULT(0)
        }
        WM_MOUSEWHEEL => {
            let delta = (wparam.0 >> 16) as i16 as i32;
            with_app(|a| a.on_sw_wheel(delta));
            LRESULT(0)
        }
        WM_KEYDOWN => {
            let vk = wparam.0 as u16;
            with_app(|a| a.on_sw_keydown(vk));
            LRESULT(0)
        }
        WM_CHAR => {
            let mut close = false;
            with_app(|a| close = a.on_sw_char(wparam.0 as u16));
            if close {
                unsafe {
                    let _ = DestroyWindow(hwnd);
                }
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            with_app(|a| a.on_sw_destroyed());
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
