use std::mem::size_of;

use windows::Win32::Foundation::{HWND, LPARAM, POINT};
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::PCWSTR;

use crate::ui::menu::Entry;

pub const WM_TRAY: u32 = WM_APP + 1;
pub const ID_TOGGLE: usize = 1;
pub const ID_SETTINGS: usize = 2;
pub const ID_RESET_POS: usize = 3;
pub const ID_EXIT: usize = 4;

const ICON_ID: u32 = 1;
const APP_ICON: usize = 1;
const TIP: &str = "Night Island";

fn data(hwnd: HWND) -> NOTIFYICONDATAW {
    let mut nid =
        NOTIFYICONDATAW { cbSize: size_of::<NOTIFYICONDATAW>() as u32, hWnd: hwnd, uID: ICON_ID, ..Default::default() };
    for (dst, src) in nid.szTip.iter_mut().zip(TIP.encode_utf16()) {
        *dst = src;
    }
    nid
}

fn app_icon() -> HICON {
    unsafe {
        let module = windows::Win32::System::LibraryLoader::GetModuleHandleW(None).unwrap_or_default();
        LoadImageW(
            Some(module.into()),
            PCWSTR(APP_ICON as *const u16),
            IMAGE_ICON,
            GetSystemMetrics(SM_CXSMICON),
            GetSystemMetrics(SM_CYSMICON),
            LR_DEFAULTCOLOR,
        )
        .map(|h| HICON(h.0))
        .or_else(|_| LoadIconW(None, IDI_APPLICATION))
        .unwrap_or_default()
    }
}

pub fn add(hwnd: HWND) {
    unsafe {
        let mut nid = data(hwnd);
        nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
        nid.uCallbackMessage = WM_TRAY;
        nid.hIcon = app_icon();
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

pub fn menu(scale: f32, with_gpu: crate::ui::menu::WithGpu) -> Option<usize> {
    let entries = vec![
        Entry::item(ID_TOGGLE, "Open / close island").glyph('\u{E8A7}'),
        Entry::item(ID_SETTINGS, "Settings").glyph('\u{E713}'),
        Entry::item(ID_RESET_POS, "Reset position").glyph('\u{E72C}'),
        Entry::divider(),
        Entry::item(ID_EXIT, "Exit").glyph('\u{E7E8}').danger(),
    ];
    let mut pt = POINT::default();
    unsafe {
        let _ = GetCursorPos(&mut pt);
    }
    crate::ui::menu::show(entries, (pt.x, pt.y), true, scale, with_gpu)
}
