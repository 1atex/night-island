use std::cell::Cell;

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PWSTR, w};

thread_local! {
    static TIP: Cell<HWND> = const { Cell::new(HWND(std::ptr::null_mut())) };
}

const TOOL_ID: usize = 1;

fn tool(owner: HWND, text: &mut [u16]) -> TTTOOLINFOW {
    TTTOOLINFOW {
        cbSize: std::mem::offset_of!(TTTOOLINFOW, lParam) as u32,
        uFlags: TTF_TRACK | TTF_ABSOLUTE,
        hwnd: owner,
        uId: TOOL_ID,
        lpszText: PWSTR(text.as_mut_ptr()),
        ..Default::default()
    }
}

fn window(owner: HWND) -> HWND {
    let tip = TIP.get();
    if !tip.0.is_null() {
        return tip;
    }
    unsafe {
        let init = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_WIN95_CLASSES,
        };
        let _ = InitCommonControlsEx(&init);
        let module = GetModuleHandleW(None).unwrap_or_default();
        let tip = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TRANSPARENT,
            TOOLTIPS_CLASSW,
            w!(""),
            WINDOW_STYLE(WS_POPUP.0 | TTS_NOPREFIX | TTS_ALWAYSTIP),
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            Some(owner),
            None,
            Some(module.into()),
            None,
        )
        .unwrap_or_default();
        if !tip.0.is_null() {
            let mut empty = [0u16; 1];
            let info = tool(owner, &mut empty);
            SendMessageW(tip, TTM_ADDTOOLW, Some(WPARAM(0)), Some(LPARAM(&info as *const _ as isize)));
        }
        TIP.set(tip);
        tip
    }
}

pub fn show(owner: HWND, text: &str, at: (i32, i32)) {
    let tip = window(owner);
    if tip.0.is_null() {
        return;
    }
    let mut wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let info = tool(owner, &mut wide);
    let pos = ((at.1 as u32 & 0xFFFF) << 16) | (at.0 as u32 & 0xFFFF);
    unsafe {
        SendMessageW(tip, TTM_UPDATETIPTEXTW, Some(WPARAM(0)), Some(LPARAM(&info as *const _ as isize)));
        SendMessageW(tip, TTM_TRACKPOSITION, Some(WPARAM(0)), Some(LPARAM(pos as isize)));
        SendMessageW(tip, TTM_TRACKACTIVATE, Some(WPARAM(1)), Some(LPARAM(&info as *const _ as isize)));
    }
}

pub fn hide(owner: HWND) {
    let tip = TIP.get();
    if tip.0.is_null() {
        return;
    }
    let mut empty = [0u16; 1];
    let info = tool(owner, &mut empty);
    unsafe {
        SendMessageW(tip, TTM_TRACKACTIVATE, Some(WPARAM(0)), Some(LPARAM(&info as *const _ as isize)));
    }
}
