use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::{CreateRectRgn, DeleteObject, GetWindowRgn, HRGN, SetWindowRgn};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
};
use windows::Win32::UI::Accessibility::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{Result, w};

static BUSY: AtomicBool = AtomicBool::new(false);

const WIN10_CHEVRON_ID: &str = "1502";

pub mod inline;

fn overflow() -> Option<HWND> {
    [w!("TopLevelWindowForOverflowXamlIsland"), w!("NotifyIconOverflowWindow")]
        .into_iter()
        .find_map(|class| unsafe { FindWindowW(class, None).ok().filter(|hwnd| IsWindowVisible(*hwnd).as_bool()) })
}

pub fn is_open() -> bool {
    overflow().is_some() || BUSY.load(Ordering::Acquire)
}

pub(super) fn chevron(automation: &IUIAutomation) -> Result<IUIAutomationElement> {
    unsafe {
        let taskbar = FindWindowW(w!("Shell_TrayWnd"), None)?;
        let root = automation.ElementFromHandle(taskbar)?;
        let condition =
            automation.CreatePropertyCondition(UIA_ClassNamePropertyId, &"SystemTray.ChevronIconView".into())?;
        if let Ok(element) = root.FindFirst(TreeScope_Descendants, &condition) {
            return Ok(element);
        }
        let condition = automation.CreatePropertyCondition(UIA_AutomationIdPropertyId, &WIN10_CHEVRON_ID.into())?;
        if let Ok(element) = root.FindFirst(TreeScope_Descendants, &condition)
            && element.CurrentClassName().is_ok_and(|c| c == "Button")
        {
            return Ok(element);
        }
        let condition = automation.CreatePropertyCondition(UIA_ClassNamePropertyId, &"TrayButton".into())?;
        if let Ok(element) = root.FindFirst(TreeScope_Descendants, &condition) {
            return Ok(element);
        }
        let condition =
            automation.CreatePropertyCondition(UIA_ClassNamePropertyId, &"SystemTray.NormalButton".into())?;
        let buttons = root.FindAll(TreeScope_Descendants, &condition)?;
        for i in 0..buttons.Length()? {
            let button = buttons.GetElement(i)?;
            let name = button.CurrentName()?.to_string().to_lowercase();
            if name.contains("скрытые значки") || name.contains("hidden icons") || name.contains("hidden icon menu")
            {
                return Ok(button);
            }
        }
        Err(windows::core::Error::empty())
    }
}

pub(super) fn show(anchor: (i32, i32)) -> Result<()> {
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
    }
    struct Apartment;
    impl Drop for Apartment {
        fn drop(&mut self) {
            unsafe {
                CoUninitialize();
            }
        }
    }
    let _apartment = Apartment;
    let closing = overflow().is_some();
    let access = TrayAccess::new(!closing)?;
    let automation: IUIAutomation = unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)? };
    let mut element = chevron(&automation);
    if element.is_err() {
        unsafe {
            if let Ok(taskbar) = FindWindowW(w!("Shell_TrayWnd"), None)
                && let Ok(root) = automation.ElementFromHandle(taskbar)
            {
                let _ = root.SetFocus();
            }
        }
    }
    for _ in 0..10 {
        if element.is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_millis(30));
        element = chevron(&automation);
    }
    let element = element.map_err(|_| {
        windows::core::Error::new(windows::core::HRESULT(0x80004005u32 as i32), "Tray chevron is not available")
    })?;
    if closing && overflow().is_none() {
        return Ok(());
    }
    unsafe {
        element.GetCurrentPatternAs::<IUIAutomationInvokePattern>(UIA_InvokePatternId)?.Invoke()?;
    }
    for attempt in 0..50 {
        if closing {
            if overflow().is_none() {
                return Ok(());
            }
        } else if let Some(hwnd) = overflow() {
            unsafe {
                let mut rect = RECT::default();
                GetWindowRect(hwnd, &mut rect)?;
                let monitor = crate::platform::desktop_dock::monitors()
                    .into_iter()
                    .find(|m| m.primary)
                    .ok_or_else(windows::core::Error::empty)?;
                let width = rect.right - rect.left;
                let height = rect.bottom - rect.top;
                let x = (anchor.0 - width)
                    .clamp(monitor.bounds.left, (monitor.bounds.right - width).max(monitor.bounds.left));
                let y = anchor.1.clamp(monitor.bounds.top, (monitor.bounds.bottom - height).max(monitor.bounds.top));
                SetWindowPos(hwnd, Some(HWND_TOPMOST), x, y, 0, 0, SWP_NOSIZE | SWP_NOACTIVATE)?;
                drop(access);
                return Ok(());
            }
        }
        if attempt == 25 && !closing {
            unsafe {
                element.GetCurrentPatternAs::<IUIAutomationInvokePattern>(UIA_InvokePatternId)?.Invoke()?;
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    Err(windows::core::Error::new(windows::core::HRESULT(0x80004005u32 as i32), "Tray popup did not respond"))
}

struct TrayAccess {
    hwnd: HWND,
    restore: Option<RECT>,
    region: Option<HRGN>,
    region_changed: bool,
    foreground: HWND,
}

impl TrayAccess {
    fn new(activate: bool) -> Result<Self> {
        unsafe {
            let hwnd = FindWindowW(w!("Shell_TrayWnd"), None)?;
            SetPropW(hwnd, w!("NightIsland.TrayAccess"), Some(windows::Win32::Foundation::HANDLE(1usize as _)))?;
            let mut access =
                Self { hwnd, restore: None, region: None, region_changed: false, foreground: GetForegroundWindow() };
            if !IsWindowVisible(hwnd).as_bool() {
                let mut rect = RECT::default();
                GetWindowRect(hwnd, &mut rect)?;
                access.restore = Some(rect);
                let original = CreateRectRgn(0, 0, 0, 0);
                if original.0.is_null() {
                    return Err(windows::core::Error::from_thread());
                }
                if GetWindowRgn(hwnd, original).0 != 0 {
                    access.region = Some(original);
                } else {
                    let _ = DeleteObject(original.into());
                }
                let empty = CreateRectRgn(0, 0, 0, 0);
                if empty.0.is_null() {
                    return Err(windows::core::Error::from_thread());
                }
                if SetWindowRgn(hwnd, Some(empty), false) == 0 {
                    let _ = DeleteObject(empty.into());
                    return Err(windows::core::Error::from_thread());
                }
                access.region_changed = true;

                let monitor = crate::platform::desktop_dock::monitors()
                    .into_iter()
                    .find(|m| m.primary)
                    .ok_or_else(windows::core::Error::empty)?;
                SetWindowPos(
                    hwnd,
                    Some(HWND_TOPMOST),
                    rect.left,
                    monitor.bounds.bottom - (rect.bottom - rect.top),
                    0,
                    0,
                    SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
                )?;
                if activate {
                    let _ = SetForegroundWindow(hwnd);
                }
            }
            Ok(access)
        }
    }
}

impl Drop for TrayAccess {
    fn drop(&mut self) {
        unsafe {
            let restore_focus = GetForegroundWindow() == self.hwnd;
            if let Some(rect) = self.restore {
                let _ = ShowWindow(self.hwnd, SW_HIDE);
                let _ = SetWindowPos(
                    self.hwnd,
                    None,
                    rect.left,
                    rect.top,
                    0,
                    0,
                    SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
            if self.region_changed && SetWindowRgn(self.hwnd, self.region, false) != 0 {
                self.region = None;
            }
            if let Some(region) = self.region.take() {
                let _ = DeleteObject(region.into());
            }
            if restore_focus {
                let _ = SetForegroundWindow(self.foreground);
            }
            let _ = RemovePropW(self.hwnd, w!("NightIsland.TrayAccess"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "Checks the tray while the real Island taskbar guardian is running"]
    fn native_tray_with_running_guardian() -> Result<()> {
        use windows::Win32::Graphics::Gdi::EqualRgn;
        struct Region(HRGN);
        impl Drop for Region {
            fn drop(&mut self) {
                unsafe {
                    let _ = DeleteObject(self.0.into());
                }
            }
        }
        let taskbar = unsafe { FindWindowW(w!("Shell_TrayWnd"), None)? };
        let before = Region(unsafe { CreateRectRgn(0, 0, 0, 0) });
        let after = Region(unsafe { CreateRectRgn(0, 0, 0, 0) });
        let region_type = unsafe { GetWindowRgn(taskbar, before.0) };
        let visible = unsafe { IsWindowVisible(taskbar) };
        let restored = || unsafe {
            assert_eq!(GetWindowRgn(taskbar, after.0), region_type);
            if region_type.0 != 0 {
                assert!(EqualRgn(before.0, after.0).as_bool());
            }
            assert_eq!(IsWindowVisible(taskbar), visible);
            assert!(GetPropW(taskbar, w!("NightIsland.TrayAccess")).0.is_null());
        };
        let foreground = unsafe { GetForegroundWindow() };
        struct Restore(HWND);
        impl Drop for Restore {
            fn drop(&mut self) {
                if overflow().is_some() {
                    let _ = show((0, 0));
                }
                unsafe {
                    let _ = SetForegroundWindow(self.0);
                }
            }
        }
        let _restore = Restore(foreground);
        if overflow().is_some() {
            show((0, 0))?;
        }
        show((1200, 56))?;
        assert!(overflow().is_some());
        restored();
        std::thread::sleep(Duration::from_millis(500));
        assert!(overflow().is_some());
        show((1200, 56))?;
        assert!(overflow().is_none());
        restored();
        show((1200, 56))?;
        assert!(overflow().is_some());
        restored();
        show((1200, 56))?;
        std::thread::sleep(Duration::from_millis(500));
        assert!(overflow().is_none());
        restored();
        Ok(())
    }

    #[test]
    #[ignore = "Reads the native Explorer tray accessibility tree"]
    fn native_tray_chevron_is_available() -> Result<()> {
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
            let result = {
                let automation: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)?;
                chevron(&automation).map(|_| ())
            };
            CoUninitialize();
            result
        }
    }
}
