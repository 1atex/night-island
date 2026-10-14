use std::ffi::c_void;
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

pub fn request(hwnd: HWND, title: &str, artist: &str, secs: Option<u32>) {
    let raw = hwnd.0 as isize;
    let (title, artist) = (title.to_string(), artist.to_string());
    let mine = LATEST.fetch_add(1, Ordering::SeqCst) + 1;
    std::thread::spawn(move || {
        std::thread::sleep(DEBOUNCE);
        if LATEST.load(Ordering::SeqCst) != mine {
            return;
        }
        unsafe {
            let _ = RoInitialize(RO_INIT_MULTITHREADED);
        }
        let lines = match fetch(&title, &artist, secs) {
            Found::Lines(l) => Some(l),
            Found::Missing => None,
            Found::Offline => return,
        };
        remember((title, artist), lines);
        unsafe {
            let _ = PostMessageW(Some(HWND(raw as *mut c_void)), WM_LYRICS, WPARAM(0), LPARAM(0));
        }
    });
}

fn fetch(title: &str, artist: &str, secs: Option<u32>) -> Found {
    let mut url = format!("{BASE}/api/get?track_name={}&artist_name={}", http::pct(title), http::pct(artist));
    if let Some(s) = secs {
        url.push_str(&format!("&duration={s}"));
    }
    match http::request("GET", &url, None, None) {
        None => return Found::Offline,
        Some(r) if r.status == 200 => {
            if let Some(lines) = synced_from_record(&r.body) {
                return Found::Lines(lines);
            }
        }
        Some(r) if r.status == 404 => {}
        Some(r) if r.status >= 500 || r.status == 429 => return Found::Offline,
        Some(_) => {}
    }
    let url =
        format!("{BASE}/api/search?track_name={}&artist_name={}", http::pct(title), http::pct(first_artist(artist)));
    match http::request("GET", &url, None, None) {
        None => Found::Offline,
        Some(r) if r.status == 200 => pick(&r.body, secs).map_or(Found::Missing, Found::Lines),
        Some(r) if r.status >= 500 || r.status == 429 => Found::Offline,
        Some(_) => Found::Missing,
    }
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

fn pick(body: &str, secs: Option<u32>) -> Option<Lines> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    let mut best: Option<(f64, Lines)> = None;
    for record in value.as_array()?.iter().take(MAX_CANDIDATES) {
        let Some(lines) = record_lines(record) else { continue };
        let diff = match (secs, record.get("duration").and_then(|d| d.as_f64())) {
            (Some(want), Some(have)) => (have - f64::from(want)).abs(),
            _ => 0.0,
        };
        if best.as_ref().is_none_or(|(d, _)| diff < *d) {
            best = Some((diff, lines));
        }
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
        assert_eq!(pick(body, Some(200)).unwrap()[0].text, "c");
        assert!(pick(body, Some(100)).is_none());
        assert!(pick("not json", None).is_none());
    }

    #[test]
    fn first_artist_of_many() {
        assert_eq!(first_artist("A & B"), "A");
        assert_eq!(first_artist("Solo"), "Solo");
    }
}
