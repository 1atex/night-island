use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetLocaleInfoW(locale: u32, kind: u32, buffer: *mut u16, length: i32) -> i32;
}

pub fn keyboard_layout() -> String {
    unsafe {
        let thread = GetWindowThreadProcessId(GetForegroundWindow(), None);
        let locale = GetKeyboardLayout(thread).0 as usize as u32 & 0xffff;
        let mut buffer = [0u16; 16];
        let len = GetLocaleInfoW(locale, 0x59, buffer.as_mut_ptr(), buffer.len() as i32);
        if len <= 1 {
            return "—".into();
        }
        String::from_utf16_lossy(&buffer[..len as usize - 1]).to_uppercase()
    }
}

pub fn next_keyboard_layout() {
    unsafe {
        let _ = PostMessageW(Some(GetForegroundWindow()), WM_INPUTLANGCHANGEREQUEST, WPARAM(0), LPARAM(1));
    }
}

pub fn task_view() {
    unsafe {
        if [VK_LWIN, VK_RWIN, VK_CONTROL, VK_MENU, VK_SHIFT].iter().any(|key| GetAsyncKeyState(key.0 as i32) < 0) {
            return;
        }
        let keys = [(VK_LWIN, false), (VK_TAB, false), (VK_TAB, true), (VK_LWIN, true)];
        let input: Vec<_> = keys
            .into_iter()
            .map(|(key, up)| INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: key,
                        dwFlags: if up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) },
                        ..Default::default()
                    },
                },
            })
            .collect();
        if SendInput(&input, size_of::<INPUT>() as i32) != input.len() as u32 {
            SendInput(&input[2..], size_of::<INPUT>() as i32);
        }
    }
}
