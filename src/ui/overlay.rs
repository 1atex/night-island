use std::sync::Once;
use std::sync::atomic::{AtomicIsize, AtomicU32, Ordering};

use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION, CreateCompatibleDC,
    CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, ReleaseDC, ScreenToClient, SelectObject,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetClientRect, GetWindowRect, HTCAPTION, HTCLIENT, IDC_ARROW,
    LoadCursorW, MA_NOACTIVATE, PostMessageW, RegisterClassW, SW_SHOWNOACTIVATE, ShowWindow, ULW_ALPHA,
    UpdateLayeredWindow, WM_APP, WM_EXITSIZEMOVE, WM_LBUTTONDOWN, WM_MOUSEACTIVATE, WM_NCHITTEST, WNDCLASSW,
    WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};
use windows::core::{Result, w};

use crate::ui::gfx::{Gpu, Painter};
use crate::view;

pub const WM_OVERLAY: u32 = WM_APP + 16;

static MAIN: AtomicIsize = AtomicIsize::new(0);
static SCALE: AtomicU32 = AtomicU32::new(0x3F80_0000);

pub struct Overlay {
    pub hwnd: HWND,
}

fn client_buttons(hwnd: HWND) -> [view::Rect; 3] {
    let mut rc = RECT::default();
    unsafe {
        let _ = GetClientRect(hwnd, &mut rc);
    }
    view::overlay_buttons((rc.right as f32, rc.bottom as f32), f32::from_bits(SCALE.load(Ordering::Relaxed)))
}

fn button_at(hwnd: HWND, x: i32, y: i32) -> Option<usize> {
    let (px, py) = (x as f32, y as f32);
    client_buttons(hwnd).iter().position(|r| px >= r.0 && px < r.2 && py >= r.1 && py < r.3)
}

extern "system" fn proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_NCHITTEST => {
            let mut pt = POINT { x: lparam.0 as i16 as i32, y: (lparam.0 >> 16) as i16 as i32 };
            unsafe {
                let _ = ScreenToClient(hwnd, &mut pt);
            }
            LRESULT(if button_at(hwnd, pt.x, pt.y).is_some() { HTCLIENT } else { HTCAPTION } as isize)
        }
        WM_LBUTTONDOWN => {
            let (x, y) = (lparam.0 as i16 as i32, (lparam.0 >> 16) as i16 as i32);
            if let Some(i) = button_at(hwnd, x, y) {
                let main = MAIN.load(Ordering::Relaxed);
                unsafe {
                    let _ = PostMessageW(Some(HWND(main as *mut _)), WM_OVERLAY, WPARAM(i + 1), LPARAM(0));
                }
            }
            LRESULT(0)
        }
        WM_EXITSIZEMOVE => {
            let main = MAIN.load(Ordering::Relaxed);
            unsafe {
                let _ = PostMessageW(Some(HWND(main as *mut _)), WM_OVERLAY, WPARAM(10), LPARAM(0));
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

pub fn create(main: HWND, at: (i32, i32), size: (i32, i32), scale: f32) -> Option<Overlay> {
    static CLASS: Once = Once::new();
    MAIN.store(main.0 as isize, Ordering::Relaxed);
    SCALE.store(scale.to_bits(), Ordering::Relaxed);
    unsafe {
        let instance = GetModuleHandleW(None).ok()?;
        CLASS.call_once(|| {
            let class = WNDCLASSW {
                lpfnWndProc: Some(proc),
                hInstance: instance.into(),
                lpszClassName: w!("NightIsland.Overlay"),
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                ..Default::default()
            };
            RegisterClassW(&class);
        });
        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            w!("NightIsland.Overlay"),
            w!("Night Island overlay"),
            WS_POPUP,
            at.0,
            at.1,
            size.0,
            size.1,
            None,
            None,
            Some(instance.into()),
            None,
        )
        .ok()?;
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        Some(Overlay { hwnd })
    }
}

impl Overlay {
    pub fn set_scale(&self, scale: f32) {
        SCALE.store(scale.to_bits(), Ordering::Relaxed);
    }

    pub fn position(&self) -> (i32, i32) {
        let mut rc = RECT::default();
        unsafe {
            let _ = GetWindowRect(self.hwnd, &mut rc);
        }
        (rc.left, rc.top)
    }

    pub fn render(&self, gpu: &Gpu, size: (i32, i32), f: impl FnOnce(&Painter)) -> Result<()> {
        unsafe {
            let screen = GetDC(None);
            let mem = CreateCompatibleDC(Some(screen));
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: size.0,
                    biHeight: -size.1,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut bits = std::ptr::null_mut();
            let result = CreateDIBSection(Some(mem), &info, DIB_RGB_COLORS, &mut bits, None, 0).and_then(|dib| {
                let old = SelectObject(mem, dib.into());
                let painted = gpu.paint_dc(mem, size, f).and_then(|()| {
                    let (src, dims) = (POINT { x: 0, y: 0 }, SIZE { cx: size.0, cy: size.1 });
                    let blend = BLENDFUNCTION {
                        BlendOp: AC_SRC_OVER as u8,
                        BlendFlags: 0,
                        SourceConstantAlpha: 255,
                        AlphaFormat: AC_SRC_ALPHA as u8,
                    };
                    UpdateLayeredWindow(
                        self.hwnd,
                        Some(screen),
                        None,
                        Some(&dims),
                        Some(mem),
                        Some(&src),
                        COLORREF(0),
                        Some(&blend),
                        ULW_ALPHA,
                    )
                });
                SelectObject(mem, old);
                let _ = DeleteObject(dib.into());
                painted
            });
            let _ = DeleteDC(mem);
            ReleaseDC(None, screen);
            result
        }
    }
}

impl Drop for Overlay {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}
