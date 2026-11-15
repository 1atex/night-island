use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, Ordering};

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows::Win32::UI::WindowsAndMessaging::{
    EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_NAMECHANGE, EVENT_SYSTEM_DESKTOPSWITCH, EVENT_SYSTEM_FOREGROUND,
    EVENT_SYSTEM_MINIMIZEEND, EVENT_SYSTEM_MINIMIZESTART, GetForegroundWindow, PostMessageW, WINEVENT_OUTOFCONTEXT,
    WINEVENT_SKIPOWNPROCESS, WM_APP,
};

pub const WM_FOREGROUND: u32 = WM_APP + 40;

static TARGET: AtomicIsize = AtomicIsize::new(0);
static LAST_LAYOUT_EVENT: AtomicU32 = AtomicU32::new(0);
static PENDING: AtomicBool = AtomicBool::new(false);
static HOOKS: Mutex<Vec<isize>> = Mutex::new(Vec::new());

unsafe extern "system" fn on_event(
    _hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    object: i32,
    _child: i32,
    _thread: u32,
    time: u32,
) {
    if matches!(event, EVENT_OBJECT_LOCATIONCHANGE | EVENT_OBJECT_NAMECHANGE) {
        if object != 0 || hwnd != unsafe { GetForegroundWindow() } {
            return;
        }
        let last = LAST_LAYOUT_EVENT.load(Ordering::Relaxed);
        if last != 0 && time.wrapping_sub(last) < 200 {
            return;
        }
        LAST_LAYOUT_EVENT.store(time, Ordering::Relaxed);
    }
    if PENDING.swap(true, Ordering::AcqRel) {
        return;
    }
    let raw = TARGET.load(Ordering::Relaxed);
    unsafe {
        let _ = PostMessageW(Some(HWND(raw as *mut _)), WM_FOREGROUND, WPARAM(0), LPARAM(0));
    }
}

pub fn acknowledge() {
    PENDING.store(false, Ordering::Release);
}

pub fn start(window: HWND) {
    let Ok(mut hooks) = HOOKS.lock() else { return };
    if !hooks.is_empty() {
        return;
    }
    TARGET.store(window.0 as isize, Ordering::Relaxed);
    for (from, to) in [
        (EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_LOCATIONCHANGE),
        (EVENT_OBJECT_NAMECHANGE, EVENT_OBJECT_NAMECHANGE),
        (EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND),
        (EVENT_SYSTEM_MINIMIZESTART, EVENT_SYSTEM_MINIMIZEEND),
        (EVENT_SYSTEM_DESKTOPSWITCH, EVENT_SYSTEM_DESKTOPSWITCH),
    ] {
        let flags = if matches!(from, EVENT_OBJECT_LOCATIONCHANGE | EVENT_OBJECT_NAMECHANGE) {
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS
        } else {
            WINEVENT_OUTOFCONTEXT
        };
        let hook = unsafe { SetWinEventHook(from, to, None, Some(on_event), 0, 0, flags) };
        if hook.0.is_null() {
            crate::log_warn!("could not listen to window events {from:#x}..{to:#x}");
        } else {
            hooks.push(hook.0 as isize);
        }
    }
}

pub fn stop() {
    let Ok(mut hooks) = HOOKS.lock() else { return };
    for h in hooks.drain(..) {
        unsafe {
            let _ = UnhookWinEvent(HWINEVENTHOOK(h as *mut _));
        }
    }
}
