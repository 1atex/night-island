use std::ffi::c_void;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Mutex;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::time::{Duration, Instant};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use sha2::{Digest, Sha256};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};

use crate::services::http::{self, pct};
use crate::services::integrations;

pub const WM_SPOTIFY: u32 = WM_APP + 14;

const SCOPES: &str = "user-read-playback-state user-modify-playback-state user-library-read user-library-modify";
const REDIRECT: &str = "http://127.0.0.1:8898/callback";
const PORT: u16 = 8898;
const SECRET_NAME: &str = "spotify";
const SIGN_IN_TIMEOUT: Duration = Duration::from_secs(180);

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Repeat {
    #[default]
    Off,
    Context,
    Track,
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub enum Status {
    #[default]
    NoClientId,
    Disconnected,
    Connecting,
    Connected,
    Error(String),
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct State {
    pub status: Status,
    pub shuffle: bool,
    pub repeat: Repeat,
    pub liked: bool,
    track_id: String,
}

struct Token {
    access: String,
    refresh: String,
    expires: Instant,
}

static STATE: Mutex<State> = Mutex::new(State {
    status: Status::NoClientId,
    shuffle: false,
    repeat: Repeat::Off,
    liked: false,
    track_id: String::new(),
});
static TOKEN: Mutex<Option<Token>> = Mutex::new(None);
static HWND_RAW: AtomicIsize = AtomicIsize::new(0);

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
            let _ = PostMessageW(Some(HWND(raw as *mut c_void)), WM_SPOTIFY, WPARAM(0), LPARAM(0));
        }
    }
}

const DEFAULT_CLIENT_ID: &str = "a53125518c654532b1be3259bc963303";

fn client_id() -> Option<String> {
    let configured = integrations::load().spotify_client_id.trim().to_string();
    let id = if configured.is_empty() { DEFAULT_CLIENT_ID.to_string() } else { configured };
    (!id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_alphanumeric())).then_some(id)
}

pub fn init(hwnd: HWND) {
    HWND_RAW.store(hwnd.0 as isize, Ordering::Relaxed);
    reload_status();
}

pub fn reload_status() {
    let has_id = client_id().is_some();
    let has_token = integrations::load_secret(SECRET_NAME).is_some();
    update(|s| {
        s.status = match (has_id, has_token) {
            (false, _) => Status::NoClientId,
            (true, true) => Status::Connected,
            (true, false) => Status::Disconnected,
        }
    });
    if has_id && has_token {
        refresh();
    }
}

pub fn disconnect() {
    integrations::delete_secret(SECRET_NAME);
    if let Ok(mut t) = TOKEN.lock() {
        *t = None;
    }
    update(|s| {
        *s = State {
            status: if client_id().is_some() { Status::Disconnected } else { Status::NoClientId },
            ..State::default()
        }
    });
}

fn parse_json(body: &str) -> Option<serde_json::Value> {
    serde_json::from_str(body).ok()
}

fn store_token(v: &serde_json::Value, previous_refresh: &str) -> Option<String> {
    let access = v.get("access_token")?.as_str()?.to_string();
    let refresh = v.get("refresh_token").and_then(|r| r.as_str()).unwrap_or(previous_refresh).to_string();
    let secs = v.get("expires_in").and_then(|e| e.as_u64()).unwrap_or(3600);
    integrations::save_secret(SECRET_NAME, &refresh);
    if let Ok(mut t) = TOKEN.lock() {
        *t = Some(Token {
            access: access.clone(),
            refresh,
            expires: Instant::now() + Duration::from_secs(secs.saturating_sub(60)),
        });
    }
    Some(access)
}

fn valid_token() -> Option<String> {
    let guard = TOKEN.lock().ok()?;
    guard.as_ref().filter(|t| Instant::now() < t.expires).map(|t| t.access.clone())
}

static REFRESH_LOCK: Mutex<()> = Mutex::new(());

fn access_token() -> Option<String> {
    if let Some(t) = valid_token() {
        return Some(t);
    }
    let _only_one = REFRESH_LOCK.lock().ok()?;
    if let Some(t) = valid_token() {
        return Some(t);
    }
    let refresh = TOKEN.lock().ok()?.as_ref().map(|t| t.refresh.clone());
    let refresh = refresh.or_else(|| integrations::load_secret(SECRET_NAME))?;
    let id = client_id()?;
    let body = format!("grant_type=refresh_token&refresh_token={}&client_id={}", pct(&refresh), pct(&id));
    let reply = http::request("POST", "https://accounts.spotify.com/api/token", None, Some(&body))?;
    if reply.status == 400 && reply.body.contains("invalid_grant") {
        disconnect();
        update(|s| s.status = Status::Error("sign-in expired, connect again".into()));
        return None;
    }
    if reply.status != 200 {
        return None;
    }
    store_token(&parse_json(&reply.body)?, &refresh)
}

pub fn connect() {
    std::thread::spawn(|| {
        let Some(id) = client_id() else {
            return update(|s| s.status = Status::NoClientId);
        };
        update(|s| s.status = Status::Connecting);
        let fail = |why: &str| update(|s| s.status = Status::Error(why.to_string()));

        let verifier = URL_SAFE_NO_PAD.encode(integrations::random_bytes(48));
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        let nonce = URL_SAFE_NO_PAD.encode(integrations::random_bytes(16));
        let Ok(listener) = TcpListener::bind(("127.0.0.1", PORT)) else {
            return fail("port 8898 is busy");
        };
        let _ = listener.set_nonblocking(true);
        let url = format!(
            "https://accounts.spotify.com/authorize?client_id={}&response_type=code&redirect_uri={}&code_challenge_method=S256&code_challenge={}&state={}&scope={}",
            pct(&id),
            pct(REDIRECT),
            challenge,
            nonce,
            pct(SCOPES)
        );
        crate::tools::cmd::shell_open(&url, None);

        let started = Instant::now();
        let query = loop {
            if started.elapsed() > SIGN_IN_TIMEOUT {
                return fail("sign-in timed out");
            }
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let _ = stream.set_nonblocking(false);
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                    let mut buf = [0u8; 8192];
                    let n = stream.read(&mut buf).unwrap_or(0);
                    let head = String::from_utf8_lossy(&buf[..n]).into_owned();
                    let line = head.lines().next().unwrap_or("");
                    let target = line.split_whitespace().nth(1).unwrap_or("");
                    let ok = target.starts_with("/callback?");
                    let page =
                        if ok { "Night Island is connected to Spotify. You can close this tab." } else { "Not found." };
                    let _ = write!(
                        stream,
                        "HTTP/1.1 {} X\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{page}",
                        if ok { 200 } else { 404 },
                        page.len()
                    );
                    if ok {
                        break target["/callback?".len()..].to_string();
                    }
                }
                Err(_) => std::thread::sleep(Duration::from_millis(100)),
            }
        };
        let param = |name: &str| {
            query.split('&').find_map(|kv| kv.strip_prefix(name).and_then(|r| r.strip_prefix('=')).map(str::to_string))
        };
        if param("state").as_deref() != Some(nonce.as_str()) {
            return fail("sign-in was rejected");
        }
        let Some(code) = param("code") else { return fail("access was denied") };
        let body = format!(
            "grant_type=authorization_code&code={}&redirect_uri={}&client_id={}&code_verifier={}",
            code,
            pct(REDIRECT),
            pct(&id),
            verifier
        );
        let Some(reply) = http::request("POST", "https://accounts.spotify.com/api/token", None, Some(&body)) else {
            return fail("no connection");
        };
        match parse_json(&reply.body).and_then(|v| store_token(&v, "")) {
            Some(_) => {
                update(|s| s.status = Status::Connected);
                refresh();
            }
            None => fail("could not get a token"),
        }
    });
}

fn api(method: &str, path: &str) -> Option<http::Response> {
    let url = format!("https://api.spotify.com/v1{path}");
    let token = access_token()?;
    let reply = http::request(method, &url, Some(&token), None)?;
    if reply.status != 401 {
        return Some(reply);
    }
    if let Ok(mut t) = TOKEN.lock()
        && let Some(t) = t.as_mut()
    {
        t.expires = Instant::now();
    }
    http::request(method, &url, Some(&access_token()?), None)
}

fn contains(reply: Option<http::Response>) -> Option<bool> {
    let reply = reply.filter(|r| r.status == 200)?;
    parse_json(&reply.body)?.get(0)?.as_bool()
}

fn check_liked(id: &str) -> Option<bool> {
    contains(api("GET", &format!("/me/tracks/contains?ids={id}")))
        .or_else(|| contains(api("GET", &format!("/me/library/contains?uris=spotify%3Atrack%3A{id}"))))
}

fn save_liked(id: &str, liked: bool) -> bool {
    let method = if liked { "PUT" } else { "DELETE" };
    let ok = |r: Option<http::Response>| r.is_some_and(|r| (200..300).contains(&r.status));
    ok(api(method, &format!("/me/tracks?ids={id}")))
        || ok(api(method, &format!("/me/library?uris=spotify%3Atrack%3A{id}")))
}

pub fn refresh() {
    std::thread::spawn(|| {
        let Some(reply) = api("GET", "/me/player") else { return };
        if reply.status == 204 || reply.status != 200 {
            return update(|s| {
                s.track_id.clear();
                s.liked = false;
            });
        }
        let Some(v) = parse_json(&reply.body) else { return };
        let shuffle = v.get("shuffle_state").and_then(|b| b.as_bool()).unwrap_or(false);
        let repeat = match v.get("repeat_state").and_then(|r| r.as_str()) {
            Some("context") => Repeat::Context,
            Some("track") => Repeat::Track,
            _ => Repeat::Off,
        };
        let id = v.pointer("/item/id").and_then(|i| i.as_str()).unwrap_or("").to_string();
        let valid = !id.is_empty() && id.bytes().all(|b| b.is_ascii_alphanumeric());
        let liked = if valid { check_liked(&id) } else { Some(false) };
        update(|s| {
            s.shuffle = shuffle;
            s.repeat = repeat;
            s.liked = match liked {
                Some(l) => l,
                None if s.track_id == id => s.liked,
                None => false,
            };
            s.track_id = id;
        });
    });
}

pub fn toggle_shuffle() {
    let on = !state().shuffle;
    update(|s| s.shuffle = on);
    std::thread::spawn(move || {
        api("PUT", &format!("/me/player/shuffle?state={on}"));
        refresh();
    });
}

pub fn cycle_repeat() {
    let next = match state().repeat {
        Repeat::Off => ("context", Repeat::Context),
        Repeat::Context => ("track", Repeat::Track),
        Repeat::Track => ("off", Repeat::Off),
    };
    update(|s| s.repeat = next.1);
    std::thread::spawn(move || {
        api("PUT", &format!("/me/player/repeat?state={}", next.0));
        refresh();
    });
}

pub fn toggle_like() {
    let st = state();
    if st.track_id.is_empty() || !st.track_id.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return;
    }
    let (id, liked) = (st.track_id, !st.liked);
    update(|s| s.liked = liked);
    std::thread::spawn(move || {
        if !save_liked(&id, liked) {
            update(|s| s.liked = !liked);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_challenge_matches_the_rfc_example() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        assert_eq!(
            URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes())),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }
}
