use std::collections::HashSet;
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

fn capability_in_use(capability: &str, running: &HashSet<String>) -> bool {
    let Some(key) = open(&format!(r"{BASE}\{capability}"), KEY_READ) else { return false };
    let mut used = false;
    for sub in subkeys(key) {
        if sub == "NonPackaged" {
            if let Some(inner) = open(&format!(r"{BASE}\{capability}\NonPackaged"), KEY_READ) {
                used |= subkeys(inner)
                    .iter()
                    .any(|s| in_use_at(inner, s) && running.contains(&s.replace('#', "\\").to_lowercase()));
                unsafe {
                    let _ = RegCloseKey(inner);
                }
            }
        } else {
            used |= in_use_at(key, &sub);
        }
        if used {
            break;
        }
    }
    unsafe {
        let _ = RegCloseKey(key);
    }
    used
}

fn compute() -> Privacy {
    let running = running_images();
    Privacy { mic: capability_in_use("microphone", &running), cam: capability_in_use("webcam", &running) }
}

pub fn start(hwnd: HWND) {
    let raw = hwnd.0 as isize;
    std::thread::spawn(move || {
        let post = |p: Privacy| {
            let changed = SNAPSHOT.lock().map(|mut g| std::mem::replace(&mut *g, p) != p).unwrap_or(false);
            if changed {
                unsafe {
                    let _ = PostMessageW(Some(HWND(raw as *mut c_void)), WM_PRIVACY, WPARAM(0), LPARAM(0));
                }
            }
        };
        post(compute());

        let (Some(mic), Some(cam)) =
            (open(&format!(r"{BASE}\microphone"), KEY_NOTIFY), open(&format!(r"{BASE}\webcam"), KEY_NOTIFY))
        else {
            return;
        };
        let (Ok(e_mic), Ok(e_cam)) =
            (unsafe { CreateEventW(None, false, false, None) }, unsafe { CreateEventW(None, false, false, None) })
        else {
            return;
        };
        let filter = REG_NOTIFY_CHANGE_LAST_SET | REG_NOTIFY_CHANGE_NAME;
        loop {
            unsafe {
                let _ = RegNotifyChangeKeyValue(mic, true, filter, Some(e_mic), true);
                let _ = RegNotifyChangeKeyValue(cam, true, filter, Some(e_cam), true);
                WaitForMultipleObjects(&[e_mic, e_cam], false, INFINITE);
            }
            std::thread::sleep(Duration::from_millis(150));
            post(compute());
        }
    });
}
