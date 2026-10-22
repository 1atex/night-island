use std::collections::HashSet;

use crate::data::usage::UseKind;
use std::ffi::c_void;
use std::sync::Mutex;
use std::time::Duration;

use windows::Win32::Foundation::CloseHandle;
use windows::Win32::Foundation::{ERROR_NO_MORE_ITEMS, HWND, LPARAM, WPARAM};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_NOTIFY, KEY_READ, REG_NOTIFY_CHANGE_LAST_SET, REG_NOTIFY_CHANGE_NAME,
    RRF_RT_REG_QWORD, RegCloseKey, RegEnumKeyExW, RegGetValueW, RegNotifyChangeKeyValue, RegOpenKeyExW,
};
use windows::Win32::System::Threading::{
    CreateEventW, INFINITE, OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    QueryFullProcessImageNameW, WaitForMultipleObjects,
};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};
use windows::core::{HSTRING, PCWSTR, PWSTR, w};

pub const WM_PRIVACY: u32 = WM_APP + 8;

const BASE: &str = r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore";

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct Privacy {
    pub mic: bool,
    pub cam: bool,
}

static SNAPSHOT: Mutex<Privacy> = Mutex::new(Privacy { mic: false, cam: false });

pub fn snapshot() -> Privacy {
    SNAPSHOT.lock().map(|g| *g).unwrap_or_default()
}

fn open(path: &str, access: windows::Win32::System::Registry::REG_SAM_FLAGS) -> Option<HKEY> {
    let mut key = HKEY::default();
    let wide = HSTRING::from(path);
    unsafe {
        RegOpenKeyExW(HKEY_CURRENT_USER, &wide, None, access, &mut key).ok().ok()?;
    }
    Some(key)
}

fn subkeys(key: HKEY) -> Vec<String> {
    let mut out = Vec::new();
    for index in 0..512u32 {
        let mut buf = [0u16; 512];
        let mut len = buf.len() as u32;
        let status =
            unsafe { RegEnumKeyExW(key, index, Some(PWSTR(buf.as_mut_ptr())), &mut len, None, None, None, None) };
        if status == ERROR_NO_MORE_ITEMS {
            break;
        }
        if status.is_err() {
            continue;
        }
        out.push(String::from_utf16_lossy(&buf[..len as usize]));
    }
    out
}

fn qword(key: HKEY, sub: &str, name: PCWSTR) -> u64 {
    let mut value = 0u64;
    let mut size = size_of::<u64>() as u32;
    let sub = HSTRING::from(sub);
    unsafe {
        let _ = RegGetValueW(
            key,
            &sub,
            name,
            RRF_RT_REG_QWORD,
            None,
            Some(&mut value as *mut u64 as *mut c_void),
            Some(&mut size),
        );
    }
    value
}

fn in_use_at(key: HKEY, sub: &str) -> bool {
    qword(key, sub, w!("LastUsedTimeStart")) > 0 && qword(key, sub, w!("LastUsedTimeStop")) == 0
}

fn running_images() -> HashSet<String> {
    let mut out = HashSet::new();
    unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else { return out };
        let mut entry = PROCESSENTRY32W { dwSize: size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut ok = Process32FirstW(snap, &mut entry).is_ok();
        while ok {
            if let Ok(h) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, entry.th32ProcessID) {
                let mut buf = [0u16; 1024];
                let mut len = buf.len() as u32;
                if QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len).is_ok() {
                    out.insert(String::from_utf16_lossy(&buf[..len as usize]).to_lowercase());
                }
                let _ = CloseHandle(h);
            }
            ok = Process32NextW(snap, &mut entry).is_ok();
        }
        let _ = CloseHandle(snap);
    }
    out
}

fn app_name(sub: &str, desktop: bool) -> String {
    if desktop {
        let path = sub.replace('#', "\\");
        let file = path.rsplit('\\').next().unwrap_or(&path);
        return file.strip_suffix(".exe").or_else(|| file.strip_suffix(".EXE")).unwrap_or(file).to_string();
    }
    let package = sub.split('_').next().unwrap_or(sub);
    package.rsplit('.').next().unwrap_or(package).to_string()
}

fn capability_users(capability: &str, running: &HashSet<String>) -> Vec<String> {
    let Some(key) = open(&format!(r"{BASE}\{capability}"), KEY_READ) else { return Vec::new() };
    let mut users = Vec::new();
    for sub in subkeys(key) {
        if sub == "NonPackaged" {
            if let Some(inner) = open(&format!(r"{BASE}\{capability}\NonPackaged"), KEY_READ) {
                for s in subkeys(inner) {
                    if in_use_at(inner, &s) && running.contains(&s.replace('#', "\\").to_lowercase()) {
                        users.push(app_name(&s, true));
                    }
                }
                unsafe {
                    let _ = RegCloseKey(inner);
                }
            }
        } else if in_use_at(key, &sub) {
            users.push(app_name(&sub, false));
        }
    }
    unsafe {
        let _ = RegCloseKey(key);
    }
    users
}

const SCREEN_CAPABILITIES: [&str; 2] = ["graphicsCaptureProgrammatic", "graphicsCaptureWithoutBorder"];

fn compute() -> (Privacy, Vec<(UseKind, String)>) {
    let running = running_images();
    let mic = capability_users("microphone", &running);
    let cam = capability_users("webcam", &running);
    let privacy = Privacy { mic: !mic.is_empty(), cam: !cam.is_empty() };
    let mut users: Vec<(UseKind, String)> = Vec::new();
    users.extend(mic.into_iter().map(|a| (UseKind::Mic, a)));
    users.extend(cam.into_iter().map(|a| (UseKind::Cam, a)));
    for capability in SCREEN_CAPABILITIES {
        users.extend(
            capability_users(capability, &running)
                .into_iter()
                .filter(|a| !a.eq_ignore_ascii_case("night-island"))
                .map(|a| (UseKind::Screen, a)),
        );
    }
    users.sort_by(|a, b| (a.0 as u8, &a.1).cmp(&(b.0 as u8, &b.1)));
    users.dedup();
    (privacy, users)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UseEvent {
    pub kind: UseKind,
    pub app: String,
    pub started: bool,
    pub at: i64,
}

static USERS: Mutex<Vec<(UseKind, String)>> = Mutex::new(Vec::new());
static EVENTS: Mutex<Vec<UseEvent>> = Mutex::new(Vec::new());

pub fn take_events() -> Vec<UseEvent> {
    EVENTS.lock().map(|mut e| std::mem::take(&mut *e)).unwrap_or_default()
}

fn diff(before: &[(UseKind, String)], now: &[(UseKind, String)], at: i64) -> Vec<UseEvent> {
    let ended = before.iter().filter(|u| !now.contains(u)).map(|(k, a)| UseEvent {
        kind: *k,
        app: a.clone(),
        started: false,
        at,
    });
    let started = now.iter().filter(|u| !before.contains(u)).map(|(k, a)| UseEvent {
        kind: *k,
        app: a.clone(),
        started: true,
        at,
    });
    ended.chain(started).collect()
}

fn unix_now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

pub fn start(hwnd: HWND) {
    let raw = hwnd.0 as isize;
    std::thread::spawn(move || {
        let post = |(p, users): (Privacy, Vec<(UseKind, String)>)| {
            let changed = SNAPSHOT.lock().map(|mut g| std::mem::replace(&mut *g, p) != p).unwrap_or(false);
            let events = match USERS.lock() {
                Ok(mut before) => {
                    let events = diff(&before, &users, unix_now());
                    *before = users;
                    events
                }
                Err(_) => Vec::new(),
            };
            let news = !events.is_empty();
            if news && let Ok(mut queue) = EVENTS.lock() {
                queue.extend(events);
                let excess = queue.len().saturating_sub(200);
                queue.drain(..excess);
            }
            if changed || news {
                unsafe {
                    let _ = PostMessageW(Some(HWND(raw as *mut c_void)), WM_PRIVACY, WPARAM(0), LPARAM(0));
                }
            }
        };
        post(compute());

        let mut keys: Vec<HKEY> = Vec::new();
        let mut events = Vec::new();
        for capability in ["microphone", "webcam"].into_iter().chain(SCREEN_CAPABILITIES) {
            if let Some(key) = open(&format!(r"{BASE}\{capability}"), KEY_NOTIFY)
                && let Ok(event) = unsafe { CreateEventW(None, false, false, None) }
            {
                keys.push(key);
                events.push(event);
            }
        }
        if events.len() < 2 {
            return;
        }
        let filter = REG_NOTIFY_CHANGE_LAST_SET | REG_NOTIFY_CHANGE_NAME;
        loop {
            unsafe {
                for (key, event) in keys.iter().zip(&events) {
                    let _ = RegNotifyChangeKeyValue(*key, true, filter, Some(*event), true);
                }
                WaitForMultipleObjects(&events, false, INFINITE);
            }
            std::thread::sleep(Duration::from_millis(150));
            post(compute());
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_names_are_readable() {
        assert_eq!(app_name("C:#Program Files#Google#Chrome#chrome.exe", true), "chrome");
        assert_eq!(app_name("Microsoft.WindowsCamera_8wekyb3d8bbwe", false), "WindowsCamera");
        assert_eq!(app_name("plain", false), "plain");
    }

    #[test]
    fn a_change_of_users_makes_events() {
        let a = (UseKind::Mic, "zoom".to_string());
        let b = (UseKind::Screen, "obs".to_string());
        let events = diff(std::slice::from_ref(&a), std::slice::from_ref(&b), 7);
        assert_eq!(events.len(), 2);
        assert!(!events[0].started && events[0].app == "zoom");
        assert!(events[1].started && events[1].kind == UseKind::Screen && events[1].at == 7);
        assert!(diff(std::slice::from_ref(&a), std::slice::from_ref(&a), 7).is_empty());
    }

    #[test]
    #[ignore = "reads this computer's registry"]
    fn lists_current_users() {
        let (privacy, users) = compute();
        println!("mic {} cam {} users {users:?}", privacy.mic, privacy.cam);
    }
}
