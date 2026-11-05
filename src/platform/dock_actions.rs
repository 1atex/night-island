use std::collections::VecDeque;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as Session, GlobalSystemMediaTransportControlsSessionManager as Manager,
    GlobalSystemMediaTransportControlsSessionMediaProperties as MediaProperties,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
};
use windows::Win32::Foundation::{CloseHandle, HWND, PROPERTYKEY};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::System::Com::StructuredStorage::PropVariantToStringAlloc;
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize};
use windows::Win32::UI::Shell::PropertiesSystem::{IPropertyStore, SHGetPropertyStoreForWindow};
use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;
use windows::core::{GUID, PWSTR};
use windows_future::{AsyncStatus, IAsyncOperation};

pub const WM_ACTIONS_READY: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 89;

#[derive(Clone, Default, PartialEq)]
pub struct Snapshot {
    pub target: String,
    pub ready: bool,
    pub media: Vec<MediaSession>,
    pub audio: Option<AppAudio>,
    pub discord: Option<DiscordVoice>,
}

#[derive(Clone, PartialEq)]
pub struct MediaSession {
    pub id: u64,
    pub title: String,
    pub artist: String,
    pub source: String,
    pub playing: bool,
    pub can_previous: bool,
    pub can_toggle: bool,
    pub can_next: bool,
}

#[derive(Clone, PartialEq)]
pub struct AppAudio {
    pub volume: f32,
    pub muted: bool,
}

#[derive(Clone, PartialEq)]
pub struct DiscordVoice {
    pub connected: bool,
    pub available: bool,
    pub error: Option<String>,
    pub mute: bool,
    pub deaf: bool,
    pub channel: String,
    pub members: Vec<crate::services::discord::Member>,
}

#[derive(Clone, Copy)]
pub enum MediaAction {
    Previous,
    PlayPause,
    Next,
}

#[derive(Clone, Default, PartialEq, Eq)]
struct Target {
    path: String,
    handles: Vec<isize>,
}

#[derive(Default)]
struct Shared {
    target: Target,
    generation: u64,
    snapshot: Snapshot,
    cache: VecDeque<(Target, Snapshot, Instant)>,
    notify: isize,
    pending_volume: Option<f32>,
}

enum Action {
    Wake,
    Media(u64, MediaAction),
    Volume(f32),
    Mute(bool),
    DiscordMute(bool),
    DiscordDeaf(bool),
}

pub struct Client {
    shared: Arc<Mutex<Shared>>,
    tx: mpsc::SyncSender<(u64, Action)>,
}

impl Client {
    pub fn new() -> Self {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let (tx, rx) = mpsc::sync_channel(16);
        let worker = shared.clone();
        let _ = std::thread::Builder::new().name("dock-actions".into()).spawn(move || run(worker, rx));
        Self { shared, tx }
    }

    pub fn request(&self, path: &str, handles: &[isize]) {
        let mut handles: Vec<_> = handles.iter().copied().take(64).collect();
        handles.sort_unstable();
        handles.dedup();
        let target = Target { path: normalized(path), handles };
        if let Ok(mut shared) = self.shared.lock() {
            if shared.target == target {
                return;
            }
            if shared.snapshot.ready && !shared.target.path.is_empty() {
                let previous = (shared.target.clone(), shared.snapshot.clone(), Instant::now());
                shared.cache.retain(|(key, _, at)| *key != previous.0 && at.elapsed() < Duration::from_secs(3));
                shared.cache.push_front(previous);
                shared.cache.truncate(8);
            }
            shared.generation = shared.generation.wrapping_add(1);
            shared.pending_volume = None;
            shared.snapshot = shared
                .cache
                .iter()
                .find(|(key, _, at)| *key == target && at.elapsed() < Duration::from_secs(3))
                .map(|(_, snapshot, _)| snapshot.clone())
                .unwrap_or_else(|| Snapshot { target: target.path.clone(), ..Default::default() });
            shared.target = target;
            if matches!(self.tx.try_send((shared.generation, Action::Wake)), Err(mpsc::TrySendError::Disconnected(_))) {
                shared.snapshot.ready = true;
            }
        }
    }

    pub fn set_notify(&self, hwnd: HWND) {
        if let Ok(mut shared) = self.shared.lock() {
            shared.notify = hwnd.0 as isize;
        }
    }

    pub fn clear(&self) {
        self.request("", &[]);
    }

    pub fn snapshot(&self) -> Snapshot {
        self.shared.lock().map(|s| s.snapshot.clone()).unwrap_or_default()
    }

    fn send(&self, action: Action) {
        if let Ok(shared) = self.shared.lock() {
            let _ = self.tx.try_send((shared.generation, action));
        }
    }

    pub fn control(&self, session_id: u64, action: MediaAction) {
        self.send(Action::Media(session_id, action));
    }

    pub fn set_volume(&self, volume: f32) {
        if volume.is_finite()
            && let Ok(mut shared) = self.shared.lock()
        {
            shared.pending_volume = Some(volume.clamp(0.0, 1.0));
            let _ = self.tx.try_send((shared.generation, Action::Wake));
        }
    }

    pub fn set_muted(&self, muted: bool) {
        self.send(Action::Mute(muted));
    }

    pub fn set_discord_mute(&self, muted: bool) {
        self.send(Action::DiscordMute(muted));
    }

    pub fn set_discord_deaf(&self, deaf: bool) {
        self.send(Action::DiscordDeaf(deaf));
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        self.clear();
    }
}

fn normalized(value: &str) -> String {
    value.trim().replace('/', "\\").to_lowercase()
}

fn filename(value: &str) -> &str {
    value.rsplit(['\\', '/']).next().unwrap_or(value)
}

fn is_discord(path: &str) -> bool {
    matches!(filename(path), "discord.exe" | "discordcanary.exe" | "discordptb.exe")
}

fn add_path_aliases(ids: &mut Vec<String>, path: &str, competing: &[String]) {
    ids.push(path.into());
    let name = filename(path);
    if !competing.iter().any(|other| filename(other) == name && other != path) {
        ids.push(name.into());
        if let Some(stem) = name.strip_suffix(".exe") {
            ids.push(stem.into());
        }
    }
}

#[derive(Default)]
struct Identity {
    ids: Vec<String>,
    audio_keys: Vec<String>,
    valid: bool,
    discord: bool,
}

fn identity(target: &Target) -> Identity {
    let path = normalized(&super::desktop_dock::resolved_target(&target.path));
    let mut result = Identity::default();
    let mut paths = Vec::new();
    for handle in &target.handles {
        unsafe {
            let hwnd = HWND(*handle as *mut _);
            let mut pid = 0;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            let Ok(process) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else { continue };
            let mut buffer = [0u16; 2048];
            let mut length = buffer.len() as u32;
            let found =
                QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, PWSTR(buffer.as_mut_ptr()), &mut length)
                    .ok()
                    .map(|_| normalized(&String::from_utf16_lossy(&buffer[..length as usize])));
            let mut id = [0u16; 512];
            let mut id_length = id.len() as u32;
            let status = windows::Win32::Storage::Packaging::Appx::GetApplicationUserModelId(
                process,
                &mut id_length,
                Some(PWSTR(id.as_mut_ptr())),
            );
            let _ = CloseHandle(process);
            let app_id = (status.0 == 0 && id_length > 1 && id_length as usize <= id.len())
                .then(|| normalized(&String::from_utf16_lossy(&id[..id_length as usize - 1])));
            let same = found.as_deref() == Some(&path)
                || app_id.as_ref().is_some_and(|id| path == format!("shell:appsfolder\\{id}"));
            if !same {
                continue;
            }
            result.valid = true;
            if let Some(found) = found {
                result.audio_keys.push(found.clone());
                paths.push(found);
            }
            if let Some(id) = app_id {
                result.audio_keys.push(format!("app:{id}"));
                result.ids.push(id);
            }
            if let Ok(store) = SHGetPropertyStoreForWindow::<IPropertyStore>(hwnd) {
                let key = PROPERTYKEY { fmtid: GUID::from_u128(0x9f4c2855_9f79_4b39_a8d0_e1d42de1d5f3), pid: 5 };
                if let Ok(value) = store.GetValue(&key)
                    && let Ok(text) = PropVariantToStringAlloc(&value)
                {
                    if let Ok(id) = text.to_string()
                        && !id.is_empty()
                    {
                        result.ids.push(normalized(&id));
                    }
                    CoTaskMemFree(Some(text.0.cast()));
                }
            }
        }
    }
    if !result.valid {
        return result;
    }
    let windows = super::desktop_dock::windows();
    let competing: Vec<_> = windows.iter().map(|window| normalized(&window.path)).collect();
    for path in &paths {
        add_path_aliases(&mut result.ids, path, &competing);
        if competing.iter().any(|other| filename(other) == filename(path) && other != path) {
            let name = filename(path);
            let stem = name.strip_suffix(".exe").unwrap_or(name);
            result.ids.retain(|id| id != name && id != stem);
        }
    }
    result.discord = is_discord(&path);
    result.ids.sort();
    result.ids.dedup();
    result.audio_keys.sort();
    result.audio_keys.dedup();
    result
}

struct Pending<T: windows::core::RuntimeType + 'static> {
    operation: IAsyncOperation<T>,
    started: Instant,
}

impl<T: windows::core::RuntimeType + 'static> Pending<T> {
    fn new(operation: windows::core::Result<IAsyncOperation<T>>) -> Option<Self> {
        operation.ok().map(|operation| Self { operation, started: Instant::now() })
    }

    fn poll(&self) -> Option<Option<T>> {
        match self.operation.Status() {
            Ok(AsyncStatus::Started) if self.started.elapsed() < Duration::from_millis(600) => None,
            Ok(AsyncStatus::Completed) => Some(self.operation.GetResults().ok()),
            _ => Some(None),
        }
    }
}

impl<T: windows::core::RuntimeType + 'static> Drop for Pending<T> {
    fn drop(&mut self) {
        if self.operation.Status().ok() == Some(AsyncStatus::Started) {
            let _ = self.operation.Cancel();
        }
    }
}

struct Tracked {
    id: u64,
    session: Session,
    properties: Option<Pending<MediaProperties>>,
    refreshed: Instant,
    title: String,
    artist: String,
}

fn matching_sessions(manager: &Manager, identity: &Identity) -> Vec<Session> {
    manager
        .GetSessions()
        .ok()
        .map(|sessions| {
            sessions
                .into_iter()
                .filter(|session| {
                    session.SourceAppUserModelId().is_ok_and(|id| identity.ids.contains(&normalized(&id.to_string())))
                })
                .take(8)
                .collect()
        })
        .unwrap_or_default()
}

fn media_snapshot(tracked: &mut Tracked) -> Option<MediaSession> {
    let info = tracked.session.GetPlaybackInfo().ok()?;
    let controls = info.Controls().ok()?;
    let playing = info.PlaybackStatus().ok() == Some(Status::Playing);
    if let Some(result) = tracked.properties.as_ref().and_then(Pending::poll) {
        if let Some(properties) = result {
            tracked.title =
                properties.Title().map(|value| value.to_string().chars().take(240).collect()).unwrap_or_default();
            tracked.artist =
                properties.Artist().map(|value| value.to_string().chars().take(240).collect()).unwrap_or_default();
        }
        tracked.properties = None;
        tracked.refreshed = Instant::now();
    }
    if tracked.properties.is_none() && tracked.refreshed.elapsed() >= Duration::from_millis(350) {
        tracked.properties = Pending::new(tracked.session.TryGetMediaPropertiesAsync());
        tracked.refreshed = Instant::now();
    }
    Some(MediaSession {
        id: tracked.id,
        title: tracked.title.clone(),
        artist: tracked.artist.clone(),
        source: tracked.session.SourceAppUserModelId().map(|value| value.to_string()).unwrap_or_default(),
        playing,
        can_previous: controls.IsPreviousEnabled().unwrap_or(false),
        can_next: controls.IsNextEnabled().unwrap_or(false),
        can_toggle: controls.IsPlayPauseToggleEnabled().unwrap_or(false)
            || if playing {
                controls.IsPauseEnabled().unwrap_or(false)
            } else {
                controls.IsPlayEnabled().unwrap_or(false)
            },
    })
}

fn execute_media(session: &Session, action: MediaAction) -> Option<Pending<bool>> {
    let info = session.GetPlaybackInfo().ok()?;
    let controls = info.Controls().ok()?;
    let operation = match action {
        MediaAction::Previous if controls.IsPreviousEnabled().unwrap_or(false) => session.TrySkipPreviousAsync(),
        MediaAction::Next if controls.IsNextEnabled().unwrap_or(false) => session.TrySkipNextAsync(),
        MediaAction::PlayPause if controls.IsPlayPauseToggleEnabled().unwrap_or(false) => {
            session.TryTogglePlayPauseAsync()
        }
        MediaAction::PlayPause
            if info.PlaybackStatus().ok() == Some(Status::Playing) && controls.IsPauseEnabled().unwrap_or(false) =>
        {
            session.TryPauseAsync()
        }
        MediaAction::PlayPause
            if info.PlaybackStatus().ok() != Some(Status::Playing) && controls.IsPlayEnabled().unwrap_or(false) =>
        {
            session.TryPlayAsync()
        }
        _ => return None,
    };
    Pending::new(operation)
}

fn discord_voice(identity: &Identity) -> Option<DiscordVoice> {
    if !identity.discord {
        return None;
    }
    let state = crate::services::discord::state();
    let connected = state.status == crate::services::discord::Status::Connected;
    let same_client = identity.audio_keys.contains(&normalized(&state.client_path));
    Some(DiscordVoice {
        connected,
        available: connected && same_client && state.voice_available,
        error: if connected && !same_client {
            Some("Connected Discord client does not match this application".into())
        } else {
            state.voice_error
        },
        mute: state.self_mute,
        deaf: state.self_deaf,
        channel: state.channel,
        members: state.members.into_iter().take(8).collect(),
    })
}

fn run(shared: Arc<Mutex<Shared>>, rx: mpsc::Receiver<(u64, Action)>) {
    if unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.is_err() {
        if let Ok(mut shared) = shared.lock() {
            shared.snapshot.ready = true;
        }
        return;
    }
    let mut manager = None;
    let mut manager_request = Pending::new(Manager::RequestAsync());
    let mut media_actions: Vec<Pending<bool>> = Vec::new();
    let mut tracked: Vec<Tracked> = Vec::new();
    let mut next_id = 1u64;
    let mut last_generation = u64::MAX;
    let mut current_identity = Identity::default();
    let mut identity_at = Instant::now();
    let mut audio_at = Instant::now();
    let mut audio = None;

    loop {
        let active = shared.lock().is_ok_and(|s| !s.target.path.is_empty());
        let command = if active {
            rx.recv_timeout(Duration::from_millis(80))
        } else {
            rx.recv().map_err(|_| mpsc::RecvTimeoutError::Disconnected)
        };
        let mut commands = match command {
            Ok(command) => vec![command],
            Err(mpsc::RecvTimeoutError::Timeout) => Vec::new(),
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        };
        commands.extend(rx.try_iter().take(16));
        let Ok((target, generation, volume)) =
            shared.lock().map(|mut s| (s.target.clone(), s.generation, s.pending_volume.take()))
        else {
            break;
        };
        if let Some(volume) = volume {
            commands.push((generation, Action::Volume(volume)));
        }
        if target.path.is_empty() {
            media_actions.clear();
            for entry in &mut tracked {
                entry.properties = None;
            }
            continue;
        }
        let changed = generation != last_generation;
        let acting = commands.iter().any(|(g, action)| *g == generation && !matches!(action, Action::Wake));
        if changed || acting || identity_at.elapsed() >= Duration::from_millis(500) {
            current_identity = identity(&target);
            identity_at = Instant::now();
            last_generation = generation;
        }
        let identity = &current_identity;
        let mut snapshot = Snapshot { target: target.path.clone(), ready: true, ..Default::default() };
        if identity.valid {
            if let Some(result) = manager_request.as_ref().and_then(Pending::poll) {
                manager = result;
                manager_request = None;
            }
            if manager.is_none() && manager_request.is_none() {
                manager_request = Pending::new(Manager::RequestAsync());
            }
            let current = manager.as_ref().map(|m| matching_sessions(m, identity)).unwrap_or_default();
            let all = manager.as_ref().and_then(|m| m.GetSessions().ok());
            tracked
                .retain(|entry| all.as_ref().is_some_and(|sessions| sessions.into_iter().any(|s| s == entry.session)));
            for session in &current {
                if !tracked.iter().any(|entry| entry.session == *session) {
                    tracked.push(Tracked {
                        id: next_id,
                        session: session.clone(),
                        properties: None,
                        refreshed: Instant::now() - Duration::from_secs(1),
                        title: String::new(),
                        artist: String::new(),
                    });
                    next_id = next_id.wrapping_add(1).max(1);
                }
            }
            media_actions.retain(|operation| operation.poll().is_none());
            let last_volume = commands.iter().rposition(|(_, action)| matches!(action, Action::Volume(_)));
            for (command_index, (command_generation, action)) in commands.into_iter().enumerate() {
                if command_generation != generation || !shared.lock().is_ok_and(|s| s.generation == generation) {
                    continue;
                }
                match action {
                    Action::Media(id, action) => {
                        if media_actions.len() < 8
                            && let Some(entry) =
                                tracked.iter_mut().find(|entry| entry.id == id && current.contains(&entry.session))
                            && let Some(operation) = execute_media(&entry.session, action)
                        {
                            media_actions.push(operation);
                            entry.refreshed = Instant::now() - Duration::from_secs(1);
                        }
                    }
                    Action::Volume(volume) if Some(command_index) == last_volume => {
                        super::audio_mixer::target_audio(&identity.audio_keys, Some(volume), None);
                    }
                    Action::Mute(muted) => {
                        super::audio_mixer::target_audio(&identity.audio_keys, None, Some(muted));
                    }
                    Action::DiscordMute(muted) if discord_voice(identity).is_some_and(|voice| voice.available) => {
                        crate::services::discord::set_voice_for(&identity.audio_keys, Some(muted), None)
                    }
                    Action::DiscordDeaf(deaf) if discord_voice(identity).is_some_and(|voice| voice.available) => {
                        crate::services::discord::set_voice_for(&identity.audio_keys, None, Some(deaf))
                    }
                    _ => {}
                }
            }
            for entry in tracked.iter_mut().filter(|entry| current.contains(&entry.session)) {
                if !shared.lock().is_ok_and(|s| s.generation == generation) {
                    break;
                }
                if let Some(media) = media_snapshot(entry) {
                    snapshot.media.push(media);
                }
            }
            snapshot
                .media
                .sort_by(|a, b| (!a.playing, &a.source, &a.title, a.id).cmp(&(!b.playing, &b.source, &b.title, b.id)));
            if changed || acting || audio_at.elapsed() >= Duration::from_millis(240) {
                audio = super::audio_mixer::target_audio(&identity.audio_keys, None, None)
                    .map(|(volume, muted)| AppAudio { volume, muted });
                audio_at = Instant::now();
            }
            snapshot.audio = audio.clone();
            snapshot.discord = discord_voice(identity);
        }
        if let Ok(mut shared) = shared.lock()
            && shared.generation == generation
            && shared.snapshot != snapshot
        {
            shared.snapshot = snapshot;
            if shared.notify != 0 {
                unsafe {
                    let _ = windows::Win32::UI::WindowsAndMessaging::PostMessageW(
                        Some(HWND(shared.notify as *mut _)),
                        WM_ACTIONS_READY,
                        windows::Win32::Foundation::WPARAM(0),
                        windows::Win32::Foundation::LPARAM(0),
                    );
                }
            }
        }
    }
    drop(media_actions);
    drop(manager_request);
    drop(tracked);
    drop(manager);
    unsafe {
        RoUninitialize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dock_actions_identity_aliases_never_match_other_installations_or_substrings() {
        let path = normalized("C:/Apps/Chrome/chrome.exe");
        let mut aliases = vec!["chrome.profile.target".into()];
        add_path_aliases(&mut aliases, &path, std::slice::from_ref(&path));
        assert!(aliases.contains(&"chrome".into()));
        assert!(aliases.contains(&"chrome.exe".into()));
        assert!(!aliases.contains(&"chrome.profile.other".into()));
        assert!(!aliases.contains(&"chromedriver.exe".into()));
        aliases.clear();
        add_path_aliases(&mut aliases, &path, &[normalized("D:/Portable/chrome.exe")]);
        assert_eq!(aliases, [path]);
    }

    #[test]
    fn dock_actions_generation_replaces_snapshot_and_bounds_pending_clicks() {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let (tx, rx) = mpsc::sync_channel(16);
        let client = Client { shared: shared.clone(), tx };
        client.request("C:/Apps/player.exe", &[20, 10, 20]);
        let first = shared.lock().unwrap().generation;
        client.request("c:\\apps\\player.exe", &[10, 20]);
        assert_eq!(shared.lock().unwrap().generation, first);
        assert_eq!(shared.lock().unwrap().target.handles, [10, 20]);
        client.set_volume(f32::NAN);
        assert_eq!(rx.try_iter().count(), 1);
        client.control(5, MediaAction::Next);
        client.request("C:/Apps/other.exe", &[30]);
        assert_ne!(shared.lock().unwrap().generation, first);
        let queued: Vec<_> = rx.try_iter().collect();
        assert_eq!(queued[0].0, first);
        assert_ne!(queued[1].0, first);
        for _ in 0..100 {
            client.control(5, MediaAction::Next);
        }
        client.set_volume(0.2);
        client.set_volume(0.8);
        assert_eq!(shared.lock().unwrap().pending_volume, Some(0.8));
        assert_eq!(rx.try_iter().count(), 16);
        client.clear();
        assert!(shared.lock().unwrap().pending_volume.is_none());
        assert_eq!(client.snapshot().target, "");
        assert!(client.snapshot().media.is_empty());
    }

    #[test]
    fn dock_actions_cached_snapshots_are_isolated_and_expire() {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let (tx, _rx) = mpsc::sync_channel(16);
        let client = Client { shared: shared.clone(), tx };
        client.request("C:/Apps/player.exe", &[10]);
        {
            let mut state = shared.lock().unwrap();
            state.snapshot.ready = true;
            state.snapshot.audio = Some(AppAudio { volume: 0.42, muted: false });
        }
        client.clear();
        client.request("C:/Apps/player.exe", &[10]);
        assert_eq!(client.snapshot().audio.unwrap().volume, 0.42);
        client.request("C:/Apps/player.exe", &[20]);
        assert!(client.snapshot().audio.is_none());
        client.request("C:/Other/player.exe", &[10]);
        assert!(client.snapshot().audio.is_none());
        {
            let mut state = shared.lock().unwrap();
            for (_, _, at) in &mut state.cache {
                *at = Instant::now() - Duration::from_secs(4);
            }
        }
        client.request("C:/Apps/player.exe", &[10]);
        assert!(client.snapshot().audio.is_none());
    }

    #[test]
    fn dock_actions_pending_media_never_waits_and_expires_on_timeout() {
        let (tx, rx) = mpsc::channel();
        let operation = IAsyncOperation::<bool>::spawn(move || Ok(rx.recv_timeout(Duration::from_secs(2)).is_ok()));
        let mut pending = Pending::new(Ok(operation.clone())).unwrap();
        let start = Instant::now();
        assert!(pending.poll().is_none());
        assert!(start.elapsed() < Duration::from_millis(100));
        pending.started = Instant::now() - Duration::from_secs(1);
        assert_eq!(pending.poll(), Some(None));
        drop(pending);
        tx.send(()).unwrap();
        assert!(operation.join().unwrap());
        let ready = Pending::new(Ok(IAsyncOperation::<bool>::ready(Ok(true)))).unwrap();
        assert_eq!(ready.poll(), Some(Some(true)));
    }

    #[test]
    fn dock_actions_discord_detection_is_executable_specific() {
        assert!(is_discord(&normalized("C:/Discord/Discord.exe")));
        assert!(is_discord(&normalized("C:/DiscordCanary/DiscordCanary.exe")));
        assert!(!is_discord(&normalized("C:/Discord/Update.exe")));
        assert!(!is_discord(&normalized("C:/Other/not-discord.exe")));
    }

    #[test]
    fn native_dock_actions_target_identity_and_expired_windows() -> windows::core::Result<()> {
        if std::env::var_os("NI_DOCK_ACTIONS_NATIVE").is_none() {
            return Ok(());
        }
        use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
        use windows::Win32::UI::WindowsAndMessaging::*;
        use windows::core::w;
        unsafe {
            RoInitialize(RO_INIT_MULTITHREADED)?;
        }
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_NOACTIVATE,
                w!("STATIC"),
                w!("Dock action fixture"),
                WS_OVERLAPPED,
                -12000,
                -12000,
                160,
                80,
                None,
                None,
                None,
                None,
            )?
        };
        let path = std::env::current_exe().unwrap().to_string_lossy().into_owned();
        let target = Target { path: normalized(&path), handles: vec![hwnd.0 as isize] };
        let key = PROPERTYKEY { fmtid: GUID::from_u128(0x9f4c2855_9f79_4b39_a8d0_e1d42de1d5f3), pid: 5 };
        let store: IPropertyStore = unsafe { SHGetPropertyStoreForWindow(hwnd)? };
        unsafe {
            store.SetValue(&key, &PROPVARIANT::from("NightIsland.DockFixture.Target"))?;
        }
        let found = identity(&target);
        assert!(found.valid);
        assert!(found.ids.contains(&"nightisland.dockfixture.target".into()));
        assert!(found.audio_keys.contains(&normalized(&path)));
        let wrong = Target { path: "c:\\other\\player.exe".into(), ..target.clone() };
        assert!(!identity(&wrong).valid);
        unsafe {
            DestroyWindow(hwnd)?;
        }
        assert!(!identity(&target).valid);
        drop(store);
        unsafe {
            RoUninitialize();
        }
        Ok(())
    }

    #[test]
    fn native_dock_actions_explicit_media_session_and_app_volume() -> windows::core::Result<()> {
        if std::env::var_os("NI_DOCK_ACTIONS_NATIVE").is_none() {
            return Ok(());
        }
        use std::sync::atomic::{AtomicUsize, Ordering};
        use windows::Foundation::TypedEventHandler;
        use windows::Media::Core::MediaSource;
        use windows::Media::Playback::MediaPlayer;
        use windows::Media::{MediaPlaybackStatus, MediaPlaybackType, SystemMediaTransportControlsButton};
        use windows::Storage::Streams::{DataWriter, InMemoryRandomAccessStream};
        use windows::Win32::UI::WindowsAndMessaging::*;
        use windows::core::{HSTRING, w};
        unsafe {
            RoInitialize(RO_INIT_MULTITHREADED)?;
        }
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_NOACTIVATE,
                w!("STATIC"),
                w!("Dock media fixture"),
                WS_OVERLAPPED,
                -12000,
                -12000,
                160,
                80,
                None,
                None,
                None,
                None,
            )?
        };
        let mut bytes = Vec::new();
        let size = 8000u32 * 2 * 30;
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(size + 36).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&8000u32.to_le_bytes());
        bytes.extend_from_slice(&16000u32.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&size.to_le_bytes());
        bytes.resize(bytes.len() + size as usize, 0);
        let mut players = Vec::new();
        let mut counters = Vec::new();
        for index in 0..2 {
            let stream = InMemoryRandomAccessStream::new()?;
            let writer = DataWriter::CreateDataWriter(&stream)?;
            writer.WriteBytes(&bytes)?;
            writer.StoreAsync()?.join()?;
            writer.DetachStream()?;
            stream.Seek(0)?;
            let player = MediaPlayer::new()?;
            player.SetVolume(0.0)?;
            player.CommandManager()?.SetIsEnabled(false)?;
            let controls = player.SystemMediaTransportControls()?;
            controls.SetIsEnabled(true)?;
            controls.SetIsPlayEnabled(true)?;
            controls.SetIsPauseEnabled(true)?;
            controls.SetIsNextEnabled(true)?;
            controls.SetIsPreviousEnabled(false)?;
            let counter = Arc::new(AtomicUsize::new(0));
            let count = counter.clone();
            controls.ButtonPressed(
                &TypedEventHandler::new(
                    move |_,
                          args: windows::core::Ref<
                        windows::Media::SystemMediaTransportControlsButtonPressedEventArgs,
                    >| {
                        if args
                            .as_ref()
                            .is_some_and(|args| args.Button().ok() == Some(SystemMediaTransportControlsButton::Next))
                        {
                            count.fetch_add(1, Ordering::SeqCst);
                        }
                        Ok(())
                    },
                ),
            )?;
            let updater = controls.DisplayUpdater()?;
            updater.SetType(MediaPlaybackType::Music)?;
            updater.MusicProperties()?.SetTitle(&HSTRING::from(format!("Dock fixture {index}")))?;
            updater.MusicProperties()?.SetArtist(&HSTRING::from("Silent fixture"))?;
            updater.Update()?;
            player.SetSource(&MediaSource::CreateFromStream(&stream, &HSTRING::from("audio/wav"))?)?;
            player.Play()?;
            controls.SetPlaybackStatus(MediaPlaybackStatus::Playing)?;
            players.push(player);
            counters.push(counter);
        }
        let client = Client::new();
        let path = std::env::current_exe().unwrap().to_string_lossy().into_owned();
        client.request(&path, &[hwnd.0 as isize]);
        let deadline = Instant::now() + Duration::from_secs(12);
        let snapshot = loop {
            let snapshot = client.snapshot();
            if snapshot.media.iter().filter(|session| session.title.starts_with("Dock fixture ")).count() == 2
                && snapshot.audio.is_some()
            {
                break snapshot;
            }
            assert!(Instant::now() < deadline, "both fixture media sessions and app audio were not discovered");
            std::thread::sleep(Duration::from_millis(40));
        };
        client.clear();
        client.request(&path, &[hwnd.0 as isize]);
        let cached = client.snapshot();
        assert_eq!(cached.media.len(), snapshot.media.len());
        assert!(cached.audio == snapshot.audio);
        let session = snapshot.media.iter().find(|session| session.title == "Dock fixture 1").unwrap();
        assert!(session.can_next);
        assert!(!session.can_previous);
        client.control(session.id, MediaAction::Next);
        let deadline = Instant::now() + Duration::from_secs(3);
        while counters[1].load(Ordering::SeqCst) == 0 {
            assert!(Instant::now() < deadline, "target session did not receive Next");
            std::thread::sleep(Duration::from_millis(30));
        }
        assert_eq!(counters[0].load(Ordering::SeqCst), 0);
        client.set_volume(0.23);
        client.set_muted(true);
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let state = client.snapshot();
            if state.audio.is_some_and(|audio| (audio.volume - 0.23).abs() < 0.01 && audio.muted) {
                break;
            }
            assert!(Instant::now() < deadline, "app output controls did not apply");
            std::thread::sleep(Duration::from_millis(30));
        }
        client.request("C:/not-the-fixture/player.exe", &[hwnd.0 as isize]);
        client.control(session.id, MediaAction::Next);
        let deadline = Instant::now() + Duration::from_secs(2);
        while !client.snapshot().ready {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(30));
        }
        assert!(client.snapshot().media.is_empty());
        assert!(client.snapshot().audio.is_none());
        assert_eq!(counters[1].load(Ordering::SeqCst), 1);
        client.clear();
        drop(client);
        for player in &players {
            player.Pause()?;
            player.Close()?;
        }
        drop(players);
        unsafe {
            DestroyWindow(hwnd)?;
            RoUninitialize();
        }
        Ok(())
    }
}
