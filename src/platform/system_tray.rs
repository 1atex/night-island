use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use windows::Win32::Foundation::{HWND, LPARAM, RECT, WPARAM};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
};
use windows::Win32::UI::Accessibility::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{Result, w};

pub const WM_TRAY_RESULT: u32 = WM_APP + 94;
static BUSY: AtomicBool = AtomicBool::new(false);

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
        for class in ["SystemTray.ChevronIconView", "TrayButton"] {
            let condition = automation.CreatePropertyCondition(UIA_ClassNamePropertyId, &class.into())?;
            if let Ok(element) = root.FindFirst(TreeScope_Descendants, &condition) {
                return Ok(element);
            }
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
    let access = TrayAccess::new()?;
    let automation: IUIAutomation = unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)? };
    let mut element = chevron(&automation);
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
}

impl TrayAccess {
    fn new() -> Result<Self> {
        unsafe {
            let hwnd = FindWindowW(w!("Shell_TrayWnd"), None)?;
            SetPropW(hwnd, w!("NightIsland.TrayAccess"), Some(windows::Win32::Foundation::HANDLE(1usize as _)))?;
            let mut access = Self { hwnd, restore: None };
            if !IsWindowVisible(hwnd).as_bool() {
                let mut rect = RECT::default();
                GetWindowRect(hwnd, &mut rect)?;
                access.restore = Some(rect);
                let bottom = crate::platform::desktop_dock::monitors()
                    .iter()
                    .map(|m| m.bounds.bottom)
                    .max()
                    .unwrap_or(rect.bottom);
                SetWindowPos(
                    hwnd,
                    None,
                    rect.left,
                    bottom + 128,
                    0,
                    0,
                    SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_SHOWWINDOW,
                )?;
            }
            Ok(access)
        }
    }
}

impl Drop for TrayAccess {
    fn drop(&mut self) {
        unsafe {
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
            let _ = RemovePropW(self.hwnd, w!("NightIsland.TrayAccess"));
        }
    }
}

pub fn toggle(parent: HWND, anchor: (i32, i32)) {
    if BUSY.swap(true, Ordering::AcqRel) {
        return;
    }
    let parent = parent.0 as isize;
    std::thread::spawn(move || {
        let ok = show(anchor).is_ok();
        BUSY.store(false, Ordering::Release);
        unsafe {
            let _ = PostMessageW(Some(HWND(parent as _)), WM_TRAY_RESULT, WPARAM(usize::from(ok)), LPARAM(0));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "Reads the native Explorer tray accessibility tree"]
    fn native_tray_chevron_is_available() -> Result<()> {
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
            let automation: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)?;
            let result = chevron(&automation).map(|_| ());
            CoUninitialize();
            result
        }
    }
}
