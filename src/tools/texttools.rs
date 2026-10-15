use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Op {
    Layout,
    Upper,
    Lower,
    Title,
    Translit,
    SortLines,
    UniqueLines,
    CleanSpaces,
    Count,
    Qr,
    Snake,
    Camel,
    Kebab,
    JsonPretty,
    JsonMinify,
    Base64Encode,
    Base64Decode,
    UrlEncode,
    UrlDecode,
    Jwt,
    Timestamp,
    HexEncode,
    HexDecode,
    Sha256,
    Uuid,
}

pub const OPS_BASIC: [(Op, &str); 10] = [
    (Op::Layout, "Wrong layout"),
    (Op::Upper, "UPPER"),
    (Op::Lower, "lower"),
    (Op::Title, "Title Case"),
    (Op::Translit, "Translit"),
    (Op::SortLines, "Sort lines"),
    (Op::UniqueLines, "Unique lines"),
    (Op::CleanSpaces, "Clean spaces"),
    (Op::Count, "Count"),
    (Op::Qr, "QR code"),
];

pub const OPS_DEV: [(Op, &str); 15] = [
    (Op::Snake, "snake_case"),
    (Op::Camel, "camelCase"),
    (Op::Kebab, "kebab-case"),
    (Op::JsonPretty, "JSON"),
    (Op::JsonMinify, "JSON min"),
    (Op::Base64Encode, "Base64 ↑"),
    (Op::Base64Decode, "Base64 ↓"),
    (Op::UrlEncode, "URL ↑"),
    (Op::UrlDecode, "URL ↓"),
    (Op::Jwt, "JWT"),
    (Op::Timestamp, "Timestamp"),
    (Op::HexEncode, "Hex ↑"),
    (Op::HexDecode, "Hex ↓"),
    (Op::Sha256, "SHA-256"),
    (Op::Uuid, "UUID"),
];

pub fn ops(developer: bool) -> Vec<(Op, &'static str)> {
    let mut all = OPS_BASIC.to_vec();
    if developer {
        all.extend(OPS_DEV);
    }
    all
}

pub fn works_on_empty(op: Op) -> bool {
    matches!(op, Op::Timestamp | Op::Uuid)
}

pub fn apply(op: Op, text: &str) -> Result<String, &'static str> {
    match op {
        Op::Layout => Ok(swap_layout(text)),
        Op::Upper => Ok(text.to_uppercase()),
        Op::Lower => Ok(text.to_lowercase()),
        Op::Title => Ok(title_case(text)),
        Op::Translit => Ok(translit(text)),
        Op::SortLines => Ok(sort_lines(text)),
        Op::UniqueLines => Ok(unique_lines(text)),
        Op::CleanSpaces => Ok(clean_spaces(text)),
        Op::Snake => Ok(join_words(text, "_", false)),
        Op::Camel => Ok(join_words(text, "", true)),
        Op::Kebab => Ok(join_words(text, "-", false)),
        Op::JsonPretty => json(text, true),
        Op::JsonMinify => json(text, false),
        Op::Base64Encode => Ok(STANDARD.encode(text.as_bytes())),
        Op::Base64Decode => base64_decode(text),
        Op::UrlEncode => Ok(url_encode(text)),
        Op::UrlDecode => Ok(url_decode(text)),
        Op::Jwt => jwt(text, chrono::Utc::now().timestamp()),
        Op::Timestamp => timestamp(text, chrono::Utc::now().timestamp()),
        Op::HexEncode => Ok(hex_encode(text.as_bytes())),
        Op::HexDecode => hex_decode(text),
        Op::Sha256 => Ok(sha256_hex(text)),
        Op::Uuid => Ok(uuid_v4()),
        Op::Qr | Op::Count => Err("This tool does not produce text"),
    }
}

const KEYS: [(char, char); 33] = [
    ('q', 'й'),
    ('w', 'ц'),
    ('e', 'у'),
    ('r', 'к'),
    ('t', 'е'),
    ('y', 'н'),
    ('u', 'г'),
    ('i', 'ш'),
    ('o', 'щ'),
    ('p', 'з'),
    ('[', 'х'),
    (']', 'ъ'),
    ('a', 'ф'),
    ('s', 'ы'),
    ('d', 'в'),
    ('f', 'а'),
    ('g', 'п'),
    ('h', 'р'),
    ('j', 'о'),
    ('k', 'л'),
    ('l', 'д'),
    (';', 'ж'),
    ('\'', 'э'),
    ('z', 'я'),
    ('x', 'ч'),
    ('c', 'с'),
    ('v', 'м'),
    ('b', 'и'),
    ('n', 'т'),
    ('m', 'ь'),
    (',', 'б'),
    ('.', 'ю'),
    ('`', 'ё'),
];

const SHIFTED: [(char, char); 11] = [
    ('{', 'Х'),
    ('}', 'Ъ'),
    (':', 'Ж'),
    ('"', 'Э'),
    ('<', 'Б'),
    ('>', 'Ю'),
    ('~', 'Ё'),
    ('@', '"'),
    ('#', '№'),
    ('$', ';'),
    ('^', ':'),
];

pub fn swap_layout(text: &str) -> String {
    let cyr = text.chars().filter(|c| ('\u{0400}'..='\u{04FF}').contains(c)).count();
    let lat = text.chars().filter(char::is_ascii_alphabetic).count();
    let to_latin = cyr > lat;
    text.chars()
        .map(|c| {
            if to_latin {
                if let Some(&(en, _)) = KEYS.iter().find(|(_, ru)| *ru == c) {
                    return en;
                }
                if let Some(en) = c.to_lowercase().next().and_then(|l| KEYS.iter().find(|(_, ru)| *ru == l)) {
                    return en.0.to_ascii_uppercase();
                }
                SHIFTED.iter().find(|(_, ru)| *ru == c).map_or(c, |(en, _)| *en)
            } else {
                if let Some(&(_, ru)) = KEYS.iter().find(|(en, _)| *en == c) {
                    return ru;
                }
                if c.is_ascii_uppercase()
                    && let Some(&(_, ru)) = KEYS.iter().find(|(en, _)| *en == c.to_ascii_lowercase())
                {
                    return ru.to_uppercase().next().unwrap_or(ru);
                }
                SHIFTED.iter().find(|(en, _)| *en == c).map_or(c, |(_, ru)| *ru)
            }
        })
        .collect()
}

fn words(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut prev: Option<char> = None;
    for c in text.chars() {
        if !c.is_alphanumeric() {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
        } else {
            if prev.is_some_and(char::is_lowercase) && c.is_uppercase() && !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            cur.push(c);
        }
        prev = Some(c);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars.flat_map(char::to_lowercase)).collect(),
        None => String::new(),
    }
}

fn join_words(text: &str, sep: &str, camel: bool) -> String {
    let ws = words(text);
    ws.iter()
        .enumerate()
        .map(|(i, w)| if camel && i > 0 { capitalize(w) } else { w.to_lowercase() })
        .collect::<Vec<_>>()
        .join(sep)
}

fn title_case(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut at_start = true;
    for c in text.chars() {
        if c.is_alphanumeric() {
            if at_start {
                out.extend(c.to_uppercase());
            } else {
                out.extend(c.to_lowercase());
            }
            at_start = false;
        } else {
            out.push(c);
            at_start = c != '\'' && c != '’';
        }
    }
    out
}

fn translit_letter(c: char) -> Option<&'static str> {
    Some(match c {
        'а' => "a",
        'б' => "b",
        'в' => "v",
        'г' => "g",
        'д' => "d",
        'е' => "e",
        'ё' => "yo",
        'ж' => "zh",
        'з' => "z",
        'и' => "i",
        'й' => "y",
        'к' => "k",
        'л' => "l",
        'м' => "m",
        'н' => "n",
        'о' => "o",
        'п' => "p",
        'р' => "r",
        'с' => "s",
        'т' => "t",
        'у' => "u",
        'ф' => "f",
        'х' => "kh",
        'ц' => "ts",
        'ч' => "ch",
        'ш' => "sh",
        'щ' => "shch",
        'ъ' => "",
        'ы' => "y",
        'ь' => "",
        'э' => "e",
        'ю' => "yu",
        'я' => "ya",
        _ => return None,
    })
}

pub fn translit(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let upper_cyr = |c: char| c.is_uppercase() && translit_letter(c.to_lowercase().next().unwrap_or(c)).is_some();
    let mut out = String::with_capacity(text.len());
    for (i, &c) in chars.iter().enumerate() {
        let lower = c.to_lowercase().next().unwrap_or(c);
        let Some(latin) = translit_letter(lower) else {
            out.push(c);
            continue;
        };
        if !c.is_uppercase() {
            out.push_str(latin);
            continue;
        }
        let neighbours_upper = (i > 0 && upper_cyr(chars[i - 1])) || chars.get(i + 1).is_some_and(|&n| upper_cyr(n));
        if neighbours_upper {
            out.push_str(&latin.to_uppercase());
        } else {
            let mut it = latin.chars();
            out.extend(it.next().map(|f| f.to_ascii_uppercase()));
            out.extend(it);
        }
    }
    out
}

fn json(text: &str, pretty: bool) -> Result<String, &'static str> {
    let value: serde_json::Value = serde_json::from_str(text.trim()).map_err(|_| "Not valid JSON")?;
    let out = if pretty { serde_json::to_string_pretty(&value) } else { serde_json::to_string(&value) };
    out.map_err(|_| "Not valid JSON")
}

fn base64_decode(text: &str) -> Result<String, &'static str> {
    use base64::engine::general_purpose::{STANDARD_NO_PAD, URL_SAFE_NO_PAD};
    let cleaned: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    let trimmed = cleaned.trim_end_matches('=');
    let bytes =
        STANDARD_NO_PAD.decode(trimmed).or_else(|_| URL_SAFE_NO_PAD.decode(trimmed)).map_err(|_| "Not valid Base64")?;
    String::from_utf8(bytes).map_err(|_| "Decoded data is not text")
}

pub fn url_encode(text: &str) -> String {
    let mut out = String::new();
    for b in text.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn url_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len()
                && bytes.get(i + 1..i + 3).is_some_and(|h| h.iter().all(u8::is_ascii_hexdigit)) =>
            {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("00");
                out.push(u8::from_str_radix(hex, 16).unwrap_or(b'%'));
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn sort_lines(text: &str) -> String {
    let mut lines: Vec<&str> = text.lines().collect();
    lines.sort_by_key(|l| l.to_lowercase());
    lines.join("\n")
}

fn unique_lines(text: &str) -> String {
    let mut seen = std::collections::HashSet::new();
    text.lines().filter(|l| seen.insert(*l)).collect::<Vec<_>>().join("\n")
}

fn clean_spaces(text: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    for line in text.lines() {
        let line = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if line.is_empty() && out.last().is_none_or(String::is_empty) {
            continue;
        }
        out.push(line);
    }
    while out.last().is_some_and(String::is_empty) {
        out.pop();
    }
    out.join("\n")
}

#[derive(Debug, PartialEq, Eq)]
pub struct Stats {
    pub chars: usize,
    pub words: usize,
    pub lines: usize,
    pub bytes: usize,
}

pub fn stats(text: &str) -> Stats {
    Stats {
        chars: text.chars().count(),
        words: text.split_whitespace().count(),
        lines: text.lines().count(),
        bytes: text.len(),
    }
}

fn pretty_json_part(part: &str) -> Result<String, &'static str> {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let bytes = URL_SAFE_NO_PAD.decode(part.trim_end_matches('=')).map_err(|_| "Not a JWT")?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| "Not a JWT")?;
    serde_json::to_string_pretty(&value).map_err(|_| "Not a JWT")
}

fn jwt(text: &str, now: i64) -> Result<String, &'static str> {
    let token = text.trim();
    let token = token.strip_prefix("Bearer ").or_else(|| token.strip_prefix("bearer ")).unwrap_or(token).trim();
    if token.len() > 100_000 {
        return Err("Too much text");
    }
    let mut parts = token.split('.');
    let (Some(header), Some(payload)) = (parts.next(), parts.next()) else { return Err("Not a JWT") };
    let header = pretty_json_part(header)?;
    let payload_text = pretty_json_part(payload)?;
    let claims: serde_json::Value = serde_json::from_str(&payload_text).map_err(|_| "Not a JWT")?;
    let mut out = format!("// header\n{header}\n\n// payload\n{payload_text}");
    for claim in ["iat", "nbf", "exp"] {
        let Some(secs) = claims.get(claim).and_then(serde_json::Value::as_i64) else { continue };
        let Some(when) = chrono::DateTime::from_timestamp(secs, 0) else { continue };
        let note = match claim {
            "exp" if secs <= now => " (expired)",
            "exp" => " (still valid)",
            _ => "",
        };
        out.push_str(&format!("\n// {claim}: {}{note}", when.format("%Y-%m-%d %H:%M:%S UTC")));
    }
    out.push_str("\n// the signature is not verified");
    Ok(out)
}

fn timestamp(text: &str, now: i64) -> Result<String, &'static str> {
    use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, TimeZone, Utc};
    let t = text.trim();
    let moment: DateTime<Utc> = if t.is_empty() {
        DateTime::from_timestamp(now, 0).ok_or("Not a date or a Unix time")?
    } else if t.trim_start_matches('-').bytes().all(|b| b.is_ascii_digit()) && t.len() <= 19 {
        let n: i64 = t.parse().map_err(|_| "Not a date or a Unix time")?;
        let digits = t.trim_start_matches('-').len();
        let (secs, nanos) = match digits {
            0..=11 => (n, 0),
            12..=14 => (n.div_euclid(1000), (n.rem_euclid(1000) * 1_000_000) as u32),
            _ => (n.div_euclid(1_000_000), (n.rem_euclid(1_000_000) * 1000) as u32),
        };
        DateTime::from_timestamp(secs, nanos).ok_or("Not a date or a Unix time")?
    } else if let Ok(fixed) = DateTime::parse_from_rfc3339(t) {
        fixed.with_timezone(&Utc)
    } else {
        const DATE_TIME: [&str; 5] =
            ["%Y-%m-%d %H:%M:%S", "%Y-%m-%dT%H:%M:%S", "%Y-%m-%d %H:%M", "%d.%m.%Y %H:%M:%S", "%d.%m.%Y %H:%M"];
        const DATE: [&str; 2] = ["%Y-%m-%d", "%d.%m.%Y"];
        let naive = DATE_TIME
            .iter()
            .find_map(|f| NaiveDateTime::parse_from_str(t, f).ok())
            .or_else(|| {
                DATE.iter().find_map(|f| NaiveDate::parse_from_str(t, f).ok()).and_then(|d| d.and_hms_opt(0, 0, 0))
            })
            .ok_or("Not a date or a Unix time")?;
        Local.from_local_datetime(&naive).earliest().ok_or("Not a date or a Unix time")?.with_timezone(&Utc)
    };
    let local = moment.with_timezone(&Local);
    Ok(format!(
        "Unix: {}\nUnix ms: {}\nUTC: {}\nLocal: {} ({})\nISO 8601: {}",
        moment.timestamp(),
        moment.timestamp_millis(),
        moment.format("%Y-%m-%d %H:%M:%S"),
        local.format("%Y-%m-%d %H:%M:%S"),
        local.format("UTC%:z"),
        moment.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    ))
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn hex_decode(text: &str) -> Result<String, &'static str> {
    let t = text.trim();
    let t = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")).unwrap_or(t);
    let digits: String = t.chars().filter(|c| !c.is_whitespace() && !matches!(c, ':' | ',' | '-')).collect();
    if digits.is_empty() || !digits.len().is_multiple_of(2) || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Not valid hex");
    }
    let bytes: Vec<u8> =
        (0..digits.len()).step_by(2).filter_map(|i| u8::from_str_radix(&digits[i..i + 2], 16).ok()).collect();
    String::from_utf8(bytes).map_err(|_| "Decoded data is not text")
}

fn sha256_hex(text: &str) -> String {
    use sha2::{Digest, Sha256};
    hex_encode(&Sha256::digest(text.as_bytes()))
}

fn uuid_v4() -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    let mut b = [0u8; 16];
    for chunk in b.chunks_mut(8) {
        chunk.copy_from_slice(&RandomState::new().build_hasher().finish().to_le_bytes());
    }
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h = hex_encode(&b);
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

pub fn qr_modules(text: &str) -> Option<(Vec<bool>, usize)> {
    if text.is_empty() {
        return None;
    }
    let code = qrcode::QrCode::new(text.as_bytes()).ok()?;
    let side = code.width();
    Some((code.to_colors().into_iter().map(|c| c == qrcode::Color::Dark).collect(), side))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_swaps_both_ways() {
        assert_eq!(swap_layout("ghbdtn"), "привет");
        assert_eq!(swap_layout("Руддщ, цщкдв!"), "Hello, world!");
        assert_eq!(swap_layout("привет"), "ghbdtn");
        assert_eq!(swap_layout("Ghbdtn"), "Привет");
        assert_eq!(swap_layout("[]"), "хъ");
    }

    #[test]
    fn cases() {
        assert_eq!(apply(Op::Upper, "abc ёж").unwrap(), "ABC ЁЖ");
        assert_eq!(apply(Op::Title, "hello wORLD, it's me").unwrap(), "Hello World, It's Me");
        assert_eq!(apply(Op::Snake, "Hello World-fooBar").unwrap(), "hello_world_foo_bar");
        assert_eq!(apply(Op::Camel, "hello_world foo-bar").unwrap(), "helloWorldFooBar");
    }

    #[test]
    fn transliteration() {
        assert_eq!(translit("Привет, мир!"), "Privet, mir!");
        assert_eq!(translit("Щука"), "Shchuka");
        assert_eq!(translit("ЩУКА"), "SHCHUKA");
        assert_eq!(translit("Ёлка и объём"), "Yolka i obyom");
    }

    #[test]
    fn json_tools() {
        assert_eq!(apply(Op::JsonMinify, "{ \"a\": [1, 2],\n \"b\": null }").unwrap(), "{\"a\":[1,2],\"b\":null}");
        assert!(apply(Op::JsonPretty, "{\"a\":1}").unwrap().contains("\n  \"a\": 1"));
        assert!(apply(Op::JsonPretty, "{oops").is_err());
    }

    #[test]
    fn base64_and_url() {
        assert_eq!(apply(Op::Base64Encode, "Привет").unwrap(), "0J/RgNC40LLQtdGC");
        assert_eq!(apply(Op::Base64Decode, "0J/RgNC40LLQtdGC").unwrap(), "Привет");
        assert_eq!(apply(Op::Base64Decode, "SGk").unwrap(), "Hi");
        assert!(apply(Op::Base64Decode, "***").is_err());
        assert_eq!(apply(Op::UrlEncode, "a b&c=д").unwrap(), "a%20b%26c%3D%D0%B4");
        assert_eq!(apply(Op::UrlDecode, "a%20b+c%D0%B4%zz%").unwrap(), "a b cд%zz%");
    }

    #[test]
    fn line_tools() {
        assert_eq!(apply(Op::SortLines, "b\nA\nc").unwrap(), "A\nb\nc");
        assert_eq!(apply(Op::UniqueLines, "a\nb\na\nc\nb").unwrap(), "a\nb\nc");
        assert_eq!(apply(Op::CleanSpaces, "  a   b \n\n\n  c\t d  \n\n").unwrap(), "a b\n\nc d");
        assert_eq!(stats("two words\nline"), Stats { chars: 14, words: 3, lines: 2, bytes: 14 });
        assert_eq!(stats("Привет"), Stats { chars: 6, words: 1, lines: 1, bytes: 12 });
        assert_eq!(stats(""), Stats { chars: 0, words: 0, lines: 0, bytes: 0 });
    }

    #[test]
    fn kebab_case() {
        assert_eq!(apply(Op::Kebab, "Hello World fooBar").unwrap(), "hello-world-foo-bar");
    }

    #[test]
    fn jwt_is_read_but_never_trusted() {
        let token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxIiwiZXhwIjoxNzAwMDAwMDAwfQ.sig";
        let out = jwt(token, 1_800_000_000).unwrap();
        assert!(out.contains("\"alg\": \"HS256\"") && out.contains("\"sub\": \"1\""));
        assert!(out.contains("// exp: 2023-11-14 22:13:20 UTC (expired)"));
        assert!(jwt(token, 1_600_000_000).unwrap().contains("(still valid)"));
        assert!(jwt(&format!("Bearer {token}"), 0).is_ok());
        assert_eq!(jwt("abc", 0), Err("Not a JWT"));
        assert_eq!(jwt("a.b.c", 0), Err("Not a JWT"));
        assert_eq!(jwt(&"x".repeat(200_000), 0), Err("Too much text"));
    }

    #[test]
    fn timestamps_go_both_ways() {
        let from_secs = timestamp("1700000000", 0).unwrap();
        assert!(from_secs.contains("Unix: 1700000000\nUnix ms: 1700000000000\nUTC: 2023-11-14 22:13:20"));
        assert!(from_secs.ends_with("ISO 8601: 2023-11-14T22:13:20Z"));
        assert!(timestamp("1700000000123", 0).unwrap().contains("Unix: 1700000000\nUnix ms: 1700000000123"));
        assert!(timestamp("2023-11-14T22:13:20Z", 0).unwrap().contains("Unix: 1700000000\n"));
        let local = timestamp("14.11.2023 12:30", 0).unwrap();
        assert!(local.contains("Local: 2023-11-14 12:30:00"));
        assert!(timestamp("", 1_700_000_000).unwrap().contains("Unix: 1700000000"));
        assert!(timestamp("not a date", 0).is_err());
        assert!(timestamp("99999999999999999999", 0).is_err());
    }

    #[test]
    fn hex_and_hashes() {
        assert_eq!(apply(Op::HexEncode, "Hi!").unwrap(), "486921");
        assert_eq!(apply(Op::HexDecode, "0x48 69:21").unwrap(), "Hi!");
        assert!(apply(Op::HexDecode, "zz").is_err());
        assert!(apply(Op::HexDecode, "abc").is_err());
        assert!(apply(Op::HexDecode, "ff").is_err());
        assert_eq!(
            apply(Op::Sha256, "abc").unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn uuids_look_right_and_differ() {
        let (a, b) = (uuid_v4(), uuid_v4());
        assert_ne!(a, b);
        assert_eq!(a.len(), 36);
        let parts: Vec<&str> = a.split('-').collect();
        assert_eq!(parts.iter().map(|p| p.len()).collect::<Vec<_>>(), [8, 4, 4, 4, 12]);
        assert!(parts[2].starts_with('4') && matches!(parts[3].as_bytes()[0], b'8' | b'9' | b'a' | b'b'));
    }

    #[test]
    fn the_developer_switch_adds_tools() {
        assert_eq!(ops(false).len(), OPS_BASIC.len());
        assert_eq!(ops(true).len(), OPS_BASIC.len() + OPS_DEV.len());
        assert!(OPS_BASIC.len() + OPS_DEV.len() <= 25, "the chips must fit five rows");
        assert!(works_on_empty(Op::Uuid) && works_on_empty(Op::Timestamp) && !works_on_empty(Op::Upper));
    }

    #[test]
    fn qr_limits() {
        assert!(qr_modules("https://example.com").is_some());
        assert!(qr_modules("").is_none());
        assert!(qr_modules(&"x".repeat(5000)).is_none());
    }
}
