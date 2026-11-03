use std::sync::Once;

use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION, CreateCompatibleDC,
    CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, GetMonitorInfoW, HBITMAP, HDC,
    MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint, ReleaseDC, SelectObject,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{Error, Result, w};

use crate::ui::characters::{self, Character, Pose};
use crate::ui::gfx::Gpu;

pub const WM_SPARK: u32 = WM_APP + 45;
const SIDE: f32 = 88.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Chat = 1,
    ToggleIsland = 2,
    Moved = 3,
    Dock = 4,
}

impl Action {
    pub fn from_id(id: usize) -> Option<Self> {
        match id {
            1 => Some(Self::Chat),
            2 => Some(Self::ToggleIsland),
            3 => Some(Self::Moved),
            4 => Some(Self::Dock),
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
struct Drag {
    down: POINT,
    origin: POINT,
    moved: bool,
}

struct State {
    parent: HWND,
    drag: Option<Drag>,
}

fn notify(hwnd: HWND, state: &State, action: Action) {
    unsafe {
        let _ = PostMessageW(Some(state.parent), WM_SPARK, WPARAM(action as usize), LPARAM(hwnd.0 as isize));
    }
}

fn clamp_position(pos: [i32; 2], side: i32, work: RECT) -> [i32; 2] {
    [
        pos[0].clamp(work.left, (work.right - side).max(work.left)),
        pos[1].clamp(work.top, (work.bottom - side).max(work.top)),
    ]
}

fn on_screen(pos: [i32; 2], side: i32) -> [i32; 2] {
    unsafe {
        let monitor = MonitorFromPoint(POINT { x: pos[0], y: pos[1] }, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
        if GetMonitorInfoW(monitor, &mut info).as_bool() { clamp_position(pos, side, info.rcWork) } else { pos }
    }
}

extern "system" fn proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        if msg == WM_MOUSEACTIVATE {
            return LRESULT(MA_NOACTIVATE as isize);
        }
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
        if ptr.is_null() {
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }
        match msg {
            WM_NCHITTEST => {
                let mut rect = RECT::default();
                let _ = GetWindowRect(hwnd, &mut rect);
                let x = lparam.0 as i16 as i32;
                let y = (lparam.0 >> 16) as i16 as i32;
                let cx = (rect.left + rect.right) as f32 / 2.0;
                let cy = (rect.top + rect.bottom) as f32 / 2.0;
                let radius = (rect.right - rect.left) as f32 * 0.38;
                LRESULT(if (x as f32 - cx).powi(2) + (y as f32 - cy).powi(2) <= radius * radius {
                    HTCLIENT as isize
                } else {
                    HTTRANSPARENT as isize
                })
            }
            WM_LBUTTONDOWN => {
                let mut down = POINT::default();
                let mut rect = RECT::default();
                if GetCursorPos(&mut down).is_ok() && GetWindowRect(hwnd, &mut rect).is_ok() {
                    (*ptr).drag = Some(Drag { down, origin: POINT { x: rect.left, y: rect.top }, moved: false });
                    SetCapture(hwnd);
                }
                LRESULT(0)
            }
            WM_MOUSEMOVE => {
                if let Some(mut drag) = (*ptr).drag {
                    let mut point = POINT::default();
                    if GetCursorPos(&mut point).is_ok() {
                        let (dx, dy) = (point.x - drag.down.x, point.y - drag.down.y);
                        drag.moved |= dx.abs().max(dy.abs()) >= 6;
                        (*ptr).drag = Some(drag);
                        if drag.moved {
                            let _ = SetWindowPos(
                                hwnd,
                                None,
                                drag.origin.x + dx,
                                drag.origin.y + dy,
                                0,
                                0,
                                SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOZORDER,
                            );
                        }
                    }
                }
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                let drag = (*ptr).drag.take();
                let _ = ReleaseCapture();
                if let Some(drag) = drag {
                    notify(hwnd, &*ptr, if drag.moved { Action::Moved } else { Action::Chat });
                }
                LRESULT(0)
            }
            WM_CAPTURECHANGED => {
                (*ptr).drag = None;
                LRESULT(0)
            }
            WM_RBUTTONUP => {
                notify(hwnd, &*ptr, Action::ToggleIsland);
                LRESULT(0)
            }
            WM_CLOSE => {
                notify(hwnd, &*ptr, Action::Dock);
                LRESULT(0)
            }
            WM_DISPLAYCHANGE => {
                notify(hwnd, &*ptr, Action::Moved);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

struct Surface {
    mem: HDC,
    dib: HBITMAP,
    side: i32,
}

impl Surface {
    fn new(side: i32) -> Result<Self> {
        unsafe {
            let screen = GetDC(None);
            let mem = CreateCompatibleDC(Some(screen));
            ReleaseDC(None, screen);
            if mem.0.is_null() {
                return Err(Error::from_thread());
            }
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: side,
                    biHeight: -side,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut bits = std::ptr::null_mut();
            let dib = match CreateDIBSection(Some(mem), &info, DIB_RGB_COLORS, &mut bits, None, 0) {
                Ok(dib) => dib,
                Err(error) => {
                    let _ = DeleteDC(mem);
                    return Err(error);
                }
            };
            SelectObject(mem, dib.into());
            Ok(Self { mem, dib, side })
        }
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteDC(self.mem);
            let _ = DeleteObject(self.dib.into());
        }
    }
}

pub struct SparkWindow {
    hwnd: HWND,
    _state: Box<State>,
    surface: Surface,
}

impl SparkWindow {
    pub fn new(parent: HWND, position: Option<[i32; 2]>, scale: f32) -> Result<Self> {
        static CLASS: Once = Once::new();
        unsafe {
            CLASS.call_once(|| {
                let class = WNDCLASSW {
                    lpfnWndProc: Some(proc),
                    hInstance: GetModuleHandleW(None).unwrap_or_default().into(),
                    hCursor: LoadCursorW(None, IDC_HAND).unwrap_or_default(),
                    lpszClassName: w!("NightIsland.Spark"),
                    ..Default::default()
                };
                RegisterClassW(&class);
            });
            let side = (SIDE * scale).round().max(32.0) as i32;
            let mut rect = RECT::default();
            let _ = GetWindowRect(parent, &mut rect);
            let position = on_screen(position.unwrap_or([rect.right + 12, rect.top]), side);
            let mut state = Box::new(State { parent, drag: None });
            let hwnd = CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST,
                w!("NightIsland.Spark"),
                w!("Spark"),
                WS_POPUP,
                position[0],
                position[1],
                side,
                side,
                None,
                None,
                Some(GetModuleHandleW(None)?.into()),
                None,
            )?;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, (&raw mut *state) as isize);
            let surface = match Surface::new(side) {
                Ok(surface) => surface,
                Err(error) => {
                    let _ = DestroyWindow(hwnd);
                    return Err(error);
                }
            };
            Ok(Self { hwnd, _state: state, surface })
        }
    }

    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    pub fn position(&self) -> [i32; 2] {
        let mut rect = RECT::default();
        unsafe {
            let _ = GetWindowRect(self.hwnd, &mut rect);
        }
        [rect.left, rect.top]
    }

    pub fn center(&self) -> (f32, f32) {
        let [x, y] = self.position();
        (x as f32 + self.surface.side as f32 / 2.0, y as f32 + self.surface.side as f32 / 2.0)
    }

    pub fn keep_on_screen(&self) {
        let pos = on_screen(self.position(), self.surface.side);
        unsafe {
            let _ = SetWindowPos(self.hwnd, None, pos[0], pos[1], 0, 0, SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOZORDER);
        }
    }

    pub fn hide(&self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    pub fn set_capture_hidden(&self, hidden: bool) {
        unsafe {
            let _ = SetWindowDisplayAffinity(self.hwnd, if hidden { WDA_EXCLUDEFROMCAPTURE } else { WDA_NONE });
        }
    }

    pub fn render(&mut self, gpu: &Gpu, pose: &Pose, scale: f32) -> Result<()> {
        let side = (SIDE * scale).round().max(32.0) as i32;
        if self.surface.side != side {
            self.surface = Surface::new(side)?;
            self.keep_on_screen();
        }
        let mut drawn = Ok(());
        gpu.paint_dc(self.surface.mem, (side, side), |p| {
            let old = p.push_offset(0.0, side as f32 * 0.125);
            drawn = characters::draw(p, Character::Spark, side as f32, pose);
            p.set_transform(&old);
        })?;
        drawn?;
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
                None,
                Some(&SIZE { cx: side, cy: side }),
                Some(self.surface.mem),
                Some(&POINT { x: 0, y: 0 }),
                COLORREF(0),
                Some(&blend),
                ULW_ALPHA,
            )?;
            let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
        }
        Ok(())
    }
}

impl Drop for SparkWindow {
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
    fn saved_positions_remain_reachable_after_a_monitor_change() {
        let work = RECT { left: 0, top: 0, right: 1920, bottom: 1040 };
        assert_eq!(clamp_position([-5000, -3000], 88, work), [0, 0]);
        assert_eq!(clamp_position([5000, 3000], 88, work), [1832, 952]);
        assert_eq!(clamp_position([300, 200], 88, work), [300, 200]);
        let left = RECT { left: -1920, top: 0, right: 0, bottom: 1040 };
        assert_eq!(clamp_position([-2500, 200], 88, left), [-1920, 200]);
    }

    #[test]
    #[ignore = "Requires an interactive Windows desktop and an isolated Composition runtime"]
    fn desktop_spark_stays_visible_without_its_owner_and_routes_actions() -> Result<()> {
        use windows::UI::Composition::Compositor;
        use windows::Win32::System::WinRT::{
            CreateDispatcherQueueController, DQTAT_COM_NONE, DQTYPE_THREAD_CURRENT, DispatcherQueueOptions,
            RO_INIT_SINGLETHREADED, RoInitialize, RoUninitialize,
        };
        struct Runtime;
        impl Drop for Runtime {
            fn drop(&mut self) {
                unsafe { RoUninitialize() };
            }
        }
        unsafe {
            RoInitialize(RO_INIT_SINGLETHREADED)?;
        }
        let _runtime = Runtime;
        let _queue = unsafe {
            CreateDispatcherQueueController(DispatcherQueueOptions {
                dwSize: size_of::<DispatcherQueueOptions>() as u32,
                threadType: DQTYPE_THREAD_CURRENT,
                apartmentType: DQTAT_COM_NONE,
            })?
        };
        let compositor = Compositor::new()?;
        let gpu = Gpu::new(&compositor)?;
        let parent = unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW,
                w!("STATIC"),
                w!("Spark test owner"),
                WS_POPUP,
                0,
                0,
                200,
                50,
                None,
                None,
                Some(GetModuleHandleW(None)?.into()),
                None,
            )?
        };
        let mut spark = SparkWindow::new(parent, Some([320, 320]), 1.0)?;
        let pose = Pose {
            t: 0.0,
            reaction: characters::Reaction::Idle,
            look: (0.0, 0.0),
            blink: false,
            accent: (0.5, 0.6, 1.0),
            beat: 0.0,
        };
        spark.render(&gpu, &pose, 1.0)?;
        unsafe {
            assert!(IsWindowVisible(spark.hwnd()).as_bool());
            assert!(GetWindow(spark.hwnd(), GW_OWNER).is_err());
            let _ = ShowWindow(parent, SW_HIDE);
            assert!(IsWindowVisible(spark.hwnd()).as_bool());
        }
        let mut message = MSG::default();
        let source = spark.hwnd().0 as isize;
        let mut next = |expected: Action| {
            assert!(unsafe { PeekMessageW(&mut message, Some(parent), WM_SPARK, WM_SPARK, PM_REMOVE) }.as_bool());
            assert_eq!(Action::from_id(message.wParam.0), Some(expected));
            assert_eq!(message.lParam.0, source);
        };
        proc(spark.hwnd(), WM_RBUTTONUP, WPARAM(0), LPARAM(0));
        next(Action::ToggleIsland);
        spark._state.drag = Some(Drag { down: POINT::default(), origin: POINT::default(), moved: false });
        proc(spark.hwnd(), WM_LBUTTONUP, WPARAM(0), LPARAM(0));
        next(Action::Chat);
        spark._state.drag = Some(Drag { down: POINT::default(), origin: POINT::default(), moved: true });
        proc(spark.hwnd(), WM_LBUTTONUP, WPARAM(0), LPARAM(0));
        next(Action::Moved);
        proc(spark.hwnd(), WM_CLOSE, WPARAM(0), LPARAM(0));
        next(Action::Dock);
        spark.hide();
        assert!(!unsafe { IsWindowVisible(spark.hwnd()) }.as_bool());
        spark.render(&gpu, &pose, 1.25)?;
        let mut rect = RECT::default();
        unsafe {
            GetWindowRect(spark.hwnd(), &mut rect)?;
        }
        assert_eq!(rect.right - rect.left, 110);
        let hwnd = spark.hwnd();
        drop(spark);
        assert!(!unsafe { IsWindow(Some(hwnd)) }.as_bool());
        unsafe {
            DestroyWindow(parent)?;
        }
        Ok(())
    }
}
