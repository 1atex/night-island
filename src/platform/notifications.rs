use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use windows::Foundation::{Size, TypedEventHandler};
use windows::Graphics::Imaging::{
    BitmapAlphaMode, BitmapDecoder, BitmapPixelFormat, BitmapTransform, ColorManagementMode, ExifOrientationMode,
};
use windows::UI::Notifications::Management::{UserNotificationListener, UserNotificationListenerAccessStatus};
use windows::UI::Notifications::{
    KnownNotificationBindings, NotificationKinds, ToastNotificationManager, ToastNotificationMode, UserNotification,
};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};
use windows::core::Result;

pub const WM_NOTIFICATIONS: u32 = WM_APP + 55;
pub const WM_NOTIFICATION_ACCESS: u32 = WM_APP + 56;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Status {
    #[default]
    Disabled,
    NeedsAccess,
    Denied,
    NeedsQuietMode,
    Ready,
    Unavailable,
}

#[derive(Clone)]
pub struct Record {
    pub id: u32,
    pub created: i64,
    pub app_id: String,
    pub app: String,
    pub title: String,
    pub body: String,
    pub icon: Option<Arc<super::apps::Icon>>,
}

impl PartialEq for Record {
    fn eq(&self, other: &Self) -> bool {
        (self.id, self.created, &self.app_id, &self.app, &self.title, &self.body)
            == (other.id, other.created, &other.app_id, &other.app, &other.title, &other.body)
    }
}

#[derive(Clone, Default)]
pub struct Snapshot {
    pub status: Status,
    pub records: Vec<Record>,
    pub added: Vec<Record>,
}

enum Command {
    Enabled(bool),
    Wake,
}
static WORKER: OnceLock<mpsc::Sender<Command>> = OnceLock::new();
static STATE: Mutex<Snapshot> =
    Mutex::new(Snapshot { status: Status::Disabled, records: Vec::new(), added: Vec::new() });

pub fn take_snapshot() -> Snapshot {
    STATE
        .lock()
        .map(|mut state| {
            let mut snapshot = state.clone();
            snapshot.added = std::mem::take(&mut state.added);
            snapshot
        })
        .unwrap_or_default()
}

pub fn configure(hwnd: HWND, enabled: bool) {
    if !enabled && WORKER.get().is_none() {
        return;
    }
    let sender = WORKER.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        let wake = tx.clone();
        let target = hwnd.0 as isize;
        std::thread::spawn(move || run(target, wake, rx));
        tx
    });
    let _ = sender.send(Command::Enabled(enabled));
}

pub fn request_access(hwnd: HWND) -> Result<()> {
    let operation = UserNotificationListener::Current()?.RequestAccessAsync()?;
    let target = hwnd.0 as isize;
    std::thread::spawn(move || {
        let initialized = unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.is_ok();
        let allowed = operation.join().is_ok_and(|status| status == UserNotificationListenerAccessStatus::Allowed);
        unsafe {
            let _ = PostMessageW(
                Some(HWND(target as *mut _)),
                WM_NOTIFICATION_ACCESS,
                WPARAM(usize::from(allowed)),
                LPARAM(0),
            );
        }
        if let Some(worker) = WORKER.get() {
            let _ = worker.send(Command::Wake);
        }
        if initialized {
            unsafe {
                RoUninitialize();
            }
        }
    });
    Ok(())
}

pub fn now_ticks() -> i64 {
    let elapsed = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    ((elapsed.as_secs() + 11_644_473_600) * 10_000_000 + u64::from(elapsed.subsec_nanos() / 100)) as i64
}

fn text(value: &str, limit: usize) -> String {
    value
        .chars()
        .filter(|c| !c.is_control() || matches!(c, '\n' | '\t'))
        .take(limit)
        .collect::<String>()
        .trim()
        .to_string()
}

fn app_icon(info: &windows::ApplicationModel::AppInfo) -> Option<Arc<super::apps::Icon>> {
    let stream =
        info.DisplayInfo().ok()?.GetLogo(Size { Width: 40.0, Height: 40.0 }).ok()?.OpenReadAsync().ok()?.join().ok()?;
    let decoder = BitmapDecoder::CreateAsync(&stream).ok()?.join().ok()?;
    let transform = BitmapTransform::new().ok()?;
    transform.SetScaledWidth(40).ok()?;
    transform.SetScaledHeight(40).ok()?;
    let provider = decoder
        .GetPixelDataTransformedAsync(
            BitmapPixelFormat::Bgra8,
            BitmapAlphaMode::Premultiplied,
            &transform,
            ExifOrientationMode::IgnoreExifOrientation,
            ColorManagementMode::DoNotColorManage,
        )
        .ok()?
        .join()
        .ok()?;
    let data = provider.DetachPixelData().ok()?;
    (data.len() == 40 * 40 * 4).then(|| Arc::new(super::apps::Icon { side: 40, bgra: data.to_vec() }))
}

fn read_record(
    notification: &UserNotification,
    icons: &mut HashMap<String, Option<Arc<super::apps::Icon>>>,
) -> Result<Record> {
    let id = notification.Id()?;
    let created = notification.CreationTime()?.UniversalTime;
    let info = notification.AppInfo().ok();
    let app_id = info
        .as_ref()
        .and_then(|info| info.AppUserModelId().ok())
        .map(|id| id.to_string())
        .unwrap_or_else(|| "Windows".into());
    let app = info
        .as_ref()
        .and_then(|info| info.DisplayInfo().ok())
        .and_then(|info| info.DisplayName().ok())
        .map(|name| text(&name.to_string(), 80))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Windows".into());
    let binding = notification
        .Notification()
        .and_then(|notification| notification.Visual())
        .and_then(|visual| {
            visual.GetBinding(&KnownNotificationBindings::ToastGeneric()?).or_else(|_| visual.Bindings()?.GetAt(0))
        })
        .ok();
    let lines: Vec<String> = binding
        .and_then(|binding| binding.GetTextElements().ok())
        .into_iter()
        .flat_map(|elements| elements.into_iter())
        .filter_map(|line| line.Text().ok())
        .map(|value| text(&value.to_string(), 1200))
        .filter(|s| !s.is_empty())
        .take(8)
        .collect();
    let icon = icons.entry(app_id.clone()).or_insert_with(|| info.as_ref().and_then(app_icon)).clone();
    Ok(Record {
        id,
        created,
        app_id,
        app,
        title: lines.first().map(|line| text(line, 180)).unwrap_or_else(|| "New notification".into()),
        body: text(&lines.into_iter().skip(1).collect::<Vec<_>>().join("\n"), 1200),
        icon,
    })
}

fn merge(state: &mut Snapshot, status: Status, records: Vec<Record>, added: Vec<Record>) -> bool {
    let changed = state.status != status || state.records != records || !added.is_empty();
    if status == Status::Ready {
        state.added.retain(|pending| records.iter().any(|record| record == pending));
        for record in added {
            if !state.added.contains(&record) && records.contains(&record) {
                state.added.push(record);
            }
        }
    } else {
        state.added.clear();
    }
    state.status = status;
    state.records = records;
    changed
}

fn publish(target: isize, status: Status, records: Vec<Record>, added: Vec<Record>) {
    if let Ok(mut state) = STATE.lock()
        && !merge(&mut state, status, records, added)
    {
        return;
    }
    unsafe {
        let _ = PostMessageW(Some(HWND(target as *mut _)), WM_NOTIFICATIONS, WPARAM(0), LPARAM(0));
    }
}

fn should_present(old: Option<&Record>, created: i64, started: i64, status: Status) -> bool {
    status == Status::Ready && created >= started && old.is_none_or(|old| old.created != created)
}

fn run(target: isize, wake: mpsc::Sender<Command>, rx: mpsc::Receiver<Command>) {
    if unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.is_err() {
        publish(target, Status::Unavailable, Vec::new(), Vec::new());
        return;
    }
    let Ok(listener) = UserNotificationListener::Current() else {
        publish(target, Status::Unavailable, Vec::new(), Vec::new());
        unsafe {
            RoUninitialize();
        }
        return;
    };
    let listening = Arc::new(AtomicBool::new(false));
    let event_enabled = listening.clone();
    let tx = wake.clone();
    let events = listener
        .NotificationChanged(&TypedEventHandler::new(move |_, _| {
            if event_enabled.load(Ordering::Relaxed) {
                let _ = tx.send(Command::Wake);
            }
            Ok(())
        }))
        .ok();
    let manager = ToastNotificationManager::GetDefault().ok();
    let mode_event = manager.as_ref().and_then(|manager| {
        let event_enabled = listening.clone();
        manager
            .NotificationModeChanged(&TypedEventHandler::new(move |_, _| {
                if event_enabled.load(Ordering::Relaxed) {
                    let _ = wake.send(Command::Wake);
                }
                Ok(())
            }))
            .ok()
    });
    let mut enabled = false;
    let mut started = now_ticks();
    let mut cache: HashMap<u32, Record> = HashMap::new();
    let mut icons = HashMap::new();
    loop {
        let command = if enabled {
            rx.recv_timeout(if events.is_some() { Duration::from_secs(20) } else { Duration::from_millis(900) })
        } else {
            rx.recv().map_err(|_| mpsc::RecvTimeoutError::Disconnected)
        };
        match command {
            Ok(Command::Enabled(on)) => {
                enabled = on;
                listening.store(on, Ordering::Relaxed);
                started = now_ticks();
                cache.clear();
                if !enabled {
                    icons.clear();
                    publish(target, Status::Disabled, Vec::new(), Vec::new());
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            _ => {}
        }
        if !enabled {
            continue;
        }
        let access = listener.GetAccessStatus();
        let status = match access {
            Ok(UserNotificationListenerAccessStatus::Allowed) => {
                let quiet = manager.as_ref().and_then(|manager| manager.NotificationMode().ok());
                if quiet == Some(ToastNotificationMode::Unrestricted) { Status::NeedsQuietMode } else { Status::Ready }
            }
            Ok(UserNotificationListenerAccessStatus::Denied) => Status::Denied,
            Ok(_) => Status::NeedsAccess,
            Err(_) => Status::Unavailable,
        };
        if !matches!(status, Status::Ready | Status::NeedsQuietMode) {
            cache.clear();
            icons.clear();
            started = now_ticks();
            publish(target, status, Vec::new(), Vec::new());
            continue;
        }
        let Ok(list) = listener.GetNotificationsAsync(NotificationKinds::Toast).and_then(|request| request.join())
        else {
            publish(target, Status::Unavailable, Vec::new(), Vec::new());
            continue;
        };
        let mut next = HashMap::new();
        let mut added = Vec::new();
        for notification in list.into_iter().take(1024) {
            let (Ok(id), Ok(created)) = (notification.Id(), notification.CreationTime()) else { continue };
            let old = cache.get(&id);
            let record = if let Some(record) = old.filter(|record| record.created == created.UniversalTime) {
                Some(record.clone())
            } else {
                read_record(&notification, &mut icons).ok()
            };
            if let Some(record) = record {
                if should_present(old, record.created, started, status) {
                    added.push(record.clone());
                }
                next.insert(id, record);
            }
        }
        cache = next;
        icons.retain(|id, _| cache.values().any(|record| &record.app_id == id));
        let mut records: Vec<_> = cache.values().cloned().collect();
        records.sort_by_key(|record| std::cmp::Reverse(record.created));
        added.sort_by_key(|record| record.created);
        publish(target, status, records, added);
    }
    if let Some(token) = events {
        let _ = listener.RemoveNotificationChanged(token);
    }
    if let (Some(manager), Some(token)) = (manager, mode_event) {
        let _ = manager.RemoveNotificationModeChanged(token);
    }
    unsafe {
        RoUninitialize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(id: u32, created: i64) -> Record {
        Record {
            id,
            created,
            app_id: "test".into(),
            app: "Test".into(),
            title: "Title".into(),
            body: "Body".into(),
            icon: None,
        }
    }

    #[test]
    fn history_is_not_replayed_and_reused_ids_are_not_lost() {
        let old = record(10, 100);
        assert!(!should_present(None, 100, 200, Status::Ready));
        assert!(!should_present(Some(&old), 100, 50, Status::Ready));
        assert!(should_present(None, 210, 200, Status::Ready));
        assert!(should_present(Some(&old), 220, 200, Status::Ready));
        for status in [Status::Disabled, Status::Denied, Status::NeedsQuietMode, Status::Unavailable] {
            assert!(!should_present(None, 210, 200, status));
        }
    }

    #[test]
    fn pending_delivery_survives_coalesced_events_and_clears_on_permission_loss() {
        let a = record(1, 200);
        let b = record(2, 201);
        let mut state = Snapshot::default();
        merge(&mut state, Status::Ready, vec![a.clone()], vec![a.clone()]);
        merge(&mut state, Status::Ready, vec![a.clone(), b.clone()], vec![b.clone(), b.clone()]);
        assert!(state.added == [a.clone(), b.clone()]);
        merge(&mut state, Status::Ready, vec![b.clone()], Vec::new());
        assert!(state.added == [b.clone()]);
        merge(&mut state, Status::Denied, Vec::new(), Vec::new());
        assert!(state.added.is_empty() && state.records.is_empty());
        merge(&mut state, Status::Ready, vec![a.clone()], vec![a]);
        merge(&mut state, Status::NeedsQuietMode, vec![b], Vec::new());
        assert!(state.added.is_empty());
    }

    #[test]
    fn notification_text_is_bounded_without_splitting_unicode() {
        assert_eq!(text("  Привет\0\r\nмир  ", 100), "Привет\nмир");
        assert_eq!(text("🌙🌧️", 2), "🌙🌧");
        let seconds = now_ticks() / 10_000_000 - 11_644_473_600;
        assert!((seconds - SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64).abs() <= 1);
    }

    #[test]
    #[ignore = "Reads notification capability status without requesting permission or reading notifications"]
    fn notification_access_probe() {
        use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize};
        unsafe {
            RoInitialize(RO_INIT_MULTITHREADED).unwrap();
        }
        match UserNotificationListener::Current().and_then(|listener| listener.GetAccessStatus()) {
            Ok(status) => {
                println!("Notification access status: {}", status.0);
                let listener = UserNotificationListener::Current().unwrap();
                if status == windows::UI::Notifications::Management::UserNotificationListenerAccessStatus::Allowed {
                    let result = listener
                        .GetNotificationsAsync(windows::UI::Notifications::NotificationKinds::Toast)
                        .and_then(|request| request.join());
                    println!("Notification snapshot readable: {}", result.is_ok());
                }
                let handler = windows::Foundation::TypedEventHandler::new(|_, _| Ok(()));
                match listener.NotificationChanged(&handler) {
                    Ok(token) => {
                        println!("Notification events available");
                        let _ = listener.RemoveNotificationChanged(token);
                    }
                    Err(error) => println!("Notification events unavailable: {:?}", error.code()),
                }
                let mode = windows::UI::Notifications::ToastNotificationManager::GetDefault()
                    .and_then(|manager| manager.NotificationMode());
                println!("Quiet mode API available: {}", mode.is_ok());
            }
            Err(error) => println!("Notification access unavailable: {:?}", error.code()),
        }
        println!("Package identity available: {}", windows::ApplicationModel::Package::Current().is_ok());
        unsafe {
            RoUninitialize();
        }
    }
}
