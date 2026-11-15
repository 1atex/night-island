use std::ffi::c_void;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{COLORREF, CloseHandle, HWND, LPARAM, WPARAM};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GWL_EXSTYLE, GetClassNameW, GetForegroundWindow, GetWindowLongPtrW, GetWindowThreadProcessId,
    IsWindowVisible, LWA_ALPHA, PostMessageW, SetForegroundWindow, SetLayeredWindowAttributes, SetWindowLongPtrW,
    WM_APP, WM_CLOSE, WS_EX_LAYERED,
};
use windows::core::{BOOL, PWSTR};

pub const WM_VPN: u32 = WM_APP + 32;

const WATCH_EVERY: Duration = Duration::from_secs(2);
const TUNNEL_PROGRAMS: [&str; 3] = ["xray.exe", "sing-box.exe", "tun2proxy-bin.exe"];

const OFF: u8 = 0;
const ON: u8 = 1;
const UNKNOWN: u8 = 2;

static STATE: AtomicU8 = AtomicU8::new(UNKNOWN);
static ENABLED: AtomicBool = AtomicBool::new(false);
static SWITCHING: AtomicBool = AtomicBool::new(false);
static WATCHING: AtomicBool = AtomicBool::new(false);
static GENERATION: AtomicU64 = AtomicU64::new(0);
static RESULT: Mutex<Option<Result<(), &'static str>>> = Mutex::new(None);

pub fn exe_path() -> Option<PathBuf> {
    static PATH: OnceLock<Option<PathBuf>> = OnceLock::new();
    PATH.get_or_init(|| {
        let base = std::env::var_os("ProgramFiles")?;
        let path = PathBuf::from(base).join("FlyFrogLLC").join("Happ").join("Happ.exe");
        path.is_file().then_some(path)
    })
    .clone()
}

pub fn installed() -> bool {
    exe_path().is_some()
}

pub fn state() -> Option<bool> {
    match STATE.load(Ordering::SeqCst) {
        ON => Some(true),
        OFF => Some(false),
        _ => None,
    }
}

pub fn switching() -> bool {
    SWITCHING.load(Ordering::SeqCst)
}

pub fn take_result() -> Option<Result<(), &'static str>> {
    RESULT.lock().ok()?.take()
}

pub fn set_enabled(on: bool) {
    GENERATION.fetch_add(1, Ordering::SeqCst);
    if let Ok(mut result) = RESULT.lock() {
        *result = None;
    }
    ENABLED.store(on && installed(), Ordering::SeqCst);
    if !on {
        STATE.store(UNKNOWN, Ordering::SeqCst);
    }
}

pub fn start_watch(hwnd: HWND) {
    if WATCHING.swap(true, Ordering::SeqCst) {
        return;
    }
    let raw = hwnd.0 as isize;
    std::thread::spawn(move || {
        loop {
            if ENABLED.load(Ordering::SeqCst) {
                let now = if tunnel_up() { ON } else { OFF };
                if STATE.swap(now, Ordering::SeqCst) != now {
                    post(raw);
                }
            }
            std::thread::sleep(WATCH_EVERY);
        }
    });
}

fn post(raw: isize) {
    unsafe {
        let _ = PostMessageW(Some(HWND(raw as *mut c_void)), WM_VPN, WPARAM(0), LPARAM(0));
    }
}

pub(super) fn image_path(pid: u32) -> Option<String> {
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len).is_ok();
        let _ = CloseHandle(process);
        ok.then(|| String::from_utf16_lossy(&buf[..len as usize]))
    }
}

fn in_happ_folder(path: &str) -> bool {
    path.to_lowercase().contains("\\happ\\")
}

fn tunnel_up() -> bool {
    let (mut happ_running, mut unknown_owner) = (false, false);
    unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else { return false };
        let mut entry = PROCESSENTRY32W { dwSize: size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut ok = Process32FirstW(snap, &mut entry).is_ok();
        while ok {
            let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
            let name = String::from_utf16_lossy(&entry.szExeFile[..len]).to_lowercase();
            if name == "happd.exe" || name == "happ.exe" {
                happ_running = true;
            } else if TUNNEL_PROGRAMS.contains(&name.as_str()) {
                match image_path(entry.th32ProcessID) {
                    Some(path) if in_happ_folder(&path) => {
                        let _ = CloseHandle(snap);
                        return true;
                    }
                    Some(_) => {}
                    None => unknown_owner = true,
                }
            }
            ok = Process32NextW(snap, &mut entry).is_ok();
        }
        let _ = CloseHandle(snap);
    }
    unknown_owner && happ_running
}

fn link_for(up: bool) -> &'static str {
    if up { "happ://disconnect" } else { "happ://connect" }
}

fn is_main_class(class: &str) -> bool {
    class.starts_with("Qt") && class.ends_with("QWindowIcon")
}

fn main_window() -> Option<HWND> {
    unsafe extern "system" fn each(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let found = unsafe { &mut *(lparam.0 as *mut Option<HWND>) };
        let mut buf = [0u16; 64];
        let n = unsafe { GetClassNameW(hwnd, &mut buf) } as usize;
        if is_main_class(&String::from_utf16_lossy(&buf[..n])) {
            let mut pid = 0u32;
            unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
            if image_path(pid).is_some_and(|p| p.to_lowercase().ends_with("\\happ.exe")) {
                *found = Some(hwnd);
                return false.into();
            }
        }
        true.into()
    }
    let mut found: Option<HWND> = None;
    unsafe {
        let _ = EnumWindows(Some(each), LPARAM((&raw mut found) as isize));
    }
    found
}

fn visible(hwnd: HWND) -> bool {
    unsafe { IsWindowVisible(hwnd) }.as_bool()
}

fn set_see_through(hwnd: HWND, on: bool) {
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        if on {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style | WS_EX_LAYERED.0 as isize);
            let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 0, LWA_ALPHA);
        } else {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style & !(WS_EX_LAYERED.0 as isize));
        }
    }
}

fn hide_main(hwnd: HWND, until: Instant) {
    while visible(hwnd) && Instant::now() < until {
        unsafe {
            let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
        }
        std::thread::sleep(Duration::from_millis(60));
    }
}

fn switch_now() -> Result<(), &'static str> {
    run_link(link_for(tunnel_up()))
}

fn run_link(link: &'static str) -> Result<(), &'static str> {
    let exe = exe_path().ok_or("Happ is not installed")?;
    let main = main_window();
    let quiet = main.is_none_or(|w| !visible(w));
    let front = unsafe { GetForegroundWindow() };
    if let Some(w) = main.filter(|_| quiet) {
        set_see_through(w, true);
    }
    let started = Command::new(exe).arg(link).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
    if started.is_err() {
        if let Some(w) = main {
            set_see_through(w, false);
        }
        return Err("Happ could not be started");
    }
    if quiet {
        let wait = Duration::from_secs(if main.is_some() { 4 } else { 25 });
        let end = Instant::now() + wait;
        let mut shown = None;
        while shown.is_none() && Instant::now() < end {
            std::thread::sleep(Duration::from_millis(8));
            shown = main_window().filter(|&w| visible(w));
        }
        if let Some(w) = shown {
            if main.is_none() {
                set_see_through(w, true);
            }
            hide_main(w, Instant::now() + Duration::from_secs(3));
        }
        if let Some(w) = main_window() {
            set_see_through(w, false);
        }
        let now = unsafe { GetForegroundWindow() };
        if shown.is_some() && now != front && !front.is_invalid() && !now.is_invalid() {
            let mut pid = 0u32;
            unsafe { GetWindowThreadProcessId(now, Some(&mut pid)) };
            if image_path(pid).is_some_and(|p| p.to_lowercase().ends_with("\\happ.exe")) {
                unsafe {
                    let _ = SetForegroundWindow(front);
                }
            }
        }
    }
    Ok(())
}

pub fn toggle(hwnd: HWND) -> bool {
    if SWITCHING.swap(true, Ordering::SeqCst) {
        return false;
    }
    let raw = hwnd.0 as isize;
    let generation = GENERATION.load(Ordering::SeqCst);
    std::thread::spawn(move || {
        let outcome = switch_now();
        if generation == GENERATION.load(Ordering::SeqCst)
            && let Ok(mut r) = RESULT.lock()
        {
            *r = Some(outcome);
        }
        SWITCHING.store(false, Ordering::SeqCst);
        post(raw);
    });
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_switch_asks_for_the_opposite_state() {
        assert_eq!(link_for(true), "happ://disconnect");
        assert_eq!(link_for(false), "happ://connect");
    }

    #[test]
    fn happs_main_window_is_found_by_class_in_any_qt_version() {
        assert!(is_main_class("Qt6101QWindowIcon"));
        assert!(is_main_class("Qt6112QWindowIcon"));
        assert!(!is_main_class("Qt6112ThemeChangeObserverWindow"));
        assert!(!is_main_class("_q_titlebar"));
    }

    #[test]
    fn only_programs_in_happs_folder_count() {
        assert!(in_happ_folder(r"C:\Program Files\FlyFrogLLC\Happ\core\xray.exe"));
        assert!(in_happ_folder(r"c:\program files\flyfrogllc\HAPP\tun\sing-box.exe"));
        assert!(!in_happ_folder(r"C:\Tools\xray\xray.exe"));
        assert!(!in_happ_folder(r"C:\Tools\Happy\xray.exe"));
    }

    #[test]
    fn no_state_is_known_before_the_watch_looks() {
        STATE.store(UNKNOWN, Ordering::SeqCst);
        assert_eq!(state(), None);
        STATE.store(ON, Ordering::SeqCst);
        assert_eq!(state(), Some(true));
        STATE.store(OFF, Ordering::SeqCst);
        assert_eq!(state(), Some(false));
        STATE.store(UNKNOWN, Ordering::SeqCst);
    }

    #[test]
    #[ignore = "needs Happ running in the tray"]
    fn opens_happ_unseen() {
        run_link("happ://open").unwrap();
        assert!(!main_window().is_some_and(visible));
    }

    #[test]
    #[ignore = "needs Happ installed"]
    fn reads_the_tunnel_state() {
        println!("installed: {}, tunnel up: {}", installed(), tunnel_up());
        assert!(installed());
    }
}
