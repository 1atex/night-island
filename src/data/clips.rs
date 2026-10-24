use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, params};

const MAX_TEXT_BYTES: usize = 256 * 1024;
const MAX_UNPINNED: i64 = 200;
const LIST_LIMIT: i64 = 100;
const LIST_PREVIEW_CHARS: i64 = 2000;

#[derive(Clone)]
pub struct Clip {
    pub id: i64,
    pub text: String,
    pub pinned: bool,
}

pub struct ShelfItem {
    pub id: i64,
    pub path: String,
}

const MAX_SHELF: i64 = 60;
const MAX_DOCK: i64 = 24;

pub struct Reminder {
    pub id: i64,
    pub due: i64,
    pub text: String,
}

const MAX_REMINDERS: i64 = 100;

pub struct Store {
    pub(super) conn: Connection,
}

impl Store {
    fn path() -> Option<PathBuf> {
        let dir = PathBuf::from(std::env::var_os("APPDATA")?).join("NightIsland");
        std::fs::create_dir_all(&dir).ok()?;
        Some(dir.join("data.db"))
    }

    pub fn open() -> Option<Self> {
        Self::with_connection(Connection::open(Self::path()?).ok()?)
    }

    #[cfg(test)]
    pub fn in_memory() -> Option<Self> {
        Self::with_connection(Connection::open_in_memory().ok()?)
    }

    fn with_connection(conn: Connection) -> Option<Self> {
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             CREATE TABLE IF NOT EXISTS clips (
                 id     INTEGER PRIMARY KEY,
                 text   TEXT NOT NULL UNIQUE,
                 ts     INTEGER NOT NULL,
                 pinned INTEGER NOT NULL DEFAULT 0
             );
             CREATE INDEX IF NOT EXISTS clips_order ON clips(pinned DESC, ts DESC);
             CREATE TABLE IF NOT EXISTS notes (
                 id   INTEGER PRIMARY KEY CHECK (id = 1),
                 text TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS reminders (
                 id   INTEGER PRIMARY KEY,
                 due  INTEGER NOT NULL,
                 text TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS dock (
                 id   INTEGER PRIMARY KEY,
                 path TEXT NOT NULL UNIQUE,
                 ord  INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS shelf (
                 id   INTEGER PRIMARY KEY,
                 path TEXT NOT NULL UNIQUE,
                 ts   INTEGER NOT NULL
             );",
        )
        .ok()?;
        super::notes::init(&conn).ok()?;
        super::usage::init(&conn).ok()?;
        super::chats::init(&conn).ok()?;
        super::groups::init(&conn).ok()?;
        Some(Self { conn })
    }

    pub fn recent(&self) -> Vec<Clip> {
        let Ok(mut stmt) = self
            .conn
            .prepare_cached("SELECT id, substr(text, 1, ?1), pinned FROM clips ORDER BY pinned DESC, ts DESC LIMIT ?2")
        else {
            return Vec::new();
        };
        stmt.query_map(params![LIST_PREVIEW_CHARS, LIST_LIMIT], |r| {
            Ok(Clip { id: r.get(0)?, text: r.get(1)?, pinned: r.get::<_, i64>(2)? != 0 })
        })
        .map(|rows| rows.flatten().collect())
        .unwrap_or_default()
    }

    pub fn text(&self, id: i64) -> Option<String> {
        self.conn.query_row("SELECT text FROM clips WHERE id = ?1", params![id], |r| r.get(0)).ok()
    }

    pub fn clear_unpinned(&self) {
        let _ = self.conn.execute("DELETE FROM clips WHERE pinned = 0", []);
    }

    pub fn add(&self, text: &str) {
        if text.trim().is_empty() || text.len() > MAX_TEXT_BYTES {
            return;
        }
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64);
        let _ = self.conn.execute(
            "INSERT INTO clips (text, ts) VALUES (?1, ?2)
             ON CONFLICT(text) DO UPDATE SET ts = excluded.ts",
            params![text, now],
        );
        let _ = self.conn.execute(
            "DELETE FROM clips WHERE pinned = 0 AND id NOT IN
             (SELECT id FROM clips WHERE pinned = 0 ORDER BY ts DESC LIMIT ?1)",
            params![MAX_UNPINNED],
        );
    }

    pub fn reminders(&self) -> Vec<Reminder> {
        let Ok(mut stmt) = self.conn.prepare_cached("SELECT id, due, text FROM reminders ORDER BY due") else {
            return Vec::new();
        };
        stmt.query_map([], |r| Ok(Reminder { id: r.get(0)?, due: r.get(1)?, text: r.get(2)? }))
            .map(|rows| rows.flatten().collect())
            .unwrap_or_default()
    }

    pub fn reminder_add(&self, due: i64, text: &str) -> bool {
        let count: i64 = self.conn.query_row("SELECT COUNT(*) FROM reminders", [], |r| r.get(0)).unwrap_or(0);
        if count >= MAX_REMINDERS {
            return false;
        }
        self.conn.execute("INSERT INTO reminders (due, text) VALUES (?1, ?2)", params![due, text]).is_ok()
    }

    pub fn reminder_remove(&self, id: i64) {
        let _ = self.conn.execute("DELETE FROM reminders WHERE id = ?1", params![id]);
    }

    pub fn reminder_set_due(&self, id: i64, due: i64) {
        let _ = self.conn.execute("UPDATE reminders SET due = ?2 WHERE id = ?1", params![id, due]);
    }

    pub fn dock(&self) -> Vec<ShelfItem> {
        let Ok(mut stmt) = self.conn.prepare_cached("SELECT id, path FROM dock ORDER BY ord LIMIT ?1") else {
            return Vec::new();
        };
        stmt.query_map(params![MAX_DOCK], |r| Ok(ShelfItem { id: r.get(0)?, path: r.get(1)? }))
            .map(|rows| rows.flatten().collect())
            .unwrap_or_default()
    }

    pub fn dock_add(&self, path: &str) {
        let _ = self.conn.execute(
            "INSERT OR IGNORE INTO dock (path, ord) SELECT ?1, COALESCE(MAX(ord), 0) + 1 FROM dock WHERE (SELECT COUNT(*) FROM dock) < ?2",
            params![path, MAX_DOCK],
        );
    }

    pub fn dock_remove(&self, id: i64) {
        let _ = self.conn.execute("DELETE FROM dock WHERE id = ?1", params![id]);
    }

    pub fn shelf(&self) -> Vec<ShelfItem> {
        let Ok(mut stmt) = self.conn.prepare_cached("SELECT id, path FROM shelf ORDER BY ts DESC LIMIT ?1") else {
            return Vec::new();
        };
        stmt.query_map(params![MAX_SHELF], |r| Ok(ShelfItem { id: r.get(0)?, path: r.get(1)? }))
            .map(|rows| rows.flatten().collect())
            .unwrap_or_default()
    }

    pub fn shelf_add(&self, path: &str) {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64);
        let _ = self.conn.execute(
            "INSERT INTO shelf (path, ts) VALUES (?1, ?2) ON CONFLICT(path) DO UPDATE SET ts = excluded.ts",
            params![path, now],
        );
        let _ = self.conn.execute(
            "DELETE FROM shelf WHERE id NOT IN (SELECT id FROM shelf ORDER BY ts DESC LIMIT ?1)",
            params![MAX_SHELF],
        );
    }

    pub fn shelf_remove(&self, id: i64) {
        let _ = self.conn.execute("DELETE FROM shelf WHERE id = ?1", params![id]);
    }

    pub fn toggle_pin(&self, id: i64) {
        let _ = self.conn.execute("UPDATE clips SET pinned = 1 - pinned WHERE id = ?1", params![id]);
    }

    pub fn delete(&self, id: i64) {
        let _ = self.conn.execute("DELETE FROM clips WHERE id = ?1", params![id]);
    }
}

pub mod sys {
    use std::mem::size_of;
    use std::thread::sleep;
    use std::time::Duration;

    use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND};
    use windows::Win32::System::DataExchange::{
        AddClipboardFormatListener, CloseClipboard, EmptyClipboard, GetClipboardData, GetClipboardSequenceNumber,
        IsClipboardFormatAvailable, OpenClipboard, RegisterClipboardFormatW, RemoveClipboardFormatListener,
        SetClipboardData,
    };
    use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock};
    use windows::Win32::System::Ole::{CF_DIB, CF_UNICODETEXT};
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput, VIRTUAL_KEY,
        VK_CONTROL,
    };
    use windows::core::w;

    use super::MAX_TEXT_BYTES;

    pub fn listen(hwnd: HWND) {
        unsafe {
            let _ = AddClipboardFormatListener(hwnd);
        }
    }

    pub fn unlisten(hwnd: HWND) {
        unsafe {
            let _ = RemoveClipboardFormatListener(hwnd);
        }
    }

    fn open(hwnd: HWND) -> bool {
        for _ in 0..8 {
            if unsafe { OpenClipboard(Some(hwnd)) }.is_ok() {
                return true;
            }
            sleep(Duration::from_millis(8));
        }
        false
    }

    fn marked_private() -> bool {
        unsafe {
            let exclude = RegisterClipboardFormatW(w!("ExcludeClipboardContentFromMonitorProcessing"));
            if exclude != 0 && IsClipboardFormatAvailable(exclude).is_ok() {
                return true;
            }
            let history = RegisterClipboardFormatW(w!("CanIncludeInClipboardHistory"));
            if history != 0
                && IsClipboardFormatAvailable(history).is_ok()
                && let Ok(h) = GetClipboardData(history)
            {
                let ptr = GlobalLock(HGLOBAL(h.0)) as *const u32;
                if !ptr.is_null() {
                    let value = ptr.read_unaligned();
                    let _ = GlobalUnlock(HGLOBAL(h.0));
                    return value == 0;
                }
            }
            false
        }
    }

    pub fn read_text(hwnd: HWND) -> Option<String> {
        if !open(hwnd) {
            return None;
        }

        unsafe {
            let result = (|| {
                if marked_private() || IsClipboardFormatAvailable(CF_UNICODETEXT.0 as u32).is_err() {
                    return None;
                }
                let handle = GetClipboardData(CF_UNICODETEXT.0 as u32).ok()?;
                let global = HGLOBAL(handle.0);
                let bytes = GlobalSize(global);
                if bytes == 0 || bytes > MAX_TEXT_BYTES * 2 + 2 {
                    return None;
                }
                let ptr = GlobalLock(global) as *const u16;
                if ptr.is_null() {
                    return None;
                }
                let units = std::slice::from_raw_parts(ptr, bytes / 2);
                let len = units.iter().position(|&c| c == 0).unwrap_or(units.len());
                let text = String::from_utf16_lossy(&units[..len]);
                let _ = GlobalUnlock(global);
                Some(text)
            })();
            let _ = CloseClipboard();
            result
        }
    }

    pub fn write_text(hwnd: HWND, text: &str) -> bool {
        let units: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        if !open(hwnd) {
            return false;
        }
        unsafe {
            let mut ok = false;
            if let Ok(global) = GlobalAlloc(GMEM_MOVEABLE, units.len() * size_of::<u16>()) {
                let ptr = GlobalLock(global) as *mut u16;
                if !ptr.is_null() {
                    std::ptr::copy_nonoverlapping(units.as_ptr(), ptr, units.len());
                    let _ = GlobalUnlock(global);
                    if EmptyClipboard().is_ok()
                        && SetClipboardData(CF_UNICODETEXT.0 as u32, Some(HANDLE(global.0))).is_ok()
                    {
                        ok = true;
                    }
                }
                if !ok {
                    let _ = GlobalFree(Some(global));
                }
            }
            let _ = CloseClipboard();
            ok
        }
    }

    fn global_from(bytes: &[u8]) -> Option<HGLOBAL> {
        unsafe {
            let global = GlobalAlloc(GMEM_MOVEABLE, bytes.len()).ok()?;
            let ptr = GlobalLock(global) as *mut u8;
            if ptr.is_null() {
                let _ = GlobalFree(Some(global));
                return None;
            }
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len());
            let _ = GlobalUnlock(global);
            Some(global)
        }
    }

    pub fn write_image(hwnd: HWND, dib: &[u8], png: &[u8]) -> bool {
        if !open(hwnd) {
            return false;
        }
        let mut ok = false;
        unsafe {
            if EmptyClipboard().is_ok() {
                if let Some(g) = global_from(dib) {
                    if SetClipboardData(CF_DIB.0 as u32, Some(HANDLE(g.0))).is_ok() {
                        ok = true;
                    } else {
                        let _ = GlobalFree(Some(g));
                    }
                }
                let png_format = RegisterClipboardFormatW(w!("PNG"));
                if png_format != 0
                    && let Some(g) = global_from(png)
                    && SetClipboardData(png_format, Some(HANDLE(g.0))).is_err()
                {
                    let _ = GlobalFree(Some(g));
                }
            }
            let _ = CloseClipboard();
        }
        ok
    }

    pub fn sequence_number() -> u32 {
        unsafe { GetClipboardSequenceNumber() }
    }

    pub fn write_and_paste(hwnd: HWND, text: &str) -> u32 {
        if !write_text(hwnd, text) {
            return 0;
        }
        let seq = sequence_number();
        send_paste();
        seq
    }

    pub fn send_paste() {
        let key = |vk: VIRTUAL_KEY, flags: KEYBD_EVENT_FLAGS| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, wScan: 0, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
        };
        let v = VIRTUAL_KEY(b'V' as u16);
        let inputs = [
            key(VK_CONTROL, KEYBD_EVENT_FLAGS(0)),
            key(v, KEYBD_EVENT_FLAGS(0)),
            key(v, KEYEVENTF_KEYUP),
            key(VK_CONTROL, KEYEVENTF_KEYUP),
        ];
        unsafe {
            SendInput(&inputs, size_of::<INPUT>() as i32);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_keeps_a_preview_and_the_whole_text_is_read_by_id() {
        let store = Store::in_memory().unwrap();
        let long = "я".repeat(LIST_PREVIEW_CHARS as usize + 500);
        store.add(&long);
        let list = store.recent();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].text.chars().count(), LIST_PREVIEW_CHARS as usize);
        assert_eq!(store.text(list[0].id).unwrap(), long);
    }

    #[test]
    fn clearing_keeps_only_the_pinned_entries() {
        let store = Store::in_memory().unwrap();
        store.add("one");
        store.add("two");
        let one = store.recent().into_iter().find(|c| c.text == "one").unwrap();
        store.toggle_pin(one.id);
        store.clear_unpinned();
        let left: Vec<String> = store.recent().into_iter().map(|c| c.text).collect();
        assert_eq!(left, ["one"]);
    }
}
