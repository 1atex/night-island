mod model;
mod pipe;
pub use model::{Action, Download, State};
use std::collections::VecDeque;
use std::sync::Mutex;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};

pub const WM_BROWSER: u32 = WM_APP + 39;

#[derive(Clone, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub connected: bool,
    pub items: Vec<Download>,
    pub error: bool,
}

#[derive(Clone)]
struct Command {
    id: String,
    action: Action,
    request: String,
}

struct Shared {
    state: Snapshot,
    commands: VecDeque<Command>,
    port: usize,
    sequence: u64,
    thread: Option<std::thread::JoinHandle<()>>,
}

static SHARED: Mutex<Shared> = Mutex::new(Shared {
    state: Snapshot { connected: false, items: Vec::new(), error: false },
    commands: VecDeque::new(),
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
        || shared.commands.len() >= 16
        || !shared.state.items.iter().any(|item| item.id == id && item.actions.contains(&action))
    {
        return false;
    }
    shared.sequence = shared.sequence.wrapping_add(1);
    let request = format!("{}", shared.sequence);
    shared.commands.push_back(Command { id: id.into(), action, request });
    pipe::signal(shared.port, 1);
    true
}

fn update(hwnd: isize, state: Snapshot) {
    if let Ok(mut shared) = SHARED.lock() {
        if shared.state == state {
            return;
        }
        shared.state = state;
    }
    unsafe {
        let _ = PostMessageW(Some(HWND(hwnd as _)), WM_BROWSER, WPARAM(0), LPARAM(0));
    }
}
