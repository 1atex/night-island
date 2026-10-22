use std::ffi::c_void;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU64, Ordering};

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::DataExchange::COPYDATASTRUCT;
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, PostMessageW, SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_APP, WM_COPYDATA,
};
use windows::core::{PCWSTR, w};

pub const WM_FILES: u32 = WM_APP + 23;

const REPLY_ID: usize = 0x4E49_5345;
const QUERY2W: usize = 18;
const REQUEST_FULL_PATH: u32 = 0x4;
const ITEM_FOLDER: u32 = 0x1;
const MAX_QUERY: usize = 256;

pub struct Hit {
    pub path: String,
    pub folder: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    NotRunning,
    Failed,
}

pub type Answer = Result<Vec<Hit>, Error>;

static REPLY: Mutex<Vec<u8>> = Mutex::new(Vec::new());
static AWAITING: AtomicBool = AtomicBool::new(false);
static GOT: AtomicBool = AtomicBool::new(false);
static ASKED: AtomicIsize = AtomicIsize::new(0);

pub fn on_copydata(sender: usize, data_id: usize, bytes: &[u8]) -> bool {
    if data_id != REPLY_ID || !AWAITING.load(Ordering::Acquire) || sender as isize != ASKED.load(Ordering::Acquire) {
        return false;
    }
    if let Ok(mut r) = REPLY.lock() {
        r.clear();
        r.extend_from_slice(bytes);
    }
    GOT.store(true, Ordering::Release);
    true
}

fn everything_window() -> Option<HWND> {
    for class in [w!("EVERYTHING_TASKBAR_NOTIFICATION"), w!("EVERYTHING_TASKBAR_NOTIFICATION_(1.5a)")] {
        if let Ok(h) = unsafe { FindWindowW(PCWSTR(class.as_ptr()), None) }
            && !h.is_invalid()
        {
            return Some(h);
        }
    }
    None
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

pub fn parse_reply(b: &[u8]) -> Vec<Hit> {
    let (Some(count), Some(flags)) = (u32_at(b, 4), u32_at(b, 12)) else { return Vec::new() };
    if flags & REQUEST_FULL_PATH == 0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for i in 0..count.min(1000) as usize {
        let at = 20 + i * 8;
        let (Some(item_flags), Some(offset)) = (u32_at(b, at), u32_at(b, at + 4)) else { break };
        let offset = offset as usize;
        let Some(len) = u32_at(b, offset) else { break };
        let Some(raw) = b.get(offset + 4..offset + 4 + len as usize * 2) else { break };
        let units: Vec<u16> = raw.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        out.push(Hit { path: String::from_utf16_lossy(&units), folder: item_flags & ITEM_FOLDER != 0 });
    }
    out
}

static LATEST: AtomicU64 = AtomicU64::new(0);
static DONE: Mutex<Option<(u64, Answer)>> = Mutex::new(None);
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

pub fn start(own: HWND, query: &str, limit: u32) {
    let id = LATEST.fetch_add(1, Ordering::SeqCst) + 1;
    let (raw, query) = (own.0 as isize, query.to_string());
    std::thread::spawn(move || {
        let _turn = ONE_AT_A_TIME.lock();
        if LATEST.load(Ordering::SeqCst) != id {
            return;
        }
        let own = HWND(raw as *mut c_void);
        let result = ask(own, &query, limit);
        if LATEST.load(Ordering::SeqCst) != id {
            return;
        }
        if let Ok(mut done) = DONE.lock() {
            *done = Some((id, result));
        }
        unsafe {
            let _ = PostMessageW(Some(own), WM_FILES, WPARAM(0), LPARAM(0));
        }
    });
}

pub fn cancel() {
    LATEST.fetch_add(1, Ordering::SeqCst);
}

pub fn take_result() -> Option<Answer> {
    let (id, result) = DONE.lock().ok()?.take()?;
    (id == LATEST.load(Ordering::SeqCst)).then_some(result)
}

fn ask(own: HWND, query: &str, limit: u32) -> Answer {
    let target = everything_window().ok_or(Error::NotRunning)?;
    let query: String = query.chars().filter(|c| *c != '\0').take(MAX_QUERY).collect();

    let mut payload: Vec<u8> = Vec::new();
    for v in [own.0 as usize as u32, REPLY_ID as u32, 0, 0, limit, REQUEST_FULL_PATH, 0] {
        payload.extend_from_slice(&v.to_le_bytes());
    }
    for unit in query.encode_utf16().chain(std::iter::once(0)) {
        payload.extend_from_slice(&unit.to_le_bytes());
    }

    if let Ok(mut r) = REPLY.lock() {
        r.clear();
    }
    ASKED.store(target.0 as isize, Ordering::Release);
    GOT.store(false, Ordering::Release);
    AWAITING.store(true, Ordering::Release);
    let cds = COPYDATASTRUCT { dwData: QUERY2W, cbData: payload.len() as u32, lpData: payload.as_mut_ptr().cast() };
    let accepted = unsafe {
        SendMessageTimeoutW(
            target,
            WM_COPYDATA,
            WPARAM(own.0 as usize),
            LPARAM(&cds as *const _ as isize),
            SMTO_ABORTIFHUNG,
            1500,
            None,
        )
    };
    let started = std::time::Instant::now();
    while accepted.0 != 0 && !GOT.load(Ordering::Acquire) && started.elapsed() < std::time::Duration::from_millis(1500)
    {
        std::thread::sleep(std::time::Duration::from_millis(3));
    }
    AWAITING.store(false, Ordering::Release);
    if accepted.0 == 0 || !GOT.load(Ordering::Acquire) {
        return Err(Error::Failed);
    }
    let reply = REPLY.lock().map(|r| r.clone()).map_err(|_| Error::Failed)?;
    Ok(parse_reply(&reply))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reply(items: &[(&str, bool)]) -> Vec<u8> {
        let mut head = Vec::new();
        for v in [items.len() as u32, items.len() as u32, 0, REQUEST_FULL_PATH, 0] {
            head.extend_from_slice(&v.to_le_bytes());
        }
        let table = head.len() + items.len() * 8;
        let (mut index, mut data) = (Vec::new(), Vec::new());
        for (path, folder) in items {
            index.extend_from_slice(&u32::from(*folder).to_le_bytes());
            index.extend_from_slice(&((table + data.len()) as u32).to_le_bytes());
            let units: Vec<u16> = path.encode_utf16().collect();
            data.extend_from_slice(&(units.len() as u32).to_le_bytes());
            for u in units.iter().chain(std::iter::once(&0)) {
                data.extend_from_slice(&u.to_le_bytes());
            }
        }
        [head, index, data].concat()
    }

    #[test]
    fn a_reply_is_read_back() {
        let hits = parse_reply(&reply(&[(r"C:\Users\me\отчёт.docx", false), (r"C:\Games", true)]));
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].path, r"C:\Users\me\отчёт.docx");
        assert!(!hits[0].folder && hits[1].folder);
    }

    #[test]
    fn broken_replies_do_not_panic() {
        assert!(parse_reply(&[]).is_empty());
        assert!(parse_reply(&[1, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0]).is_empty());
        let mut cut = reply(&[(r"C:\a", false)]);
        cut.truncate(cut.len() - 6);
        assert!(parse_reply(&cut).is_empty());
    }

    #[test]
    fn foreign_replies_are_ignored() {
        assert!(!on_copydata(1, REPLY_ID, b"x"));
    }
}

#[cfg(test)]
mod live {
    use super::*;
    use windows::Win32::Foundation::{HINSTANCE, LRESULT};
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, HWND_MESSAGE, RegisterClassW, WINDOW_STYLE, WNDCLASSW, WS_EX_TOOLWINDOW,
    };

    extern "system" fn proc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
        if m == WM_COPYDATA {
            let cds = unsafe { &*(l.0 as *const COPYDATASTRUCT) };
            let bytes = unsafe { std::slice::from_raw_parts(cds.lpData as *const u8, cds.cbData as usize) };
            if on_copydata(w.0, cds.dwData, bytes) {
                return LRESULT(1);
            }
        }
        unsafe { DefWindowProcW(h, m, w, l) }
    }

    #[test]
    #[ignore]
    fn asks_the_real_everything() {
        unsafe {
            let class = WNDCLASSW {
                lpfnWndProc: Some(proc),
                lpszClassName: w!("NiEverythingTest"),
                hInstance: HINSTANCE::default(),
                ..Default::default()
            };
            RegisterClassW(&class);
            let hwnd = CreateWindowExW(
                WS_EX_TOOLWINDOW,
                w!("NiEverythingTest"),
                w!("t"),
                WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                None,
                None,
            )
            .unwrap();
            let raw = hwnd.0 as isize;
            let worker = std::thread::spawn(move || ask(HWND(raw as *mut std::ffi::c_void), "notepad.exe", 5));
            while !worker.is_finished() {
                let mut msg = windows::Win32::UI::WindowsAndMessaging::MSG::default();
                while windows::Win32::UI::WindowsAndMessaging::PeekMessageW(
                    &mut msg,
                    None,
                    0,
                    0,
                    windows::Win32::UI::WindowsAndMessaging::PM_REMOVE,
                )
                .as_bool()
                {
                    windows::Win32::UI::WindowsAndMessaging::DispatchMessageW(&msg);
                }
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            let hits = worker.join().unwrap().expect("everything answered");
            for h in &hits {
                println!("{} folder={}", h.path, h.folder);
            }
            assert!(!hits.is_empty());
        }
    }
}
