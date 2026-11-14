use std::cell::RefCell;
use std::sync::Once;
use std::time::Instant;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetCapture, ReleaseCapture, SetCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{Result, w};

use super::companion::{Action, WM_COMPANION};
use super::gfx::Gpu;
use super::popup::PopupSurface;
use crate::platform::media::Media;
use crate::view::{self, COMPANION_PLAYER_SIZE};

const WM_MOUSELEAVE: u32 = 0x02A3;

fn button_at(x: f32, y: f32, scale: f32) -> Option<Action> {
    [Action::Previous, Action::PlayPause, Action::Next].into_iter().enumerate().find_map(|(i, action)| {
        let r = view::companion_player_button(i, scale);
        (x >= r.0 && x < r.2 && y >= r.1 && y < r.3).then_some(action)
    })
}

struct State {
    parent: HWND,
    source: HWND,
    scale: f32,
    pressed: Option<Action>,
    hover: Option<Action>,
    gpu: Gpu,
    surface: PopupSurface,
    size: (i32, i32),
    media: Media,
    progress: Option<(f32, f32)>,
    opened: Option<Instant>,
}

impl State {
    fn render(&self, hwnd: HWND) -> Result<()> {
        let index =
            |action| [Action::Previous, Action::PlayPause, Action::Next].into_iter().position(|a| Some(a) == action);
        self.surface.paint(&self.gpu, |p| {
            view::paint_companion_player(
                p,
                self.scale,
                &self.media,
                self.progress,
                index(self.hover),
                index(self.pressed),
            )
        })?;
        let alpha = self.opened.map_or(1.0, |at| {
            let t = (at.elapsed().as_secs_f32() / 0.16).clamp(0.0, 1.0);
            1.0 - (1.0 - t).powi(3)
        });
        self.surface.present(hwnd, None, (alpha * 255.0).round() as u8)
    }

    fn redraw(&self, hwnd: HWND) {
        if let Err(error) = self.render(hwnd) {
            crate::log_warn!("could not draw companion player: {error}");
        }
    }
}

extern "system" fn proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    unsafe {
        if msg == WM_MOUSEACTIVATE {
            return LRESULT(MA_NOACTIVATE as isize);
        }
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut RefCell<State>;
        if ptr.is_null() {
            return DefWindowProcW(hwnd, msg, wp, lp);
        }
        let Ok(mut state) = (*ptr).try_borrow_mut() else {
            return DefWindowProcW(hwnd, msg, wp, lp);
        };
        let point = || (lp.0 as i16 as f32, (lp.0 >> 16) as i16 as f32);
        match msg {
            WM_LBUTTONDOWN => {
                let (x, y) = point();
                state.pressed = button_at(x, y, state.scale);
                SetCapture(hwnd);
                state.redraw(hwnd);
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                let pressed = state.pressed.take();
                let _ = ReleaseCapture();
                let (x, y) = point();
                if let Some(action) = pressed.filter(|action| Some(*action) == button_at(x, y, state.scale)) {
                    let _ = PostMessageW(
                        Some(state.parent),
                        WM_COMPANION,
                        WPARAM(action as usize),
                        LPARAM(state.source.0 as isize),
                    );
                }
                state.redraw(hwnd);
                LRESULT(0)
            }
            WM_MOUSEMOVE => {
                let (x, y) = point();
                let hover = button_at(x, y, state.scale);
                if state.hover != hover {
                    state.hover = hover;
                    state.redraw(hwnd);
                }
                let mut track = TRACKMOUSEEVENT {
                    cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                let _ = TrackMouseEvent(&mut track);
                LRESULT(0)
            }
            WM_MOUSELEAVE => {
                state.hover = None;
                state.redraw(hwnd);
                LRESULT(0)
            }
            WM_CAPTURECHANGED => {
                state.pressed = None;
                state.redraw(hwnd);
                LRESULT(0)
            }
            WM_TIMER => {
                if state.opened.is_some_and(|at| at.elapsed().as_secs_f32() >= 0.16) {
                    state.opened = None;
                    let _ = KillTimer(Some(hwnd), 1);
                }
                state.redraw(hwnd);
                LRESULT(0)
            }
            WM_NCHITTEST => {
                let mut rect = RECT::default();
                let _ = GetWindowRect(hwnd, &mut rect);
                let (x, y) = point();
                LRESULT(
                    if super::popup::hit_panel(
                        (x - rect.left as f32, y - rect.top as f32),
                        (state.size.0 as f32, state.size.1 as f32),
                        state.scale,
                    ) {
                        HTCLIENT as isize
                    } else {
                        HTTRANSPARENT as isize
                    },
                )
            }
            WM_CLOSE => {
                let _ = ShowWindow(hwnd, SW_HIDE);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wp, lp),
        }
    }
}

pub(super) struct Controls {
    hwnd: HWND,
    state: Box<RefCell<State>>,
}

impl Controls {
    pub(super) fn new(parent: HWND, source: HWND, scale: f32, gpu: &Gpu) -> Result<Self> {
        static CLASS: Once = Once::new();
        unsafe {
            let instance = GetModuleHandleW(None)?;
            CLASS.call_once(|| {
                RegisterClassW(&WNDCLASSW {
                    lpfnWndProc: Some(proc),
                    hInstance: instance.into(),
                    hCursor: LoadCursorW(None, IDC_HAND).unwrap_or_default(),
                    lpszClassName: w!("NightIsland.CompanionControls"),
                    ..Default::default()
                });
            });
            let size =
                ((COMPANION_PLAYER_SIZE.0 * scale).round() as i32, (COMPANION_PLAYER_SIZE.1 * scale).round() as i32);
            let mut state = Box::new(RefCell::new(State {
                parent,
                source,
                scale,
                pressed: None,
                hover: None,
                gpu: gpu.popup(),
                surface: PopupSurface::new(size)?,
                size,
                media: Media::default(),
                progress: None,
                opened: None,
            }));
            let hwnd = CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST,
                w!("NightIsland.CompanionControls"),
                w!("Music controls"),
                WS_POPUP,
                0,
                0,
                size.0,
                size.1,
                Some(source),
                None,
                Some(instance.into()),
                None,
            )?;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, (&raw mut *state) as isize);
            Ok(Self { hwnd, state })
        }
    }

    pub(super) fn hwnd(&self) -> HWND {
        self.hwnd
    }

    pub(super) fn hide(&mut self) {
        let mut state = self.state.borrow_mut();
        state.pressed = None;
        state.hover = None;
        state.opened = None;
        unsafe {
            if GetCapture() == self.hwnd {
                let _ = ReleaseCapture();
            }
            let _ = KillTimer(Some(self.hwnd), 1);
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    pub(super) fn rect_at(&self, character: RECT, work: RECT, scale: f32) -> RECT {
        let w = (COMPANION_PLAYER_SIZE.0 * scale).round() as i32;
        let h = (COMPANION_PLAYER_SIZE.1 * scale).round() as i32;
        let x = ((character.left + character.right - w) / 2).clamp(work.left, (work.right - w).max(work.left));
        let below = character.bottom - (4.0 * scale) as i32;
        let y = if below + h <= work.bottom { below } else { character.top - h + (4.0 * scale) as i32 };
        let y = y.clamp(work.top, (work.bottom - h).max(work.top));
        RECT { left: x, top: y, right: x + w, bottom: y + h }
    }

    pub(super) fn show(
        &mut self,
        rect: RECT,
        cursor: POINT,
        media: &Media,
        progress: Option<(f32, f32)>,
        scale: f32,
    ) -> Result<()> {
        let mut state = self.state.borrow_mut();
        let size = (rect.right - rect.left, rect.bottom - rect.top);
        if state.size != size {
            state.surface = PopupSurface::new(size)?;
            state.size = size;
        }
        state.scale = scale;
        state.media = media.clone();
        state.progress = progress;
        state.hover = button_at((cursor.x - rect.left) as f32, (cursor.y - rect.top) as f32, scale);
        unsafe {
            if !IsWindowVisible(self.hwnd).as_bool() {
                state.opened = Some(Instant::now());
                SetTimer(Some(self.hwnd), 1, 16, None);
            }
            let _ = SetWindowPos(self.hwnd, None, rect.left, rect.top, size.0, size.1, SWP_NOACTIVATE | SWP_NOZORDER);
            state.render(self.hwnd)?;
            let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
        }
        Ok(())
    }

    pub(super) fn set_capture_hidden(&self, hidden: bool) {
        unsafe {
            let _ = SetWindowDisplayAffinity(self.hwnd, if hidden { WDA_EXCLUDEFROMCAPTURE } else { WDA_NONE });
        }
    }

    #[cfg(test)]
    pub(super) fn preview(&self) -> Vec<u8> {
        self.state.borrow().surface.pixels()
    }

    #[cfg(test)]
    pub(super) fn verify_actions(&self, parent: HWND) {
        unsafe {
            let scale = self.state.borrow().scale;
            let source = self.state.borrow().source;
            for (i, action) in [Action::Previous, Action::PlayPause, Action::Next].into_iter().enumerate() {
                let r = view::companion_player_button(i, scale);
                let lp = LPARAM(((((r.1 + r.3) / 2.0) as isize) << 16) | ((r.0 + r.2) / 2.0) as isize);
                proc(self.hwnd, WM_LBUTTONDOWN, WPARAM(0), lp);
                proc(self.hwnd, WM_LBUTTONUP, WPARAM(0), lp);
                let mut msg = MSG::default();
                assert!(PeekMessageW(&mut msg, Some(parent), WM_COMPANION, WM_COMPANION, PM_REMOVE).as_bool());
                assert_eq!(msg.wParam.0, action as usize);
                assert_eq!(msg.lParam.0, source.0 as isize);
            }
            let r = view::companion_player_button(0, scale);
            let lp = LPARAM(((((r.1 + r.3) / 2.0) as isize) << 16) | ((r.0 + r.2) / 2.0) as isize);
            proc(self.hwnd, WM_LBUTTONDOWN, WPARAM(0), lp);
            proc(self.hwnd, WM_LBUTTONUP, WPARAM(0), LPARAM(-1));
            let mut msg = MSG::default();
            assert!(!PeekMessageW(&mut msg, Some(parent), WM_COMPANION, WM_COMPANION, PM_REMOVE).as_bool());
        }
    }
}

impl Drop for Controls {
    fn drop(&mut self) {
        unsafe {
            let _ = KillTimer(Some(self.hwnd), 1);
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn music_buttons_have_distinct_targets_at_every_scale() {
        for scale in [0.75, 1.0, 1.5, 2.0] {
            for (i, action) in [Action::Previous, Action::PlayPause, Action::Next].into_iter().enumerate() {
                let r = view::companion_player_button(i, scale);
                assert_eq!(button_at((r.0 + r.2) / 2.0, (r.1 + r.3) / 2.0, scale), Some(action));
            }
            assert_eq!(button_at(2.0 * scale, 22.0 * scale, scale), None);
            assert_eq!(button_at(66.0 * scale, 2.0 * scale, scale), None);
        }
    }
}
