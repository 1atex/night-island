use crate::data::settings::VpnProvider;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::PostMessageW;
#[path = "happ.rs"]
mod happ;
#[path = "svoi_vpn.rs"]
mod svoi;
pub use svoi::Snapshot;
pub const WM_VPN: u32 = happ::WM_VPN;
static PROVIDER: AtomicU8 = AtomicU8::new(0);
static GENERATION: AtomicU64 = AtomicU64::new(0);
static WATCHING: AtomicBool = AtomicBool::new(false);
static BUSY: AtomicBool = AtomicBool::new(false);
static SNAPSHOT: OnceLock<Mutex<Snapshot>> = OnceLock::new();
static LAST_ACTION: Mutex<Option<Instant>> = Mutex::new(None);
fn store() -> &'static Mutex<Snapshot> {
    SNAPSHOT.get_or_init(|| Mutex::new(Snapshot::closed()))
}
pub fn snapshot() -> Snapshot {
    store().lock().unwrap().clone()
}
pub fn provider() -> VpnProvider {
    match PROVIDER.load(Ordering::SeqCst) {
        1 => VpnProvider::Svoi,
        2 => VpnProvider::Off,
        _ => VpnProvider::Happ,
    }
}
pub fn set_provider(value: VpnProvider) {
    PROVIDER.store(
        match value {
            VpnProvider::Happ => 0,
            VpnProvider::Svoi => 1,
            VpnProvider::Off => 2,
        },
        Ordering::SeqCst,
    );
    GENERATION.fetch_add(1, Ordering::SeqCst);
    BUSY.store(false, Ordering::SeqCst);
    *store().lock().unwrap() = Snapshot::closed();
    happ::set_enabled(value == VpnProvider::Happ);
}
pub fn state() -> Option<bool> {
    match provider() {
        VpnProvider::Happ => happ::state(),
        VpnProvider::Off => None,
        VpnProvider::Svoi => snapshot().connected(),
    }
}
pub fn switching() -> bool {
    if provider() == VpnProvider::Happ { happ::switching() } else { BUSY.load(Ordering::SeqCst) }
}
pub fn take_result() -> Option<Result<(), &'static str>> {
    if provider() == VpnProvider::Happ { happ::take_result() } else { None }
}
fn post(raw: isize) {
    unsafe {
        let _ = PostMessageW(Some(HWND(raw as *mut _)), WM_VPN, WPARAM(0), LPARAM(0));
    }
}
fn accept(value: Snapshot, generation: u64, raw: isize) {
    if generation != GENERATION.load(Ordering::SeqCst) || provider() != VpnProvider::Svoi {
        return;
    }
    let mut previous = store().lock().unwrap();
    if value.instance > 0
        && previous.instance > 0
        && (value.instance < previous.instance
            || (value.instance == previous.instance && value.sequence < previous.sequence))
    {
        return;
    }
    let mut a = value.clone();
    let mut b = previous.clone();
    a.sequence = 0;
    b.sequence = 0;
    let changed = a != b;
    *previous = value;
    drop(previous);
    if changed {
        post(raw);
    }
}
pub fn start_watch(hwnd: HWND) {
    happ::start_watch(hwnd);
    if WATCHING.swap(true, Ordering::SeqCst) {
        return;
    }
    let raw = hwnd.0 as isize;
    std::thread::spawn(move || {
        loop {
            if provider() == VpnProvider::Svoi {
                let generation = GENERATION.load(Ordering::SeqCst);
                accept(svoi::request("state", ""), generation, raw);
            }
            std::thread::sleep(Duration::from_secs(2));
        }
    });
}
pub fn toggle(hwnd: HWND) -> bool {
    match provider() {
        VpnProvider::Happ => happ::toggle(hwnd),
        VpnProvider::Off => false,
        VpnProvider::Svoi => command(hwnd, if state() == Some(true) { "disconnect" } else { "connect" }, ""),
    }
}
pub fn command(hwnd: HWND, action: &str, id: &str) -> bool {
    if provider() != VpnProvider::Svoi {
        return false;
    }
    {
        let mut last = LAST_ACTION.lock().unwrap();
        if last.is_some_and(|at| at.elapsed() < Duration::from_millis(450)) {
            return false;
        }
        *last = Some(Instant::now());
    }
    if BUSY.swap(true, Ordering::SeqCst) {
        return false;
    }
    let raw = hwnd.0 as isize;
    let generation = GENERATION.load(Ordering::SeqCst);
    let (action, id) = (action.to_string(), id.to_string());
    std::thread::spawn(move || {
        let result = svoi::request(&action, &id);
        accept(result, generation, raw);
        if generation == GENERATION.load(Ordering::SeqCst) {
            BUSY.store(false, Ordering::SeqCst);
            post(raw);
        }
    });
    true
}
pub fn open_svoi() -> bool {
    svoi::open()
}
pub fn announced_change(previous: &Snapshot, next: &Snapshot) -> Option<String> {
    if next.access != "allowed" || !next.verified || next.state != "running" {
        return None;
    }
    let active = next.active.as_ref()?;
    let changed = previous.access == "allowed"
        && (previous.active.as_ref().map(|s| &s.id) != Some(&active.id)
            || previous.started_at != next.started_at
            || !previous.verified);
    changed.then(|| format!("SVOI · {}", active.name))
}

pub fn handle_cli() -> bool {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) != Some("--svoi-integration-check") {
        return false;
    }
    let action = args.get(2).map(String::as_str).unwrap_or("state");
    let id = args.get(3).map(String::as_str).unwrap_or("");
    if ["state", "favorites", "connect", "switch", "disconnect"].contains(&action)
        && let Ok(value) = serde_json::to_string(&svoi::request(action, id))
    {
        println!("{value}");
    }
    true
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    fn connected(id: &str) -> Snapshot {
        serde_json::from_value(serde_json::json!({"version":1,"access":"allowed","state":"running","active":{"id":id,"name":id},"verified":true,"startedAt":"2026-11-12T12:00:00Z"})).unwrap()
    }
    #[test]
    fn confirmed_switches_only_announce_once_and_never_use_selected_as_active() {
        let a = connected("Finland");
        let mut b = connected("Germany");
        b.verified = false;
        assert!(announced_change(&a, &b).is_none());
        b.verified = true;
        assert_eq!(announced_change(&a, &b).as_deref(), Some("SVOI · Germany"));
        assert!(announced_change(&b, &b).is_none());
        assert!(announced_change(&Snapshot::closed(), &b).is_none());
        let mut failed = a.clone();
        failed.selected = b.active.clone();
        failed.error = "failed".into();
        assert!(announced_change(&a, &failed).is_none());
        assert_eq!(failed.active.unwrap().id, "Finland");
    }
    #[test]
    fn old_profiles_keep_happ_and_provider_is_exported_without_permission() {
        let old: crate::data::settings::Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(old.vpn_provider, VpnProvider::Happ);
        let mut now = old;
        now.vpn_provider = VpnProvider::Svoi;
        let value = serde_json::to_value(&now).unwrap();
        assert_eq!(value["vpn_provider"], "svoi");
        let text = value.to_string();
        assert!(!text.contains("candidate"));
        assert!(!text.contains("grant"));
        let restored: crate::data::settings::Settings = serde_json::from_value(value).unwrap();
        assert_eq!(restored.vpn_provider, VpnProvider::Svoi);
        assert_eq!(restored.card_shadows, now.card_shadows);
        assert_eq!(restored.desktop_dock, now.desktop_dock);
        assert_eq!(restored.top_panel, now.top_panel);
    }
}
