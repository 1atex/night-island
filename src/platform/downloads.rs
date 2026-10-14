use std::ffi::c_void;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, WPARAM};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_LIST_DIRECTORY, FILE_NOTIFY_CHANGE_FILE_NAME,
    FILE_NOTIFY_CHANGE_LAST_WRITE, FILE_NOTIFY_CHANGE_SIZE, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    OPEN_EXISTING, ReadDirectoryChangesW,
};
use windows::Win32::UI::Shell::{FOLDERID_Downloads, KF_FLAG_DEFAULT, SHGetKnownFolderPath};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};
use windows::core::HSTRING;

pub const WM_DOWNLOAD: u32 = WM_APP + 12;

const ACTION_ADDED: u32 = 1;
const ACTION_REMOVED: u32 = 2;
const ACTION_MODIFIED: u32 = 3;
const ACTION_RENAMED_OLD: u32 = 4;
const ACTION_RENAMED_NEW: u32 = 5;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Event {
    Active(Option<String>),
    Done(PathBuf),
}

static QUEUE: Mutex<Vec<Event>> = Mutex::new(Vec::new());

pub fn take_events() -> Vec<Event> {
    QUEUE.lock().map(|mut q| std::mem::take(&mut *q)).unwrap_or_default()
}

fn is_temporary(name: &str) -> bool {
    let lower = name.to_lowercase();
    ["crdownload", "part", "tmp", "download", "opdownload", "partial", "!ut", "aria2"]
        .iter()
        .any(|ext| lower.rsplit_once('.').is_some_and(|(_, e)| e == *ext))
}

fn ignored(name: &str) -> bool {
    name.starts_with('.') || name.starts_with("~$") || name.eq_ignore_ascii_case("desktop.ini") || name.contains('\\')
}

#[derive(Debug, PartialEq, Eq)]
pub enum Out {
    Active(Option<String>),
    Done(String),
    WatchSize(String),
}

#[derive(Default)]
pub struct Tracker {
    active: Vec<String>,
    old_name: Option<String>,
}

impl Tracker {
    fn newest(&self) -> Option<String> {
        self.active.last().cloned()
    }

    fn drop_active(&mut self, name: &str) -> bool {
        let before = self.active.len();
        self.active.retain(|n| n != name);
        self.active.len() != before
    }

    pub fn event(&mut self, action: u32, name: &str) -> Vec<Out> {
        if ignored(name) {
            return Vec::new();
        }
        let temporary = is_temporary(name);
        match action {
            ACTION_ADDED | ACTION_MODIFIED if temporary => {
                if self.active.iter().any(|n| n == name) {
                    return Vec::new();
                }
                self.active.push(name.to_string());
                vec![Out::Active(self.newest())]
            }
            ACTION_ADDED if !temporary => vec![Out::WatchSize(name.to_string())],
            ACTION_REMOVED if temporary => {
                if self.drop_active(name) {
                    vec![Out::Active(self.newest())]
                } else {
                    Vec::new()
                }
            }
            ACTION_RENAMED_OLD => {
                self.old_name = Some(name.to_string());
                Vec::new()
            }
            ACTION_RENAMED_NEW => {
                let old = self.old_name.take();
                let old_was_temp = old.as_deref().is_some_and(is_temporary);
                let mut out = Vec::new();
                if let Some(old) = &old {
                    self.drop_active(old);
                }
                if temporary {
                    if !self.active.iter().any(|n| n == name) {
                        self.active.push(name.to_string());
                    }
                    out.push(Out::Active(self.newest()));
                } else if old_was_temp {
                    out.push(Out::Active(self.newest()));
                    out.push(Out::Done(name.to_string()));
                }
                out
            }
            _ => Vec::new(),
        }
    }
}

fn push(event: Event, hwnd: isize) {
    if let Ok(mut q) = QUEUE.lock() {
        q.push(event);
    }
    unsafe {
        let _ = PostMessageW(Some(HWND(hwnd as *mut c_void)), WM_DOWNLOAD, WPARAM(0), LPARAM(0));
    }
}

fn downloads_dir() -> Option<PathBuf> {
    unsafe {
        let path = SHGetKnownFolderPath(&FOLDERID_Downloads, KF_FLAG_DEFAULT, None).ok()?;
        let dir = path.to_string().ok().map(PathBuf::from);
        windows::Win32::System::Com::CoTaskMemFree(Some(path.0 as *const _));
        dir
    }
}

fn watch_size(dir: PathBuf, name: String, hwnd: isize) {
    std::thread::spawn(move || {
        let path = dir.join(&name);
        let mut last = None;
        for _ in 0..120 {
            std::thread::sleep(Duration::from_millis(700));
            let Ok(meta) = std::fs::metadata(&path) else { return };
            if !meta.is_file() {
                return;
            }
            if last == Some(meta.len()) && meta.len() > 0 {
                return push(Event::Done(path), hwnd);
            }
            last = Some(meta.len());
        }
    });
}

pub fn start(owner: HWND) {
    let hwnd = owner.0 as isize;
    let Some(dir) = downloads_dir() else { return };
    std::thread::spawn(move || unsafe {
        let Ok(handle) = CreateFileW(
            &HSTRING::from(dir.as_os_str()),
            FILE_LIST_DIRECTORY.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            None,
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            None,
        ) else {
            return;
        };
        let mut tracker = Tracker::default();
        let mut buffer = vec![0u32; 16 * 1024];
        loop {
            let mut returned = 0u32;
            let read = ReadDirectoryChangesW(
                handle,
                buffer.as_mut_ptr() as *mut c_void,
                (buffer.len() * 4) as u32,
                false,
                FILE_NOTIFY_CHANGE_FILE_NAME | FILE_NOTIFY_CHANGE_LAST_WRITE | FILE_NOTIFY_CHANGE_SIZE,
                Some(&mut returned),
                None,
                None,
            );
            if read.is_err() {
                break;
            }
            let bytes = buffer.as_ptr() as *const u8;
            let mut offset = 0usize;
            while returned > 0 && offset + 12 <= returned as usize {
                let next = (bytes.add(offset) as *const u32).read() as usize;
                let action = (bytes.add(offset + 4) as *const u32).read();
                let name_len = (bytes.add(offset + 8) as *const u32).read() as usize;
                if offset + 12 + name_len > returned as usize {
                    break;
                }
                let units = std::slice::from_raw_parts(bytes.add(offset + 12) as *const u16, name_len / 2);
                let name = String::from_utf16_lossy(units);
                for out in tracker.event(action, &name) {
                    match out {
                        Out::Active(n) => push(Event::Active(n), hwnd),
                        Out::Done(n) => push(Event::Done(dir.join(n)), hwnd),
                        Out::WatchSize(n) => watch_size(dir.clone(), n, hwnd),
                    }
                }
                if next == 0 {
                    break;
                }
                offset += next;
            }
        }
        let _ = CloseHandle(handle);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_download_flow() {
        let mut t = Tracker::default();
        assert_eq!(
            t.event(ACTION_ADDED, "Unconfirmed 1.crdownload"),
            vec![Out::Active(Some("Unconfirmed 1.crdownload".into()))]
        );
        assert!(t.event(ACTION_MODIFIED, "Unconfirmed 1.crdownload").is_empty());
        assert!(t.event(ACTION_RENAMED_OLD, "Unconfirmed 1.crdownload").is_empty());
        assert_eq!(
            t.event(ACTION_RENAMED_NEW, "movie.mkv.crdownload"),
            vec![Out::Active(Some("movie.mkv.crdownload".into()))]
        );
        assert!(t.event(ACTION_RENAMED_OLD, "movie.mkv.crdownload").is_empty());
        assert_eq!(t.event(ACTION_RENAMED_NEW, "movie.mkv"), vec![Out::Active(None), Out::Done("movie.mkv".into())]);
    }

    #[test]
    fn cancelled_download_clears_the_indicator() {
        let mut t = Tracker::default();
        t.event(ACTION_ADDED, "a.part");
        assert_eq!(t.event(ACTION_REMOVED, "a.part"), vec![Out::Active(None)]);
    }

    #[test]
    fn two_downloads_show_the_newest() {
        let mut t = Tracker::default();
        t.event(ACTION_ADDED, "one.part");
        assert_eq!(t.event(ACTION_ADDED, "two.part"), vec![Out::Active(Some("two.part".into()))]);
        t.event(ACTION_RENAMED_OLD, "two.part");
        assert_eq!(
            t.event(ACTION_RENAMED_NEW, "two.zip"),
            vec![Out::Active(Some("one.part".into())), Out::Done("two.zip".into())]
        );
    }

    #[test]
    fn plain_files_are_watched_for_a_stable_size() {
        let mut t = Tracker::default();
        assert_eq!(t.event(ACTION_ADDED, "report.pdf"), vec![Out::WatchSize("report.pdf".into())]);
        t.event(ACTION_RENAMED_OLD, "a.txt");
        assert!(t.event(ACTION_RENAMED_NEW, "b.txt").is_empty());
    }

    #[test]
    fn noise_is_ignored() {
        let mut t = Tracker::default();
        assert!(t.event(ACTION_ADDED, "desktop.ini").is_empty());
        assert!(t.event(ACTION_ADDED, "~$doc.docx").is_empty());
        assert!(t.event(ACTION_ADDED, ".hidden").is_empty());
        assert!(is_temporary("X.CRDOWNLOAD") && !is_temporary("notes.txt"));
    }
}
