use std::mem::size_of;

use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{HSTRING, w};

use crate::data::settings::{Settings, Style};

pub const WM_TRAY: u32 = WM_APP + 1;
pub const ID_TOGGLE: usize = 1;
pub const ID_CAPTURE: usize = 2;
pub const ID_EXIT: usize = 3;
pub const ID_AUTOHIDE: usize = 4;
pub const ID_ACRYLIC: usize = 5;
pub const ID_LIQUID: usize = 6;
pub const ID_SETTINGS: usize = 7;
pub const ID_RESET_POS: usize = 8;
pub const ID_AUTOSTART: usize = 9;
pub const ID_SIZE_BASE: usize = 1000;
pub const SIZES: [u32; 6] = [80, 100, 125, 150, 175, 200];

const ICON_ID: u32 = 1;
const TIP: &str = "Night Island";

fn data(hwnd: HWND) -> NOTIFYICONDATAW {
    let mut nid =
        NOTIFYICONDATAW { cbSize: size_of::<NOTIFYICONDATAW>() as u32, hWnd: hwnd, uID: ICON_ID, ..Default::default() };
    for (dst, src) in nid.szTip.iter_mut().zip(TIP.encode_utf16()) {
        *dst = src;
    }
    nid
}

pub fn add(hwnd: HWND) {
    unsafe {
        let mut nid = data(hwnd);
        nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
        nid.uCallbackMessage = WM_TRAY;
        nid.hIcon = LoadIconW(None, IDI_APPLICATION).unwrap_or_default();
        let _ = Shell_NotifyIconW(NIM_ADD, &nid);
    }
}

pub fn remove(hwnd: HWND) {
    unsafe {
        let _ = Shell_NotifyIconW(NIM_DELETE, &data(hwnd));
    }
}

pub enum Click {
    Primary,
    Secondary,
    None,
}

pub fn click(lparam: LPARAM) -> Click {
    match (lparam.0 & 0xFFFF) as u32 {
        WM_LBUTTONUP => Click::Primary,
        WM_RBUTTONUP => Click::Secondary,
        _ => Click::None,
    }
}

pub fn menu(hwnd: HWND, settings: &Settings) -> Option<usize> {
    unsafe {
        let menu = CreatePopupMenu().ok()?;
        let check = |on: bool| if on { MF_CHECKED } else { MF_UNCHECKED };

        let sizes = CreatePopupMenu().ok()?;
        for pct in SIZES {
            let label = HSTRING::from(format!("{pct}%"));
            let _ = AppendMenuW(
                sizes,
                MF_STRING | check(settings.scale_percent == pct),
                ID_SIZE_BASE + pct as usize,
                &label,
            );
        }

        let _ = AppendMenuW(menu, MF_STRING, ID_TOGGLE, w!("Open / close island"));
        let _ = AppendMenuW(menu, MF_STRING, ID_SETTINGS, w!("Settings…"));
        let _ = AppendMenuW(menu, MF_STRING | check(settings.auto_hide), ID_AUTOHIDE, w!("Auto-hide"));
        let _ = AppendMenuW(menu, MF_STRING, ID_RESET_POS, w!("Reset position"));
        let _ = AppendMenuW(
            menu,
            MF_STRING | check(crate::platform::autostart::enabled()),
            ID_AUTOSTART,
            w!("Start with Windows"),
        );
        let _ = AppendMenuW(menu, MF_STRING | check(settings.acrylic), ID_ACRYLIC, w!("Acrylic blur"));
        let _ = AppendMenuW(menu, MF_STRING | check(settings.style == Style::Liquid), ID_LIQUID, w!("Liquid edge"));
        let _ = AppendMenuW(menu, MF_POPUP, sizes.0 as usize, w!("Size"));
        let _ = AppendMenuW(menu, MF_STRING | check(settings.capture_hidden), ID_CAPTURE, w!("Hide from screenshots"));
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
        let _ = AppendMenuW(menu, MF_STRING, ID_EXIT, w!("Exit"));

        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        let _ = windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(
            menu,
            TPM_RIGHTBUTTON | TPM_BOTTOMALIGN | TPM_RETURNCMD | TPM_NONOTIFY,
            pt.x,
            pt.y,
            None,
            hwnd,
            None,
        );
        let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(menu);
        (cmd.0 > 0).then_some(cmd.0 as usize)
    }
}
