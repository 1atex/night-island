use std::sync::Once;

use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{DWMWINDOWATTRIBUTE, DwmSetWindowAttribute};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::{EM_SCROLLCARET, EM_SETLIMITTEXT, EM_SETSEL};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, SetFocus, VK_RETURN, VK_SHIFT};
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{HSTRING, Result, w};

pub const WM_MINI_CHAT: u32 = WM_APP + 46;
pub const SEND: usize = 1;
pub const SETUP: usize = 3;

struct State {
    parent: HWND,
    history: HWND,
    input: HWND,
    send: HWND,
    setup: HWND,
    status: HWND,
    scale: f32,
    brush: HBRUSH,
    font: HFONT,
}

fn tr(text: &str) -> HSTRING {
    HSTRING::from(crate::ui::i18n::tr(text).as_ref())
}

fn notify(hwnd: HWND, state: &State, action: usize) {
    unsafe {
        let _ = PostMessageW(Some(state.parent), WM_MINI_CHAT, WPARAM(action), LPARAM(hwnd.0 as isize));
    }
}

unsafe extern "system" fn input_proc(
    hwnd: HWND,
    msg: u32,
    wp: WPARAM,
    lp: LPARAM,
    _id: usize,
    parent: usize,
) -> LRESULT {
    unsafe {
        if msg == WM_KEYDOWN && wp.0 == VK_RETURN.0 as usize && GetKeyState(VK_SHIFT.0 as i32) >= 0 {
            let _ = PostMessageW(Some(HWND(parent as *mut _)), WM_COMMAND, WPARAM(SEND), LPARAM(0));
            return LRESULT(0);
        }
        if msg == WM_CHAR && wp.0 == 13 && GetKeyState(VK_SHIFT.0 as i32) >= 0 {
            return LRESULT(0);
        }
        if msg == WM_KEYDOWN && wp.0 == 27 {
            let _ = PostMessageW(Some(HWND(parent as *mut _)), WM_CLOSE, WPARAM(0), LPARAM(0));
            return LRESULT(0);
        }
        DefSubclassProc(hwnd, msg, wp, lp)
    }
}

fn layout(hwnd: HWND, state: &State) {
    unsafe {
        let mut r = RECT::default();
        let _ = GetClientRect(hwnd, &mut r);
        let s = state.scale;
        let pad = (14.0 * s) as i32;
        let width = r.right - pad * 2;
        let footer = r.bottom - (48.0 * s) as i32;
        let input_top = footer - (76.0 * s) as i32;
        let status_top = input_top - (30.0 * s) as i32;
        for (control, x, y, w, h) in [
            (state.history, pad, pad, width, status_top - pad - (8.0 * s) as i32),
            (state.status, pad, status_top, width, (24.0 * s) as i32),
            (state.input, pad, input_top, width, (66.0 * s) as i32),
            (state.setup, pad, footer, (130.0 * s) as i32, (32.0 * s) as i32),
            (state.send, r.right - pad - (108.0 * s) as i32, footer, (108.0 * s) as i32, (32.0 * s) as i32),
        ] {
            let _ = MoveWindow(control, x, y, w.max(1), h.max(1), true);
        }
    }
}

extern "system" fn proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
        if ptr.is_null() {
            return DefWindowProcW(hwnd, msg, wp, lp);
        }
        let state = &*ptr;
        match msg {
            WM_COMMAND => {
                let id = wp.0 & 0xffff;
                if id == SEND || id == SETUP {
                    notify(hwnd, state, id);
                } else if id == 2 {
                    let _ = ShowWindow(hwnd, SW_HIDE);
                }
                LRESULT(0)
            }
            WM_CLOSE => {
                let _ = ShowWindow(hwnd, SW_HIDE);
                LRESULT(0)
            }
            WM_SIZE => {
                layout(hwnd, state);
                LRESULT(0)
            }
            WM_GETMINMAXINFO => {
                let info = &mut *(lp.0 as *mut MINMAXINFO);
                info.ptMinTrackSize = POINT { x: (340.0 * state.scale) as i32, y: (330.0 * state.scale) as i32 };
                LRESULT(0)
            }
            WM_SETFOCUS => {
                let _ = SetFocus(Some(state.input));
                LRESULT(0)
            }
            WM_ERASEBKGND => {
                let mut r = RECT::default();
                let _ = GetClientRect(hwnd, &mut r);
                FillRect(HDC(wp.0 as *mut _), &r, state.brush);
                LRESULT(1)
            }
            WM_CTLCOLOREDIT | WM_CTLCOLORSTATIC => {
                let dc = HDC(wp.0 as *mut _);
                SetTextColor(dc, COLORREF(0x00eeeeee));
                SetBkColor(dc, COLORREF(0x001e1917));
                LRESULT(state.brush.0 as isize)
            }
            _ => DefWindowProcW(hwnd, msg, wp, lp),
        }
    }
}

pub struct MiniChatWindow {
    hwnd: HWND,
    state: Box<State>,
}

impl MiniChatWindow {
    pub fn new(parent: HWND, scale: f32) -> Result<Self> {
        static CLASS: Once = Once::new();
        unsafe {
            let instance = GetModuleHandleW(None)?;
            CLASS.call_once(|| {
                RegisterClassW(&WNDCLASSW {
                    lpfnWndProc: Some(proc),
                    hInstance: instance.into(),
                    hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                    lpszClassName: w!("NightIsland.MiniChat"),
                    ..Default::default()
                });
            });
            let hwnd = CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
                w!("NightIsland.MiniChat"),
                w!("AI"),
                WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_THICKFRAME | WS_CLIPCHILDREN,
                0,
                0,
                (392.0 * scale) as i32,
                (440.0 * scale) as i32,
                None,
                None,
                Some(instance.into()),
                None,
            )?;
            let controls = (|| -> Result<_> {
                let child = |class, text, style: WINDOW_STYLE, id| {
                    CreateWindowExW(
                        WINDOW_EX_STYLE(0),
                        class,
                        text,
                        WS_CHILD | WS_VISIBLE | style,
                        0,
                        0,
                        1,
                        1,
                        Some(hwnd),
                        Some(HMENU(id as *mut _)),
                        Some(instance.into()),
                        None,
                    )
                };
                Ok((
                    child(
                        w!("EDIT"),
                        w!(""),
                        WS_VSCROLL | WINDOW_STYLE((ES_MULTILINE | ES_READONLY | ES_AUTOVSCROLL) as u32),
                        10,
                    )?,
                    child(
                        w!("EDIT"),
                        w!(""),
                        WS_TABSTOP
                            | WS_BORDER
                            | WS_VSCROLL
                            | WINDOW_STYLE((ES_MULTILINE | ES_AUTOVSCROLL | ES_WANTRETURN) as u32),
                        11,
                    )?,
                    child(
                        w!("BUTTON"),
                        windows::core::PCWSTR(tr("Send").as_ptr()),
                        WS_TABSTOP | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32),
                        SEND,
                    )?,
                    child(w!("BUTTON"), windows::core::PCWSTR(tr("AI settings").as_ptr()), WS_TABSTOP, SETUP)?,
                    child(
                        w!("STATIC"),
                        windows::core::PCWSTR(tr("Enter to send · Shift+Enter for a new line").as_ptr()),
                        WINDOW_STYLE(0),
                        12,
                    )?,
                ))
            })();
            let (history, input, send, setup, status) = match controls {
                Ok(value) => value,
                Err(error) => {
                    let _ = DestroyWindow(hwnd);
                    return Err(error);
                }
            };
            let mut lf = LOGFONTW { lfHeight: -(14.0 * scale).round() as i32, ..Default::default() };
            for (dst, ch) in lf.lfFaceName.iter_mut().zip("Segoe UI".encode_utf16()) {
                *dst = ch;
            }
            let font = CreateFontIndirectW(&lf);
            let mut state = Box::new(State {
                parent,
                history,
                input,
                send,
                setup,
                status,
                scale,
                brush: CreateSolidBrush(COLORREF(0x001e1917)),
                font,
            });
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, (&raw mut *state) as isize);
            for control in [history, input, send, setup, status] {
                SendMessageW(control, WM_SETFONT, Some(WPARAM(font.0 as usize)), Some(LPARAM(1)));
            }
            SendMessageW(input, EM_SETLIMITTEXT, Some(WPARAM(32_000)), Some(LPARAM(0)));
            SendMessageW(history, EM_SETLIMITTEXT, Some(WPARAM(4_000_000)), Some(LPARAM(0)));
            let _ = SetWindowSubclass(input, Some(input_proc), 1, hwnd.0 as usize);
            let dark = 1i32;
            let _ = DwmSetWindowAttribute(hwnd, DWMWINDOWATTRIBUTE(20), (&raw const dark).cast(), 4);
            layout(hwnd, &state);
            Ok(Self { hwnd, state })
        }
    }

    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    pub fn show(&self, anchor: (f32, f32), character: &str) {
        unsafe {
            let _ = SetWindowTextW(self.hwnd, &HSTRING::from(format!("{character} · AI")));
            if !IsWindowVisible(self.hwnd).as_bool() {
                let mut r = RECT::default();
                let _ = GetWindowRect(self.hwnd, &mut r);
                let (w, h) = (r.right - r.left, r.bottom - r.top);
                let monitor =
                    MonitorFromPoint(POINT { x: anchor.0 as i32, y: anchor.1 as i32 }, MONITOR_DEFAULTTONEAREST);
                let mut info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
                if GetMonitorInfoW(monitor, &mut info).as_bool() {
                    let work = info.rcWork;
                    let gap = (48.0 * self.state.scale) as i32;
                    let x = if anchor.0 as i32 + gap + w <= work.right {
                        anchor.0 as i32 + gap
                    } else {
                        anchor.0 as i32 - gap - w
                    };
                    let x = x.clamp(work.left, (work.right - w).max(work.left));
                    let y = (anchor.1 as i32 - h / 2).clamp(work.top, (work.bottom - h).max(work.top));
                    let _ = SetWindowPos(self.hwnd, None, x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER);
                }
            }
            let _ = ShowWindow(self.hwnd, SW_SHOW);
            let _ = SetForegroundWindow(self.hwnd);
            let _ = SetFocus(Some(self.state.input));
        }
    }

    pub fn input(&self) -> String {
        unsafe {
            let len = GetWindowTextLengthW(self.state.input).max(0) as usize;
            let mut buffer = vec![0u16; len + 1];
            let count = GetWindowTextW(self.state.input, &mut buffer).max(0) as usize;
            String::from_utf16_lossy(&buffer[..count])
        }
    }

    pub fn clear_input(&self) {
        self.set_input("");
    }

    pub fn set_input(&self, text: &str) {
        unsafe {
            let _ = SetWindowTextW(self.state.input, &HSTRING::from(text));
        }
    }

    pub fn update(&self, transcript: &str, status: &str, pending: bool) {
        unsafe {
            let _ = SetWindowTextW(self.state.history, &HSTRING::from(transcript.replace('\n', "\r\n")));
            SendMessageW(self.state.history, EM_SETSEL, Some(WPARAM(usize::MAX)), Some(LPARAM(-1)));
            SendMessageW(self.state.history, EM_SCROLLCARET, Some(WPARAM(0)), Some(LPARAM(0)));
            let _ = SetWindowTextW(self.state.status, &tr(status));
            let _ = windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow(self.state.send, !pending);
        }
    }

    pub fn set_capture_hidden(&self, hidden: bool) {
        unsafe {
            let _ = SetWindowDisplayAffinity(self.hwnd, if hidden { WDA_EXCLUDEFROMCAPTURE } else { WDA_NONE });
        }
    }
}

impl Drop for MiniChatWindow {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.hwnd);
            let _ = DeleteObject(self.state.brush.into());
            let _ = DeleteObject(self.state.font.into());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "Requires an interactive Windows desktop"]
    fn mini_chat_preserves_drafts_and_works_with_a_hidden_island() -> Result<()> {
        unsafe {
            let parent = CreateWindowExW(
                WS_EX_TOOLWINDOW,
                w!("STATIC"),
                w!("Mini chat test"),
                WS_POPUP,
                0,
                0,
                100,
                40,
                None,
                None,
                Some(GetModuleHandleW(None)?.into()),
                None,
            )?;
            let chat = MiniChatWindow::new(parent, 1.0)?;
            assert!(GetWindow(chat.hwnd(), GW_OWNER).is_err());
            let draft = "Привет, Spark! 🐈\r\nВторая строка";
            chat.set_input(draft);
            chat.update("Вы\nПривет\n\nИИ\nЗдравствуйте", "Thinking…", true);
            assert!(!windows::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled(chat.state.send).as_bool());
            chat.show((600.0, 300.0), "Cat");
            assert!(IsWindowVisible(chat.hwnd()).as_bool());
            assert!(!IsWindowVisible(parent).as_bool());
            assert_eq!(chat.input(), draft);
            proc(chat.hwnd(), WM_CLOSE, WPARAM(0), LPARAM(0));
            assert!(!IsWindowVisible(chat.hwnd()).as_bool());
            assert_eq!(chat.input(), draft);
            chat.show((600.0, 300.0), "Robot");
            assert_eq!(chat.input(), draft);
            chat.update("Переписка", "Enter to send · Shift+Enter for a new line", false);
            assert!(windows::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled(chat.state.send).as_bool());
            for action in [SEND, SETUP] {
                proc(chat.hwnd(), WM_COMMAND, WPARAM(action), LPARAM(0));
                let mut msg = MSG::default();
                assert!(PeekMessageW(&mut msg, Some(parent), WM_MINI_CHAT, WM_MINI_CHAT, PM_REMOVE).as_bool());
                assert_eq!(msg.wParam.0, action);
                assert_eq!(msg.lParam.0, chat.hwnd().0 as isize);
            }
            for (width, height) in [(340, 330), (600, 500)] {
                let _ =
                    SetWindowPos(chat.hwnd(), None, 0, 0, width, height, SWP_NOMOVE | SWP_NOACTIVATE | SWP_NOZORDER);
                layout(chat.hwnd(), &chat.state);
                let mut client = RECT::default();
                GetClientRect(chat.hwnd(), &mut client)?;
                for control in
                    [chat.state.history, chat.state.input, chat.state.setup, chat.state.send, chat.state.status]
                {
                    let mut r = RECT::default();
                    GetWindowRect(control, &mut r)?;
                    let mut origin = POINT { x: r.left, y: r.top };
                    ScreenToClient(chat.hwnd(), &mut origin).ok()?;
                    assert!(origin.x >= 0 && origin.y >= 0);
                    assert!(origin.x + r.right - r.left <= client.right);
                    assert!(origin.y + r.bottom - r.top <= client.bottom);
                }
            }
            let hwnd = chat.hwnd();
            drop(chat);
            assert!(!IsWindow(Some(hwnd)).as_bool());
            DestroyWindow(parent)?;
        }
        Ok(())
    }
}
