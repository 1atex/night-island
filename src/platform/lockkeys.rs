use std::ffi::c_void;
use std::sync::Mutex;
use std::sync::atomic::{AtomicIsize, Ordering};

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_CAPITAL, VK_NUMLOCK, VK_SCROLL};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, HC_ACTION, KBDLLHOOKSTRUCT, MSG, PostMessageW, PostThreadMessageW,
    SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_APP, WM_KEYUP, WM_QUIT, WM_SYSKEYUP,
};

pub const WM_LOCKKEY: u32 = WM_APP + 20;

static MAIN: AtomicIsize = AtomicIsize::new(0);
static THREAD: Mutex<Option<u32>> = Mutex::new(None);

pub fn describe(vk: usize, on: bool) -> Option<&'static str> {
    Some(match (vk as u16, on) {
        (v, true) if v == VK_CAPITAL.0 => "Caps Lock on",
        (v, false) if v == VK_CAPITAL.0 => "Caps Lock off",
        (v, true) if v == VK_NUMLOCK.0 => "Num Lock on",
        (v, false) if v == VK_NUMLOCK.0 => "Num Lock off",
        (v, true) if v == VK_SCROLL.0 => "Scroll Lock on",
        (v, false) if v == VK_SCROLL.0 => "Scroll Lock off",
        _ => return None,
    })
}

pub fn state(vk: u16) -> bool {
    std::thread::spawn(move || unsafe { GetKeyState(i32::from(vk)) } & 1 != 0).join().unwrap_or(false)
}

extern "system" fn hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 && (wparam.0 as u32 == WM_KEYUP || wparam.0 as u32 == WM_SYSKEYUP) {
        let vk = unsafe { (*(lparam.0 as *const KBDLLHOOKSTRUCT)).vkCode } as u16;
        if vk == VK_CAPITAL.0 || vk == VK_NUMLOCK.0 || vk == VK_SCROLL.0 {
            let main = MAIN.load(Ordering::Relaxed);
            if main != 0 {
                unsafe {
                    let _ = PostMessageW(Some(HWND(main as *mut c_void)), WM_LOCKKEY, WPARAM(vk as usize), LPARAM(0));
                }
            }
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

pub fn start(main: HWND) {
    let Ok(mut thread) = THREAD.lock() else { return };
    if thread.is_some() {
        return;
    }
    MAIN.store(main.0 as isize, Ordering::Relaxed);
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || unsafe {
        let Ok(handle) = SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook), None, 0) else {
            crate::log_warn!("could not install the lock-key hook");
            let _ = tx.send(None);
            return;
        };
        let _ = tx.send(Some(GetCurrentThreadId()));
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        let _ = UnhookWindowsHookEx(handle);
    });
    *thread = rx.recv().ok().flatten();
}

pub fn stop() {
    let Ok(mut thread) = THREAD.lock() else { return };
    if let Some(id) = thread.take() {
        unsafe {
            let _ = PostThreadMessageW(id, WM_QUIT, WPARAM(0), LPARAM(0));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_lock_keys_have_a_message() {
        assert_eq!(describe(VK_CAPITAL.0 as usize, true), Some("Caps Lock on"));
        assert_eq!(describe(VK_NUMLOCK.0 as usize, false), Some("Num Lock off"));
        assert_eq!(describe(VK_SCROLL.0 as usize, true), Some("Scroll Lock on"));
        assert_eq!(describe(0x41, true), None);
    }
}
