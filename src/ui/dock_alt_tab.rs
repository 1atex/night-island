use std::cell::RefCell;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
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

struct Route {
    dock: HWND,
    enabled: Arc<AtomicBool>,
    chord: Chord,
}
thread_local! { static ROUTE: RefCell<Option<Route>> = const { RefCell::new(None) }; }

extern "system" fn shortcut(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
    unsafe {
        if code == HC_ACTION as i32 {
            let event = &*(l.0 as *const KBDLLHOOKSTRUCT);
            let key = VIRTUAL_KEY(event.vkCode as u16);
            if matches!(key, VK_TAB | VK_MENU | VK_LMENU | VK_RMENU | VK_ESCAPE) {
                let consumed = ROUTE.with_borrow_mut(|route| {
                    let Some(route) = route else { return false };
                    let down = |k: VIRTUAL_KEY| GetAsyncKeyState(i32::from(k.0)) < 0;
                    let (eat, command) = route.chord.input(
                        key,
                        matches!(w.0 as u32, WM_KEYUP | WM_SYSKEYUP),
                        event.flags.contains(LLKHF_ALTDOWN),
                        key == VK_TAB && down(VK_SHIFT),
                        key == VK_TAB && (down(VK_CONTROL) || down(VK_LWIN) || down(VK_RWIN)),
                        route.enabled.load(Ordering::Acquire),
                    );
                    if let Some(command) = command
                        && PostMessageW(Some(route.dock), MESSAGE, WPARAM(command as usize), LPARAM(0)).is_err()
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
    enabled: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl Shortcut {
    pub fn install(dock: HWND) -> Option<Self> {
        let dock = dock.0 as isize;
        let enabled = Arc::new(AtomicBool::new(false));
        let flag = enabled.clone();
        let (tx, rx) = mpsc::sync_channel(1);
        let worker = std::thread::Builder::new()
            .name("Island Alt+Tab".into())
            .spawn(move || unsafe {
                let mut msg = MSG::default();
                let _ = PeekMessageW(&mut msg, None, 0, 0, PM_NOREMOVE);
                let Ok(hook) = SetWindowsHookExW(WH_KEYBOARD_LL, Some(shortcut), None, 0) else { return };
                ROUTE.replace(Some(Route { dock: HWND(dock as _), enabled: flag, chord: Chord::default() }));
                if tx.send(GetCurrentThreadId()).is_ok() {
                    while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
                        DispatchMessageW(&msg);
                    }
                }
                let _ = UnhookWindowsHookEx(hook);
                ROUTE.replace(None);
            })
            .ok()?;
        Some(Self { thread: rx.recv().ok()?, enabled, worker: Some(worker) })
    }
    pub fn enable(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Release);
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
