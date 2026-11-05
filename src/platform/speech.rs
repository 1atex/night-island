use std::ffi::c_void;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use windows::Media::SpeechSynthesis::{SpeechSynthesizer, VoiceInformation};
use windows::Storage::Streams::DataReader;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::Media::Audio::{PlaySoundW, SND_MEMORY, SND_NODEFAULT, SND_PURGE, SND_SYNC};
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};
use windows::core::{HSTRING, PCWSTR};

pub const MAX_CHARS: usize = 600;

static GENERATION: AtomicU32 = AtomicU32::new(0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    Russian,
    Other,
}

pub fn language_of(text: &str) -> Lang {
    let (mut cyrillic, mut latin) = (0usize, 0usize);
    for c in text.chars() {
        if ('\u{400}'..='\u{4FF}').contains(&c) {
            cyrillic += 1;
        } else if c.is_ascii_alphabetic() {
            latin += 1;
        }
    }
    if cyrillic > 0 && cyrillic >= latin { Lang::Russian } else { Lang::Other }
}

pub fn clip(text: &str) -> String {
    let flat: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    flat.chars().take(MAX_CHARS).collect()
}

fn data_chunk(wav: &[u8]) -> Option<std::ops::Range<usize>> {
    if wav.len() < 12 || &wav[..4] != b"RIFF" || &wav[8..12] != b"WAVE" {
        return None;
    }
    let mut at = 12;
    while at + 8 <= wav.len() {
        let size = u32::from_le_bytes(wav[at + 4..at + 8].try_into().ok()?) as usize;
        let body = at + 8;
        if &wav[at..at + 4] == b"data" {
            return Some(body..body.saturating_add(size).min(wav.len()));
        }
        at = body.checked_add(size)?.checked_add(size & 1)?;
    }
    None
}

pub fn scale_wav(wav: &mut [u8], volume_pct: u8) {
    let factor = f32::from(volume_pct.min(100)) / 100.0;
    if (factor - 1.0).abs() < f32::EPSILON {
        return;
    }
    let Some(range) = data_chunk(wav) else { return };
    for pair in wav[range].chunks_exact_mut(2) {
        let v = i16::from_le_bytes([pair[0], pair[1]]);
        pair.copy_from_slice(&((f32::from(v) * factor) as i16).to_le_bytes());
    }
}

fn pick_voice(lang: Lang) -> Option<VoiceInformation> {
    let voices = SpeechSynthesizer::AllVoices().ok()?;
    let want = match lang {
        Lang::Russian => "ru",
        Lang::Other => "en",
    };
    let mut fallback = None;
    for voice in voices {
        let tag = voice.Language().ok()?.to_string().to_lowercase();
        if tag.starts_with(want) {
            return Some(voice);
        }
        fallback.get_or_insert(voice);
    }
    if lang == Lang::Russian { None } else { fallback }
}

pub fn synthesize(text: &str) -> windows::core::Result<Vec<u8>> {
    let synth = SpeechSynthesizer::new()?;
    if let Some(voice) = pick_voice(language_of(text)) {
        let _ = synth.SetVoice(&voice);
    }
    let stream = synth.SynthesizeTextToStreamAsync(&HSTRING::from(text))?.join()?;
    let size = stream.Size()? as u32;
    let reader = DataReader::CreateDataReader(&stream.GetInputStreamAt(0)?)?;
    reader.LoadAsync(size)?.join()?;
    let mut out = vec![0u8; size as usize];
    reader.ReadBytes(&mut out)?;
    Ok(out)
}

pub fn stop() {
    GENERATION.fetch_add(1, Ordering::SeqCst);
    unsafe {
        let _ = PlaySoundW(PCWSTR::null(), None, SND_PURGE);
    }
}

pub const WM_SPOKEN: u32 = WM_APP + 83;
const ECHO_PAUSE: Duration = Duration::from_millis(250);

fn post_spoken(raw: isize) {
    unsafe {
        let _ = PostMessageW(Some(HWND(raw as *mut c_void)), WM_SPOKEN, WPARAM(0), LPARAM(0));
    }
}

const MAX_SPEECH_BYTES: usize = 8 * 1024 * 1024;
const NATURAL_RATE: u32 = 24_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Natural {
    pub model: String,
    pub voice: String,
}

pub fn pcm_wav(pcm: &[u8], rate: u32) -> Vec<u8> {
    let pcm = &pcm[..pcm.len() & !1];
    let mut out = Vec::with_capacity(44 + pcm.len());
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + pcm.len() as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(pcm.len() as u32).to_le_bytes());
    out.extend_from_slice(pcm);
    out
}

fn as_wav(bytes: Vec<u8>) -> Option<Vec<u8>> {
    if bytes.starts_with(b"RIFF") {
        return Some(bytes);
    }
    (bytes.len() >= NATURAL_RATE as usize / 10).then(|| pcm_wav(&bytes, NATURAL_RATE))
}

fn natural_request(text: &str, natural: &Natural) -> String {
    let mut body = serde_json::json!({
        "model": natural.model,
        "input": text,
        "voice": natural.voice,
        "response_format": "pcm",
    });
    if natural.model.starts_with("gpt-4o") {
        body["instructions"] = "Speak naturally and warmly at a calm, even pace, in the language of the text.".into();
    }
    body.to_string()
}

fn natural_wav(text: &str, natural: &Natural) -> Option<Vec<u8>> {
    let (base, key) = crate::services::ai::speech_credentials()?;
    let auth = format!("Bearer {key}");
    let reply = crate::services::http::post_json_for_bytes(
        &format!("{base}/audio/speech"),
        &[("Authorization", auth.as_str())],
        &natural_request(text, natural),
        MAX_SPEECH_BYTES,
        Some("NightIsland/0.1"),
    )?;
    if reply.status != 200 {
        crate::log_warn!("The natural voice answered with status {}", reply.status);
        return None;
    }
    as_wav(reply.body)
}

pub fn speak(text: &str, volume_pct: u8, natural: Option<Natural>) {
    start(text, volume_pct, None, natural);
}

pub fn speak_then_notify(text: &str, volume_pct: u8, hwnd: HWND, natural: Option<Natural>) {
    start(text, volume_pct, Some(hwnd.0 as isize), natural);
}

fn start(text: &str, volume_pct: u8, done: Option<isize>, natural: Option<Natural>) {
    let text = clip(text);
    if text.is_empty() || volume_pct == 0 {
        if let Some(raw) = done {
            post_spoken(raw);
        }
        return;
    }
    let mine = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    unsafe {
        let _ = PlaySoundW(PCWSTR::null(), None, SND_PURGE);
    }
    std::thread::spawn(move || {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }
        let natural_wav = natural.as_ref().and_then(|n| natural_wav(&text, n));
        if natural.is_some() && natural_wav.is_none() {
            crate::log_info!("The natural voice is not available, the system voice speaks");
        }
        let wav = natural_wav.or_else(|| synthesize(&text).ok()).filter(|_| GENERATION.load(Ordering::SeqCst) == mine);
        if let Some(mut wav) = wav {
            scale_wav(&mut wav, volume_pct);
            unsafe {
                let _ = PlaySoundW(PCWSTR(wav.as_ptr().cast()), None, SND_MEMORY | SND_SYNC | SND_NODEFAULT);
            }
        }
        if let Some(raw) = done.filter(|_| GENERATION.load(Ordering::SeqCst) == mine) {
            std::thread::sleep(ECHO_PAUSE);
            post_spoken(raw);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_language_follows_the_letters() {
        assert_eq!(language_of("Привет, как дела?"), Lang::Russian);
        assert_eq!(language_of("Hello there"), Lang::Other);
        assert_eq!(language_of("Открой Chrome"), Lang::Russian);
        assert_eq!(language_of("12345"), Lang::Other);
    }

    #[test]
    fn long_text_is_cut_and_whitespace_flattened() {
        assert_eq!(clip("a \n\n b\t c"), "a b c");
        assert_eq!(clip(&"x".repeat(MAX_CHARS * 3)).chars().count(), MAX_CHARS);
    }

    #[test]
    fn scaling_walks_to_the_data_chunk_and_leaves_the_header_alone() {
        let mut wav = crate::platform::dictation::wav(&[1000, -1000, 20000]);
        let header = wav[..44].to_vec();
        scale_wav(&mut wav, 50);
        assert_eq!(wav[..44], header[..]);
        let s: Vec<i16> = wav[44..].chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]])).collect();
        assert_eq!(s, vec![500, -500, 10000]);
    }

    #[test]
    fn a_broken_file_is_left_untouched() {
        let mut junk = vec![1u8, 2, 3, 4, 5];
        scale_wav(&mut junk, 10);
        assert_eq!(junk, vec![1, 2, 3, 4, 5]);
        let mut cut = crate::platform::dictation::wav(&[100, 200]);
        cut.truncate(30);
        scale_wav(&mut cut, 10);
    }

    #[test]
    fn raw_samples_get_a_proper_wav_header() {
        let wav = pcm_wav(&[1, 0, 2, 0, 3], 24_000);
        assert_eq!(&wav[..4], b"RIFF");
        assert_eq!(&wav[36..40], b"data");
        assert_eq!(u32::from_le_bytes(wav[40..44].try_into().unwrap()), 4, "the odd byte is dropped");
        assert_eq!(u32::from_le_bytes(wav[24..28].try_into().unwrap()), 24_000);
        assert_eq!(wav.len(), 48);
        let data = data_chunk(&wav).expect("a data chunk");
        assert_eq!(data.len(), 4);
    }

    #[test]
    fn a_reply_is_a_wav_already_or_raw_samples_but_never_a_tiny_error() {
        let wav = pcm_wav(&vec![0u8; 8000], 24_000);
        assert_eq!(as_wav(wav.clone()), Some(wav));
        let raw = vec![1u8; 5000];
        assert_eq!(as_wav(raw).map(|w| w.len()), Some(5044));
        assert_eq!(as_wav(b"{\"error\":1}".to_vec()), None);
        assert_eq!(as_wav(Vec::new()), None);
    }

    #[test]
    fn the_natural_voice_request_is_plain_json_and_only_new_models_get_instructions() {
        let new = Natural { model: "gpt-4o-mini-tts".into(), voice: "nova".into() };
        let body: serde_json::Value = serde_json::from_str(&natural_request("Привет", &new)).unwrap();
        assert_eq!(body["input"], "Привет");
        assert_eq!(body["voice"], "nova");
        assert_eq!(body["response_format"], "pcm");
        assert!(body["instructions"].is_string());
        let old = Natural { model: "tts-1".into(), voice: "alloy".into() };
        let body: serde_json::Value = serde_json::from_str(&natural_request("Hi", &old)).unwrap();
        assert!(body.get("instructions").is_none());
    }

    #[test]
    #[ignore = "needs an audio device and an installed voice"]
    fn speaks_aloud() {
        let wav = synthesize("Привет").expect("a voice synthesized the text");
        assert!(data_chunk(&wav).is_some());
        speak("Привет, это остров", 60, None);
        std::thread::sleep(std::time::Duration::from_secs(3));
    }
}
