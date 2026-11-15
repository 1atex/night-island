mod hub;
mod model;
pub use hub::{Features, RequestItem, TabItem};
mod pipe;
pub use model::{Action, Download, State};
use std::collections::VecDeque;
use std::sync::Mutex;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};

pub const WM_BROWSER: u32 = WM_APP + 39;
pub const WM_BROWSER_SHOW: u32 = WM_APP + 114;

#[derive(Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub struct Appearance {
    pub panel: [u8; 3],
    pub accent: [u8; 3],
    pub motion: bool,
}

pub fn set_appearance(settings: &crate::data::settings::Settings) {
    let to_bytes = |(r, g, b): (f32, f32, f32)| [r, g, b].map(|n| (n * 255.0).round().clamp(0.0, 255.0) as u8);
    let (panel, accent) = crate::ui::theme::colors(settings);
    let appearance =
        Appearance { panel: to_bytes(panel), accent: to_bytes(accent), motion: settings.island_animations };
    if let Ok(mut shared) = SHARED.lock()
        && shared.appearance != appearance
    {
        shared.appearance = appearance;
        pipe::signal(shared.port, pipe::APPEARANCE_CHANGED);
    }
}

fn appearance_frame() -> serde_json::Value {
    let value = SHARED.lock().map(|shared| shared.appearance).unwrap_or_default();
    serde_json::json!({"v":1,"type":"appearance","panel":value.panel,"accent":value.accent,"motion":value.motion})
}

#[derive(Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct Snapshot {
    pub connected: bool,
    pub items: Vec<Download>,
    pub error: bool,
    pub pending: Vec<String>,
    pub features: Features,
    pub tabs: Vec<TabItem>,
    pub query: String,
    pub searching: bool,
    pub activity: Vec<TabItem>,
    pub requests: Vec<RequestItem>,
}

#[derive(Clone)]
struct Command {
    id: String,
    action: Action,
    request: String,
}

struct Shared {
    state: Snapshot,
    appearance: Appearance,
    commands: VecDeque<Command>,
    outgoing: VecDeque<serde_json::Value>,
    pocket: VecDeque<(String, crate::data::pocket::PocketItem)>,
    peer_pid: u32,
    port: usize,
    sequence: u64,
    thread: Option<std::thread::JoinHandle<()>>,
}

static SHARED: Mutex<Shared> = Mutex::new(Shared {
    appearance: Appearance { panel: [0; 3], accent: [166, 179, 255], motion: true },
    state: Snapshot {
        connected: false,
        items: Vec::new(),
        error: false,
        pending: Vec::new(),
        features: Features { tabs: false, pocket: false, activity: false, requests: false },
        tabs: Vec::new(),
        query: String::new(),
        searching: false,
        activity: Vec::new(),
        requests: Vec::new(),
    },
    commands: VecDeque::new(),
    outgoing: VecDeque::new(),
    pocket: VecDeque::new(),
    peer_pid: 0,
    port: 0,
    sequence: 0,
    thread: None,
});

pub fn snapshot() -> Snapshot {
    SHARED.lock().map(|s| s.state.clone()).unwrap_or_default()
}

pub fn configure(hwnd: HWND, enabled: bool) {
    let old = if let Ok(mut shared) = SHARED.lock() {
        if shared.port != 0 {
            pipe::signal(shared.port, 0);
        }
        shared.port = 0;
        shared.commands.clear();
        shared.outgoing.clear();
        shared.pocket.clear();
        shared.peer_pid = 0;
        shared.state = Snapshot::default();
        shared.thread.take()
    } else {
        None
    };
    if let Some(thread) = old {
        let _ = thread.join();
    }
    if enabled {
        pipe::start(hwnd);
    }
}

pub fn command(id: &str, action: Action) -> bool {
    let Ok(mut shared) = SHARED.lock() else { return false };
    if !shared.state.connected
        || shared.state.pending.len() >= 16
        || shared.state.pending.iter().any(|pending| pending == id)
        || !shared.state.items.iter().any(|item| item.id == id && item.actions.contains(&action))
    {
        return false;
    }
    shared.sequence = shared.sequence.wrapping_add(1);
    let request = format!("{}", shared.sequence);
    shared.state.pending.push(id.into());
    shared.state.error = false;
    shared.commands.push_back(Command { id: id.into(), action, request });
    pipe::signal(shared.port, 1);
    true
}

fn update(hwnd: isize, state: Snapshot) {
    change(hwnd, |current| *current = state);
}

fn change(hwnd: isize, action: impl FnOnce(&mut Snapshot)) {
    if let Ok(mut shared) = SHARED.lock() {
        let before = shared.state.clone();
        action(&mut shared.state);
        if shared.state == before {
            return;
        }
    }
    unsafe {
        let _ = PostMessageW(Some(HWND(hwnd as _)), WM_BROWSER, WPARAM(0), LPARAM(0));
    }
}

pub fn search_tabs(query: &str) -> bool {
    let Ok(mut shared) = SHARED.lock() else { return false };
    if !shared.state.connected || !shared.state.features.tabs {
        return false;
    }
    shared.sequence = shared.sequence.wrapping_add(1);
    let request = format!("hub-{}", shared.sequence);
    shared.state.query = request.clone();
    shared.state.searching = true;
    shared.state.tabs.clear();
    shared.outgoing.retain(|frame| frame["type"] != "tab-query");
    if shared.outgoing.len() >= 16 {
        return false;
    }
    shared.outgoing.push_back(serde_json::json!({"v":1,"type":"tab-query","request":request,"query":query.chars().take(128).collect::<String>()}));
    pipe::signal(shared.port, 4);
    true
}

pub fn hub_action(kind: &str, id: &str, action: &str) -> bool {
    let Ok(mut shared) = SHARED.lock() else { return false };
    let valid = match kind {
        "tab-action" => {
            ["focus", "mute"].contains(&action)
                && (shared.state.features.tabs || shared.state.features.activity)
                && shared.state.tabs.iter().chain(&shared.state.activity).any(|tab| tab.id == id)
        }
        "request-action" => {
            shared.state.features.requests
                && ["forward", "abort", "edit"].contains(&action)
                && shared.state.requests.iter().any(|r| r.id == id)
        }
        "open-link" => shared.state.features.pocket && crate::data::pocket::web_url(id),
        _ => false,
    };
    if !valid
        || !shared.state.connected
        || shared.outgoing.len() >= 16
        || shared.state.pending.len() >= 16
        || shared.state.pending.iter().any(|pending| pending == id)
    {
        return false;
    }
    shared.sequence = shared.sequence.wrapping_add(1);
    let request = format!("hub-{}", shared.sequence);
    shared.state.pending.push(id.into());
    shared.state.error = false;
    let frame = if kind == "open-link" {
        serde_json::json!({"v":1,"type":kind,"url":id,"request":request})
    } else {
        serde_json::json!({"v":1,"type":kind,"id":id,"action":action,"request":request})
    };
    if shared.peer_pid != 0 && (action == "focus" || action == "edit" || kind == "open-link") {
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow(shared.peer_pid);
        }
    }
    shared.outgoing.push_back(frame);
    pipe::signal(shared.port, 4);
    true
}

pub fn take_pocket() -> Vec<(String, crate::data::pocket::PocketItem)> {
    SHARED.lock().map(|mut s| s.pocket.drain(..).collect()).unwrap_or_default()
}

pub fn pocket_result(request: &str, ok: bool) {
    if let Ok(mut shared) = SHARED.lock()
        && shared.state.connected
        && shared.outgoing.len() < 16
    {
        shared.outgoing.push_back(serde_json::json!({"v":1,"type":"pocket-result","request":request,"ok":ok}));
        pipe::signal(shared.port, 4);
    }
}

pub fn end_search() {
    if let Ok(mut shared) = SHARED.lock()
        && !shared.state.query.is_empty()
    {
        shared.state.query.clear();
        shared.state.tabs.clear();
        shared.state.searching = false;
        shared.sequence = shared.sequence.wrapping_add(1);
        let request = format!("hub-{}", shared.sequence);
        shared.outgoing.retain(|frame| frame["type"] != "tab-query");
        if shared.outgoing.len() < 16 {
            shared.outgoing.push_back(serde_json::json!({"v":1,"type":"tab-query-end","request":request}));
            pipe::signal(shared.port, 4);
        }
    }
}
