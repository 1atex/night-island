use std::collections::HashMap;
use std::ffi::c_void;
use std::io::Read;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use rusqlite::{Connection, OpenFlags};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};

pub const WM_AGY: u32 = WM_APP + 17;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const MAX_PROMPT_CHARS: usize = 8000;
const MAX_OUTPUT_BYTES: u64 = 2 * 1024 * 1024;
const MAX_REPLY_CHARS: usize = 4000;

#[derive(Clone, Debug, PartialEq)]
pub struct Chat {
    pub id: String,
    pub title: String,
    pub folder: String,
    pub workspace: Option<PathBuf>,
    pub modified: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Job {
    Running,
    Done { reply: String, ok: bool, secs: f32 },
}

static JOBS: Mutex<Vec<(String, Job, String)>> = Mutex::new(Vec::new());
static CHILDREN: Mutex<Option<HashMap<String, Arc<Mutex<Child>>>>> = Mutex::new(None);

static SESSION_TOKENS: AtomicU64 = AtomicU64::new(0);

pub fn session_tokens() -> u64 {
    SESSION_TOKENS.load(Ordering::Relaxed)
}

pub fn jobs() -> HashMap<String, Job> {
    JOBS.lock().map(|j| j.iter().map(|(id, job, _)| (id.clone(), job.clone())).collect()).unwrap_or_default()
}

fn set_job(id: &str, job: Job, title: &str) {
    if let Ok(mut j) = JOBS.lock() {
        j.retain(|(i, _, _)| i != id);
        j.push((id.to_string(), job, title.to_string()));
    }
}

pub fn job_title(id: &str) -> String {
    JOBS.lock().ok().and_then(|j| j.iter().find(|(i, _, _)| i == id).map(|(_, _, t)| t.clone())).unwrap_or_default()
}

fn exe() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(|d| PathBuf::from(d).join(r"agy\bin\agy.exe"))
        .filter(|p| p.exists())
        .unwrap_or_else(|| PathBuf::from("agy"))
}

pub fn installed() -> bool {
    exe().is_absolute()
}

fn db_path() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("USERPROFILE")?).join(r".gemini\antigravity-cli\conversation_summaries.db"))
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let (mut out, mut i) = (Vec::with_capacity(b.len()), 0);
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16)
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file:///")?;
    let path = percent_decode(rest).replace('/', "\\");
    (path.len() < 1024 && !path.contains('\0')).then(|| PathBuf::from(path))
}

fn parse_time(s: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(&s.replacen(' ', "T", 1)).map_or(0, |t| t.timestamp())
}

pub fn chats(limit: usize) -> Vec<Chat> {
    let Some(path) = db_path().filter(|p| p.exists()) else { return Vec::new() };
    let Ok(conn) =
        Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)
    else {
        return Vec::new();
    };
    let _ = conn.busy_timeout(std::time::Duration::from_millis(300));
    let Ok(mut stmt) = conn.prepare(
        "SELECT conversation_id, title, preview, last_modified_time, workspace_uris FROM conversation_summaries WHERE app_data_dir = 'antigravity-cli' AND parent_conversation_id = '' AND step_count > 0 ORDER BY last_modified_time DESC LIMIT ?1",
    ) else {
        return Vec::new();
    };
    let rows = stmt.query_map([limit as i64], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, String>(4)?,
        ))
    });
    let Ok(rows) = rows else { return Vec::new() };
    rows.flatten()
        .filter(|(id, ..)| id.len() <= 64 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'))
        .map(|(id, title, preview, modified, uris)| {
            let workspace =
                serde_json::from_str::<Vec<String>>(&uris).ok().and_then(|v| v.first().and_then(|u| uri_to_path(u)));
            let folder = workspace
                .as_ref()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let title = if title.trim().is_empty() { preview } else { title };
            Chat { id, title: title.chars().take(120).collect(), folder, workspace, modified: parse_time(&modified) }
        })
        .collect()
}

pub fn running(id: &str) -> bool {
    JOBS.lock().is_ok_and(|j| j.iter().any(|(i, job, _)| i == id && *job == Job::Running))
}

pub fn send(hwnd: HWND, chat: &Chat, text: &str, skip_permissions: bool) -> bool {
    let text: String = text.trim().chars().filter(|c| *c != '\0').take(MAX_PROMPT_CHARS).collect();
    if text.is_empty() || running(&chat.id) {
        return false;
    }
    let mut cmd = Command::new(exe());
    cmd.args(["--conversation", &chat.id, "-p", &text, "--output-format", "json"]);
    if skip_permissions {
        cmd.arg("--dangerously-skip-permissions");
    }
    if let Some(dir) = chat.workspace.as_ref().filter(|d| d.is_dir()) {
        cmd.current_dir(dir);
    }
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).creation_flags(CREATE_NO_WINDOW);
    let Ok(mut child) = cmd.spawn() else {
        set_job(&chat.id, Job::Done { reply: "agy did not start".into(), ok: false, secs: 0.0 }, &chat.title);
        notify(hwnd);
        return true;
    };
    let stdout = child.stdout.take();
    let child = Arc::new(Mutex::new(child));
    if let Ok(mut c) = CHILDREN.lock() {
        c.get_or_insert_with(HashMap::new).insert(chat.id.clone(), child.clone());
    }
    set_job(&chat.id, Job::Running, &chat.title);
    notify(hwnd);

    let raw = hwnd.0 as isize;
    let (id, title) = (chat.id.clone(), chat.title.clone());
    std::thread::spawn(move || {
        let started = std::time::Instant::now();
        let mut output = String::new();
        if let Some(out) = stdout {
            let _ = out.take(MAX_OUTPUT_BYTES).read_to_string(&mut output);
        }
        let status = child.lock().ok().and_then(|mut c| c.wait().ok());
        if let Ok(mut c) = CHILDREN.lock()
            && let Some(map) = c.as_mut()
        {
            map.remove(&id);
        }
        let secs = started.elapsed().as_secs_f32();
        let job = parse_output(&output, status.is_some_and(|s| s.success()), secs);
        set_job(&id, job, &title);
        notify(HWND(raw as *mut c_void));
    });
    true
}

fn parse_output(output: &str, exited_ok: bool, secs: f32) -> Job {
    let clip = |s: &str| s.trim().chars().take(MAX_REPLY_CHARS).collect::<String>();
    match serde_json::from_str::<serde_json::Value>(output.trim()) {
        Ok(v) => {
            let ok = v.get("status").and_then(|s| s.as_str()) == Some("SUCCESS");
            let reply = v.get("response").and_then(|r| r.as_str()).map(clip).unwrap_or_default();
            let secs =
                v.get("duration_seconds").and_then(|d| d.as_f64()).filter(|d| *d < 86_400.0).map_or(secs, |d| d as f32);
            if let Some(t) = v.pointer("/usage/total_tokens").and_then(|t| t.as_u64()) {
                SESSION_TOKENS.fetch_add(t, Ordering::Relaxed);
            }
            Job::Done { reply, ok, secs }
        }
        Err(_) => Job::Done { reply: clip(output), ok: exited_ok && !output.trim().is_empty(), secs },
    }
}

pub fn stop(id: &str) {
    let child = CHILDREN.lock().ok().and_then(|c| c.as_ref().and_then(|m| m.get(id).cloned()));
    if let Some(child) = child
        && let Ok(mut c) = child.lock()
    {
        let _ = c.kill();
    }
}

fn notify(hwnd: HWND) {
    unsafe {
        let _ = PostMessageW(Some(hwnd), WM_AGY, WPARAM(0), LPARAM(0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_uris_become_paths() {
        assert_eq!(uri_to_path("file:///g%3A/Coding/night%20island"), Some(PathBuf::from(r"g:\Coding\night island")));
        assert_eq!(uri_to_path("file:///G:/Coding/x"), Some(PathBuf::from(r"G:\Coding\x")));
        assert_eq!(uri_to_path("https://example.com"), None);
    }

    #[test]
    fn index_times_are_read() {
        assert!(parse_time("2026-10-08 22:10:44.4243809+00:00") > 1_700_000_000);
        assert_eq!(parse_time("garbage"), 0);
    }

    #[test]
    fn json_and_plain_answers_are_understood() {
        let ok = parse_output(
            r#"{"conversation_id":"x","status":"SUCCESS","response":"OK\n","duration_seconds":5.5}"#,
            true,
            1.0,
        );
        assert_eq!(ok, Job::Done { reply: "OK".into(), ok: true, secs: 5.5 });
        let bad = parse_output(r#"{"status":"ERROR","response":""}"#, true, 1.0);
        assert!(matches!(bad, Job::Done { ok: false, .. }));
        assert!(matches!(parse_output("boom", false, 2.0), Job::Done { ok: false, .. }));
    }

    #[test]
    fn the_real_index_can_be_listed() {
        let list = chats(5);
        assert!(list.len() <= 5);
        assert!(list.iter().all(|c| !c.id.is_empty()));
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    User,
    Assistant,
    Thinking,
    Tool,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Msg {
    pub kind: Kind,
    pub doc: crate::ui::md::Doc,
}

enum Val<'a> {
    Varint(u64),
    Bytes(&'a [u8]),
}

fn read_varint(b: &[u8], i: &mut usize) -> Option<u64> {
    let (mut v, mut shift) = (0u64, 0u32);
    loop {
        let byte = *b.get(*i)?;
        *i += 1;
        v |= u64::from(byte & 0x7F).checked_shl(shift)?;
        if byte < 0x80 {
            return Some(v);
        }
        shift += 7;
        if shift > 63 {
            return None;
        }
    }
}

fn fields(b: &[u8]) -> Vec<(u32, Val<'_>)> {
    let (mut i, mut out) = (0, Vec::new());
    while i < b.len() {
        let Some(key) = read_varint(b, &mut i) else { break };
        let (num, wire) = ((key >> 3) as u32, key & 7);
        match wire {
            0 => match read_varint(b, &mut i) {
                Some(v) => out.push((num, Val::Varint(v))),
                None => break,
            },
            2 => {
                let Some(len) = read_varint(b, &mut i) else { break };
                let Some(data) = b.get(i..i.saturating_add(len as usize)) else { break };
                out.push((num, Val::Bytes(data)));
                i += len as usize;
            }
            1 => i += 8,
            5 => i += 4,
            _ => break,
        }
    }
    out
}

fn bytes_of<'a>(f: &[(u32, Val<'a>)], num: u32) -> Option<&'a [u8]> {
    f.iter().find_map(|(n, v)| match v {
        Val::Bytes(b) if *n == num => Some(*b),
        _ => None,
    })
}

fn all_bytes<'a>(f: &[(u32, Val<'a>)], num: u32) -> Vec<&'a [u8]> {
    f.iter()
        .filter_map(|(n, v)| match v {
            Val::Bytes(b) if *n == num => Some(*b),
            _ => None,
        })
        .collect()
}

fn text_of(b: Option<&[u8]>) -> String {
    b.and_then(|b| std::str::from_utf8(b).ok()).unwrap_or("").to_string()
}

fn varint_of(f: &[(u32, Val<'_>)], num: u32) -> Option<u64> {
    f.iter().find_map(|(n, v)| match v {
        Val::Varint(x) if *n == num => Some(*x),
        _ => None,
    })
}

fn clip(s: String, n: usize) -> String {
    if s.chars().count() <= n { s } else { format!("{}…", s.chars().take(n).collect::<String>()) }
}

fn plain(text: String) -> crate::ui::md::Doc {
    crate::ui::md::Doc { text, spans: Vec::new() }
}

fn step_to_msgs(step_type: i64, payload: &[u8]) -> Vec<Msg> {
    let top = fields(payload);
    let mut out = Vec::new();
    match step_type {
        14 => {
            let text = text_of(bytes_of(&fields(bytes_of(&top, 19).unwrap_or(&[])), 2));
            if !text.trim().is_empty() {
                out.push(Msg { kind: Kind::User, doc: crate::ui::md::parse(&clip(text, 4000)) });
            }
        }
        15 => {
            let body = fields(bytes_of(&top, 20).unwrap_or(&[]));
            let thinking = text_of(bytes_of(&body, 3));
            if !thinking.trim().is_empty() {
                out.push(Msg { kind: Kind::Thinking, doc: crate::ui::md::parse(&clip(thinking, 1200)) });
            }
            let mut reply = text_of(bytes_of(&body, 1));
            if reply.trim().is_empty() {
                reply = text_of(bytes_of(&body, 8));
            }
            if !reply.trim().is_empty() {
                out.push(Msg { kind: Kind::Assistant, doc: crate::ui::md::parse(&clip(reply, 12_000)) });
            }
        }
        132 => {
            let body = fields(bytes_of(&top, 140).unwrap_or(&[]));
            let (mut action, mut summary) = (String::new(), String::new());
            for pair in all_bytes(&body, 1) {
                let kv = fields(pair);
                match text_of(bytes_of(&kv, 1)).as_str() {
                    "toolAction" => action = text_of(bytes_of(&kv, 2)),
                    "toolSummary" => summary = text_of(bytes_of(&kv, 2)),
                    _ => {}
                }
            }
            let output = text_of(bytes_of(&fields(bytes_of(&body, 2).unwrap_or(&[])), 1));
            let head = match (action.is_empty(), summary.is_empty()) {
                (false, false) => format!("{action} — {summary}"),
                (false, true) => action,
                (true, false) => summary,
                (true, true) => String::new(),
            };
            if !head.is_empty() {
                let snippet: String =
                    output.lines().map(str::trim).filter(|l| !l.is_empty()).take(2).collect::<Vec<_>>().join("  ");
                let text = if snippet.is_empty() { head } else { format!("{head}\n{}", clip(snippet, 200)) };
                out.push(Msg { kind: Kind::Tool, doc: plain(text) });
            }
        }
        17 => {
            let err = fields(bytes_of(&fields(bytes_of(&top, 24).unwrap_or(&[])), 3).unwrap_or(&[]));
            let text = text_of(bytes_of(&err, 1));
            if !text.is_empty() {
                out.push(Msg { kind: Kind::Error, doc: plain(clip(text, 400)) });
            }
        }
        _ => {}
    }
    out
}

fn conversation_path(id: &str) -> Option<PathBuf> {
    if id.is_empty() || id.len() > 64 || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
        return None;
    }
    Some(
        PathBuf::from(std::env::var_os("USERPROFILE")?).join(format!(r".gemini\antigravity-cli\conversations\{id}.db")),
    )
}

pub fn history(id: &str) -> Vec<Msg> {
    let Some(path) = conversation_path(id).filter(|p| p.exists()) else { return Vec::new() };
    let Ok(conn) =
        Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)
    else {
        return Vec::new();
    };
    let _ = conn.busy_timeout(std::time::Duration::from_millis(300));
    let Ok(mut stmt) = conn.prepare("SELECT step_type, step_payload FROM steps ORDER BY idx DESC LIMIT 150") else {
        return Vec::new();
    };
    let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<Vec<u8>>>(1)?)));
    let Ok(rows) = rows else { return Vec::new() };
    let mut steps: Vec<(i64, Vec<u8>)> = rows.flatten().filter_map(|(t, p)| p.map(|p| (t, p))).collect();
    steps.reverse();
    steps.iter().flat_map(|(t, p)| step_to_msgs(*t, p)).collect()
}

fn parse_reset_delay(text: &str) -> Option<i64> {
    let rest = &text[text.find("Resets in ")? + "Resets in ".len()..];
    let (mut total, mut number, mut any) = (0i64, 0i64, false);
    for c in rest.chars() {
        match c {
            '0'..='9' => number = number * 10 + i64::from(c as u8 - b'0'),
            'h' => (total, number, any) = (total + number * 3600, 0, true),
            'm' => (total, number, any) = (total + number * 60, 0, true),
            's' => (total, number, any) = (total + number, 0, true),
            _ => break,
        }
    }
    any.then_some(total)
}

pub fn quota_reset_at() -> Option<i64> {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
    for chat in chats(6) {
        let Some(path) = conversation_path(&chat.id).filter(|p| p.exists()) else { continue };
        let Ok(conn) =
            Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)
        else {
            continue;
        };
        let Ok(payload) =
            conn.query_row("SELECT step_payload FROM steps WHERE step_type = 17 ORDER BY idx DESC LIMIT 1", [], |r| {
                r.get::<_, Vec<u8>>(0)
            })
        else {
            continue;
        };
        let top = fields(&payload);
        let when = bytes_of(&top, 5)
            .and_then(|m| bytes_of(&fields(m), 1).and_then(|t| varint_of(&fields(t), 1)))
            .map_or(0, |t| t as i64);
        let err = fields(bytes_of(&fields(bytes_of(&top, 24).unwrap_or(&[])), 3).unwrap_or(&[]));
        if let Some(delay) = parse_reset_delay(&text_of(bytes_of(&err, 1)))
            && when > 0
            && when + delay > now
        {
            return Some(when + delay);
        }
    }
    None
}

#[cfg(test)]
mod history_tests {
    use super::*;

    fn field_bytes(num: u32, data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut key = (num << 3) | 2;
        while key >= 0x80 {
            out.push((key as u8 & 0x7F) | 0x80);
            key >>= 7;
        }
        out.push(key as u8);
        out.push(data.len() as u8);
        out.extend_from_slice(data);
        out
    }

    #[test]
    fn a_user_step_and_a_model_step_are_read() {
        let user = field_bytes(19, &field_bytes(2, b"Hello **there**"));
        let msgs = step_to_msgs(14, &user);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].kind, Kind::User);
        assert_eq!(msgs[0].doc.text, "Hello there");

        let model = field_bytes(20, &[field_bytes(1, b"OK"), field_bytes(3, b"thinking")].concat());
        let kinds: Vec<Kind> = step_to_msgs(15, &model).iter().map(|m| m.kind).collect();
        assert_eq!(kinds, [Kind::Thinking, Kind::Assistant]);
    }

    #[test]
    fn broken_protobuf_does_not_panic() {
        assert!(step_to_msgs(14, &[0xFF, 0xFF, 0xFF]).is_empty());
        assert!(step_to_msgs(15, &[0x0A, 0x80]).is_empty());
        assert!(step_to_msgs(999, b"whatever").is_empty());
    }

    #[test]
    fn the_quota_delay_is_read() {
        assert_eq!(parse_reset_delay("Individual quota reached. Resets in 1h38m15s."), Some(3600 + 38 * 60 + 15));
        assert_eq!(parse_reset_delay("... Resets in 42s"), Some(42));
        assert_eq!(parse_reset_delay("no delay here"), None);
    }

    #[test]
    fn a_real_conversation_can_be_read() {
        for chat in chats(2) {
            let _ = history(&chat.id);
        }
    }
}
