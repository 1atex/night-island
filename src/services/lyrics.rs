use std::ffi::c_void;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};

use crate::services::http;

pub const WM_LYRICS: u32 = WM_APP + 19;

const BASE: &str = "https://lrclib.net";
const DEBOUNCE: Duration = Duration::from_millis(500);
const MAX_LINES: usize = 2000;
const MAX_LINE_CHARS: usize = 160;
const MAX_SOURCE_LINES: usize = 6000;
const MAX_CANDIDATES: usize = 20;
const CACHE_TRACKS: usize = 24;
const USER_AGENT: &str = "NightIsland/0.1 (https://gitlab.night-watchers.com/Shurik24/night-island)";
const RETRY_WAITS: [Duration; 5] = [
    Duration::from_secs(2),
    Duration::from_secs(5),
    Duration::from_secs(10),
    Duration::from_secs(30),
    Duration::from_secs(60),
];
const DISK_FILES_MAX: usize = 600;
const DISK_FILE_MAX_BYTES: u64 = 200_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    pub ms: u32,
    pub text: String,
}

pub type Lines = Arc<Vec<Line>>;
type Key = (String, String);

enum Found {
    Lines(Lines),
    Missing,
    Offline,
}

static CACHE: Mutex<Vec<(Key, Option<Lines>)>> = Mutex::new(Vec::new());
static LATEST: AtomicU64 = AtomicU64::new(0);

pub fn cached(title: &str, artist: &str) -> Option<Option<Lines>> {
    let cache = CACHE.lock().ok()?;
    cache.iter().find(|((t, a), _)| t == title && a == artist).map(|(_, l)| l.clone())
}

fn remember(key: Key, lines: Option<Lines>) {
    if let Ok(mut cache) = CACHE.lock() {
        cache.retain(|(k, _)| *k != key);
        if cache.len() >= CACHE_TRACKS {
            cache.remove(0);
        }
        cache.push((key, lines));
    }
}

pub fn request(hwnd: HWND, title: &str, artist: &str, secs: Option<u32>, shared: bool) {
    let raw = hwnd.0 as isize;
    let (title, artist) = (title.to_string(), artist.to_string());
    let mine = LATEST.fetch_add(1, Ordering::SeqCst) + 1;
    crate::log_info!("lyrics request #{mine}: title={title:?} artist={artist:?} secs={secs:?} shared={shared}");
    std::thread::spawn(move || {
        let wake = || unsafe {
            let _ = PostMessageW(Some(HWND(raw as *mut c_void)), WM_LYRICS, WPARAM(0), LPARAM(0));
        };
        if let Some(lines) = disk_get(&title, &artist) {
            crate::log_info!("lyrics #{mine}: disk cache hit ({} lines)", lines.len());
            remember((title, artist), Some(lines));
            return wake();
        }
        std::thread::sleep(DEBOUNCE);
        if LATEST.load(Ordering::SeqCst) != mine {
            crate::log_info!("lyrics #{mine}: dropped, another track is playing");
            return;
        }
        unsafe {
            let _ = RoInitialize(RO_INIT_MULTITHREADED);
        }
        let id = crate::services::sync::lyrics_id(&artist, &title);
        if shared {
            let hit = crate::services::sync::fetch_lrc(&id);
            crate::log_info!("lyrics #{mine}: shared cache {}", if hit.is_some() { "answered" } else { "has nothing" });
            if let Some(lrc) = hit {
                let lines = parse_lrc(&lrc);
                if lines.len() >= 2 {
                    let lines = Arc::new(lines);
                    crate::log_info!("lyrics #{mine}: from the shared cache ({} lines)", lines.len());
                    disk_put(&title, &artist, &lines);
                    remember((title, artist), Some(lines));
                    return wake();
                }
                crate::log_info!("lyrics #{mine}: shared record has too few synced lines ({})", lines.len());
            }
        }
        let mut found = fetch(mine, &title, &artist, secs);
        for (n, wait) in RETRY_WAITS.into_iter().enumerate() {
            if !matches!(found, Found::Offline) {
                break;
            }
            crate::log_info!("lyrics #{mine}: offline or limited, retry {} in {:?}", n + 1, wait);
            std::thread::sleep(wait);
            if LATEST.load(Ordering::SeqCst) != mine {
                crate::log_info!("lyrics #{mine}: dropped, another track is playing");
                return;
            }
            found = fetch(mine, &title, &artist, secs);
        }
        let lines = match found {
            Found::Lines(l) => {
                crate::log_info!("lyrics #{mine}: found on LRCLIB ({} lines)", l.len());
                Some(l)
            }
            Found::Missing => {
                crate::log_info!("lyrics #{mine}: no synced lyrics on LRCLIB for the track");
                None
            }
            Found::Offline => {
                crate::log_warn!("lyrics #{mine}: lookup failed (offline, rate limited or server error)");
                return;
            }
        };
        if let Some(l) = &lines {
            disk_put(&title, &artist, l);
        }
        let to_share = lines.clone().filter(|_| shared);
        remember((title, artist), lines);
        wake();
        if let Some(l) = to_share {
            crate::services::sync::upload(id, to_lrc(&l), plain_text(&l));
        }
    });
}

const MAX_SONG_SECS: u32 = 900;
const MAX_VARIANTS: usize = 5;

enum Step {
    Hit(Lines),
    Miss,
    Limited,
    Down,
}

fn lrclib(job: u64, what: &str, url: &str, exact: bool, secs: Option<u32>) -> Step {
    crate::log_info!("lyrics #{job}: {what}: GET {url}");
    match http::request_as("GET", url, None, None, Some(USER_AGENT)) {
        None => {
            crate::log_info!("lyrics #{job}: {what}: no response");
            Step::Down
        }
        Some(r) if r.status == 200 => {
            let got = if exact { synced_from_record(&r.body) } else { pick(job, &r.body, secs) };
            match got {
                Some(lines) => {
                    crate::log_info!("lyrics #{job}: {what}: hit ({} lines)", lines.len());
                    Step::Hit(lines)
                }
                None => {
                    crate::log_info!("lyrics #{job}: {what}: nothing usable");
                    Step::Miss
                }
            }
        }
        Some(r) if r.status == 429 || r.status >= 500 => {
            crate::log_info!("lyrics #{job}: {what}: answered {}", r.status);
            Step::Limited
        }
        Some(r) => {
            crate::log_info!("lyrics #{job}: {what}: answered {}", r.status);
            Step::Miss
        }
    }
}

fn variants(title: &str, artist: &str) -> Vec<(String, String)> {
    let artist_clean = clean_artist(artist);
    let title_clean = strip_noise(title);
    let mut out: Vec<(String, String)> = vec![(title.trim().to_string(), artist.trim().to_string())];
    let mut add = |t: &str, a: &str| {
        let (t, a) = (squash(t), squash(a));
        if !t.is_empty() && !out.iter().any(|(ot, oa)| *ot == t && *oa == a) {
            out.push((t, a));
        }
    };
    let dashed: Vec<String> =
        title_clean.replace(" \u{2013} ", " - ").replace(" \u{2014} ", " - ").split(" - ").map(squash).collect();
    let main =
        |t: &str| -> String { t.split(" / ").next().unwrap_or(t).split(" | ").next().unwrap_or(t).trim().to_string() };
    if dashed.len() >= 2 {
        let song = main(&dashed[1..].join(" - "));
        add(&song, &clean_artist(&dashed[0]));
        add(&song, &artist_clean);
    }
    let whole = main(&title_clean);
    add(&whole, &artist_clean);
    let bare = strip_brackets(&whole);
    add(&bare, &artist_clean);
    if let Some(inner) = last_bracket(&whole).filter(|i| i.chars().count() >= 3) {
        add(&inner, &artist_clean);
    }
    out.truncate(MAX_VARIANTS);
    out
}

const NOISE: [&str; 22] = [
    "official",
    "audio",
    "video",
    "lyric",
    "cover",
    "mv",
    "m/v",
    "hd",
    "4k",
    "remaster",
    "live",
    "ver.",
    "version",
    "feat",
    "ft.",
    "visualizer",
    "clip",
    "full",
    "topic",
    "tv size",
    "sub",
    "караоке",
];

fn has_noise(s: &str) -> bool {
    let low = s.to_lowercase();
    let words: Vec<&str> = low.split(|c: char| !c.is_alphanumeric() && c != '/' && c != '.').collect();
    NOISE.iter().any(|n| {
        if n.contains(['/', '.', ' ']) {
            low.contains(n)
        } else {
            words.contains(n) || (*n == "lyric" && low.contains("lyrics"))
        }
    })
}

const BRACKETS: [(char, char); 6] =
    [('(', ')'), ('[', ']'), ('{', '}'), ('\u{3010}', '\u{3011}'), ('\u{FF08}', '\u{FF09}'), ('\u{300C}', '\u{300D}')];

fn squash(s: impl AsRef<str>) -> String {
    s.as_ref()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches(|c: char| "-\u{2013}\u{2014}|/ ".contains(c))
        .to_string()
}

fn drop_brackets(s: &str, only_noisy: bool) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if let Some(&(_, close)) = BRACKETS.iter().find(|(o, _)| *o == chars[i])
            && let Some(len) = chars[i + 1..].iter().position(|c| *c == close)
        {
            let inner: String = chars[i + 1..i + 1 + len].iter().collect();
            if !only_noisy || has_noise(&inner) {
                i += len + 2;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    squash(out)
}

fn strip_noise(title: &str) -> String {
    let t = drop_brackets(title, true);
    let parts: Vec<&str> = t.split(" - ").collect();
    let kept: Vec<&str> = parts.iter().copied().filter(|p| !has_noise(p)).collect();
    squash(if kept.is_empty() { t.clone() } else { kept.join(" - ") })
}

fn strip_brackets(s: &str) -> String {
    drop_brackets(s, false)
}

fn last_bracket(s: &str) -> Option<String> {
    let chars: Vec<char> = s.chars().collect();
    let close_at = chars.iter().rposition(|c| BRACKETS.iter().any(|(_, cl)| cl == c))?;
    let close = chars[close_at];
    let open = BRACKETS.iter().find(|(_, cl)| *cl == close)?.0;
    let open_at = chars[..close_at].iter().rposition(|c| *c == open)?;
    Some(squash(chars[open_at + 1..close_at].iter().collect::<String>()))
}

fn clean_artist(artist: &str) -> String {
    let a = drop_brackets(artist, false);
    let a = a.trim_end_matches(" - Topic").trim_end_matches("VEVO").trim_end_matches("Official").to_string();
    first_artist(&a).to_string()
}

fn netease_json(job: u64, url: &str) -> Option<serde_json::Value> {
    let r = http::request_as("GET", url, None, None, Some(USER_AGENT))?;
    if r.status != 200 {
        crate::log_info!("lyrics #{job}: netease answered {}", r.status);
        return None;
    }
    serde_json::from_str(&r.body).ok()
}

fn netease(job: u64, title: &str, artist: &str, secs: Option<u32>) -> Option<Lines> {
    let query = format!("{artist} {title}");
    let url = format!(
        "https://music.163.com/api/search/get/web?csrf_token=&hostUid=0&type=1&offset=0&limit=8&s={}",
        http::pct(query.trim())
    );
    crate::log_info!("lyrics #{job}: netease: search {query:?}");
    let found = netease_json(job, &url)?;
    let songs = found.get("result")?.get("songs")?.as_array()?;
    crate::log_info!("lyrics #{job}: netease: {} songs", songs.len());
    let want = title.to_lowercase();
    for song in songs.iter().take(8) {
        let id = song.get("id")?.as_i64()?;
        let name = song.get("name").and_then(|n| n.as_str()).unwrap_or("").to_lowercase();
        let ms = song.get("duration").and_then(|d| d.as_f64());
        let length_ok = match (secs, ms) {
            (Some(s), Some(ms)) => (ms / 1000.0 - f64::from(s)).abs() <= 6.0,
            _ => name.contains(&want) || want.contains(&name),
        };
        if !length_ok {
            continue;
        }
        let url = format!("https://music.163.com/api/song/lyric?id={id}&lv=1&kv=1&tv=-1");
        let Some(lyric) = netease_json(job, &url) else { continue };
        let Some(text) = lyric.get("lrc").and_then(|l| l.get("lyric")).and_then(|l| l.as_str()) else { continue };
        let lines = parse_lrc(text);
        if lines.len() >= 2 {
            crate::log_info!("lyrics #{job}: netease: hit song {id} ({} lines)", lines.len());
            return Some(Arc::new(lines));
        }
    }
    crate::log_info!("lyrics #{job}: netease: no fitting song with synced lyrics");
    None
}

fn fetch(job: u64, title: &str, artist: &str, secs: Option<u32>) -> Found {
    let secs = secs.filter(|s| *s <= MAX_SONG_SECS);
    let names = variants(title, artist);
    crate::log_info!("lyrics #{job}: {} names to try: {:?}", names.len(), names);
    let mut flaky = false;
    for (n, (t, a)) in names.iter().enumerate() {
        let mut tries: Vec<(&str, String, bool)> = Vec::new();
        let mut get = format!("{BASE}/api/get?track_name={}&artist_name={}", http::pct(t), http::pct(a));
        if let Some(s) = secs {
            get.push_str(&format!("&duration={s}"));
        }
        if n == 0 {
            tries.push(("exact", get, true));
        }
        tries.push((
            "search",
            format!("{BASE}/api/search?track_name={}&artist_name={}", http::pct(t), http::pct(first_artist(a))),
            false,
        ));
        tries.push((
            "free search",
            format!("{BASE}/api/search?q={}", http::pct(&format!("{} {}", first_artist(a), t))),
            false,
        ));
        if n + 1 == names.len() || n == 1 {
            tries.push(("title only", format!("{BASE}/api/search?track_name={}", http::pct(t)), false));
        }
        for (what, url, exact) in tries {
            match lrclib(job, &format!("{what} #{n}"), &url, exact, secs) {
                Step::Hit(lines) => return Found::Lines(lines),
                Step::Limited | Step::Down => flaky = true,
                Step::Miss => {}
            }
        }
    }
    if flaky {
        return Found::Offline;
    }
    for (t, a) in names.iter().skip(1).take(2) {
        if let Some(lines) = netease(job, t, &clean_artist(a), secs) {
            return Found::Lines(lines);
        }
    }
    Found::Missing
}

fn disk_dir() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("APPDATA")?).join("NightIsland").join("lyrics"))
}

fn disk_file(title: &str, artist: &str) -> Option<PathBuf> {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    hash.update(title.as_bytes());
    hash.update([0]);
    hash.update(artist.as_bytes());
    let hex: String = hash.finalize().iter().take(16).map(|b| format!("{b:02x}")).collect();
    Some(disk_dir()?.join(format!("{hex}.lrc")))
}

fn disk_get(title: &str, artist: &str) -> Option<Lines> {
    let path = disk_file(title, artist)?;
    if fs::metadata(&path).ok()?.len() > DISK_FILE_MAX_BYTES {
        return None;
    }
    let lines = parse_lrc(&fs::read_to_string(path).ok()?);
    (lines.len() >= 2).then(|| Arc::new(lines))
}

fn disk_put(title: &str, artist: &str, lines: &[Line]) {
    let Some(path) = disk_file(title, artist) else { return };
    let Some(dir) = path.parent() else { return };
    if fs::create_dir_all(dir).is_err() {
        return;
    }
    let text = to_lrc(lines);
    if text.len() as u64 <= DISK_FILE_MAX_BYTES {
        let tmp = path.with_extension("tmp");
        if fs::write(&tmp, text).is_ok() {
            let _ = fs::rename(&tmp, &path);
        }
    }
    prune(dir);
}

fn prune(dir: &std::path::Path) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = entries
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "lrc"))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    if files.len() <= DISK_FILES_MAX {
        return;
    }
    files.sort();
    for (_, path) in files.iter().take(files.len() - DISK_FILES_MAX) {
        let _ = fs::remove_file(path);
    }
}

fn plain_text(lines: &[Line]) -> String {
    lines.iter().filter(|l| !l.text.is_empty()).map(|l| l.text.as_str()).collect::<Vec<_>>().join("\n")
}

fn to_lrc(lines: &[Line]) -> String {
    let mut out = String::new();
    for l in lines {
        let (m, s, cs) = (l.ms / 60_000, l.ms / 1000 % 60, l.ms % 1000 / 10);
        out.push_str(&format!("[{m:02}:{s:02}.{cs:02}]{}\n", l.text));
    }
    out
}

fn first_artist(artist: &str) -> &str {
    artist.split([',', ';', '&']).next().unwrap_or(artist).trim()
}

fn synced_from_record(body: &str) -> Option<Lines> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    record_lines(&value)
}

fn record_lines(record: &serde_json::Value) -> Option<Lines> {
    let lines = parse_lrc(record.get("syncedLyrics")?.as_str()?);
    (lines.len() >= 2).then(|| Arc::new(lines))
}

fn pick(job: u64, body: &str, secs: Option<u32>) -> Option<Lines> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    let records = value.as_array()?;
    crate::log_info!("lyrics #{job}: search returned {} candidates, track length {secs:?}", records.len());
    let mut best: Option<(f64, Lines)> = None;
    for record in records.iter().take(MAX_CANDIDATES) {
        let Some(lines) = record_lines(record) else {
            crate::log_info!("lyrics #{job}: candidate skipped, no synced lyrics");
            continue;
        };
        let diff = match (secs, record.get("duration").and_then(|d| d.as_f64())) {
            (Some(want), Some(have)) => (have - f64::from(want)).abs(),
            _ => 0.0,
        };
        if best.as_ref().is_none_or(|(d, _)| diff < *d) {
            best = Some((diff, lines));
        }
    }
    if let Some((d, _)) = best.as_ref().filter(|(d, _)| *d > 5.0) {
        crate::log_info!("lyrics #{job}: best candidate rejected, its length differs by {d:.1}s");
    }
    best.filter(|(d, _)| *d <= 5.0).map(|(_, l)| l)
}

pub fn parse_lrc(src: &str) -> Vec<Line> {
    let mut out: Vec<Line> = Vec::new();
    let mut offset: i64 = 0;
    for raw in src.lines().take(MAX_SOURCE_LINES) {
        let mut rest = raw.trim();
        let mut stamps: Vec<u32> = Vec::new();
        while let Some(inner) = rest.strip_prefix('[') {
            let Some(end) = inner.find(']') else { break };
            let tag = &inner[..end];
            if let Some(ms) = parse_stamp(tag) {
                if stamps.len() < 8 {
                    stamps.push(ms);
                }
            } else if let Some(v) = tag.strip_prefix("offset:") {
                offset = v.trim().parse::<i64>().unwrap_or(0).clamp(-60_000, 60_000);
            }
            rest = inner[end + 1..].trim_start();
        }
        if stamps.is_empty() {
            continue;
        }
        let text = clean_text(rest);
        for ms in stamps {
            if out.len() >= MAX_LINES {
                break;
            }
            let shifted = (i64::from(ms) + offset).clamp(0, i64::from(u32::MAX)) as u32;
            out.push(Line { ms: shifted, text: text.clone() });
        }
    }
    out.sort_by_key(|l| l.ms);
    out
}

fn clean_text(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '<' {
            let mut mark = String::new();
            let mut closed = false;
            for n in chars.by_ref() {
                if n == '>' {
                    closed = true;
                    break;
                }
                mark.push(n);
                if mark.len() > 12 {
                    break;
                }
            }
            if closed && parse_stamp(&mark).is_some() {
                continue;
            }
            out.push('<');
            out.push_str(&mark);
            if closed {
                out.push('>');
            }
        } else if !c.is_control() {
            out.push(c);
        }
    }
    out.trim().chars().take(MAX_LINE_CHARS).collect()
}

fn parse_stamp(tag: &str) -> Option<u32> {
    let (min, rest) = tag.split_once(':')?;
    let (sec, frac) = match rest.split_once(['.', ':']) {
        Some((s, f)) => (s, f),
        None => (rest, ""),
    };
    let digits = |s: &str, max: usize| !s.is_empty() && s.len() <= max && s.bytes().all(|b| b.is_ascii_digit());
    if !digits(min, 4) || !digits(sec, 2) || !(frac.is_empty() || digits(frac, 3)) {
        return None;
    }
    let ms = match frac.len() {
        0 => 0,
        1 => frac.parse::<u32>().ok()? * 100,
        2 => frac.parse::<u32>().ok()? * 10,
        _ => frac.parse::<u32>().ok()?,
    };
    Some(min.parse::<u32>().ok()? * 60_000 + sec.parse::<u32>().ok()? * 1000 + ms)
}

pub fn line_at(lines: &[Line], ms: u32) -> Option<usize> {
    lines.partition_point(|l| l.ms <= ms).checked_sub(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_stamps_and_orders_lines() {
        let src = "[ar:Someone]\n[00:12.50]second\n[00:01.5][00:30.00]first and chorus\n[bad]x\nplain";
        let l = parse_lrc(src);
        let got: Vec<(u32, &str)> = l.iter().map(|l| (l.ms, l.text.as_str())).collect();
        assert_eq!(got, [(1500, "first and chorus"), (12_500, "second"), (30_000, "first and chorus")]);
    }

    #[test]
    fn offset_and_word_marks() {
        let l = parse_lrc("[offset:-500]\n[00:10.00]<00:10.00>he<00:10.40>llo");
        assert_eq!(l, [Line { ms: 9500, text: "hello".into() }]);
    }

    #[test]
    fn hostile_input_is_bounded() {
        let big = "[00:01.00]x".to_string() + &"\n[99:99:99999999999]y".repeat(10);
        assert!(parse_lrc(&big).len() <= MAX_LINES);
        let many: String = (0..10_000).map(|i| format!("[00:{:02}.00]a\n", i % 60)).collect();
        assert!(parse_lrc(&many).len() <= MAX_LINES);
        let long = format!("[00:01.00]{}", "я".repeat(5000));
        assert_eq!(parse_lrc(&long)[0].text.chars().count(), MAX_LINE_CHARS);
        assert!(parse_lrc("[").is_empty() && parse_lrc("[00:01.00").is_empty() && parse_lrc("]\u{0}").is_empty());
    }

    #[test]
    fn finds_current_line() {
        let l = parse_lrc("[00:01.00]a\n[00:05.00]b\n[00:09.00]c");
        assert_eq!(line_at(&l, 500), None);
        assert_eq!(line_at(&l, 1000), Some(0));
        assert_eq!(line_at(&l, 8999), Some(1));
        assert_eq!(line_at(&l, 600_000), Some(2));
    }

    #[test]
    fn picks_the_closest_recording() {
        let body = r#"[
            {"duration":300,"syncedLyrics":"[00:01.00]a\n[00:02.00]b"},
            {"duration":201,"syncedLyrics":"[00:03.00]c\n[00:04.00]d"},
            {"duration":200,"syncedLyrics":null}]"#;
        assert_eq!(pick(0, body, Some(200)).unwrap()[0].text, "c");
        assert!(pick(0, body, Some(100)).is_none());
        assert!(pick(0, "not json", None).is_none());
    }

    #[test]
    fn lrc_round_trips_and_file_names_hide_the_text() {
        let lines = parse_lrc("[01:02.50]one\n[00:05.00]two");
        assert_eq!(parse_lrc(&to_lrc(&lines)), lines);
        let name = disk_file("Some Song", "Some Artist").unwrap();
        let name = name.file_name().unwrap().to_string_lossy().into_owned();
        assert!(name.ends_with(".lrc") && name.len() == 36 && !name.to_lowercase().contains("song"));
        assert_ne!(disk_file("a", "bc"), disk_file("ab", "c"));
    }

    #[test]
    fn video_titles_are_cleaned_into_names() {
        let v =
            variants("SQWOZ BAB \u{2013}  \u{41c}\u{410}\u{413}\u{41d}\u{418}\u{422} (Official audio)", "SQWOZ BAB");
        assert!(
            v.contains(&("\u{41c}\u{410}\u{413}\u{41d}\u{418}\u{422}".to_string(), "SQWOZ BAB".to_string())),
            "{v:?}"
        );
        let v = variants(
            "\u{30b5}\u{30c3}\u{30c9} (Sad Girl Sex) / Finana Ryugu (Cover)",
            "Finana Ryugu \u{3010}NIJISANJI EN\u{3011}",
        );
        assert!(v.iter().any(|(t, a)| t == "Sad Girl Sex" && a == "Finana Ryugu"), "{v:?}");
        assert!(v.iter().any(|(t, _)| t == "\u{30b5}\u{30c3}\u{30c9}"), "{v:?}");
        let v = variants("Song (feat. Someone)", "Band - Topic");
        assert!(v.iter().any(|(t, a)| t == "Song" && a == "Band"), "{v:?}");
        assert!(v.len() <= MAX_VARIANTS);
    }

    #[test]
    fn first_artist_of_many() {
        assert_eq!(first_artist("A & B"), "A");
        assert_eq!(first_artist("Solo"), "Solo");
    }
}
