use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::BOOL;

pub(super) struct WindowEdge {
    pub handle: isize,
    pub rect: RECT,
    pub maximized: bool,
    pub radius: f32,
}

fn desktop_or_overlay(class: &str, style: isize) -> bool {
    style & WS_EX_TRANSPARENT.0 as isize != 0
        || (class == "CEF-OSC-WIDGET" && style & WS_EX_TOOLWINDOW.0 as isize != 0)
        || matches!(
            class,
            "Progman"
                | "WorkerW"
                | "NightIsland.Winter"
                | "NightIsland.Window"
                | "NightIsland.DesktopDock"
                | "NightIsland.TopPanel"
                | "NightIsland.Companion"
        )
}

pub(super) fn window_edges() -> Vec<WindowEdge> {
    unsafe extern "system" fn each(hwnd: HWND, data: LPARAM) -> BOOL {
        let out = unsafe { &mut *(data.0 as *mut Vec<WindowEdge>) };
        unsafe {
            if !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
                return true.into();
            }
            let mut class = [0u16; 128];
            let len = GetClassNameW(hwnd, &mut class) as usize;
            if desktop_or_overlay(&String::from_utf16_lossy(&class[..len]), GetWindowLongPtrW(hwnd, GWL_EXSTYLE)) {
                return true.into();
            }
            let mut cloaked = 0u32;
            let _ = DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED, (&raw mut cloaked).cast(), size_of::<u32>() as u32);
            if cloaked != 0 {
                return true.into();
            }
            let mut rect = RECT::default();
            if GetWindowRect(hwnd, &mut rect).is_err() {
                return true.into();
            }
            let _ = DwmGetWindowAttribute(
                hwnd,
                DWMWA_EXTENDED_FRAME_BOUNDS,
                (&raw mut rect).cast(),
                size_of::<RECT>() as u32,
            );
            if rect.right > rect.left && rect.bottom > rect.top {
                let mut preference = 0u32;
                let rounded = DwmGetWindowAttribute(
                    hwnd,
                    windows::Win32::Graphics::Dwm::DWMWA_WINDOW_CORNER_PREFERENCE,
                    (&raw mut preference).cast(),
                    4,
                )
                .is_ok();
                let dpi = windows::Win32::UI::HiDpi::GetDpiForWindow(hwnd) as f32 / 96.0;
                let radius = if !rounded || preference == 1 {
                    0.0
                } else if preference == 3 {
                    4.0 * dpi
                } else {
                    8.0 * dpi
                };
                out.push(WindowEdge { handle: hwnd.0 as isize, rect, maximized: IsZoomed(hwnd).as_bool(), radius });
            }
        }
        (out.len() < 256).into()
    }
    let mut out = Vec::new();
    unsafe {
        let _ = EnumWindows(Some(each), LPARAM((&raw mut out) as isize));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn passive_overlays_do_not_hide_the_desktop_snow() {
        assert!(desktop_or_overlay("CEF-OSC-WIDGET", 0x800a8));
        assert!(desktop_or_overlay("CEF-OSC-WIDGET", 0x8080080));
        assert!(desktop_or_overlay("AnyOverlay", (WS_EX_LAYERED | WS_EX_TRANSPARENT).0 as isize));
        assert!(!desktop_or_overlay("MozillaWindowClass", 0));
        assert!(!desktop_or_overlay("CabinetWClass", WS_EX_APPWINDOW.0 as isize));
        assert!(!desktop_or_overlay("Chrome_WidgetWin_1", 0x240100));
        assert!(!desktop_or_overlay("NightIsland.Settings", 0x100));
    }
}

pub(super) fn exposed_spans(left: f32, right: f32, roof: f32, front: &[RECT]) -> Vec<(f32, f32)> {
    let mut spans = vec![(left, right)];
    for cover in front {
        if cover.top as f32 >= roof || cover.bottom as f32 <= roof - 24.0 {
            continue;
        }
        spans = spans
            .into_iter()
            .flat_map(|(a, b)| {
                let l = cover.left as f32;
                let r = cover.right as f32;
                let mut remaining = Vec::new();
                if r <= a || l >= b {
                    remaining.push((a, b));
                } else {
                    if l > a {
                        remaining.push((a, l));
                    }
                    if r < b {
                        remaining.push((r, b));
                    }
                }
                remaining
            })
            .collect();
    }
    spans
}

#[cfg(test)]
mod movement_tests {
    use super::*;
    #[test]
    fn front_windows_clip_caps_without_changing_the_rear_windows_identity() {
        let front = RECT { left: 180, top: 90, right: 320, bottom: 260 };
        assert_eq!(exposed_spans(100.0, 400.0, 180.0, &[front]), vec![(100.0, 180.0), (320.0, 400.0)]);
        let moved = RECT { left: 240, right: 380, ..front };
        assert_eq!(exposed_spans(100.0, 400.0, 180.0, &[moved]), vec![(100.0, 240.0), (380.0, 400.0)]);
        assert_eq!(exposed_spans(100.0, 400.0, 180.0, &[]), vec![(100.0, 400.0)]);
        assert!(exposed_spans(100.0, 400.0, 180.0, &[RECT { left: 0, right: 800, ..front }]).is_empty());
    }
}
