use crate::island::*;
use windows::Win32::UI::Input::KeyboardAndMouse::HOT_KEY_MODIFIERS;

pub(in crate::island) const GAME_POLL_MS: u32 = 2000;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(in crate::island) enum Hotkey {
    Toggle = 1,
    Clipboard = 2,
    PlainPaste = 3,
    Command = 4,
    Mic = 5,
    ShotArea = 6,
    ShotFull = 7,
    Picker = 8,
    Ocr = 9,
    Qr = 10,
    Rec = 11,
}

const BINDINGS: [(Hotkey, HOT_KEY_MODIFIERS, u32); 11] = [
    (Hotkey::Toggle, CTRL_ALT, 'I' as u32),
    (Hotkey::Clipboard, CTRL_ALT, 'V' as u32),
    (Hotkey::PlainPaste, CTRL_ALT_SHIFT, 'V' as u32),
    (Hotkey::Command, MOD_ALT, VK_SPACE.0 as u32),
    (Hotkey::Mic, CTRL_ALT, 'M' as u32),
    (Hotkey::ShotArea, CTRL_ALT, 'S' as u32),
    (Hotkey::ShotFull, CTRL_ALT_SHIFT, 'S' as u32),
    (Hotkey::Picker, CTRL_ALT, 'C' as u32),
    (Hotkey::Ocr, CTRL_ALT, 'O' as u32),
    (Hotkey::Qr, CTRL_ALT, 'Q' as u32),
    (Hotkey::Rec, CTRL_ALT, 'R' as u32),
];
const CTRL_ALT: HOT_KEY_MODIFIERS = HOT_KEY_MODIFIERS(MOD_CONTROL.0 | MOD_ALT.0);
const CTRL_ALT_SHIFT: HOT_KEY_MODIFIERS = HOT_KEY_MODIFIERS(MOD_CONTROL.0 | MOD_ALT.0 | MOD_SHIFT.0);

impl Hotkey {
    pub(in crate::island) fn from_id(id: i32) -> Option<Hotkey> {
        BINDINGS.iter().map(|b| b.0).find(|h| *h as i32 == id)
    }
}

pub(in crate::island) fn register_hotkeys(hwnd: HWND) {
    for (key, mods, vk) in BINDINGS {
        unsafe {
            let _ = RegisterHotKey(Some(hwnd), key as i32, mods | MOD_NOREPEAT, vk);
        }
    }
}

pub(in crate::island) fn unregister_hotkeys(hwnd: HWND) {
    for (key, _, _) in BINDINGS {
        unsafe {
            let _ = UnregisterHotKey(Some(hwnd), key as i32);
        }
    }
}

pub(in crate::island) fn fullscreen_app_in_front(own: HWND) -> bool {
    use windows::Win32::UI::Shell::{
        QUNS_BUSY, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN, SHQueryUserNotificationState,
    };
    unsafe {
        if SHQueryUserNotificationState()
            .is_ok_and(|s| s == QUNS_RUNNING_D3D_FULL_SCREEN || s == QUNS_BUSY || s == QUNS_PRESENTATION_MODE)
        {
            return true;
        }
        let fg = GetForegroundWindow();
        if fg.is_invalid() || fg == own || fg == GetShellWindow() || fg == GetDesktopWindow() {
            return false;
        }
        let mut class = [0u16; 64];
        let n = GetClassNameW(fg, &mut class) as usize;
        let class = String::from_utf16_lossy(&class[..n]);
        if matches!(class.as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd") {
            return false;
        }
        let style = GetWindowLongW(fg, GWL_STYLE) as u32;
        if style & WS_CAPTION.0 == WS_CAPTION.0 {
            return false;
        }
        let mut rc = windows::Win32::Foundation::RECT::default();
        if GetWindowRect(fg, &mut rc).is_err() {
            return false;
        }
        let monitor = windows::Win32::Graphics::Gdi::MonitorFromWindow(
            fg,
            windows::Win32::Graphics::Gdi::MONITOR_DEFAULTTONEAREST,
        );
        let mut info = windows::Win32::Graphics::Gdi::MONITORINFO {
            cbSize: size_of::<windows::Win32::Graphics::Gdi::MONITORINFO>() as u32,
            ..Default::default()
        };
        if !windows::Win32::Graphics::Gdi::GetMonitorInfoW(monitor, &mut info).as_bool() {
            return false;
        }
        let m = info.rcMonitor;
        rc.left <= m.left && rc.top <= m.top && rc.right >= m.right && rc.bottom >= m.bottom
    }
}

impl App {
    pub(in crate::island) fn start_game_watch(&mut self) {
        if self.settings.game_mode {
            self.set_timer(Timer::Game, GAME_POLL_MS);
        } else {
            self.kill_timer(Timer::Game);
            self.set_gaming(false);
        }
    }

    pub(in crate::island) fn on_game_timer(&mut self) {
        if !self.settings.game_mode {
            return;
        }
        let gaming = self.state != State::Expanded && fullscreen_app_in_front(self.hwnd);
        self.set_gaming(gaming);
        self.set_timer(Timer::Game, GAME_POLL_MS);
    }

    pub(in crate::island) fn set_gaming(&mut self, on: bool) {
        if on == self.gaming {
            return;
        }
        self.gaming = on;
        if on {
            unregister_hotkeys(self.hwnd);
            self.kill_timer(Timer::Expand);
            self.kill_timer(Timer::Liquid);
            self.stop_idle();
            self.set_bump_visible(false);
            self.update_eq();
            unsafe {
                let _ = ShowWindow(self.hwnd, SW_HIDE);
            }
        } else {
            unsafe {
                let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
            }
            register_hotkeys(self.hwnd);
            self.update_eq();
            self.start_liquid();
            self.repaint_collapsed();
        }
    }
}
