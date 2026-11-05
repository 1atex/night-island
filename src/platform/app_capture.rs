use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};

use sha2::{Digest, Sha256};
use windows::Win32::Foundation::{CloseHandle, FreeLibrary, HANDLE, HMODULE, HWND, LPARAM, WPARAM};
use windows::Win32::Security::{GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation};
use windows::Win32::System::Diagnostics::ToolHelp::*;
use windows::Win32::System::LibraryLoader::{
    GetProcAddress, LOAD_LIBRARY_SEARCH_DEFAULT_DIRS, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR, LoadLibraryExW,
};
use windows::Win32::System::SystemInformation::GetTickCount64;
use windows::Win32::System::Threading::{
    GetCurrentProcessId, IsWow64Process, OpenProcess, OpenProcessToken, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{BOOL, HSTRING, PWSTR, s, w};

use super::desktop_dock::Window;

pub const WM_APP_CAPTURE: u32 = WM_APP + 95;
const HELPER: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/night_island_capture.dll"));
static BUSY: AtomicBool = AtomicBool::new(false);
static RESULT: Mutex<VecDeque<Outcome>> = Mutex::new(VecDeque::new());

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    Disabled,
    Unsupported,
    AntiCheat,
    Access,
    Architecture,
    Changed,
    Helper,
    Timeout,
    Rejected,
}

impl Failure {
    pub fn text(self) -> &'static str {
        match self {
            Self::Disabled => "Enable experimental capture protection in desktop dock settings",
            Self::Unsupported => "Capture protection is not supported for this application",
            Self::AntiCheat => "Capture integration is unavailable while anti-cheat is running",
            Self::Access => "Protected or elevated applications are not supported",
            Self::Architecture => "Capture integration requires a 64-bit application",
            Self::Changed => "The application window has closed or changed",
            Self::Helper => "Could not load the capture integration",
            Self::Timeout => "The application did not confirm capture protection",
            Self::Rejected => "Windows did not apply capture protection",
        }
    }
}

pub struct Outcome {
    pub hidden: bool,
    pub applied: usize,
    pub failed: usize,
    pub failure: Option<Failure>,
}

struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}
struct Module(HMODULE);
impl Drop for Module {
    fn drop(&mut self) {
        unsafe {
            let _ = FreeLibrary(self.0);
        }
    }
}
struct Hook(HHOOK);
impl Drop for Hook {
    fn drop(&mut self) {
        unsafe {
            let _ = UnhookWindowsHookEx(self.0);
        }
    }
}
struct Request(HWND);
impl Drop for Request {
    fn drop(&mut self) {
        unsafe {
            let _ = RemovePropW(self.0, w!("NightIsland.Capture.Request"));
            let _ = RemovePropW(self.0, w!("NightIsland.Capture.Result"));
        }
    }
}

pub fn marked(window: &Window) -> bool {
    unsafe { !GetPropW(HWND(window.handle as _), w!("NightIsland.Capture.Original")).0.is_null() }
}

pub fn supported(path: &str) -> bool {
    let path = path.to_lowercase().replace('/', "\\");
    if path.contains("\\steamapps\\") || path.contains("\\games\\") {
        return false;
    }
    matches!(
        Path::new(&path).file_name().and_then(|p| p.to_str()),
        Some(
            "telegram.exe"
                | "discord.exe"
                | "discordcanary.exe"
                | "discordptb.exe"
                | "chrome.exe"
                | "msedge.exe"
                | "firefox.exe"
                | "zen.exe"
                | "browser.exe"
                | "spotify.exe"
                | "yandexmusic.exe"
                | "vlc.exe"
                | "foobar2000.exe"
                | "notepad.exe"
                | "notepad++.exe"
                | "code.exe"
                | "obs64.exe"
                | "slack.exe"
                | "ms-teams.exe"
                | "signal.exe"
                | "whatsapp.exe"
                | "thunderbird.exe"
                | "winword.exe"
                | "excel.exe"
                | "powerpnt.exe"
        )
    )
}

fn anti_cheat() -> Result<bool, Failure> {
    unsafe {
        let snapshot = Handle(CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).map_err(|_| Failure::Access)?);
        let mut item = PROCESSENTRY32W { dwSize: size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        if Process32FirstW(snapshot.0, &mut item).is_err() {
            return Err(Failure::Access);
        }
        loop {
            let end = item.szExeFile.iter().position(|c| *c == 0).unwrap_or(item.szExeFile.len());
            let name = String::from_utf16_lossy(&item.szExeFile[..end]).to_lowercase();
            if ["easyanticheat", "beservice", "vgc.exe", "vgtray.exe", "faceit", "aceguard", "anticheatexpert", "equ8"]
                .iter()
                .any(|part| name.contains(part))
            {
                return Ok(true);
            }
            if Process32NextW(snapshot.0, &mut item).is_err() {
                break;
            }
        }
        Ok(false)
    }
}

fn validate(window: &Window) -> Result<(u32, u32), Failure> {
    unsafe {
        let hwnd = HWND(window.handle as _);
        let mut pid = 0;
        let thread = GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if thread == 0 || pid == GetCurrentProcessId() || GetAncestor(hwnd, GA_ROOT) != hwnd {
            return Err(Failure::Changed);
        }
        let process = Handle(OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).map_err(|_| Failure::Access)?);
        let mut buffer = [0u16; 4096];
        let mut len = buffer.len() as u32;
        QueryFullProcessImageNameW(process.0, PROCESS_NAME_WIN32, PWSTR(buffer.as_mut_ptr()), &mut len)
            .map_err(|_| Failure::Access)?;
        if crate::data::desktop_dock::key(&String::from_utf16_lossy(&buffer[..len as usize]))
            != crate::data::desktop_dock::key(&window.path)
        {
            return Err(Failure::Changed);
        }
        let mut wow = BOOL(0);
        IsWow64Process(process.0, &mut wow).map_err(|_| Failure::Access)?;
        if wow.as_bool() || !cfg!(target_arch = "x86_64") {
            return Err(Failure::Architecture);
        }
        let mut token = HANDLE::default();
        OpenProcessToken(process.0, TOKEN_QUERY, &mut token).map_err(|_| Failure::Access)?;
        let token = Handle(token);
        let mut elevation = TOKEN_ELEVATION::default();
        let mut size = 0;
        GetTokenInformation(
            token.0,
            TokenElevation,
            Some((&raw mut elevation).cast()),
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut size,
        )
        .map_err(|_| Failure::Access)?;
        if elevation.TokenIsElevated != 0 {
            return Err(Failure::Access);
        }
        Ok((thread, pid))
    }
}

fn helper_file() -> Result<PathBuf, Failure> {
    let hash = format!("{:x}", Sha256::digest(HELPER));
    let folder = PathBuf::from(std::env::var_os("LOCALAPPDATA").ok_or(Failure::Helper)?)
        .join("NightIsland")
        .join("native")
        .join(hash);
    std::fs::create_dir_all(&folder).map_err(|_| Failure::Helper)?;
    let path = folder.join("night_island_capture.dll");
    if !path.exists() {
        use std::io::Write;
        let mut file =
            std::fs::OpenOptions::new().write(true).create_new(true).open(&path).map_err(|_| Failure::Helper)?;
        file.write_all(HELPER).and_then(|_| file.sync_all()).map_err(|_| Failure::Helper)?;
    }
    if std::fs::read(&path).map_err(|_| Failure::Helper)?.as_slice() != HELPER {
        return Err(Failure::Helper);
    }
    Ok(path)
}

fn apply(window: &Window, enable: bool, module: &Module) -> Result<(), Failure> {
    let (thread, pid) = validate(window)?;
    unsafe {
        let hwnd = HWND(window.handle as _);
        let proc = GetProcAddress(module.0, s!("IslandCaptureHook")).ok_or(Failure::Helper)?;
        let callback: HOOKPROC = Some(std::mem::transmute::<
            unsafe extern "system" fn() -> isize,
            unsafe extern "system" fn(i32, WPARAM, LPARAM) -> windows::Win32::Foundation::LRESULT,
        >(proc));
        let _hook = Hook(
            SetWindowsHookExW(WH_CALLWNDPROC, callback, Some(module.0.into()), thread).map_err(|_| Failure::Access)?,
        );
        let nonce = (GetTickCount64() as usize ^ (std::process::id() as usize) << 16).max(1);
        let _request = Request(hwnd);
        let _ = RemovePropW(hwnd, w!("NightIsland.Capture.Result"));
        SetPropW(hwnd, w!("NightIsland.Capture.Request"), Some(HANDLE(nonce as _))).map_err(|_| Failure::Access)?;
        let message = RegisterWindowMessageW(w!("NightIsland.CapturePrivacy.v1"));
        if message == 0 {
            return Err(Failure::Helper);
        }
        let mut response = 0;
        let sent = SendMessageTimeoutW(
            hwnd,
            message,
            WPARAM(if enable { 0x11 } else { 0 }),
            LPARAM(nonce as isize),
            SMTO_ABORTIFHUNG | SMTO_BLOCK,
            700,
            Some(&mut response),
        );
        let mut current = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut current));
        if current != pid {
            return Err(Failure::Changed);
        }
        let result = GetPropW(hwnd, w!("NightIsland.Capture.Result")).0 as usize;
        if sent.0 == 0 && result == 0 {
            return Err(Failure::Timeout);
        }
        match result {
            1..=3 => Ok(()),
            0 => Err(Failure::Timeout),
            _ => Err(Failure::Rejected),
        }
    }
}

fn batch(windows: &[Window], allowed: bool) -> Outcome {
    let enable = !windows.iter().any(marked);
    let targets: Vec<_> = windows.iter().filter(|window| enable || marked(window)).cloned().collect();
    let windows = targets.as_slice();
    let mut result = Outcome { hidden: enable, applied: 0, failed: windows.len(), failure: None };
    let operation = || -> Result<Module, Failure> {
        if enable && !allowed {
            return Err(Failure::Disabled);
        }
        if windows.is_empty() || windows.len() > 16 || windows.iter().any(|w| !supported(&w.path)) {
            return Err(Failure::Unsupported);
        }
        if anti_cheat()? {
            return Err(Failure::AntiCheat);
        }
        for window in windows {
            validate(window)?;
        }
        let path = helper_file()?;
        unsafe {
            LoadLibraryExW(
                &HSTRING::from(path.as_os_str()),
                None,
                LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
            )
            .map(Module)
            .map_err(|_| Failure::Helper)
        }
    };
    match operation() {
        Ok(module) => {
            for window in windows {
                match apply(window, enable, &module) {
                    Ok(()) => {
                        result.applied += 1;
                        result.failed -= 1;
                    }
                    Err(error) => {
                        result.failure = Some(error);
                    }
                }
            }
        }
        Err(error) => {
            result.failed = result.failed.max(1);
            result.failure = Some(error);
        }
    }
    result
}

pub fn start(parent: HWND, windows: Vec<Window>, allowed: bool) -> bool {
    if BUSY.swap(true, Ordering::AcqRel) {
        return false;
    }
    let parent = parent.0 as isize;
    std::thread::spawn(move || {
        let outcome = batch(&windows, allowed);
        if let Ok(mut result) = RESULT.lock() {
            result.push_back(outcome);
        }
        BUSY.store(false, Ordering::Release);
        unsafe {
            let parent = HWND(parent as _);
            let mut pid = 0;
            GetWindowThreadProcessId(parent, Some(&mut pid));
            if pid == std::process::id() {
                let _ = PostMessageW(Some(parent), WM_APP_CAPTURE, WPARAM(0), LPARAM(0));
            }
        }
    });
    true
}

pub fn take_result() -> Option<Outcome> {
    RESULT.lock().ok()?.pop_front()
}

#[cfg(test)]
mod tests;
