use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::time::Duration;

use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, WPARAM};
use windows::Win32::Media::Audio::{
    AudioSessionStateActive, AudioSessionStateExpired, DEVICE_STATE_ACTIVE, IAudioSessionControl2,
    IAudioSessionManager2, IMMDeviceEnumerator, ISimpleAudioVolume, MMDeviceEnumerator, eRender,
};
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize,
};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};
use windows::core::{Interface, PWSTR, Result};

pub const WM_MIXER: u32 = WM_APP + 54;

#[derive(Clone)]
pub struct AppVolume {
    pub key: String,
    pub name: String,
    pub level: f32,
    pub muted: bool,
    pub active: bool,
    pub icon: Option<Arc<super::apps::Icon>>,
}

#[derive(Clone, Default)]
pub struct Snapshot {
    pub apps: Vec<AppVolume>,
    pub ready: bool,
    pub error: Option<String>,
}

struct Session {
    key: String,
    name: String,
    path: Option<String>,
    active: bool,
    volume: ISimpleAudioVolume,
}

enum Command {
    Visible(bool),
    Level(String, f32),
    Mute(String, bool),
}

static WORKER: OnceLock<mpsc::Sender<Command>> = OnceLock::new();
static SNAPSHOT: Mutex<Snapshot> = Mutex::new(Snapshot { apps: Vec::new(), ready: false, error: None });

pub fn snapshot() -> Snapshot {
    SNAPSHOT.lock().map(|s| s.clone()).unwrap_or_default()
}

pub fn visible(hwnd: HWND, on: bool) {
    if !on && WORKER.get().is_none() {
        return;
    }
    let worker = WORKER.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        let target = hwnd.0 as isize;
        std::thread::spawn(move || run(target, rx));
        tx
    });
    let _ = worker.send(Command::Visible(on));
}

pub fn set_level(key: &str, level: f32) {
    if !level.is_finite() {
        return;
    }
    if let Some(worker) = WORKER.get() {
        let _ = worker.send(Command::Level(key.into(), level.clamp(0.0, 1.0)));
    }
}

pub fn set_muted(key: &str, muted: bool) {
    if let Some(worker) = WORKER.get() {
        let _ = worker.send(Command::Mute(key.into(), muted));
    }
}

struct ProcessInfo {
    key: String,
    name: String,
    icon: String,
}

fn process_info(pid: u32) -> Option<ProcessInfo> {
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buffer = [0u16; 2048];
        let mut length = buffer.len() as u32;
        let path_result =
            QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, PWSTR(buffer.as_mut_ptr()), &mut length);
        let mut app_id = [0u16; 512];
        let mut id_length = app_id.len() as u32;
        let id_result = windows::Win32::Storage::Packaging::Appx::GetApplicationUserModelId(
            process,
            &mut id_length,
            Some(PWSTR(app_id.as_mut_ptr())),
        );
        let _ = CloseHandle(process);
        let path = path_result.ok().map(|_| String::from_utf16_lossy(&buffer[..length as usize]));
        if id_result.0 == 0 && id_length > 1 {
            let id = String::from_utf16_lossy(&app_id[..id_length as usize - 1]);
            let name = windows::ApplicationModel::AppInfo::GetFromAppUserModelId(&windows::core::HSTRING::from(&id))
                .ok()
                .and_then(|info| info.DisplayInfo().ok())
                .and_then(|info| info.DisplayName().ok())
                .map(|name| name.to_string())
                .filter(|name| !name.is_empty())
                .or_else(|| path.as_deref().map(app_name))
                .unwrap_or_else(|| "Application".into());
            return Some(ProcessInfo {
                key: format!("app:{}", id.to_lowercase()),
                name,
                icon: format!("shell:AppsFolder\\{id}"),
            });
        }
        let path = path?;
        Some(ProcessInfo { key: path.to_lowercase(), name: app_name(&path), icon: path })
    }
}

fn take_string(value: PWSTR) -> String {
    unsafe {
        let text = value.to_string().unwrap_or_default();
        CoTaskMemFree(Some(value.0.cast()));
        text
    }
}

fn app_name(path: &str) -> String {
    let stem = std::path::Path::new(path).file_stem().unwrap_or_default().to_string_lossy();
    match stem.to_lowercase().as_str() {
        "chrome" => "Google Chrome".into(),
        "msedge" => "Microsoft Edge".into(),
        "firefox" => "Firefox".into(),
        "spotify" => "Spotify".into(),
        "discord" => "Discord".into(),
        "telegram" => "Telegram".into(),
        "yandexmusic" => "Яндекс Музыка".into(),
        _ => stem.into_owned(),
    }
}

fn sessions() -> Result<Vec<Session>> {
    let mut sessions = Vec::new();
    unsafe {
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let devices = enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)?;
        for index in 0..devices.GetCount()? {
            let Ok(device) = devices.Item(index) else { continue };
            let Ok(manager) = device.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) else { continue };
            let Ok(list) = manager.GetSessionEnumerator() else { continue };
            for index in 0..list.GetCount().unwrap_or(0) {
                let Ok(control) = list.GetSession(index) else { continue };
                let Ok(state) = control.GetState() else { continue };
                if state == AudioSessionStateExpired {
                    continue;
                }
                let (Ok(control2), Ok(volume)) =
                    (control.cast::<IAudioSessionControl2>(), control.cast::<ISimpleAudioVolume>())
                else {
                    continue;
                };
                let system = control2.IsSystemSoundsSession().0 == 0;
                let process = control2.GetProcessId().ok().filter(|pid| *pid != 0).and_then(process_info);
                let key = if system {
                    "system-sounds".into()
                } else if let Some(process) = &process {
                    process.key.clone()
                } else if let Ok(identifier) = control2.GetSessionIdentifier() {
                    format!("session:{}", take_string(identifier))
                } else {
                    continue;
                };
                let display = control.GetDisplayName().map(take_string).unwrap_or_default();
                let name = if system {
                    "System sounds".into()
                } else if !display.is_empty() && !display.starts_with('@') {
                    display
                } else {
                    process.as_ref().map(|process| process.name.clone()).unwrap_or_else(|| "Application".into())
                };
                sessions.push(Session {
                    key,
                    name,
                    path: process.map(|process| process.icon),
                    active: state == AudioSessionStateActive,
                    volume,
                });
            }
        }
    }
    Ok(sessions)
}

fn group(sessions: &[Session]) -> Vec<AppVolume> {
    let mut apps: BTreeMap<String, AppVolume> = BTreeMap::new();
    for session in sessions {
        let (Ok(level), Ok(muted)) = (unsafe { session.volume.GetMasterVolume() }, unsafe { session.volume.GetMute() })
        else {
            continue;
        };
        let app = apps.entry(session.key.clone()).or_insert_with(|| AppVolume {
            key: session.key.clone(),
            name: session.name.clone(),
            level: 0.0,
            muted: true,
            active: false,
            icon: session.path.as_deref().and_then(super::apps::icon_for_path),
        });
        app.level = app.level.max(level.clamp(0.0, 1.0));
        app.muted &= muted.as_bool();
        app.active |= session.active;
    }
    let mut apps: Vec<_> = apps.into_values().collect();
    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then_with(|| a.key.cmp(&b.key)));
    apps
}

fn apply(sessions: &[Session], key: &str, level: Option<f32>, muted: Option<bool>) -> usize {
    sessions
        .iter()
        .filter(|s| s.key == key)
        .filter(|session| unsafe {
            let level_ok = level
                .is_none_or(|level| session.volume.SetMasterVolume(level.clamp(0.0, 1.0), std::ptr::null()).is_ok());
            let mute_ok = muted.is_none_or(|muted| session.volume.SetMute(muted, std::ptr::null()).is_ok());
            level_ok && mute_ok
        })
        .count()
}

fn publish(target: isize, state: Snapshot) {
    if let Ok(mut snapshot) = SNAPSHOT.lock() {
        let unchanged = snapshot.ready == state.ready
            && snapshot.error == state.error
            && snapshot.apps.len() == state.apps.len()
            && snapshot.apps.iter().zip(&state.apps).all(|(a, b)| {
                a.key == b.key
                    && a.name == b.name
                    && (a.level - b.level).abs() < 0.0001
                    && a.muted == b.muted
                    && a.active == b.active
            });
        if unchanged {
            return;
        }
        *snapshot = state;
    }
    unsafe {
        let _ = PostMessageW(Some(HWND(target as *mut _)), WM_MIXER, WPARAM(0), LPARAM(0));
    }
}

fn run(target: isize, rx: mpsc::Receiver<Command>) {
    if unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_err() {
        publish(
            target,
            Snapshot { ready: true, error: Some("Audio mixer is unavailable".into()), ..Default::default() },
        );
        return;
    }
    let mut visible = false;
    let mut current = Vec::new();
    let mut audible: HashMap<String, f32> = HashMap::new();
    loop {
        let command = if visible {
            rx.recv_timeout(Duration::from_millis(700)).map(Some)
        } else {
            rx.recv().map(Some).map_err(|_| mpsc::RecvTimeoutError::Disconnected)
        };
        let command = match command {
            Ok(command) => command,
            Err(mpsc::RecvTimeoutError::Timeout) => None,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        };
        let mut refresh = command.is_none();
        let mut changes: HashMap<String, (Option<f32>, Option<bool>)> = HashMap::new();
        for command in command.into_iter().chain(rx.try_iter()) {
            match command {
                Command::Visible(on) => {
                    visible = on;
                    refresh |= on;
                }
                Command::Level(key, level) => {
                    changes.insert(key, (Some(level), Some(false)));
                }
                Command::Mute(key, muted) => {
                    changes.entry(key).or_default().1 = Some(muted);
                }
            }
        }
        for (key, (mut level, muted)) in changes {
            if muted == Some(false)
                && level.is_none()
                && current
                    .iter()
                    .filter(|s: &&Session| s.key == key)
                    .all(|s| unsafe { s.volume.GetMasterVolume().unwrap_or(0.0) <= 0.001 })
            {
                level = Some(*audible.get(&key).unwrap_or(&0.5));
            }
            if let Some(value) = level.filter(|v| *v > 0.001) {
                audible.insert(key.clone(), value);
            }
            apply(&current, &key, level, muted);
        }
        if !visible {
            continue;
        }
        let result = if refresh { sessions() } else { Ok(std::mem::take(&mut current)) };
        match result {
            Ok(list) => {
                current = list;
                let apps = group(&current);
                for app in &apps {
                    if app.level > 0.001 {
                        audible.insert(app.key.clone(), app.level);
                    }
                }
                publish(target, Snapshot { apps, ready: true, error: None });
            }
            Err(_) => publish(
                target,
                Snapshot { ready: true, error: Some("Audio mixer is unavailable".into()), ..Default::default() },
            ),
        }
    }
    unsafe {
        CoUninitialize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "Creates isolated Windows audio sessions without playing sound"]
    fn grouped_sessions_change_only_the_exact_application() -> Result<()> {
        use windows::Win32::Media::Audio::eConsole;
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
        }
        struct Runtime;
        impl Drop for Runtime {
            fn drop(&mut self) {
                unsafe {
                    CoUninitialize();
                }
            }
        }
        let _runtime = Runtime;
        let enumerator: IMMDeviceEnumerator = unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)? };
        let device = unsafe { enumerator.GetDefaultAudioEndpoint(eRender, eConsole)? };
        let manager: IAudioSessionManager2 = unsafe { device.Activate(CLSCTX_ALL, None)? };
        struct Restore(ISimpleAudioVolume, f32, bool);
        impl Drop for Restore {
            fn drop(&mut self) {
                unsafe {
                    let _ = self.0.SetMasterVolume(self.1, std::ptr::null());
                    let _ = self.0.SetMute(self.2, std::ptr::null());
                }
            }
        }
        let mut restores = Vec::new();
        let mut list = Vec::new();
        for (i, (key, level, muted)) in
            [("test-exact", 0.25, true), ("test-exact", 0.75, false), ("test-exact-other", 0.9, false)]
                .into_iter()
                .enumerate()
        {
            let guid = windows::core::GUID::from_u128(0xbdaf0e70974e445193554a431abc0010 + i as u128);
            let volume = unsafe { manager.GetSimpleAudioVolume(Some(&guid), 0)? };
            restores.push(Restore(volume.clone(), unsafe { volume.GetMasterVolume()? }, unsafe {
                volume.GetMute()?.as_bool()
            }));
            unsafe {
                volume.SetMasterVolume(level, std::ptr::null())?;
                volume.SetMute(muted, std::ptr::null())?;
            }
            list.push(Session {
                key: key.into(),
                name: "Same display name".into(),
                path: None,
                active: i == 1,
                volume,
            });
        }
        let grouped = group(&list);
        assert_eq!(grouped.len(), 2);
        let app = grouped.iter().find(|app| app.key == "test-exact").unwrap();
        assert!((app.level - 0.75).abs() < 0.001 && !app.muted && app.active);
        assert_eq!(apply(&list, "test-exact", Some(0.4), Some(true)), 2);
        unsafe {
            assert!((list[0].volume.GetMasterVolume()? - 0.4).abs() < 0.001);
            assert!(list[1].volume.GetMute()?.as_bool());
            assert!((list[2].volume.GetMasterVolume()? - 0.9).abs() < 0.001);
            assert!(!list[2].volume.GetMute()?.as_bool());
        }
        assert_eq!(apply(&list, "missing", Some(0.0), Some(true)), 0);
        assert_eq!(apply(&list, "test-exact", Some(4.0), Some(false)), 2);
        unsafe {
            assert!((list[0].volume.GetMasterVolume()? - 1.0).abs() < 0.001);
        }
        let live = sessions()?;
        assert!(live.iter().all(|session| !session.key.is_empty()));
        Ok(())
    }
}
