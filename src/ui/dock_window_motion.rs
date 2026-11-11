use crate::platform::{
    desktop_dock::{self as desktop, Window},
    window_preview::Thumbnail,
};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{COLORREF, HWND, RECT};
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::w;

struct Overlay(HWND);
impl Drop for Overlay {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.0);
        }
    }
}
pub(super) struct Flight {
    thumbnail: Thumbnail,
    overlay: Overlay,
    window: Window,
    restoring: bool,
    restore_requested: bool,
    from: RECT,
    to: RECT,
    started: Instant,
}

pub(super) fn system_animates_windows() -> bool {
    let mut info = ANIMATIONINFO { cbSize: size_of::<ANIMATIONINFO>() as u32, ..Default::default() };
    let result = unsafe {
        SystemParametersInfoW(
            SPI_GETANIMATION,
            info.cbSize,
            Some((&raw mut info).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    result.is_err() || info.iMinAnimate != 0
}

fn bounds(hwnd: HWND, restoring: bool) -> Option<RECT> {
    unsafe {
        if !restoring {
            let mut rect = RECT::default();
            GetWindowRect(hwnd, &mut rect).ok()?;
            return Some(rect);
        }
        let mut placement = WINDOWPLACEMENT { length: size_of::<WINDOWPLACEMENT>() as u32, ..Default::default() };
        GetWindowPlacement(hwnd, &mut placement).ok()?;
        let mut monitor = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
        if !GetMonitorInfoW(MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST), &mut monitor).as_bool() {
            return None;
        }
        if placement.flags.0 & WPF_RESTORETOMAXIMIZED.0 != 0 {
            return Some(monitor.rcWork);
        }
        let mut r = placement.rcNormalPosition;
        if GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW.0 != 0 {
            return Some(r);
        }
        let dx = monitor.rcWork.left - monitor.rcMonitor.left;
        let dy = monitor.rcWork.top - monitor.rcMonitor.top;
        r.left += dx;
        r.right += dx;
        r.top += dy;
        r.bottom += dy;
        Some(r)
    }
}

fn interpolate(from: RECT, to: RECT, t: f32) -> RECT {
    let t = t.clamp(0.0, 1.0);
    let t = t * t * (3.0 - 2.0 * t);
    let lerp = |a: i32, b: i32| (a as f32 + (b - a) as f32 * t).round() as i32;
    RECT {
        left: lerp(from.left, to.left),
        top: lerp(from.top, to.top),
        right: lerp(from.right, to.right),
        bottom: lerp(from.bottom, to.bottom),
    }
}
impl Flight {
    pub fn start(window: &Window, anchor: RECT, restoring: bool, hidden: bool) -> Option<Self> {
        let hwnd = HWND(window.handle as _);
        let large = bounds(hwnd, restoring)?;
        let width = (large.right - large.left) as f32;
        let height = (large.bottom - large.top) as f32;
        if width <= 0.0 || height <= 0.0 {
            return None;
        }
        let small = super::dock_preview::fit(
            (width as i32, height as i32),
            (anchor.left as f32, anchor.top as f32, anchor.right as f32, anchor.bottom as f32),
        );
        let (from, to) = if restoring { (small, large) } else { (large, small) };
        unsafe {
            let overlay = Overlay(
                CreateWindowExW(
                    WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT | WS_EX_LAYERED,
                    w!("STATIC"),
                    w!("Night Island Window Motion"),
                    WS_POPUP,
                    from.left,
                    from.top,
                    from.right - from.left,
                    from.bottom - from.top,
                    None,
                    None,
                    None,
                    None,
                )
                .ok()?,
            );
            SetLayeredWindowAttributes(overlay.0, COLORREF(0), 255, LWA_ALPHA).ok()?;
            if hidden {
                SetWindowDisplayAffinity(overlay.0, WDA_EXCLUDEFROMCAPTURE).ok()?;
            }
            let thumbnail = Thumbnail::new(overlay.0, window)?;
            let mut flight = Self {
                thumbnail,
                overlay,
                window: window.clone(),
                restoring,
                restore_requested: false,
                from,
                to,
                started: Instant::now(),
            };
            if !flight.place(from) {
                return None;
            }
            if !restoring && !desktop::change_window(window, desktop::WindowAction::Minimize) {
                return None;
            }
            Some(flight)
        }
    }
    fn place(&mut self, r: RECT) -> bool {
        let width = (r.right - r.left).max(1);
        let height = (r.bottom - r.top).max(1);
        if !self.thumbnail.move_registered(RECT { left: 0, top: 0, right: width, bottom: height }) {
            return false;
        }
        unsafe {
            SetWindowPos(
                self.overlay.0,
                Some(HWND_TOPMOST),
                r.left,
                r.top,
                width,
                height,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            )
            .is_ok()
        }
    }
    pub fn matches(&self, window: &Window, restoring: bool) -> bool {
        self.window.handle == window.handle && self.window.path == window.path && self.restoring == restoring
    }
    pub fn set_capture(&self, hidden: bool) {
        unsafe {
            let _ = SetWindowDisplayAffinity(self.overlay.0, if hidden { WDA_EXCLUDEFROMCAPTURE } else { WDA_NONE });
        }
    }
    pub fn tick(&mut self) -> bool {
        let elapsed = self.started.elapsed();
        let duration = Duration::from_millis(205);
        if self.restore_requested {
            return elapsed < duration + Duration::from_millis(150)
                && unsafe { IsIconic(HWND(self.window.handle as _)).as_bool() };
        }
        if !self.place(interpolate(self.from, self.to, elapsed.as_secs_f32() / duration.as_secs_f32())) {
            return false;
        }
        if elapsed < duration {
            return true;
        }
        if self.restoring {
            desktop::activate(&self.window, false);
            self.restore_requested = true;
            return true;
        }
        false
    }
    pub fn finish(self) {
        if self.restoring && !self.restore_requested {
            desktop::activate(&self.window, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_flight_reaches_both_endpoints_on_offset_monitors() {
        let window = RECT { left: -1611, top: -960, right: -690, bottom: -450 };
        let icon = RECT { left: -501, top: 12, right: -448, bottom: 42 };
        for (from, to) in [(window, icon), (icon, window)] {
            assert_eq!(interpolate(from, to, -0.01), from);
            assert_eq!(interpolate(from, to, 1.01), to);
            let r = interpolate(from, to, 0.5);
            assert!((r.left * 2 - from.left - to.left).abs() <= 1);
            assert!((r.bottom * 2 - from.bottom - to.bottom).abs() <= 1);
        }
    }
}
