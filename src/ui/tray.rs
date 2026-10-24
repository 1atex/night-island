use std::mem::size_of;

use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{HSTRING, PCWSTR};

use crate::ui::i18n::tr;

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

pub fn menu(hwnd: HWND) -> Option<usize> {
    unsafe {
        let menu = CreatePopupMenu().ok()?;
        let item = |id: usize, text: &str| {
            let _ = AppendMenuW(menu, MF_STRING, id, &HSTRING::from(tr(text).as_ref()));
        };
        item(ID_TOGGLE, "Open / close island");
        item(ID_SETTINGS, "Settings");
        item(ID_RESET_POS, "Reset position");
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
        item(ID_EXIT, "Exit");

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
