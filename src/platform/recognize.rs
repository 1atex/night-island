use std::ffi::c_void;
use std::sync::Mutex;

use windows::Globalization::Language;
use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
use windows::Media::Ocr::OcrEngine;
use windows::Storage::Streams::DataWriter;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};
use windows::core::{HSTRING, Result};

pub const WM_RECOGNIZED: u32 = WM_APP + 10;

const MAX_PIXELS: usize = 40_000_000;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Ocr,
    Qr,
}

pub struct Recognized {
    pub kind: Kind,
    pub text: Option<String>,
}

static RESULT: Mutex<Option<Recognized>> = Mutex::new(None);

pub fn take_result() -> Option<Recognized> {
    RESULT.lock().ok().and_then(|mut g| g.take())
}

pub fn spawn(kind: Kind, owner: HWND, bgra: Vec<u8>, w: i32, h: i32) {
    let raw = owner.0 as isize;
    std::thread::spawn(move || {
        unsafe {
            let _ = RoInitialize(RO_INIT_MULTITHREADED);
        }
        let text = if (w as usize) * (h as usize) > MAX_PIXELS {
            None
        } else {
            match kind {
                Kind::Ocr => ocr(&bgra, w, h),
                Kind::Qr => qr(&bgra, w, h),
            }
        };
        if let Ok(mut slot) = RESULT.lock() {
            *slot = Some(Recognized { kind, text: text.filter(|t| !t.trim().is_empty()) });
        }
        unsafe {
            let _ = PostMessageW(Some(HWND(raw as *mut c_void)), WM_RECOGNIZED, WPARAM(0), LPARAM(0));
        }
    });
}

fn qr(bgra: &[u8], w: i32, h: i32) -> Option<String> {
    let (w, h) = (w as usize, h as usize);
    let gray: Vec<u8> = bgra
        .chunks_exact(4)
        .map(|p| ((u32::from(p[2]) * 299 + u32::from(p[1]) * 587 + u32::from(p[0]) * 114) / 1000) as u8)
        .collect();
    let mut image = rqrr::PreparedImage::prepare_from_greyscale(w, h, |x, y| gray[y * w + x]);
    image.detect_grids().into_iter().find_map(|grid| grid.decode().ok().map(|(_, text)| text))
}

fn software_bitmap(bgra: &[u8], w: i32, h: i32) -> Result<SoftwareBitmap> {
    let writer = DataWriter::new()?;
    writer.WriteBytes(bgra)?;
    let buffer = writer.DetachBuffer()?;
    SoftwareBitmap::CreateCopyFromBuffer(&buffer, BitmapPixelFormat::Bgra8, w, h)
}

struct Line {
    top: f32,
    bottom: f32,
    text: String,
}

fn read_lines(engine: &OcrEngine, bitmap: &SoftwareBitmap) -> Option<Vec<Line>> {
    let result = engine.RecognizeAsync(bitmap).ok()?.join().ok()?;
    let mut out = Vec::new();
    for line in result.Lines().ok()? {
        let Ok(text) = line.Text() else { continue };
        let (mut top, mut bottom) = (f32::MAX, f32::MIN);
        for word in line.Words().ok()? {
            if let Ok(r) = word.BoundingRect() {
                top = top.min(r.Y);
                bottom = bottom.max(r.Y + r.Height);
            }
        }
        if top <= bottom {
            out.push(Line { top, bottom, text: text.to_string() });
        }
    }
    Some(out)
}

fn script(c: char) -> u8 {
    if c.is_ascii_alphabetic() {
        1
    } else if ('\u{0400}'..='\u{04FF}').contains(&c) {
        2
    } else {
        3
    }
}

fn quality(line: &str) -> f32 {
    let mut score = 0.0;
    for token in line.split_whitespace() {
        let core = token.trim_matches(|c: char| !c.is_alphanumeric());
        let chars: Vec<char> = core.chars().collect();
        if chars.is_empty() {
            continue;
        }
        let letters: Vec<char> = chars.iter().copied().filter(|c| c.is_alphabetic()).collect();
        let digits = chars.iter().filter(|c| c.is_ascii_digit()).count();
        if digits == chars.len() {
            score += 1.0;
            continue;
        }
        let one_script = letters.windows(2).all(|w| script(w[0]) == script(w[1]));
        let odd_case = chars.windows(2).any(|w| w[0].is_lowercase() && w[1].is_uppercase());
        let clean = letters.len() == chars.len() && one_script && !odd_case;
        if clean {
            score += 2.0 + letters.len() as f32 * 0.1;
        } else {
            let noise = chars.len() - letters.len();
            score -= 1.0 + noise as f32;
        }
    }
    score
}

fn overlap(a: &Line, b: &Line) -> f32 {
    let inter = (a.bottom.min(b.bottom) - a.top.max(b.top)).max(0.0);
    inter / (a.bottom - a.top).min(b.bottom - b.top).max(1.0)
}

fn merge(base: Vec<Line>, other: Vec<Line>) -> Vec<Line> {
    let mut used = vec![false; other.len()];
    let mut out: Vec<Line> = Vec::new();
    for line in base {
        let best = other
            .iter()
            .enumerate()
            .filter(|(i, o)| !used[*i] && overlap(&line, o) > 0.5)
            .max_by(|a, b| overlap(&line, a.1).total_cmp(&overlap(&line, b.1)));
        match best {
            Some((i, o)) => {
                used[i] = true;
                let keep_other = quality(&o.text) > quality(&line.text);
                let chosen = if keep_other { o.text.clone() } else { line.text };
                out.push(Line { top: line.top.min(o.top), bottom: line.bottom.max(o.bottom), text: chosen });
            }
            None => out.push(line),
        }
    }
    for (i, o) in other.into_iter().enumerate() {
        if !used[i] {
            out.push(o);
        }
    }
    out.sort_by(|a, b| a.top.total_cmp(&b.top));
    out
}

fn ocr(bgra: &[u8], w: i32, h: i32) -> Option<String> {
    let max = OcrEngine::MaxImageDimension().ok()? as i32;
    if w > max || h > max {
        return None;
    }
    let bitmap = software_bitmap(bgra, w, h).ok()?;

    let mut engines: Vec<OcrEngine> = Vec::new();
    for tag in ["ru", "en-US"] {
        if let Ok(lang) = Language::CreateLanguage(&HSTRING::from(tag))
            && OcrEngine::IsLanguageSupported(&lang).unwrap_or(false)
            && let Ok(engine) = OcrEngine::TryCreateFromLanguage(&lang)
        {
            engines.push(engine);
        }
    }
    if engines.is_empty() {
        engines.extend(OcrEngine::TryCreateFromUserProfileLanguages().ok());
    }

    let merged = engines.iter().filter_map(|e| read_lines(e, &bitmap)).reduce(merge)?;
    Some(merged.iter().map(|l| l.text.as_str()).collect::<Vec<_>>().join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render_qr(text: &str) -> (Vec<u8>, i32, i32) {
        let code = qrcode::QrCode::new(text.as_bytes()).unwrap();
        let (modules, scale, quiet) = (code.width(), 8usize, 4usize);
        let side = (modules + 2 * quiet) * scale;
        let colors = code.to_colors();
        let mut bgra = vec![255u8; side * side * 4];
        for y in 0..side {
            for x in 0..side {
                let (mx, my) = (x / scale, y / scale);
                if mx >= quiet
                    && my >= quiet
                    && mx < modules + quiet
                    && my < modules + quiet
                    && colors[(my - quiet) * modules + (mx - quiet)] == qrcode::Color::Dark
                {
                    let i = (y * side + x) * 4;
                    bgra[i..i + 3].copy_from_slice(&[0, 0, 0]);
                }
            }
        }
        (bgra, side as i32, side as i32)
    }

    #[test]
    fn qr_roundtrip() {
        for text in ["https://example.com/?a=1&b=2", "Привет, остров! 42", "x"] {
            let (px, w, h) = render_qr(text);
            assert_eq!(qr(&px, w, h).as_deref(), Some(text));
        }
    }

    fn line(top: f32, text: &str) -> Line {
        Line { top, bottom: top + 30.0, text: text.to_string() }
    }

    #[test]
    fn garbled_text_scores_below_real_text() {
        assert!(quality("Привет, остров! Тест 42") > quality("IlPL,1BeT, OCTPOB! TecT 42"));
        assert!(quality("Night Island OCR test 2026") > quality("N1ght Is1and 0CR tes7 2O26"));
    }

    #[test]
    fn merge_keeps_the_better_reading_per_line() {
        let ru = vec![line(0.0, "N1ght Is1and 0CR tes7"), line(50.0, "Привет, остров! Тест 42")];
        let en = vec![line(2.0, "Night Island OCR test"), line(52.0, "IlPL,1BeT, OCTPOB! TecT 42")];
        let merged: Vec<String> = merge(ru, en).into_iter().map(|l| l.text).collect();
        assert_eq!(merged, ["Night Island OCR test", "Привет, остров! Тест 42"]);
    }

    #[test]
    fn unmatched_lines_are_kept_in_order() {
        let a = vec![line(0.0, "first")];
        let b = vec![line(100.0, "second")];
        let merged: Vec<String> = merge(a, b).into_iter().map(|l| l.text).collect();
        assert_eq!(merged, ["first", "second"]);
    }

    #[test]
    fn qr_on_blank_image_finds_nothing() {
        let px = vec![255u8; 200 * 200 * 4];
        assert_eq!(qr(&px, 200, 200), None);
    }
}
