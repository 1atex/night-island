use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Op {
    Layout,
    Upper,
    Lower,
    Title,
    Snake,
    Camel,
    Translit,
    JsonPretty,
    JsonMinify,
    Base64Encode,
    Base64Decode,
    UrlEncode,
    UrlDecode,
    Qr,
}

pub const OPS: [(Op, &str); 14] = [
    (Op::Layout, "Wrong layout"),
    (Op::Upper, "UPPER"),
    (Op::Lower, "lower"),
    (Op::Title, "Title Case"),
    (Op::Snake, "snake_case"),
    (Op::Camel, "camelCase"),
    (Op::Translit, "Translit"),
    (Op::JsonPretty, "JSON"),
    (Op::JsonMinify, "JSON min"),
    (Op::Base64Encode, "Base64 ↑"),
    (Op::Base64Decode, "Base64 ↓"),
    (Op::UrlEncode, "URL ↑"),
    (Op::UrlDecode, "URL ↓"),
    (Op::Qr, "QR code"),
];

pub fn apply(op: Op, text: &str) -> Result<String, &'static str> {
    match op {
        Op::Layout => Ok(swap_layout(text)),
        Op::Upper => Ok(text.to_uppercase()),
        Op::Lower => Ok(text.to_lowercase()),
        Op::Title => Ok(title_case(text)),
        Op::Snake => Ok(join_words(text, "_", false)),
        Op::Camel => Ok(join_words(text, "", true)),
        Op::Translit => Ok(translit(text)),
        Op::JsonPretty => json(text, true),
        Op::JsonMinify => json(text, false),
        Op::Base64Encode => Ok(STANDARD.encode(text.as_bytes())),
        Op::Base64Decode => base64_decode(text),
        Op::UrlEncode => Ok(url_encode(text)),
        Op::UrlDecode => Ok(url_decode(text)),
        Op::Qr => Err("QR is not a text operation"),
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
    fn qr_limits() {
        assert!(qr_modules("https://example.com").is_some());
        assert!(qr_modules("").is_none());
        assert!(qr_modules(&"x".repeat(5000)).is_none());
    }
}
