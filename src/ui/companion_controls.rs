use std::sync::Once;

use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{AC_SRC_ALPHA, AC_SRC_OVER, BLENDFUNCTION};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetCapture, ReleaseCapture, SetCapture};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{Result, w};

use super::companion::{Action, Surface, WM_COMPANION};
use super::gfx::{Gpu, rgba};

const WIDTH: f32 = 132.0;
const HEIGHT: f32 = 44.0;

fn button_at(x: f32, y: f32, scale: f32) -> Option<Action> {
    if !(6.0..HEIGHT - 6.0).contains(&(y / scale)) {
        return None;
    }
    [Action::Previous, Action::PlayPause, Action::Next].into_iter().enumerate().find_map(|(i, action)| {
        let left = 6.0 + i as f32 * 40.0;
        (x / scale >= left && x / scale < left + 40.0).then_some(action)
    })
}

struct State {
    parent: HWND,
    source: HWND,
    scale: f32,
    pressed: Option<Action>,
}

extern "system" fn proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    unsafe {
        if msg == WM_MOUSEACTIVATE {
            return LRESULT(MA_NOACTIVATE as isize);
        }
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
        if ptr.is_null() {
            return DefWindowProcW(hwnd, msg, wp, lp);
        }
        let point = || (lp.0 as i16 as f32, (lp.0 >> 16) as i16 as f32);
        match msg {
            WM_LBUTTONDOWN => {
                let (x, y) = point();
                (*ptr).pressed = button_at(x, y, (*ptr).scale);
                SetCapture(hwnd);
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                let pressed = (*ptr).pressed.take();
                let _ = ReleaseCapture();
                let (x, y) = point();
                if let Some(action) = pressed.filter(|action| Some(*action) == button_at(x, y, (*ptr).scale)) {
                    let _ = PostMessageW(
                        Some((*ptr).parent),
                        WM_COMPANION,
                        WPARAM(action as usize),
                        LPARAM((*ptr).source.0 as isize),
                    );
                }
                LRESULT(0)
            }
            WM_CAPTURECHANGED => {
                (*ptr).pressed = None;
                LRESULT(0)
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
    state: Box<State>,
    surface: Surface,
    key: Option<(bool, Option<Action>, Option<Action>, i32)>,
}

impl Controls {
    pub(super) fn new(parent: HWND, source: HWND, scale: f32) -> Result<Self> {
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
            let surface = Surface::new((WIDTH * scale).round() as i32)?;
            let mut state = Box::new(State { parent, source, scale, pressed: None });
            let hwnd = CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST,
                w!("NightIsland.CompanionControls"),
                w!("Music controls"),
                WS_POPUP,
                0,
                0,
                (WIDTH * scale).round() as i32,
                (HEIGHT * scale).round() as i32,
                Some(source),
                None,
                Some(instance.into()),
                None,
            )?;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, (&raw mut *state) as isize);
            Ok(Self { hwnd, state, surface, key: None })
        }
    }

    pub(super) fn hwnd(&self) -> HWND {
        self.hwnd
    }

    #[cfg(test)]
    pub(super) fn surface(&self) -> &Surface {
        &self.surface
    }

    pub(super) fn hide(&mut self) {
        self.state.pressed = None;
        unsafe {
            if GetCapture() == self.hwnd {
                let _ = ReleaseCapture();
            }
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    #[cfg(test)]
    pub(super) fn verify_actions(&self, parent: HWND) {
        unsafe {
            let scale = self.state.scale;
            for (x, action) in [(26, Action::Previous), (66, Action::PlayPause), (106, Action::Next)] {
                let lp = LPARAM((((22.0 * scale) as isize) << 16) | (x as f32 * scale) as isize);
                proc(self.hwnd, WM_LBUTTONDOWN, WPARAM(0), lp);
                proc(self.hwnd, WM_LBUTTONUP, WPARAM(0), lp);
                let mut msg = MSG::default();
                assert!(PeekMessageW(&mut msg, Some(parent), WM_COMPANION, WM_COMPANION, PM_REMOVE).as_bool());
                assert_eq!(msg.wParam.0, action as usize);
                assert_eq!(msg.lParam.0, self.state.source.0 as isize);
            }
            let lp = LPARAM((((22.0 * scale) as isize) << 16) | (26.0 * scale) as isize);
            proc(self.hwnd, WM_LBUTTONDOWN, WPARAM(0), lp);
            proc(self.hwnd, WM_LBUTTONUP, WPARAM(0), LPARAM(-1));
            let mut msg = MSG::default();
            assert!(!PeekMessageW(&mut msg, Some(parent), WM_COMPANION, WM_COMPANION, PM_REMOVE).as_bool());
        }
    }

    pub(super) fn rect_at(&self, character: RECT, work: RECT, scale: f32) -> RECT {
        let w = (WIDTH * scale).round() as i32;
        let h = (HEIGHT * scale).round() as i32;
        let x = ((character.left + character.right - w) / 2).clamp(work.left, (work.right - w).max(work.left));
        let below = character.bottom - (4.0 * scale) as i32;
        let y = if below + h <= work.bottom { below } else { character.top - h + (4.0 * scale) as i32 };
        let y = y.clamp(work.top, (work.bottom - h).max(work.top));
        RECT { left: x, top: y, right: x + w, bottom: y + h }
    }

    pub(super) fn show(&mut self, gpu: &Gpu, rect: RECT, cursor: POINT, playing: bool, scale: f32) -> Result<()> {
        let w = (WIDTH * scale).round() as i32;
        let h = (HEIGHT * scale).round() as i32;
        if self.surface.side != w {
            self.surface = Surface::new(w)?;
        }
        self.state.scale = scale;
        let hover = button_at((cursor.x - rect.left) as f32, (cursor.y - rect.top) as f32, scale);
        let key = (playing, hover, self.state.pressed, w);
        if self.key != Some(key) {
            let mut drawn: Result<()> = Ok(());
            gpu.paint_dc(self.surface.mem, (w, h), |p| {
                drawn = (|| {
                    p.fill_round_rect(
                        (1.0, 1.0, w as f32 - 1.0, h as f32 - 1.0),
                        20.0 * scale,
                        rgba(0.07, 0.075, 0.09, 0.98),
                    )?;
                    p.stroke_round_rect(
                        (1.0, 1.0, w as f32 - 1.0, h as f32 - 1.0),
                        20.0 * scale,
                        scale,
                        rgba(1.0, 1.0, 1.0, 0.15),
                    )?;
                    for (i, action) in [Action::Previous, Action::PlayPause, Action::Next].into_iter().enumerate() {
                        let left = (6.0 + i as f32 * 40.0) * scale;
                        let r = (left + 2.0 * scale, 6.0 * scale, left + 38.0 * scale, 38.0 * scale);
                        if Some(action) == hover || action == Action::PlayPause {
                            let alpha = if self.state.pressed == Some(action) {
                                0.3
                            } else if Some(action) == hover {
                                0.2
                            } else {
                                0.09
                            };
                            p.fill_round_rect(r, 16.0 * scale, rgba(1.0, 1.0, 1.0, alpha))?;
                        }
                        let glyph = match action {
                            Action::Previous => '\u{E892}',
                            Action::Next => '\u{E893}',
                            _ if playing => '\u{E769}',
                            _ => '\u{E768}',
                        };
                        p.glyph(glyph, r, 14.0 * scale, rgba(1.0, 1.0, 1.0, 0.95))?;
                    }
                    Ok(())
                })();
            })?;
            drawn?;
            self.key = Some(key);
        }
        unsafe {
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            UpdateLayeredWindow(
                self.hwnd,
                None,
                Some(&POINT { x: rect.left, y: rect.top }),
                Some(&SIZE { cx: w, cy: h }),
                Some(self.surface.mem),
                Some(&POINT::default()),
                COLORREF(0),
                Some(&blend),
                ULW_ALPHA,
            )?;
            let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
        }
        Ok(())
    }

    pub(super) fn set_capture_hidden(&self, hidden: bool) {
        unsafe {
            let _ = SetWindowDisplayAffinity(self.hwnd, if hidden { WDA_EXCLUDEFROMCAPTURE } else { WDA_NONE });
        }
    }
}

impl Drop for Controls {
    fn drop(&mut self) {
        unsafe {
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
            assert_eq!(button_at(26.0 * scale, 22.0 * scale, scale), Some(Action::Previous));
            assert_eq!(button_at(66.0 * scale, 22.0 * scale, scale), Some(Action::PlayPause));
            assert_eq!(button_at(106.0 * scale, 22.0 * scale, scale), Some(Action::Next));
            assert_eq!(button_at(2.0 * scale, 22.0 * scale, scale), None);
            assert_eq!(button_at(66.0 * scale, 2.0 * scale, scale), None);
        }
    }
}
