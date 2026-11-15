use crate::platform::desktop_dock::{self as desktop, Window};
use std::cell::RefCell;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
    mpsc,
};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

pub(super) const MESSAGE: u32 = WM_APP + 118;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub(super) enum Command {
    Next,
    Previous,
    Accept,
    Cancel,
}

impl Command {
    pub fn decode(value: usize) -> Option<Self> {
        [Self::Next, Self::Previous, Self::Accept, Self::Cancel].get(value).copied()
    }
}

#[derive(Default)]
struct Chord {
    held: bool,
    tab: bool,
    escape: bool,
}

impl Chord {
    fn input(
        &mut self,
        key: VIRTUAL_KEY,
        up: bool,
        alt: bool,
        reverse: bool,
        other_mod: bool,
        enabled: bool,
    ) -> (bool, Option<Command>) {
        if up && key == VK_TAB {
            return (std::mem::take(&mut self.tab), None);
        }
        if up && key == VK_ESCAPE {
            return (std::mem::take(&mut self.escape), None);
        }
        if up && matches!(key, VK_LMENU | VK_RMENU | VK_MENU) && std::mem::take(&mut self.held) {
            return (false, Some(if enabled { Command::Accept } else { Command::Cancel }));
        }
        if !up && key == VK_ESCAPE && self.held {
            self.held = false;
            self.escape = true;
            return (true, Some(Command::Cancel));
        }
        if !up && key == VK_TAB && other_mod && std::mem::take(&mut self.held) {
            self.tab = false;
            return (false, Some(Command::Cancel));
        }
        if !up && key == VK_TAB && alt && !other_mod && enabled {
            self.held = true;
            self.tab = true;
            return (true, Some(if reverse { Command::Previous } else { Command::Next }));
        }
        (false, None)
    }
}

#[derive(Clone)]
pub(super) struct Cycle {
    pub windows: Arc<[Window]>,
    index: Arc<AtomicUsize>,
}

impl Cycle {
    pub fn selected(&self) -> Option<usize> {
        let index = self.index.load(Ordering::Acquire);
        (index < self.windows.len()).then_some(index)
    }

    pub fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.index, &other.index)
    }

    pub fn select(&self, index: usize) {
        if index < self.windows.len() {
            let _ = self
                .index
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |old| (old != usize::MAX).then_some(index));
        }
    }

    pub fn finish(&self, choice: Option<usize>) -> Option<&Window> {
        let index = self.index.swap(usize::MAX, Ordering::AcqRel);
        if index == usize::MAX {
            return None;
        }
        self.windows.get(choice.unwrap_or(index))
    }
}

#[derive(Default)]
struct Sessions {
    windows: Vec<Window>,
    current: Option<Cycle>,
}

impl Sessions {
    fn step(&mut self, reverse: bool) {
        let delta = if reverse { -1 } else { 1 };
        if let Some(cycle) = &self.current {
            let _ = cycle.index.fetch_update(Ordering::AcqRel, Ordering::Acquire, |i| {
                (i != usize::MAX).then(|| (i as isize + delta).rem_euclid(cycle.windows.len() as isize) as usize)
            });
            return;
        }
        let front = unsafe { GetForegroundWindow() };
        let mut windows: Vec<_> = self
            .windows
            .iter()
            .filter(|w| unsafe { IsWindowVisible(HWND(w.handle as _)).as_bool() })
            .cloned()
            .collect();
        let owner = unsafe { GetAncestor(front, GA_ROOTOWNER) };
        let recent = RECENT.with_borrow(Clone::clone);
        for handle in [front.0 as isize, owner.0 as isize].into_iter().chain(recent.iter().take(4).copied()) {
            if handle != 0
                && !windows.iter().any(|w| w.handle == handle)
                && let Some(window) = desktop::uncached_window(HWND(handle as _), front)
            {
                windows.push(window);
            }
        }
        if windows.is_empty() {
            return;
        }
        let current = [front, owner].into_iter().map(|h| h.0 as isize).find(|h| windows.iter().any(|w| w.handle == *h));
        windows.sort_by_key(|w| {
            (Some(w.handle) != current, recent.iter().position(|h| *h == w.handle).unwrap_or(usize::MAX))
        });
        let selected = if reverse { windows.len() - 1 } else { usize::from(current.is_some()) % windows.len() };
        self.current = Some(Cycle { windows: windows.into(), index: Arc::new(AtomicUsize::new(selected)) });
    }
}

struct Route {
    enabled: Arc<AtomicBool>,
    chord: Chord,
}
thread_local! { static ROUTE: RefCell<Option<Route>> = const { RefCell::new(None) }; }

thread_local! { static RECENT: RefCell<Vec<isize>> = const { RefCell::new(Vec::new()) }; }

unsafe extern "system" fn foreground_changed(_: HWINEVENTHOOK, _: u32, hwnd: HWND, _: i32, _: i32, _: u32, _: u32) {
    if hwnd.is_invalid() {
        return;
    }
    let owner = unsafe { GetAncestor(hwnd, GA_ROOTOWNER) };
    RECENT.with_borrow_mut(|recent| {
        for handle in [owner, hwnd] {
            let handle = handle.0 as isize;
            if handle != 0 && recent.first() != Some(&handle) {
                recent.retain(|h| *h != handle);
                recent.truncate(255);
                recent.insert(0, handle);
            }
        }
    });
}

fn mask_alt_menu() -> bool {
    let event = |flags| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: VIRTUAL_KEY(0xe8), dwFlags: flags, ..Default::default() } },
    };
    let pair = [event(KEYBD_EVENT_FLAGS(0)), event(KEYEVENTF_KEYUP)];
    unsafe { SendInput(&pair, size_of::<INPUT>() as i32) == 2 }
}

extern "system" fn shortcut(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
    unsafe {
        if code == HC_ACTION as i32 {
            let event = &*(l.0 as *const KBDLLHOOKSTRUCT);
            let key = VIRTUAL_KEY(event.vkCode as u16);
            if matches!(key, VK_TAB | VK_MENU | VK_LMENU | VK_RMENU | VK_ESCAPE) {
                let consumed = ROUTE.with_borrow_mut(|route| {
                    let Some(route) = route else { return false };
                    let down = |k: VIRTUAL_KEY| GetAsyncKeyState(i32::from(k.0)) < 0;
                    let already_held = route.chord.held;
                    let (eat, command) = route.chord.input(
                        key,
                        matches!(w.0 as u32, WM_KEYUP | WM_SYSKEYUP),
                        event.flags.contains(LLKHF_ALTDOWN),
                        key == VK_TAB && down(VK_SHIFT),
                        key == VK_TAB && (down(VK_CONTROL) || down(VK_LWIN) || down(VK_RWIN)),
                        route.enabled.load(Ordering::Acquire),
                    );
                    if !already_held && matches!(command, Some(Command::Next | Command::Previous)) && !mask_alt_menu() {
                        route.chord = Chord::default();
                        return false;
                    }
                    if let Some(command) = command
                        && PostThreadMessageW(GetCurrentThreadId(), MESSAGE, WPARAM(command as usize), LPARAM(0))
                            .is_err()
                    {
                        route.chord = Chord::default();
                        return false;
                    }
                    eat
                });
                if consumed {
                    return LRESULT(1);
                }
            }
        }
        CallNextHookEx(None, code, w, l)
    }
}

pub(super) struct Shortcut {
    thread: u32,
    sessions: Arc<Mutex<Sessions>>,
    enabled: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl Shortcut {
    pub fn install(dock: HWND) -> Option<Self> {
        let dock = dock.0 as isize;
        let enabled = Arc::new(AtomicBool::new(false));
        let flag = enabled.clone();
        let sessions = Arc::new(Mutex::new(Sessions::default()));
        let worker_sessions = sessions.clone();
        let (tx, rx) = mpsc::sync_channel(1);
        let worker = std::thread::Builder::new()
            .name("Island Alt+Tab".into())
            .spawn(move || unsafe {
                let mut msg = MSG::default();
                let _ = PeekMessageW(&mut msg, None, 0, 0, PM_NOREMOVE);
                let Ok(hook) = SetWindowsHookExW(WH_KEYBOARD_LL, Some(shortcut), None, 0) else { return };
                let foreground_hook = SetWinEventHook(
                    EVENT_SYSTEM_FOREGROUND,
                    EVENT_SYSTEM_FOREGROUND,
                    None,
                    Some(foreground_changed),
                    0,
                    0,
                    WINEVENT_OUTOFCONTEXT,
                );
                if foreground_hook.is_invalid() {
                    let _ = UnhookWindowsHookEx(hook);
                    return;
                }
                ROUTE.replace(Some(Route { enabled: flag.clone(), chord: Chord::default() }));
                if tx.send(GetCurrentThreadId()).is_ok() {
                    while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
                        if msg.message != MESSAGE {
                            DispatchMessageW(&msg);
                            continue;
                        }
                        let Some(cmd) = Command::decode(msg.wParam.0) else { continue };
                        let command = if flag.load(Ordering::Acquire) { cmd } else { Command::Cancel };
                        let cycle = {
                            let mut sessions = worker_sessions.lock().unwrap();
                            match command {
                                Command::Next | Command::Previous => {
                                    sessions.step(command == Command::Previous);
                                    None
                                }
                                Command::Accept | Command::Cancel => sessions.current.take(),
                            }
                        };
                        if let Some(cycle) = cycle
                            && let Some(window) = cycle.finish(None)
                            && command == Command::Accept
                        {
                            desktop::switch_window(window);
                        }
                        let _ = PostMessageW(Some(HWND(dock as _)), MESSAGE, WPARAM(0), LPARAM(0));
                    }
                }
                let _ = UnhookWinEvent(foreground_hook);
                let _ = UnhookWindowsHookEx(hook);
                ROUTE.replace(None);
            })
            .ok()?;
        Some(Self { thread: rx.recv().ok()?, sessions, enabled, worker: Some(worker) })
    }
    pub fn snapshot(&self) -> Option<Cycle> {
        self.sessions.lock().unwrap().current.clone().filter(|c| c.selected().is_some())
    }

    pub fn update(&self, windows: &[Window]) {
        let mut sessions = self.sessions.lock().unwrap();
        if sessions.windows == windows {
            return;
        }
        sessions.windows = windows.to_vec();
    }

    pub fn enable(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Release);
        if !enabled && let Some(cycle) = self.sessions.lock().unwrap().current.take() {
            cycle.finish(None);
        }
    }
}
impl Drop for Shortcut {
    fn drop(&mut self) {
        self.enable(false);
        unsafe {
            if PostThreadMessageW(self.thread, WM_QUIT, WPARAM(0), LPARAM(0)).is_ok()
                && let Some(worker) = self.worker.take()
            {
                let _ = worker.join();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_enabled_alt_tab_is_claimed_and_release_commits_once() {
        let mut chord = Chord::default();
        for (alt, other, enabled) in [(false, false, true), (true, true, true), (true, false, false)] {
            assert_eq!(chord.input(VK_TAB, false, alt, false, other, enabled), (false, None));
        }
        assert_eq!(chord.input(VK_TAB, false, true, false, false, true), (true, Some(Command::Next)));
        assert_eq!(chord.input(VK_TAB, false, true, true, false, true), (true, Some(Command::Previous)));
        assert_eq!(chord.input(VK_LMENU, true, false, false, false, true), (false, Some(Command::Accept)));
        assert_eq!(chord.input(VK_TAB, true, false, false, false, true), (true, None));
        assert_eq!(chord.input(VK_RMENU, true, false, false, false, true), (false, None));
        chord.input(VK_TAB, false, true, false, false, true);
        assert_eq!(chord.input(VK_ESCAPE, false, true, false, false, true), (true, Some(Command::Cancel)));
        assert_eq!(chord.input(VK_LMENU, true, false, false, false, true), (false, None));
    }
}
