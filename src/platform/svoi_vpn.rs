use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::os::windows::io::AsRawHandle;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{CloseHandle, HANDLE, HLOCAL, LocalFree};
use windows::Win32::Security::Authorization::ConvertSidToStringSidW;
use windows::Win32::Security::{GetTokenInformation, TOKEN_QUERY, TOKEN_USER, TokenUser};
use windows::Win32::System::Pipes::{GetNamedPipeServerProcessId, PeekNamedPipe};
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_SZ, RegGetValueW};
use windows::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::core::{PCWSTR, PWSTR};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Server {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub protocol: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct Snapshot {
    pub version: u32,
    pub access: String,
    pub state: String,
    pub active: Option<Server>,
    pub selected: Option<Server>,
    pub favorites: Vec<Server>,
    pub mode: String,
    pub core: String,
    pub verified: bool,
    pub latency: Option<i64>,
    #[serde(rename = "startedAt")]
    pub started_at: Option<String>,
    pub busy: bool,
    pub error: String,
    pub sequence: u64,
    pub instance: i64,
}
impl Snapshot {
    pub fn closed() -> Self {
        Self { access: "closed".into(), ..Self::default() }
    }
    pub fn connected(&self) -> Option<bool> {
        (self.access == "allowed").then_some(self.state == "running" && self.active.is_some())
    }
    pub fn status(&self) -> &str {
        if self.busy {
            return "Connecting…";
        }
        match self.access.as_str() {
            "closed" => "SVOI is closed",
            "denied" => "Allow control in SVOI",
            _ => match self.state.as_str() {
                "running" if self.verified => "Connected",
                "running" => "VPN on · Check connection",
                "starting" => "Connecting…",
                "stopping" => "Disconnecting…",
                _ => "Off",
            },
        }
    }
    pub fn action(&self) -> &str {
        match self.access.as_str() {
            "closed" => "Open SVOI",
            "denied" => "Allow control in SVOI",
            _ => {
                if self.connected() == Some(true) {
                    "Disconnect"
                } else {
                    "Connect"
                }
            }
        }
    }
}
static APPLICATION: Mutex<Option<PathBuf>> = Mutex::new(None);
fn user(process: HANDLE) -> Option<String> {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(process, TOKEN_QUERY, &mut token).ok()?;
        let mut needed = 0;
        let _ = GetTokenInformation(token, TokenUser, None, 0, &mut needed);
        let mut buffer = vec![0u64; (needed as usize).div_ceil(8)];
        let ok = GetTokenInformation(token, TokenUser, Some(buffer.as_mut_ptr().cast()), needed, &mut needed).is_ok();
        let _ = CloseHandle(token);
        if !ok {
            return None;
        }
        let info = &*(buffer.as_ptr() as *const TOKEN_USER);
        let mut sid = PWSTR::null();
        ConvertSidToStringSidW(info.User.Sid, &mut sid).ok()?;
        let text = sid.to_string().ok();
        let _ = LocalFree(Some(HLOCAL(sid.0 as *mut _)));
        text
    }
}
fn current_user() -> Option<String> {
    static USER: OnceLock<Option<String>> = OnceLock::new();
    USER.get_or_init(|| user(unsafe { GetCurrentProcess() })).clone()
}
pub fn request(action: &str, id: &str) -> Snapshot {
    try_request(action, id)
        .or_else(|| if action != "state" { try_request("state", "") } else { None })
        .unwrap_or_else(Snapshot::closed)
}
fn try_request(action: &str, id: &str) -> Option<Snapshot> {
    let owner = current_user()?;
    let path = format!(r"\\.\pipe\SVOI.NightIsland.v1.{}", owner);
    let mut file = OpenOptions::new().read(true).write(true).open(path).ok()?;
    let handle = HANDLE(file.as_raw_handle());
    let mut pid = 0;
    unsafe {
        GetNamedPipeServerProcessId(handle, &mut pid).ok()?;
    }
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()? };
    let same = user(process).as_deref() == Some(owner.as_str());
    unsafe {
        let _ = CloseHandle(process);
    }
    let server = PathBuf::from(super::happ::image_path(pid)?);
    if !same || !server.file_name()?.to_string_lossy().eq_ignore_ascii_case("svoi-backend.exe") {
        return None;
    }
    let application = server.parent()?.parent()?.join("svoi.exe");
    if application.is_file() {
        *APPLICATION.lock().unwrap() = Some(application);
    }
    let mut request = serde_json::to_vec(&serde_json::json!({"action":action,"serverId":id})).ok()?;
    request.push(b'\n');
    file.write_all(&request).ok()?;
    let until = Instant::now() + Duration::from_secs(if action == "state" { 3 } else { 43 });
    let mut result = Vec::new();
    let mut bytes = [0u8; 4096];
    loop {
        let mut available = 0u32;
        unsafe {
            PeekNamedPipe(handle, None, 0, None, Some(&mut available), None).ok()?;
        }
        if available == 0 {
            if Instant::now() > until {
                return None;
            }
            std::thread::sleep(Duration::from_millis(12));
            continue;
        }
        match file.read(&mut bytes) {
            Ok(0) => return None,
            Ok(n) => {
                result.extend_from_slice(&bytes[..n]);
                if result.len() > 65536 {
                    return None;
                }
                if result.contains(&b'\n') {
                    break;
                }
            }
            Err(e) if e.raw_os_error() == Some(232) => {}
            Err(_) => return None,
        }
        if Instant::now() > until {
            return None;
        }
        std::thread::sleep(Duration::from_millis(12));
    }
    let value: Snapshot = serde_json::from_slice(&result).ok()?;
    (value.version == 1 && ["allowed", "denied"].contains(&value.access.as_str())).then_some(value)
}
fn registry_path(key: &str, value: &str) -> Option<PathBuf> {
    unsafe {
        let key: Vec<u16> = key.encode_utf16().chain(Some(0)).collect();
        let value: Vec<u16> = value.encode_utf16().chain(Some(0)).collect();
        let mut buffer = [0u16; 32768];
        let mut size = (buffer.len() * 2) as u32;
        RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(key.as_ptr()),
            PCWSTR(value.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(buffer.as_mut_ptr().cast()),
            Some(&mut size),
        )
        .ok()
        .ok()?;
        Some(PathBuf::from(String::from_utf16_lossy(&buffer[..size as usize / 2]).trim_end_matches('\0')))
    }
}
pub fn open() -> bool {
    let path = APPLICATION
        .lock()
        .unwrap()
        .clone()
        .or_else(|| registry_path(r"Software\SVOI\NightIsland", "Application"))
        .or_else(|| {
            registry_path(
                r"Software\Microsoft\Windows\CurrentVersion\Uninstall\SVOI.FlutterDesktop_is1",
                "InstallLocation",
            )
            .map(|p| p.join("svoi.exe"))
        });
    if let Some(path) = path.filter(|p| p.is_file()) {
        crate::tools::cmd::shell_open(&path.to_string_lossy(), None);
        true
    } else {
        false
    }
}
