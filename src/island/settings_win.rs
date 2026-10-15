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
        let Some(hit) = view::sw_hit(self.sw_size(), self.dpi_scale, &self.ui, x as f32, y as f32) else { return };
        match hit {
            view::SwHit::Group(i) => {
                self.ui.sw.group = i;
                self.ui.sw.scroll = 0.0;
                self.ui.sw.search.clear();
            }
            view::SwHit::Row(id, hit) => self.sw_row(id, hit, x),
        }
        self.render_settings_win();
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
            view::SetHit::Button => match id {
                view::Sid::OpenIntegrations => self.open_tab(view::Tab::Integrations),
                view::Sid::OpenLogs => {
                    if let Some(dir) = crate::data::log::dir() {
                        let _ = std::fs::create_dir_all(&dir);
                        cmd::shell_open(&dir.to_string_lossy(), None);
                    }
                }
                _ => {}
            },
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

    pub(in crate::island) fn on_sw_destroyed(&mut self) {
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
