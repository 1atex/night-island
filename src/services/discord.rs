use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::{Mutex, mpsc};
use std::time::Duration;

use serde_json::{Value, json};
use windows::Win32::Foundation::{
    CloseHandle, ERROR_IO_PENDING, GENERIC_READ, GENERIC_WRITE, HANDLE, HWND, LPARAM, WAIT_OBJECT_0, WPARAM,
};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAG_OVERLAPPED, FILE_SHARE_NONE, OPEN_EXISTING, ReadFile, WriteFile,
};
use windows::Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED};
use windows::Win32::System::Threading::{
    CreateEventW, INFINITE, ResetEvent, SetEvent, WaitForMultipleObjects, WaitForSingleObject,
};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};
use windows::core::{HSTRING, PCWSTR};

use crate::services::http::{self, pct};
use crate::services::integrations;

pub const WM_DISCORD: u32 = WM_APP + 15;

const SECRET_NAME: &str = "discord";
const REDIRECT: &str = "http://127.0.0.1";
const MAX_FRAME: u32 = 2 * 1024 * 1024;
const RETRY: Duration = Duration::from_secs(10);

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub enum Status {
    #[default]
    NoConfig,
    Idle,
    NotRunning,
    Authorizing,
    Connected,
    Error(String),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Member {
    pub name: String,
    pub muted: bool,
    pub deaf: bool,
    pub speaking: bool,
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct State {
    pub status: Status,
    pub channel: String,
    pub members: Vec<Member>,
    pub self_mute: bool,
    pub self_deaf: bool,
    pub note: (String, String),
    pub note_seq: u32,
}

static STATE: Mutex<State> = Mutex::new(State {
    status: Status::NoConfig,
    channel: String::new(),
    members: Vec::new(),
    self_mute: false,
    self_deaf: false,
    note: (String::new(), String::new()),
    note_seq: 0,
});
static HWND_RAW: AtomicIsize = AtomicIsize::new(0);
static CMD_EVENT: AtomicIsize = AtomicIsize::new(0);
static OUTBOX: Mutex<Option<mpsc::Sender<(u32, String)>>> = Mutex::new(None);
static WAKE: Mutex<Option<mpsc::Sender<()>>> = Mutex::new(None);
static WANT_SIGN_IN: AtomicBool = AtomicBool::new(false);
static SIGN_OUT: AtomicBool = AtomicBool::new(false);

pub fn credentials_look_valid(id: &str, secret: &str) -> bool {
    valid_client_id(id.trim()) && !secret.trim().is_empty() && secret.trim().len() <= 128
}

pub fn set_credentials(id: &str, secret: &str) -> bool {
    if !credentials_look_valid(id, secret) {
        return false;
    }
    let mut c = integrations::load();
    c.discord_client_id = id.trim().to_string();
    c.discord_client_secret = secret.trim().to_string();
    integrations::save(&c);
    update(|s| s.status = Status::Idle);
    wake();
    true
}

pub fn state() -> State {
    STATE.lock().map(|g| g.clone()).unwrap_or_default()
}

fn update(f: impl FnOnce(&mut State)) {
    if let Ok(mut g) = STATE.lock() {
        f(&mut g);
    }
    let raw = HWND_RAW.load(Ordering::Relaxed);
    if raw != 0 {
        unsafe {
            let _ = PostMessageW(Some(HWND(raw as *mut c_void)), WM_DISCORD, WPARAM(0), LPARAM(0));
        }
    }
}

fn wake() {
    if let Ok(g) = WAKE.lock()
        && let Some(tx) = g.as_ref()
    {
        let _ = tx.send(());
    }
}

fn frame(op: u32, json: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + json.len());
    out.extend_from_slice(&op.to_le_bytes());
    out.extend_from_slice(&(json.len() as u32).to_le_bytes());
    out.extend_from_slice(json.as_bytes());
    out
}

fn header(h: &[u8]) -> (u32, u32) {
    (u32::from_le_bytes([h[0], h[1], h[2], h[3]]), u32::from_le_bytes([h[4], h[5], h[6], h[7]]))
}

fn valid_client_id(id: &str) -> bool {
    (15..=22).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_digit())
}

fn configured() -> Option<(String, String)> {
    let c = integrations::load();
    let (id, secret) = (c.discord_client_id.trim().to_string(), c.discord_client_secret.trim().to_string());
    (valid_client_id(&id) && !secret.is_empty() && secret.len() <= 128).then_some((id, secret))
}

fn parse_channel(data: &Value, speaking: &HashSet<String>) -> (String, Vec<Member>) {
    if data.is_null() {
        return (String::new(), Vec::new());
    }
    let name = data.get("name").and_then(|n| n.as_str()).unwrap_or("").chars().take(60).collect();
    let members = data
        .get("voice_states")
        .and_then(|v| v.as_array())
        .map(|list| {
            list.iter()
                .map(|vs| {
                    let user = &vs["user"];
                    let text = |v: &Value| v.as_str().filter(|s| !s.is_empty()).map(str::to_string);
                    let name = text(&vs["nick"])
                        .or_else(|| text(&user["global_name"]))
                        .or_else(|| text(&user["username"]))
                        .unwrap_or_else(|| "?".into());
                    let flag =
                        |k: &str| vs["voice_state"][k].as_bool().unwrap_or(false) || vs[k].as_bool().unwrap_or(false);
                    let id = user["id"].as_str().unwrap_or("");
                    Member {
                        name: name.chars().take(40).collect(),
                        muted: flag("mute") || flag("self_mute"),
                        deaf: flag("deaf") || flag("self_deaf"),
                        speaking: speaking.contains(id),
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    (name, members)
}

struct Link {
    handle: HANDLE,
    ev_read: HANDLE,
    ev_cmd: HANDLE,
    ev_write: HANDLE,
}

impl Link {
    fn open() -> Option<Link> {
        unsafe {
            for n in 0..10 {
                let path = HSTRING::from(format!(r"\\.\pipe\discord-ipc-{n}"));
                let Ok(handle) = CreateFileW(
                    PCWSTR(path.as_ptr()),
                    (GENERIC_READ | GENERIC_WRITE).0,
                    FILE_SHARE_NONE,
                    None,
                    OPEN_EXISTING,
                    FILE_FLAG_OVERLAPPED,
                    None,
                ) else {
                    continue;
                };
                let events = (
                    CreateEventW(None, true, false, None),
                    CreateEventW(None, true, false, None),
                    CreateEventW(None, true, false, None),
                );
                let (Ok(ev_read), Ok(ev_cmd), Ok(ev_write)) = events else {
                    let _ = CloseHandle(handle);
                    return None;
                };
                return Some(Link { handle, ev_read, ev_cmd, ev_write });
            }
        }
        None
    }

    fn write_all(&self, bytes: &[u8]) -> Result<(), String> {
        unsafe {
            let mut ov = OVERLAPPED { hEvent: self.ev_write, ..Default::default() };
            let _ = ResetEvent(self.ev_write);
            match WriteFile(self.handle, Some(bytes), None, Some(&mut ov)) {
                Ok(()) => {}
                Err(e) if e.code() == ERROR_IO_PENDING.to_hresult() => {}
                Err(e) => return Err(e.to_string()),
            }
            WaitForSingleObject(self.ev_write, INFINITE);
            let mut written = 0u32;
            GetOverlappedResult(self.handle, &ov, &mut written, false).map_err(|e| e.to_string())?;
            if written as usize != bytes.len() {
                return Err("short write".into());
            }
        }
        Ok(())
    }
}

impl Drop for Link {
    fn drop(&mut self) {
        unsafe {
            let _ = CancelIoEx(self.handle, None);
            let _ = CloseHandle(self.handle);
            for e in [self.ev_read, self.ev_cmd, self.ev_write] {
                let _ = CloseHandle(e);
            }
        }
        CMD_EVENT.store(0, Ordering::Relaxed);
    }
}

fn send(op: u32, json: String) {
    if let Ok(g) = OUTBOX.lock()
        && let Some(tx) = g.as_ref()
    {
        let _ = tx.send((op, json));
    }
    let ev = CMD_EVENT.load(Ordering::Relaxed);
    if ev != 0 {
        unsafe {
            let _ = SetEvent(HANDLE(ev as *mut c_void));
        }
    }
}

struct Session {
    client_id: String,
    secret: String,
    nonce: u64,
    speaking: HashSet<String>,
    subscribed: HashMap<&'static str, String>,
    tier: u8,
    abort: Option<String>,
}

const CHANNEL_EVENTS: [&str; 3] = ["VOICE_STATE_CREATE", "VOICE_STATE_UPDATE", "VOICE_STATE_DELETE"];

impl Session {
    fn cmd(&mut self, cmd: &str, evt: Option<&str>, args: Value) {
        self.nonce += 1;
        let mut msg = json!({ "cmd": cmd, "args": args, "nonce": self.nonce.to_string() });
        if let Some(e) = evt {
            msg["evt"] = json!(e);
        }
        send(1, msg.to_string());
    }

    fn authorize(&mut self) {
        let mut scopes = vec!["rpc", "rpc.voice.read", "rpc.voice.write", "identify"];
        if self.tier == 0 {
            scopes.push("rpc.notifications.read");
        }
        update(|s| s.status = Status::Authorizing);
        let id = self.client_id.clone();
        self.cmd("AUTHORIZE", None, json!({ "client_id": id, "scopes": scopes }));
    }

    fn token_request(&self, body: String) -> Result<Value, bool> {
        let reply =
            http::request("POST", "https://discord.com/api/v10/oauth2/token", None, Some(&body)).ok_or(false)?;
        if (400..500).contains(&reply.status) {
            return Err(true);
        }
        let v: Value = serde_json::from_str(&reply.body).map_err(|_| false)?;
        if reply.status == 200 { Ok(v) } else { Err(false) }
    }

    fn accept_tokens(&mut self, v: &Value) -> bool {
        let (Some(access), Some(refresh)) = (v["access_token"].as_str(), v["refresh_token"].as_str()) else {
            return false;
        };
        integrations::save_secret(SECRET_NAME, refresh);
        self.cmd("AUTHENTICATE", None, json!({ "access_token": access }));
        true
    }

    fn on_ready(&mut self) {
        let refresh = integrations::load_secret(SECRET_NAME);
        match refresh {
            Some(r) => {
                let body = format!(
                    "grant_type=refresh_token&refresh_token={}&client_id={}&client_secret={}",
                    pct(&r),
                    pct(&self.client_id),
                    pct(&self.secret)
                );
                match self.token_request(body) {
                    Ok(v) if self.accept_tokens(&v) => {}
                    Err(false) => {
                        self.abort = Some("no connection".into());
                    }
                    _ => {
                        integrations::delete_secret(SECRET_NAME);
                        if WANT_SIGN_IN.load(Ordering::Relaxed) {
                            self.authorize();
                        } else {
                            update(|s| s.status = Status::Error("sign-in expired, connect again".into()));
                        }
                    }
                }
            }
            None if WANT_SIGN_IN.load(Ordering::Relaxed) => self.authorize(),
            None => update(|s| s.status = Status::Idle),
        }
    }

    fn after_login(&mut self) {
        WANT_SIGN_IN.store(false, Ordering::Relaxed);
        update(|s| s.status = Status::Connected);
        self.cmd("SUBSCRIBE", Some("VOICE_CHANNEL_SELECT"), json!({}));
        self.cmd("SUBSCRIBE", Some("VOICE_SETTINGS_UPDATE"), json!({}));
        self.cmd("SUBSCRIBE", Some("NOTIFICATION_CREATE"), json!({}));
        self.cmd("GET_VOICE_SETTINGS", None, json!({}));
        self.cmd("GET_SELECTED_VOICE_CHANNEL", None, json!({}));
    }

    fn follow_channel(&mut self, channel_id: Option<&str>) {
        let old: Vec<(&'static str, String)> = self.subscribed.drain().collect();
        for (evt, id) in old {
            self.cmd("UNSUBSCRIBE", Some(evt), json!({ "channel_id": id }));
        }
        let Some(id) = channel_id.filter(|i| !i.is_empty()) else { return };
        for evt in CHANNEL_EVENTS.into_iter().chain(["SPEAKING_START", "SPEAKING_STOP"]) {
            self.subscribed.insert(evt, id.to_string());
            self.cmd("SUBSCRIBE", Some(evt), json!({ "channel_id": id }));
        }
    }

    fn on_message(&mut self, v: &Value) {
        let cmd = v["cmd"].as_str().unwrap_or("");
        let evt = v["evt"].as_str().unwrap_or("");
        let data = &v["data"];
        if evt == "ERROR" {
            let message = data["message"].as_str().unwrap_or("error").chars().take(80).collect::<String>();
            match cmd {
                "AUTHORIZE" if self.tier < 1 => {
                    self.tier += 1;
                    self.authorize();
                }
                "AUTHORIZE" | "AUTHENTICATE" => {
                    WANT_SIGN_IN.store(false, Ordering::Relaxed);
                    integrations::delete_secret(SECRET_NAME);
                    update(|s| s.status = Status::Error(message));
                }
                _ => {}
            }
            return;
        }
        match (cmd, evt) {
            (_, "READY") => self.on_ready(),
            ("AUTHORIZE", _) => {
                let Some(code) = data["code"].as_str() else { return };
                let body = format!(
                    "grant_type=authorization_code&code={}&client_id={}&client_secret={}&redirect_uri={}",
                    pct(code),
                    pct(&self.client_id),
                    pct(&self.secret),
                    pct(REDIRECT)
                );
                match self.token_request(body) {
                    Ok(t) if self.accept_tokens(&t) => {}
                    _ => {
                        WANT_SIGN_IN.store(false, Ordering::Relaxed);
                        update(|s| s.status = Status::Error("token exchange failed (check the redirect URI)".into()));
                    }
                }
            }
            ("AUTHENTICATE", _) => self.after_login(),
            ("GET_VOICE_SETTINGS", _) | (_, "VOICE_SETTINGS_UPDATE") => {
                let (mute, deaf) = (data["mute"].as_bool().unwrap_or(false), data["deaf"].as_bool().unwrap_or(false));
                update(|s| {
                    s.self_mute = mute;
                    s.self_deaf = deaf;
                });
            }
            ("GET_SELECTED_VOICE_CHANNEL", _) => {
                let (name, members) = parse_channel(data, &self.speaking);
                if name.is_empty() {
                    self.speaking.clear();
                }
                let id = data["id"].as_str().map(str::to_string);
                let changed = self.subscribed.values().next().map(String::as_str) != id.as_deref();
                update(|s| {
                    s.channel = name;
                    s.members = members;
                });
                if changed {
                    self.follow_channel(id.as_deref());
                }
            }
            (_, "VOICE_CHANNEL_SELECT") => self.cmd("GET_SELECTED_VOICE_CHANNEL", None, json!({})),
            (_, "VOICE_STATE_CREATE" | "VOICE_STATE_UPDATE" | "VOICE_STATE_DELETE") => {
                self.cmd("GET_SELECTED_VOICE_CHANNEL", None, json!({}));
            }
            (_, "SPEAKING_START" | "SPEAKING_STOP") => {
                let Some(user) = data["user_id"].as_str() else { return };
                if evt == "SPEAKING_START" {
                    self.speaking.insert(user.to_string());
                } else {
                    self.speaking.remove(user);
                }
                self.cmd("GET_SELECTED_VOICE_CHANNEL", None, json!({}));
            }
            (_, "NOTIFICATION_CREATE") => {
                let clip = |s: &str, n: usize| s.chars().take(n).collect::<String>();
                let title = clip(data["title"].as_str().unwrap_or("Discord"), 40);
                let body = clip(data["body"].as_str().unwrap_or(""), 120);
                update(|s| {
                    s.note = (title, body);
                    s.note_seq = s.note_seq.wrapping_add(1);
                });
            }
            _ => {}
        }
    }
}

fn run_session(
    link: Link,
    rx: &mpsc::Receiver<(u32, String)>,
    client_id: String,
    secret: String,
) -> Result<(), String> {
    CMD_EVENT.store(link.ev_cmd.0 as isize, Ordering::Relaxed);
    let mut session = Session {
        client_id: client_id.clone(),
        secret,
        nonce: 0,
        speaking: HashSet::new(),
        subscribed: HashMap::new(),
        tier: 0,
        abort: None,
    };
    link.write_all(&frame(0, &json!({ "v": 1, "client_id": client_id }).to_string()))?;

    let mut buf = vec![0u8; 8];
    let mut filled = 0usize;
    let mut in_header = true;
    let mut op = 0u32;
    let mut ov = Box::new(OVERLAPPED { hEvent: link.ev_read, ..Default::default() });
    let issue = |buf: &mut [u8], ov: &mut OVERLAPPED| -> Result<(), String> {
        unsafe {
            let _ = ResetEvent(link.ev_read);
            *ov = OVERLAPPED { hEvent: link.ev_read, ..Default::default() };
            match ReadFile(link.handle, Some(buf), None, Some(ov)) {
                Ok(()) => Ok(()),
                Err(e) if e.code() == ERROR_IO_PENDING.to_hresult() => Ok(()),
                Err(e) => Err(e.to_string()),
            }
        }
    };
    issue(&mut buf[filled..], &mut ov)?;
    loop {
        let r = unsafe { WaitForMultipleObjects(&[link.ev_read, link.ev_cmd], false, INFINITE) };
        match r.0.wrapping_sub(WAIT_OBJECT_0.0) {
            0 => {
                let mut n = 0u32;
                unsafe { GetOverlappedResult(link.handle, &*ov, &mut n, false) }.map_err(|e| e.to_string())?;
                filled += n as usize;
                if filled < buf.len() {
                    issue(&mut buf[filled..], &mut ov)?;
                    continue;
                }
                if in_header {
                    let (o, len) = header(&buf);
                    if len > MAX_FRAME {
                        return Err("oversized frame".into());
                    }
                    op = o;
                    if len > 0 {
                        buf = vec![0u8; len as usize];
                        filled = 0;
                        in_header = false;
                        issue(&mut buf, &mut ov)?;
                        continue;
                    }
                    buf = vec![0u8; 8];
                    filled = 0;
                    issue(&mut buf, &mut ov)?;
                    continue;
                }
                let body = String::from_utf8_lossy(&buf).into_owned();
                match op {
                    1 => {
                        if let Ok(v) = serde_json::from_str::<Value>(&body) {
                            session.on_message(&v);
                        }
                        if let Some(why) = session.abort.take() {
                            return Err(why);
                        }
                    }
                    2 => return Err("Discord closed the connection".into()),
                    3 => link.write_all(&frame(4, &body))?,
                    _ => {}
                }
                buf = vec![0u8; 8];
                filled = 0;
                in_header = true;
                issue(&mut buf, &mut ov)?;
            }
            1 => {
                unsafe {
                    let _ = ResetEvent(link.ev_cmd);
                }
                while let Ok((op, text)) = rx.try_recv() {
                    link.write_all(&frame(op, &text))?;
                }
                if SIGN_OUT.swap(false, Ordering::Relaxed) {
                    return Ok(());
                }
            }
            _ => return Err("wait failed".into()),
        }
    }
}

pub fn start(hwnd: HWND) {
    HWND_RAW.store(hwnd.0 as isize, Ordering::Relaxed);
    let (out_tx, out_rx) = mpsc::channel();
    let (wake_tx, wake_rx) = mpsc::channel::<()>();
    if let (Ok(mut o), Ok(mut w)) = (OUTBOX.lock(), WAKE.lock()) {
        *o = Some(out_tx);
        *w = Some(wake_tx);
    }
    std::thread::spawn(move || {
        loop {
            let wait = match configured() {
                None => {
                    update(|s| s.status = Status::NoConfig);
                    Duration::from_secs(60)
                }
                Some((id, secret)) => {
                    let has_token = integrations::load_secret(SECRET_NAME).is_some();
                    if !has_token && !WANT_SIGN_IN.load(Ordering::Relaxed) {
                        update(|s| s.status = Status::Idle);
                        Duration::from_secs(3600)
                    } else {
                        match Link::open() {
                            None => {
                                update(|s| {
                                    s.status = if has_token {
                                        Status::NotRunning
                                    } else {
                                        Status::Error("Discord is not running".into())
                                    }
                                });
                                if !has_token {
                                    WANT_SIGN_IN.store(false, Ordering::Relaxed);
                                }
                                RETRY
                            }
                            Some(link) => {
                                while out_rx.try_recv().is_ok() {}
                                let result = run_session(link, &out_rx, id, secret);
                                update(|s| {
                                    s.channel.clear();
                                    s.members.clear();
                                    s.status = match &result {
                                        Err(e)
                                            if integrations::load_secret(SECRET_NAME).is_some()
                                                && s.status == Status::Connected =>
                                        {
                                            let _ = e;
                                            Status::NotRunning
                                        }
                                        Err(e) if s.status == Status::Authorizing => Status::Error(e.clone()),
                                        _ if integrations::load_secret(SECRET_NAME).is_some() => Status::NotRunning,
                                        _ => Status::Idle,
                                    };
                                });
                                RETRY
                            }
                        }
                    }
                }
            };
            if wake_rx.recv_timeout(wait).is_err_and(|e| e == mpsc::RecvTimeoutError::Disconnected) {
                return;
            }
        }
    });
}

pub fn connect() {
    WANT_SIGN_IN.store(true, Ordering::Relaxed);
    wake();
}

pub fn disconnect() {
    WANT_SIGN_IN.store(false, Ordering::Relaxed);
    integrations::delete_secret(SECRET_NAME);
    SIGN_OUT.store(true, Ordering::Relaxed);
    send(1, json!({ "cmd": "GET_VOICE_SETTINGS", "args": {}, "nonce": "bye" }).to_string());
    update(|s| {
        *s = State { status: if configured().is_some() { Status::Idle } else { Status::NoConfig }, ..State::default() }
    });
}

pub fn set_mute(on: bool) {
    update(|s| s.self_mute = on);
    send(1, json!({ "cmd": "SET_VOICE_SETTINGS", "args": { "mute": on }, "nonce": "mute" }).to_string());
}

pub fn set_deaf(on: bool) {
    update(|s| s.self_deaf = on);
    send(1, json!({ "cmd": "SET_VOICE_SETTINGS", "args": { "deaf": on }, "nonce": "deaf" }).to_string());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_are_little_endian_with_length() {
        let f = frame(1, "{}");
        assert_eq!(&f[..8], &[1, 0, 0, 0, 2, 0, 0, 0]);
        assert_eq!(header(&f[..8]), (1, 2));
        assert_eq!(&f[8..], b"{}");
    }

    #[test]
    fn client_ids_are_snowflakes() {
        assert!(valid_client_id("1234567890123456789"));
        assert!(!valid_client_id("a53125518c654532b1be3259bc963303"));
        assert!(!valid_client_id(""));
    }

    #[test]
    fn channel_answer_is_parsed_with_names_and_flags() {
        let v: Value = serde_json::from_str(
            r#"{"id":"1","name":"General","voice_states":[
                {"nick":"Ann","voice_state":{"self_mute":true,"deaf":false},"user":{"id":"10","username":"ann1"}},
                {"voice_state":{"mute":false,"deaf":true},"user":{"id":"11","username":"bob","global_name":"Bob"}}]}"#,
        )
        .unwrap();
        let speaking: HashSet<String> = ["11".to_string()].into();
        let (name, members) = parse_channel(&v, &speaking);
        assert_eq!(name, "General");
        assert_eq!(members.len(), 2);
        assert_eq!((members[0].name.as_str(), members[0].muted, members[0].speaking), ("Ann", true, false));
        assert_eq!((members[1].name.as_str(), members[1].deaf, members[1].speaking), ("Bob", true, true));
        assert_eq!(parse_channel(&Value::Null, &speaking).0, "");
    }
}
