use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::UI::Shell::{
    ABE_TOP, ABM_NEW, ABM_QUERYPOS, ABM_REMOVE, ABM_SETPOS, ABM_WINDOWPOSCHANGED, ABN_POSCHANGED, APPBARDATA,
    SHAppBarMessage,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{Result, w};

use super::top_panel::{TopPanel, WM_TOP_PANEL};
use crate::platform::desktop_dock as desktop;

const NOTIFY: u32 = WM_APP + 92;

pub(super) struct Reservation {
    hwnd: HWND,
    rect: RECT,
}

extern "system" fn proc(hwnd: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    unsafe {
        if message == NOTIFY && w.0 == ABN_POSCHANGED as usize {
            let parent = HWND(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut _);
            if !parent.0.is_null() {
                let _ = PostMessageW(Some(parent), WM_TOP_PANEL, WPARAM(3), LPARAM(0));
            }
        }
        if message == WM_WINDOWPOSCHANGED {
            let mut data = APPBARDATA { cbSize: size_of::<APPBARDATA>() as u32, hWnd: hwnd, ..Default::default() };
            SHAppBarMessage(ABM_WINDOWPOSCHANGED, &mut data);
        }
        DefWindowProcW(hwnd, message, w, l)
    }
}

impl Reservation {
    pub fn new(parent: HWND, monitor: &desktop::Monitor, height: i32) -> Result<Self> {
        static CLASS: std::sync::Once = std::sync::Once::new();
        let instance = unsafe { windows::Win32::System::LibraryLoader::GetModuleHandleW(None)? };
        CLASS.call_once(|| unsafe {
            RegisterClassW(&WNDCLASSW {
                lpfnWndProc: Some(proc),
                hInstance: instance.into(),
                lpszClassName: w!("NightIsland.TopPanelSpace"),
                ..Default::default()
            });
        });
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                w!("NightIsland.TopPanelSpace"),
                w!(""),
                WS_POPUP,
                0,
                0,
                1,
                1,
                None,
                None,
                Some(instance.into()),
                None,
            )?
        };
        let mut result = Self { hwnd, rect: RECT::default() };
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, parent.0 as isize);
        }
        unsafe {
            let mut data = APPBARDATA {
                cbSize: size_of::<APPBARDATA>() as u32,
                hWnd: hwnd,
                uCallbackMessage: NOTIFY,
                ..Default::default()
            };
            if SHAppBarMessage(ABM_NEW, &mut data) == 0 {
                return Err(windows::core::Error::empty());
            }
        }
        result.refresh(monitor, height);
        Ok(result)
    }

    pub fn refresh(&mut self, monitor: &desktop::Monitor, height: i32) {
        unsafe {
            let mut data = APPBARDATA {
                cbSize: size_of::<APPBARDATA>() as u32,
                hWnd: self.hwnd,
                uCallbackMessage: NOTIFY,
                uEdge: ABE_TOP,
                rc: monitor.bounds,
                ..Default::default()
            };
            SHAppBarMessage(ABM_QUERYPOS, &mut data);
            data.rc.bottom = data.rc.top + height;
            if data.rc != self.rect {
                SHAppBarMessage(ABM_SETPOS, &mut data);
                self.rect = data.rc;
                let _ = SetWindowPos(
                    self.hwnd,
                    None,
                    data.rc.left,
                    data.rc.top,
                    data.rc.right - data.rc.left,
                    data.rc.bottom - data.rc.top,
                    SWP_NOACTIVATE | SWP_NOZORDER,
                );
            }
        }
    }

    pub fn rect(&self) -> RECT {
        self.rect
    }
}

impl Drop for Reservation {
    fn drop(&mut self) {
        unsafe {
            SetWindowLongPtrW(self.hwnd, GWLP_USERDATA, 0);
            let mut data = APPBARDATA { cbSize: size_of::<APPBARDATA>() as u32, hWnd: self.hwnd, ..Default::default() };
            data.uEdge = ABE_TOP;
            data.rc = self.rect;
            data.rc.bottom = data.rc.top;
            SHAppBarMessage(ABM_SETPOS, &mut data);
            SHAppBarMessage(ABM_REMOVE, &mut data);
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

impl TopPanel {
    pub fn shell_restarted(&mut self) {
        self.reservation.take();
        self.reflow();
    }

    pub fn reflow(&mut self) {
        if let Some(monitor) = desktop::monitors().into_iter().find(|m| m.primary) {
            self.monitor = monitor;
        }
        if self.settings.reserve_space && !self.settings.auto_hide && self.settings.has_blocks() {
            let height = (50.0 * self.scale).ceil() as i32;
            if let Some(reservation) = &mut self.reservation {
                reservation.refresh(&self.monitor, height);
            } else {
                self.reservation = Reservation::new(self.parent(), &self.monitor, height).ok();
            }
        } else {
            self.reservation.take();
            if let Some(monitor) = desktop::monitors().into_iter().find(|m| m.primary) {
                self.monitor = monitor;
            }
        }
    }
}
