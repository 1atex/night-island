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

pub fn own_client_id() -> String {
    integrations::load().spotify_client_id
}

pub fn set_client_id(id: &str) -> bool {
    let id = id.trim();
    if id.len() > 64 || !id.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return false;
    }
    let mut c = integrations::load();
    c.spotify_client_id = id.to_string();
    integrations::save(&c);
    reload_status();
    true
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

fn api_json(method: &str, path: &str, json: &str) -> Option<http::Response> {
    let url = format!("https://api.spotify.com/v1{path}");
    let reply = http::request_json(method, &url, &access_token()?, json)?;
    if reply.status != 401 {
        return Some(reply);
    }
    if let Ok(mut t) = TOKEN.lock()
        && let Some(t) = t.as_mut()
    {
        t.expires = Instant::now();
    }
    http::request_json(method, &url, &access_token()?, json)
}

const ALL_KINDS: [&str; 4] = ["track", "artist", "album", "playlist"];
const GOOD_MATCH: f32 = 0.6;
const SEARCH_LIMIT: usize = 5;

pub fn kinds_of(word: &str) -> Vec<&'static str> {
    match word.trim().to_ascii_lowercase().as_str() {
        "track" | "song" => vec!["track"],
        "album" => vec!["album"],
        "artist" => vec!["artist"],
        "playlist" => vec!["playlist"],
        _ => ALL_KINDS.to_vec(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub kind: &'static str,
    pub uri: String,
    pub name: String,
    pub by: String,
    pub popularity: u32,
}

impl Candidate {
    pub fn describe(&self) -> String {
        if self.by.is_empty() {
            format!("{} ({})", self.name, self.kind)
        } else {
            format!("{} by {} ({})", self.name, self.by, self.kind)
        }
    }
}

pub fn candidates(body: &str) -> Vec<Candidate> {
    let Some(v) = parse_json(body) else { return Vec::new() };
    let mut out = Vec::new();
    for kind in ALL_KINDS {
        let Some(items) = v.pointer(&format!("/{kind}s/items")).and_then(|i| i.as_array()) else { continue };
        for item in items.iter().filter(|i| i.is_object()) {
            let Some(uri) = item.get("uri").and_then(|u| u.as_str()) else { continue };
            let valid = uri.starts_with("spotify:") && uri.bytes().all(|b| b.is_ascii_alphanumeric() || b == b':');
            if !valid {
                continue;
            }
            let text = |p: &str| item.pointer(p).and_then(|n| n.as_str()).unwrap_or("").to_string();
            let by = [text("/artists/0/name"), text("/owner/display_name")]
                .into_iter()
                .find(|s| !s.is_empty())
                .unwrap_or_default();
            out.push(Candidate {
                kind,
                uri: uri.to_string(),
                name: text("/name"),
                by,
                popularity: item.get("popularity").and_then(|p| p.as_u64()).unwrap_or(0) as u32,
            });
        }
    }
    out
}

pub fn normalize(text: &str) -> String {
    let lowered: String = text.to_lowercase().chars().map(|c| if c.is_alphanumeric() { c } else { ' ' }).collect();
    lowered.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn edit_distance(a: &[char], b: &[char]) -> usize {
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, &x) in a.iter().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, &y) in b.iter().enumerate() {
            let above = row[j + 1];
            row[j + 1] = (diagonal + usize::from(x != y)).min(row[j] + 1).min(above + 1);
            diagonal = above;
        }
    }
    row[b.len()]
}

pub fn similarity(a: &str, b: &str) -> f32 {
    let (a, b) = (normalize(a), normalize(b));
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    if a == b {
        return 1.0;
    }
    let (ca, cb): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let (short, long) = if ca.len() <= cb.len() { (&a, &b) } else { (&b, &a) };
    let (shorter, longer) = (ca.len().min(cb.len()), ca.len().max(cb.len()));
    let spaced = |s: &str| s.replace(' ', "");
    if spaced(&a) == spaced(&b) {
        return 0.97;
    }
    let contained = shorter >= 3 && long.contains(short.as_str());
    let by_edits = 1.0 - edit_distance(&ca, &cb) as f32 / longer as f32;
    if contained { by_edits.max(0.75 + 0.2 * shorter as f32 / longer as f32) } else { by_edits }
}

pub fn score(query: &str, artist: &str, c: &Candidate) -> f32 {
    let name = similarity(query, &c.name);
    let joined =
        similarity(query, &format!("{} {}", c.name, c.by)).max(similarity(query, &format!("{} {}", c.by, c.name)));
    let base = name.max(joined);
    if artist.trim().is_empty() || c.by.is_empty() {
        return base;
    }
    0.7 * base + 0.3 * similarity(artist, &c.by)
}

#[derive(Debug, PartialEq)]
pub struct Choice {
    pub best: Option<Candidate>,
    pub close: Vec<Candidate>,
}

pub fn choose(found: &[Candidate], query: &str, artist: &str, kinds: &[&str]) -> Choice {
    let mut scored: Vec<(f32, &Candidate)> =
        found.iter().filter(|c| kinds.contains(&c.kind)).map(|c| (score(query, artist, c), c)).collect();
    scored.sort_by(|x, y| {
        let (a, b) = (x.0, y.0);
        if (a - b).abs() < 0.05 { y.1.popularity.cmp(&x.1.popularity) } else { b.total_cmp(&a) }
    });
    let best = scored.first().filter(|(s, _)| *s >= GOOD_MATCH).map(|(_, c)| (*c).clone());
    let close = scored.iter().skip(usize::from(best.is_some())).take(3).map(|(_, c)| (*c).clone()).collect();
    Choice { best, close }
}

pub fn play_body(kind: &str, uri: &str) -> String {
    if kind == "track" {
        serde_json::json!({ "uris": [uri] }).to_string()
    } else {
        serde_json::json!({ "context_uri": uri }).to_string()
    }
}

pub fn pick_device(body: &str) -> Option<String> {
    let v = parse_json(body)?;
    let devices = v.get("devices")?.as_array()?;
    let usable = |d: &&serde_json::Value| d.get("is_restricted").and_then(|r| r.as_bool()) != Some(true);
    let chosen = devices
        .iter()
        .filter(usable)
        .find(|d| d.get("is_active").and_then(|a| a.as_bool()) == Some(true))
        .or_else(|| devices.iter().filter(usable).find(|d| d.get("type").and_then(|t| t.as_str()) == Some("Computer")))
        .or_else(|| devices.iter().find(usable))?;
    let id = chosen.get("id")?.as_str()?;
    (!id.is_empty() && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')).then(|| id.to_string())
}

pub fn search_text(query: &str, artist: &str, kinds: &[&str]) -> String {
    let artist: String = artist.chars().filter(|c| !matches!(c, '"' | '\\') && !c.is_control()).collect();
    let artist = artist.trim();
    let narrows =
        !artist.is_empty() && !query.trim().is_empty() && kinds.iter().any(|k| matches!(*k, "track" | "album"));
    if narrows {
        format!("{} artist:\"{artist}\"", query.trim())
    } else {
        format!("{} {artist}", query.trim()).trim().to_string()
    }
}

pub fn play_search(query: &str, kind: &str, artist: &str) -> Result<String, String> {
    let kinds = kinds_of(kind);
    let query = if query.trim().is_empty() { artist.trim() } else { query.trim() };
    if query.is_empty() {
        return Err("Say what to play".into());
    }
    if state().status != Status::Connected {
        return Err("Spotify is not connected: connect it in the settings, Integrations".into());
    }
    let q = search_text(query, artist, &kinds);
    let reply = api("GET", &format!("/search?q={}&type={}&limit={SEARCH_LIMIT}", pct(&q), kinds.join(",")))
        .ok_or("No connection to Spotify")?;
    if reply.status != 200 {
        return Err(format!("Spotify search answered with status {}", reply.status));
    }
    let found = candidates(&reply.body);
    let choice = choose(&found, query, artist, &kinds);
    let Some(best) = choice.best else {
        let near: Vec<String> = choice.close.iter().map(Candidate::describe).collect();
        let hint = if near.is_empty() { String::new() } else { format!(" The closest were: {}.", near.join("; ")) };
        return Err(format!(
            "Nothing on Spotify matches \"{query}\" well.{hint} Try the Latin spelling or the artist field, or ask the user which one."
        ));
    };
    let device = api("GET", "/me/player/devices").filter(|r| r.status == 200).and_then(|r| pick_device(&r.body));
    let path = match &device {
        Some(id) => format!("/me/player/play?device_id={id}"),
        None => "/me/player/play".to_string(),
    };
    let reply = api_json("PUT", &path, &play_body(best.kind, &best.uri)).ok_or("No connection to Spotify")?;
    match reply.status {
        200..=299 => {
            refresh();
            let others: Vec<String> = choice.close.iter().map(Candidate::describe).collect();
            let more =
                if others.is_empty() { String::new() } else { format!(" Other matches: {}.", others.join("; ")) };
            Ok(format!("Playing {}.{more}", best.describe()))
        }
        404 => Err("Spotify has no device to play on: open Spotify on this computer, then try again".into()),
        403 => Err("Spotify refused: playback control needs Premium, or the device is restricted".into()),
        other => Err(format!("Spotify answered with status {other}")),
    }
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

    fn cand(kind: &'static str, name: &str, by: &str, popularity: u32) -> Candidate {
        Candidate { kind, uri: format!("spotify:{kind}:x"), name: name.into(), by: by.into(), popularity }
    }

    #[test]
    fn a_search_answer_gives_every_kind_and_skips_junk() {
        let body = r#"{"tracks":{"items":[{"uri":"spotify:track:4uLU6hMCjMI75M1A2tKUQC","name":"Never Gonna Give You Up","artists":[{"name":"Rick Astley"}],"popularity":80},null]},
            "artists":{"items":[{"uri":"spotify:artist:abc","name":"Rick Astley","popularity":70}]},
            "playlists":{"items":[{"uri":"spotify:playlist:37i9dQZF1DX","name":"Chill","owner":{"display_name":"Spotify"}},null]},
            "albums":{"items":[]}}"#;
        let found = candidates(body);
        assert_eq!(found.len(), 3);
        assert_eq!((found[0].kind, found[0].by.as_str(), found[0].popularity), ("track", "Rick Astley", 80));
        assert_eq!(found[1].describe(), "Rick Astley (artist)");
        assert_eq!(found[2].by, "Spotify", "a playlist is credited to its owner");
        assert!(candidates("not json").is_empty() && candidates(r#"{"tracks":{"items":[null]}}"#).is_empty());
        let evil = r#"{"tracks":{"items":[{"uri":"spotify:track:x\"}; calc","name":"a"}]}}"#;
        assert!(candidates(evil).is_empty());
    }

    #[test]
    fn a_track_plays_by_uri_and_everything_else_by_context() {
        assert_eq!(play_body("track", "spotify:track:abc"), r#"{"uris":["spotify:track:abc"]}"#);
        assert_eq!(play_body("album", "spotify:album:abc"), r#"{"context_uri":"spotify:album:abc"}"#);
        assert_eq!(kinds_of(" Album "), ["album"]);
        assert_eq!(kinds_of("song"), ["track"]);
        assert_eq!(kinds_of("PLAYLIST"), ["playlist"]);
        assert_eq!(kinds_of(""), ALL_KINDS);
        assert_eq!(kinds_of("anything"), ALL_KINDS);
    }

    #[test]
    fn names_are_compared_the_way_a_person_would() {
        assert_eq!(similarity("Never Love", "Neverlove"), 0.97);
        assert!(similarity("Neverlow", "Neverlove") > 0.7);
        assert!(similarity("Чёрство", "Черство") > 0.8, "e and yo are one letter to a listener");
        assert!(similarity("Never Love", "Mockingbird") < 0.3);
        assert!(similarity("сквозь баб", "ОЗЕРО В ЛЕСУ") < 0.3);
        assert!(similarity("Lido", "Lido Remix Sunshine") >= 0.75);
        assert_eq!(similarity("", "x"), 0.0);
        assert_eq!(normalize("  Ain't No   Sunshine - Lido Remix! "), "ain t no sunshine lido remix");
    }

    #[test]
    fn a_poor_match_is_refused_instead_of_playing_something_unrelated() {
        let found = vec![
            cand("track", "Mockingbird", "Eminem", 90),
            cand("track", "Luv Ni Love Storys", "Yashika Sikka", 10),
            cand("artist", "Lido", "", 60),
        ];
        let choice = choose(&found, "Never Love", "", &ALL_KINDS);
        assert!(choice.best.is_none(), "{choice:?}");
        assert_eq!(choice.close.len(), 3);
    }

    #[test]
    fn the_closest_name_wins_and_popularity_breaks_a_tie() {
        let found = vec![
            cand("track", "Neverlove", "Other", 5),
            cand("artist", "Neverlow", "", 55),
            cand("track", "Yellow", "Neverlow", 40),
        ];
        let best = choose(&found, "Neverlow", "", &ALL_KINDS).best.expect("a match");
        assert_eq!((best.kind, best.name.as_str()), ("artist", "Neverlow"));
        let tie = vec![cand("track", "Numb", "Linkin Park", 90), cand("track", "Numb", "Cover Band", 3)];
        assert_eq!(choose(&tie, "Numb", "", &["track"]).best.map(|c| c.by), Some("Linkin Park".into()));
        let asked_for_albums = choose(&found, "Neverlow", "", &["album"]);
        assert!(asked_for_albums.best.is_none() && asked_for_albums.close.is_empty());
    }

    #[test]
    fn an_artist_narrows_a_track_and_decides_between_equal_titles() {
        let found = vec![cand("track", "Numb", "Linkin Park", 90), cand("track", "Numb", "Cover Band", 95)];
        let best = choose(&found, "Numb", "Cover Band", &["track"]).best.expect("a match");
        assert_eq!(best.by, "Cover Band");
        assert_eq!(search_text("Numb", "Linkin Park", &["track"]), "Numb artist:\"Linkin Park\"");
        assert_eq!(search_text("Neverlow", "", &["artist"]), "Neverlow");
        assert_eq!(search_text("Chill", "Spotify", &["playlist"]), "Chill Spotify");
        assert_eq!(search_text("Numb", "Lin\"kin", &["track"]), "Numb artist:\"Linkin\"");
    }

    #[test]
    fn the_device_is_the_active_one_then_a_computer_then_any() {
        let list = |s: &str| format!(r#"{{"devices":[{s}]}}"#);
        let phone = r#"{"id":"p1","type":"Smartphone","is_active":false}"#;
        let pc = r#"{"id":"c1","type":"Computer","is_active":false}"#;
        let live = r#"{"id":"a1","type":"Speaker","is_active":true}"#;
        let locked = r#"{"id":"r1","type":"Computer","is_active":true,"is_restricted":true}"#;
        assert_eq!(pick_device(&list(&format!("{phone},{pc},{live}"))).as_deref(), Some("a1"));
        assert_eq!(pick_device(&list(&format!("{phone},{pc}"))).as_deref(), Some("c1"));
        assert_eq!(pick_device(&list(phone)).as_deref(), Some("p1"));
        assert_eq!(pick_device(&list(locked)), None);
        assert_eq!(pick_device(&list("")), None);
        assert_eq!(pick_device(&list(r#"{"id":"x y","is_active":true}"#)), None);
    }

    #[test]
    fn pkce_challenge_matches_the_rfc_example() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        assert_eq!(
            URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes())),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }
}
