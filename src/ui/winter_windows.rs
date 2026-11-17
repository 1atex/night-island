use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::BOOL;

pub(super) struct WindowEdge {
    pub handle: isize,
    pub rect: RECT,
    pub maximized: bool,
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
                out.push(WindowEdge { handle: hwnd.0 as isize, rect, maximized: IsZoomed(hwnd).as_bool() });
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
