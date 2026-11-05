use super::*;
use std::sync::{Arc, Mutex};
use windows::Win32::Foundation::ERROR_NO_MORE_ITEMS;
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::System::Registry::*;
use windows::core::{HSTRING, Interface, PWSTR};

pub const WM_RESULT: u32 = WM_APP + 96;
static RESULT: Mutex<Option<std::result::Result<Vec<Entry>, ()>>> = Mutex::new(None);
type IconRecords = Vec<(String, String, Arc<crate::platform::apps::Icon>)>;
static ICON_CACHE: Mutex<Option<(std::time::Instant, IconRecords)>> = Mutex::new(None);

#[derive(Clone)]
pub struct Entry {
    pub name: String,
    pub icon: Option<Arc<crate::platform::apps::Icon>>,
}

pub fn recover() {
    unsafe {
        for class in [w!("TopLevelWindowForOverflowXamlIsland"), w!("NotifyIconOverflowWindow")] {
            if let Ok(hwnd) = FindWindowW(class, None)
                && !GetPropW(hwnd, w!("NightIsland.TrayMirror")).0.is_null()
            {
                let _ = ShowWindow(hwnd, SW_HIDE);
                SetWindowRgn(hwnd, None, false);
                let _ = RemovePropW(hwnd, w!("NightIsland.TrayMirror"));
            }
        }
    }
}

struct HiddenPopup {
    hwnd: HWND,
    region: Option<HRGN>,
}

impl HiddenPopup {
    fn new() -> Result<Self> {
        recover();
        unsafe {
            let find = || {
                [w!("TopLevelWindowForOverflowXamlIsland"), w!("NotifyIconOverflowWindow")]
                    .into_iter()
                    .find_map(|class| FindWindowW(class, None).ok())
            };
            let hwnd = if let Some(hwnd) = find() {
                hwnd
            } else {
                show((0, 0))?;
                find().ok_or_else(windows::core::Error::empty)?
            };
            let original = CreateRectRgn(0, 0, 0, 0);
            if original.0.is_null() {
                return Err(windows::core::Error::from_thread());
            }
            let region = if GetWindowRgn(hwnd, original).0 != 0 {
                Some(original)
            } else {
                let _ = DeleteObject(original.into());
                None
            };
            let mut guard = Self { hwnd, region };
            let empty = CreateRectRgn(0, 0, 0, 0);
            if empty.0.is_null() {
                return Err(windows::core::Error::from_thread());
            }
            if SetWindowRgn(hwnd, Some(empty), false) == 0 {
                let _ = DeleteObject(empty.into());
                guard.hwnd = HWND::default();
                return Err(windows::core::Error::from_thread());
            }
            SetPropW(
                hwnd,
                w!("NightIsland.TrayMirror"),
                Some(windows::Win32::Foundation::HANDLE(std::process::id() as usize as _)),
            )?;
            Ok(guard)
        }
    }
}

impl Drop for HiddenPopup {
    fn drop(&mut self) {
        unsafe {
            if !self.hwnd.0.is_null() {
                let _ = ShowWindow(self.hwnd, SW_HIDE);
                if SetWindowRgn(self.hwnd, self.region, false) != 0 {
                    self.region = None;
                }
                let _ = RemovePropW(self.hwnd, w!("NightIsland.TrayMirror"));
            }
            if let Some(region) = self.region.take() {
                let _ = DeleteObject(region.into());
            }
        }
    }
}

fn registry_icons() -> IconRecords {
    if let Ok(cache) = ICON_CACHE.lock()
        && let Some((time, icons)) = &*cache
        && time.elapsed() < Duration::from_secs(60)
    {
        return icons.clone();
    }
    let icons = load_registry_icons();
    if let Ok(mut cache) = ICON_CACHE.lock() {
        *cache = Some((std::time::Instant::now(), icons.clone()));
    }
    icons
}

fn load_registry_icons() -> IconRecords {
    unsafe {
        let mut key = HKEY::default();
        if RegOpenKeyExW(HKEY_CURRENT_USER, w!("Control Panel\\NotifyIconSettings"), None, KEY_READ, &mut key).is_err()
        {
            return Vec::new();
        }
        struct Key(HKEY);
        impl Drop for Key {
            fn drop(&mut self) {
                unsafe {
                    let _ = RegCloseKey(self.0);
                }
            }
        }
        let _key = Key(key);
        let mut result = Vec::new();
        for index in 0..4096 {
            let mut name = [0u16; 256];
            let mut len = name.len() as u32;
            let status = RegEnumKeyExW(key, index, Some(PWSTR(name.as_mut_ptr())), &mut len, None, None, None, None);
            if status == ERROR_NO_MORE_ITEMS {
                break;
            }
            if status.is_err() {
                continue;
            }
            let sub = HSTRING::from(String::from_utf16_lossy(&name[..len as usize]));
            let read = |field, flags| -> Vec<u8> {
                let mut size = 0;
                if RegGetValueW(key, &sub, field, flags, None, None, Some(&mut size)).is_err() || size > 262144 {
                    return Vec::new();
                }
                let mut data = vec![0u8; size as usize];
                if RegGetValueW(key, &sub, field, flags, None, Some(data.as_mut_ptr().cast()), Some(&mut size)).is_err()
                {
                    return Vec::new();
                }
                data.truncate(size as usize);
                data
            };
            let string = |field| -> String {
                let bytes = read(field, RRF_RT_REG_SZ);
                let units: Vec<_> =
                    bytes.chunks_exact(2).map(|b| u16::from_le_bytes([b[0], b[1]])).take_while(|v| *v != 0).collect();
                String::from_utf16_lossy(&units)
            };
            let tip = string(w!("InitialTooltip"));
            let path = string(w!("ExecutablePath"));
            let data = read(w!("IconSnapshot"), RRF_RT_REG_BINARY);
            if data.len() < 24 || &data[..8] != b"\x89PNG\r\n\x1a\n" {
                continue;
            }
            let width = u32::from_be_bytes(data[16..20].try_into().unwrap());
            let height = u32::from_be_bytes(data[20..24].try_into().unwrap());
            if width == 0 || width != height || width > 256 {
                continue;
            }
            let mut decoder = png::Decoder::new(std::io::Cursor::new(data));
            decoder.set_transformations(
                png::Transformations::EXPAND | png::Transformations::STRIP_16 | png::Transformations::ALPHA,
            );
            let Ok(mut reader) = decoder.read_info() else {
                continue;
            };
            let Some(size) = reader.output_buffer_size() else {
                continue;
            };
            let mut bgra = vec![0; size];
            let Ok(info) = reader.next_frame(&mut bgra) else {
                continue;
            };
            bgra.truncate(info.buffer_size());
            if info.color_type != png::ColorType::Rgba {
                continue;
            }
            for pixel in bgra.chunks_exact_mut(4) {
                pixel.swap(0, 2);
                for channel in 0..3 {
                    pixel[channel] = (u16::from(pixel[channel]) * u16::from(pixel[3]) / 255) as u8;
                }
            }
            result.push((tip, path, Arc::new(crate::platform::apps::Icon { side: width, bgra })));
        }
        result
    }
}

#[cfg(test)]
pub(crate) fn preview_entries() -> Vec<Entry> {
    registry_icons()
        .into_iter()
        .filter(|(name, _, _)| !name.is_empty())
        .take(9)
        .map(|(name, _, icon)| Entry { name, icon: Some(icon) })
        .collect()
}

fn match_icon(
    name: &str,
    icons: &[(String, String, Arc<crate::platform::apps::Icon>)],
) -> Option<Arc<crate::platform::apps::Icon>> {
    let name = name.trim().to_lowercase();
    icons
        .iter()
        .filter_map(|(tip, path, icon)| {
            let tip = tip.trim().to_lowercase();
            let stem = std::path::Path::new(path).file_stem().unwrap_or_default().to_string_lossy().to_lowercase();
            let score = if !tip.is_empty() && name == tip {
                10000 + tip.len()
            } else if tip.len() > 2 && (name.starts_with(&tip) || tip.starts_with(&name)) {
                tip.len()
            } else if stem.len() > 2 && name.contains(&stem) {
                stem.len()
            } else {
                name.split(|c: char| !c.is_alphanumeric())
                    .filter(|word| word.len() >= 4 && path.to_lowercase().contains(word))
                    .map(str::len)
                    .max()
                    .unwrap_or(0)
            };
            (score > 0).then_some((score, icon))
        })
        .max_by_key(|(score, _)| *score)
        .map(|(_, icon)| icon.clone())
}

fn elements(automation: &IUIAutomation, hwnd: HWND) -> Result<Vec<IUIAutomationElement>> {
    unsafe {
        let root = automation.ElementFromHandle(hwnd)?;
        let condition =
            automation.CreatePropertyCondition(UIA_ControlTypePropertyId, &UIA_ButtonControlTypeId.0.into())?;
        let all = root.FindAll(TreeScope_Descendants, &condition)?;
        let mut result = Vec::new();
        for i in 0..all.Length()?.min(128) {
            let item = all.GetElement(i)?;
            let class = item.CurrentClassName()?.to_string();
            if matches!(
                class.as_str(),
                "SystemTray.NotifyIconView" | "SystemTray.NormalButton" | "ToolbarWindow32" | "Button"
            ) {
                result.push(item);
            }
        }
        Ok(result)
    }
}

fn menu_open() -> bool {
    unsafe extern "system" fn visit(hwnd: HWND, data: LPARAM) -> windows::core::BOOL {
        unsafe {
            if IsWindowVisible(hwnd).as_bool() {
                let mut class = [0u16; 16];
                let len = GetClassNameW(hwnd, &mut class) as usize;
                if class[..len] == [35, 51, 50, 55, 54, 56] {
                    *(data.0 as *mut bool) = true;
                    return windows::core::BOOL(0);
                }
            }
            windows::core::BOOL(1)
        }
    }
    let mut open = false;
    unsafe {
        let _ = EnumWindows(Some(visit), LPARAM((&raw mut open) as isize));
    }
    open
}

fn collect(anchor: (i32, i32), action: Option<(&str, bool)>) -> Result<Vec<Entry>> {
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
    let automation: IUIAutomation = unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)? };
    let hidden = HiddenPopup::new()?;
    let hwnd = hidden.hwnd;
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
    }
    let items = elements(&automation, hwnd)?;
    if let Some((name, context)) = action {
        let item =
            items.iter().find(|item| unsafe { item.CurrentName().is_ok_and(|n| n == name) }).ok_or_else(|| {
                windows::core::Error::new(windows::core::HRESULT(0x80004005u32 as i32), "Tray item unavailable")
            })?;
        unsafe {
            let rect = item.CurrentBoundingRectangle()?;
            let mut popup = RECT::default();
            GetWindowRect(hwnd, &mut popup)?;
            SetWindowPos(
                hwnd,
                None,
                popup.left + anchor.0 - (rect.left + rect.right) / 2,
                popup.top + anchor.1 - rect.bottom,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            )?;
            if context {
                item.cast::<IUIAutomationElement3>()?.ShowContextMenu()?;
                let start = std::time::Instant::now();
                while !menu_open() && start.elapsed() < Duration::from_millis(150) {
                    std::thread::sleep(Duration::from_millis(10));
                }
                while menu_open() {
                    std::thread::sleep(Duration::from_millis(25));
                }
            } else {
                item.GetCurrentPatternAs::<IUIAutomationInvokePattern>(UIA_InvokePatternId)?.Invoke()?;
            }
        }
        return Ok(Vec::new());
    }
    let icons = registry_icons();
    let mut entries = Vec::new();
    for item in items {
        let name = unsafe { item.CurrentName()?.to_string() };
        if name.trim().is_empty() {
            continue;
        }
        let icon = match_icon(&name, &icons);
        entries.push(Entry { name, icon });
    }
    if entries.is_empty() {
        return Err(windows::core::Error::empty());
    }
    Ok(entries)
}

pub fn request(parent: HWND, anchor: (i32, i32), action: Option<(String, bool)>) {
    if BUSY.swap(true, Ordering::AcqRel) {
        return;
    }
    let parent = parent.0 as isize;
    std::thread::spawn(move || {
        let result = collect(anchor, action.as_ref().map(|(name, context)| (name.as_str(), *context))).map_err(|_| ());
        if let Ok(mut slot) = RESULT.lock() {
            *slot = Some(result);
        }
        BUSY.store(false, Ordering::Release);
        unsafe {
            let _ = PostMessageW(Some(HWND(parent as _)), WM_RESULT, WPARAM(usize::from(action.is_some())), LPARAM(0));
        }
    });
}

pub fn take() -> Option<std::result::Result<Vec<Entry>, ()>> {
    RESULT.lock().ok()?.take()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "Creates a disposable notification icon and verifies its left and right click callbacks"]
    fn native_inline_tray_actions() -> Result<()> {
        use std::sync::atomic::AtomicU32;
        use windows::Win32::UI::Shell::*;
        static CLICKS: AtomicU32 = AtomicU32::new(0);
        unsafe extern "system" fn proc(
            hwnd: HWND,
            msg: u32,
            w: WPARAM,
            l: LPARAM,
        ) -> windows::Win32::Foundation::LRESULT {
            unsafe {
                if msg == WM_APP + 121 {
                    match (l.0 & 0xffff) as u32 {
                        WM_LBUTTONUP => {
                            CLICKS.fetch_or(1, Ordering::Release);
                        }
                        WM_RBUTTONUP | WM_CONTEXTMENU => {
                            CLICKS.fetch_or(2, Ordering::Release);
                            let menu = CreatePopupMenu().unwrap();
                            AppendMenuW(menu, MF_STRING, 1, w!("Fixture action")).unwrap();
                            let _ = SetForegroundWindow(hwnd);
                            SetTimer(Some(hwnd), 1, 3000, None);
                            let _ = TrackPopupMenu(menu, TPM_RETURNCMD, 1200, 100, None, hwnd, None);
                            let _ = KillTimer(Some(hwnd), 1);
                            let _ = DestroyMenu(menu);
                        }
                        _ => {}
                    }
                }
                if msg == WM_TIMER {
                    let _ = EndMenu();
                }
                if msg == WM_CLOSE {
                    let _ = DestroyWindow(hwnd);
                    PostQuitMessage(0);
                }
                DefWindowProcW(hwnd, msg, w, l)
            }
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let thread = std::thread::spawn(move || unsafe {
            let class = w!("NightIsland.InlineTrayFixture");
            RegisterClassW(&WNDCLASSW { lpfnWndProc: Some(proc), lpszClassName: class, ..Default::default() });
            let hwnd =
                CreateWindowExW(WS_EX_TOOLWINDOW, class, class, WS_POPUP, 0, 0, 1, 1, None, None, None, None).unwrap();
            let mut data = NOTIFYICONDATAW {
                cbSize: size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: hwnd,
                uID: 71,
                uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
                uCallbackMessage: WM_APP + 121,
                hIcon: LoadIconW(None, IDI_APPLICATION).unwrap(),
                ..Default::default()
            };
            for (dst, src) in data.szTip.iter_mut().zip("Island tray fixture".encode_utf16()) {
                *dst = src;
            }
            assert!(Shell_NotifyIconW(NIM_ADD, &data).as_bool());
            tx.send(hwnd.0 as isize).unwrap();
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                DispatchMessageW(&msg);
            }
            let _ = Shell_NotifyIconW(NIM_DELETE, &data);
        });
        struct Fixture(isize, Option<std::thread::JoinHandle<()>>);
        impl Drop for Fixture {
            fn drop(&mut self) {
                unsafe {
                    let _ = PostMessageW(Some(HWND(self.0 as _)), WM_CLOSE, WPARAM(0), LPARAM(0));
                }
                if let Some(thread) = self.1.take() {
                    let _ = thread.join();
                }
            }
        }
        let _fixture = Fixture(rx.recv_timeout(Duration::from_secs(5)).unwrap(), Some(thread));
        std::thread::sleep(Duration::from_millis(250));
        collect((1400, 60), Some(("Island tray fixture", false)))?;
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(CLICKS.load(Ordering::Acquire) & 1, 1);
        let fixture = _fixture.0;
        let observer = std::thread::spawn(move || unsafe {
            let deadline = std::time::Instant::now() + Duration::from_secs(2);
            let owner = HWND(fixture as _);
            let thread = GetWindowThreadProcessId(owner, None);
            let menu = loop {
                if let Ok(menu) = FindWindowW(w!("#32768"), None)
                    && GetWindowThreadProcessId(menu, None) == thread
                    && IsWindowVisible(menu).as_bool()
                {
                    break Some(menu);
                }
                if std::time::Instant::now() >= deadline {
                    break None;
                }
                std::thread::sleep(Duration::from_millis(10));
            };
            std::thread::sleep(Duration::from_millis(500));
            let stayed_open = menu.is_some_and(|menu| IsWindowVisible(menu).as_bool());
            let _ = PostMessageW(Some(owner), WM_CANCELMODE, WPARAM(0), LPARAM(0));
            stayed_open
        });
        collect((1400, 60), Some(("Island tray fixture", true)))?;
        assert!(observer.join().unwrap(), "The actual context menu must stay open after the bridge returns");
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(CLICKS.load(Ordering::Acquire) & 2, 2);
        Ok(())
    }

    #[test]
    #[ignore = "Reads actual Explorer tray icons without invoking applications"]
    fn native_inline_tray_reads_icons() -> Result<()> {
        let entries = collect((1400, 60), None)?;
        assert!(entries.iter().filter(|entry| entry.icon.is_some()).count() >= entries.len() / 2);
        for entry in entries {
            eprintln!("icon available: {} / {}", entry.icon.is_some(), entry.name);
        }
        assert!(overflow().is_none());
        Ok(())
    }
}
