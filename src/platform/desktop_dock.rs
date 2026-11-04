use std::collections::HashMap;

use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, RECT};
use windows::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute};
use windows::Win32::Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFOEXW};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, IPersistFile, STGM_READ};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::Shell::{IShellLinkW, SLGP_RAWPATH, ShellLink};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{BOOL, HSTRING, Interface, PWSTR};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Window {
    pub handle: isize,
    pub path: String,
    pub title: String,
    pub active: bool,
}

#[derive(Clone, Debug)]
pub struct Monitor {
    pub device: String,
    pub bounds: RECT,
    pub work: RECT,
    pub primary: bool,
}

pub fn monitors() -> Vec<Monitor> {
    unsafe extern "system" fn each(handle: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> BOOL {
        let out = unsafe { &mut *(data.0 as *mut Vec<Monitor>) };
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
        if unsafe { GetMonitorInfoW(handle, &mut info.monitorInfo) }.as_bool() {
            let end = info.szDevice.iter().position(|c| *c == 0).unwrap_or(info.szDevice.len());
            out.push(Monitor {
                device: String::from_utf16_lossy(&info.szDevice[..end]),
                bounds: info.monitorInfo.rcMonitor,
                work: info.monitorInfo.rcWork,
                primary: info.monitorInfo.dwFlags & 1 != 0,
            });
        }
        true.into()
    }
    let mut out = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(None, None, Some(each), LPARAM((&raw mut out) as isize));
    }
    out.sort_by_key(|m: &Monitor| (!m.primary, m.bounds.left, m.bounds.top));
    out
}

pub fn resolved_target(target: &str) -> String {
    if !target.to_lowercase().ends_with(".lnk") {
        return target.to_string();
    }
    let resolve = || -> windows::core::Result<String> {
        unsafe {
            let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
            link.cast::<IPersistFile>()?.Load(&HSTRING::from(target), STGM_READ)?;
            let mut path = [0u16; 2048];
            link.GetPath(&mut path, std::ptr::null_mut(), SLGP_RAWPATH.0 as u32)?;
            let end = path.iter().position(|c| *c == 0).unwrap_or(path.len());
            Ok(String::from_utf16_lossy(&path[..end]))
        }
    };
    resolve().ok().filter(|p| !p.is_empty()).unwrap_or_else(|| target.to_string())
}

fn process_path(pid: u32) -> Option<String> {
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buffer = [0u16; 2048];
        let mut len = buffer.len() as u32;
        let result = QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, PWSTR(buffer.as_mut_ptr()), &mut len);
        let _ = CloseHandle(process);
        result.ok().map(|_| String::from_utf16_lossy(&buffer[..len as usize]))
    }
}

pub fn windows() -> Vec<Window> {
    struct Scan {
        out: Vec<Window>,
        paths: HashMap<u32, Option<String>>,
        foreground: HWND,
    }
    unsafe extern "system" fn each(hwnd: HWND, param: LPARAM) -> BOOL {
        let scan = unsafe { &mut *(param.0 as *mut Scan) };
        unsafe {
            if !IsWindowVisible(hwnd).as_bool() || GetWindowTextLengthW(hwnd) == 0 {
                return true.into();
            }
            let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
            if style & WS_EX_TOOLWINDOW.0 != 0
                || (GetWindow(hwnd, GW_OWNER).is_ok_and(|owner| !owner.0.is_null()) && style & WS_EX_APPWINDOW.0 == 0)
            {
                return true.into();
            }
            let mut cloaked = 0u32;
            let _ = DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED, (&raw mut cloaked).cast(), size_of::<u32>() as u32);
            if cloaked != 0 {
                return true.into();
            }
            let mut class = [0u16; 128];
            let n = GetClassNameW(hwnd, &mut class) as usize;
            if matches!(
                String::from_utf16_lossy(&class[..n]).as_str(),
                "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd"
            ) {
                return true.into();
            }
            let mut pid = 0;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid == std::process::id() {
                return true.into();
            }
            let path = scan.paths.entry(pid).or_insert_with(|| process_path(pid));
            let Some(path) = path else { return true.into() };
            let mut title = [0u16; 512];
            let n = GetWindowTextW(hwnd, &mut title) as usize;
            scan.out.push(Window {
                handle: hwnd.0 as isize,
                path: path.clone(),
                title: String::from_utf16_lossy(&title[..n]),
                active: hwnd == scan.foreground,
            });
        }
        (scan.out.len() < 256).into()
    }
    let mut scan = Scan { out: Vec::new(), paths: HashMap::new(), foreground: unsafe { GetForegroundWindow() } };
    unsafe {
        let _ = EnumWindows(Some(each), LPARAM((&raw mut scan) as isize));
    }
    scan.out
}

pub fn activate(window: &Window, minimize_active: bool) {
    let hwnd = HWND(window.handle as *mut _);
    unsafe {
        if !IsWindow(Some(hwnd)).as_bool() {
            return;
        }
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if process_path(pid)
            .is_none_or(|path| crate::data::desktop_dock::key(&path) != crate::data::desktop_dock::key(&window.path))
        {
            return;
        }
        if minimize_active && GetForegroundWindow() == hwnd {
            let _ = ShowWindowAsync(hwnd, SW_MINIMIZE);
        } else {
            if IsIconic(hwnd).as_bool() {
                let _ = ShowWindowAsync(hwnd, SW_RESTORE);
            }
            let _ = SetForegroundWindow(hwnd);
        }
    }
}

pub fn fullscreen(monitor: &Monitor) -> bool {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() || IsIconic(hwnd).as_bool() {
            return false;
        }
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == std::process::id() {
            return false;
        }
        let mut class = [0u16; 128];
        let n = GetClassNameW(hwnd, &mut class) as usize;
        if matches!(String::from_utf16_lossy(&class[..n]).as_str(), "Progman" | "WorkerW") {
            return false;
        }
        let mut rect = RECT::default();
        GetWindowRect(hwnd, &mut rect).is_ok()
            && rect.left <= monitor.bounds.left
            && rect.top <= monitor.bounds.top
            && rect.right >= monitor.bounds.right
            && rect.bottom >= monitor.bounds.bottom
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::windows::process::CommandExt;
    use std::process::{Child, Command, Stdio};
    use std::time::{Duration, Instant};
    use windows::Win32::Foundation::WPARAM;
    use windows::core::w;

    #[test]
    #[ignore = "Helper for the isolated dock window test"]
    fn native_dock_window_fixture() {
        let Some(file) = std::env::var_os("NI_DOCK_WINDOW_FIXTURE") else { return };
        unsafe {
            let instance = windows::Win32::System::LibraryLoader::GetModuleHandleW(None).unwrap();
            let handles: Vec<_> = (0..2)
                .map(|_| {
                    CreateWindowExW(
                        WS_EX_NOACTIVATE,
                        w!("STATIC"),
                        w!("Night Island dock test fixture"),
                        WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                        -12000,
                        -12000,
                        320,
                        180,
                        None,
                        None,
                        Some(instance.into()),
                        None,
                    )
                    .unwrap()
                })
                .collect();
            std::fs::write(
                file,
                serde_json::to_vec(&handles.iter().map(|h| h.0 as isize).collect::<Vec<_>>()).unwrap(),
            )
            .unwrap();
            let started = Instant::now();
            while started.elapsed() < Duration::from_secs(15) && handles.iter().any(|h| IsWindow(Some(*h)).as_bool()) {
                let mut msg = MSG::default();
                while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            for hwnd in handles {
                let _ = DestroyWindow(hwnd);
            }
        }
    }

    #[test]
    #[ignore = "Creates temporary offscreen native windows in a child process"]
    fn native_dock_discovers_and_restores_other_process_windows() {
        struct Fixture {
            child: Child,
            file: std::path::PathBuf,
            foreground: HWND,
        }
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = self.child.kill();
                let _ = self.child.wait();
                let _ = std::fs::remove_file(&self.file);
                unsafe {
                    let _ = SetForegroundWindow(self.foreground);
                }
            }
        }
        let file = std::env::temp_dir().join(format!("night-island-dock-fixture-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&file);
        let child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "platform::desktop_dock::tests::native_dock_window_fixture",
                "--test-threads=1",
            ])
            .env("NI_DOCK_WINDOW_FIXTURE", &file)
            .creation_flags(0x08000000)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let fixture = Fixture { child, file, foreground: unsafe { GetForegroundWindow() } };
        let wait = |predicate: &dyn Fn() -> bool| {
            let started = Instant::now();
            while !predicate() && started.elapsed() < Duration::from_secs(5) {
                std::thread::sleep(Duration::from_millis(20));
            }
            assert!(predicate());
        };
        wait(&|| {
            std::fs::read(&fixture.file).ok().and_then(|b| serde_json::from_slice::<Vec<isize>>(&b).ok()).is_some()
        });
        let handles: Vec<isize> = serde_json::from_slice(&std::fs::read(&fixture.file).unwrap()).unwrap();
        let snapshot = windows();
        let found: Vec<_> = snapshot.into_iter().filter(|w| handles.contains(&w.handle)).collect();
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].path, found[1].path);
        assert!(!found[0].path.is_empty());
        let hwnd = HWND(found[0].handle as *mut _);
        unsafe {
            let _ = ShowWindowAsync(hwnd, SW_MINIMIZE);
        }
        wait(&|| unsafe { IsIconic(hwnd) }.as_bool());
        assert!(windows().iter().any(|w| w.handle == found[0].handle));
        activate(&found[0], false);
        wait(&|| !unsafe { IsIconic(hwnd) }.as_bool());
        unsafe {
            let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
        }
        wait(&|| !windows().iter().any(|w| w.handle == found[0].handle));
    }
}
