use std::ffi::c_void;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

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
const ECHO_PAUSE: Duration = Duration::from_millis(600);

fn post_spoken(raw: isize) {
    unsafe {
        let _ = PostMessageW(Some(HWND(raw as *mut c_void)), WM_SPOKEN, WPARAM(0), LPARAM(0));
    }
}

const MAX_SPEECH_BYTES: usize = 8 * 1024 * 1024;
const NATURAL_RATE: u32 = 24_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Natural {
    pub provider: crate::services::assistant::TtsProvider,
    pub base: String,
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

fn canonical_wav(pcm: &[u8], rate: u32, channels: u16) -> Vec<u8> {
    let block = usize::from(channels) * 2;
    let pcm = &pcm[..pcm.len() / block * block];
    let mut out = Vec::with_capacity(44 + pcm.len());
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + pcm.len() as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * block as u32).to_le_bytes());
    out.extend_from_slice(&(block as u16).to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(pcm.len() as u32).to_le_bytes());
    out.extend_from_slice(pcm);
    out
}

fn normalize_wav(bytes: &[u8]) -> Option<Vec<u8>> {
    if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }
    let mut format: Option<(u16, u32)> = None;
    let mut at = 12usize;
    while at + 8 <= bytes.len() {
        let id = &bytes[at..at + 4];
        let size = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().ok()?) as usize;
        let body = at + 8;
        if id == b"fmt " && size >= 16 && body + 16 <= bytes.len() {
            let tag = u16::from_le_bytes(bytes[body..body + 2].try_into().ok()?);
            let channels = u16::from_le_bytes(bytes[body + 2..body + 4].try_into().ok()?);
            let rate = u32::from_le_bytes(bytes[body + 4..body + 8].try_into().ok()?);
            let bits = u16::from_le_bytes(bytes[body + 14..body + 16].try_into().ok()?);
            if tag != 1 || bits != 16 || !(1..=2).contains(&channels) || !(8_000..=96_000).contains(&rate) {
                return None;
            }
            format = Some((channels, rate));
        } else if id == b"data" {
            let (channels, rate) = format?;
            let available = bytes.len() - body;
            let length = if size == 0 || size > available { available } else { size };
            let data = &bytes[body..body + length];
            return (data.len() >= rate as usize / 20).then(|| canonical_wav(data, rate, channels));
        }
        at = body.checked_add(size)?.checked_add(size & 1)?;
    }
    None
}

fn as_wav(bytes: Vec<u8>) -> Option<Vec<u8>> {
    if bytes.starts_with(b"RIFF") {
        return normalize_wav(&bytes);
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

fn eleven_request(text: &str, natural: &Natural) -> String {
    serde_json::json!({ "text": text, "model_id": natural.model }).to_string()
}

pub fn eleven_url(base: &str, voice: &str) -> String {
    let base = base.trim_end_matches('/').trim_end_matches("/v1");
    format!("{base}/v1/text-to-speech/{voice}?output_format=wav_24000")
}

fn fish_request(text: &str, natural: &Natural) -> String {
    let mut body = serde_json::json!({
        "text": text,
        "format": "wav",
        "sample_rate": NATURAL_RATE,
        "latency": "balanced",
    });
    if natural.voice != crate::services::assistant::FISH_DEFAULT_VOICE {
        body["reference_id"] = natural.voice.clone().into();
    }
    body.to_string()
}

pub fn fish_url(base: &str) -> String {
    let base = base.trim_end_matches('/').trim_end_matches("/v1");
    format!("{base}/v1/tts")
}

fn natural_wav(text: &str, natural: &Natural) -> Option<Vec<u8>> {
    use crate::services::assistant::TtsProvider;
    let Some((base, key)) = crate::services::ai::tts_credentials(&natural.base, natural.provider) else {
        crate::log_warn!("The natural voice has no key or a wrong address: see the voice service settings");
        return None;
    };
    let bearer = format!("Bearer {key}");
    let (url, headers, body): (String, Vec<(&str, &str)>, String) = match natural.provider {
        TtsProvider::Eleven => {
            (eleven_url(&base, &natural.voice), vec![("xi-api-key", key.as_str())], eleven_request(text, natural))
        }
        TtsProvider::Fish => (
            fish_url(&base),
            vec![("Authorization", bearer.as_str()), ("model", natural.model.as_str())],
            fish_request(text, natural),
        ),
        TtsProvider::OpenAi => {
            (format!("{base}/audio/speech"), vec![("Authorization", bearer.as_str())], natural_request(text, natural))
        }
    };
    let reply =
        crate::services::http::post_json_for_bytes(&url, &headers, &body, MAX_SPEECH_BYTES, Some("NightIsland/0.1"))?;
    if reply.status != 200 {
        let why: String = String::from_utf8_lossy(&reply.body).chars().filter(|c| !c.is_control()).take(160).collect();
        crate::log_warn!("The natural voice answered with status {}: {why}", reply.status);
        return None;
    }
    as_wav(reply.body)
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Voice {
    pub local: bool,
    pub natural: Option<Natural>,
}

pub fn speak(text: &str, volume_pct: u8, voice: Voice) {
    start(text, volume_pct, None, voice);
}

pub fn speak_then_notify(text: &str, volume_pct: u8, hwnd: HWND, voice: Voice) {
    start(text, volume_pct, Some(hwnd.0 as isize), voice);
}

fn render(text: &str, voice: &Voice) -> Option<Vec<u8>> {
    if voice.local {
        match crate::platform::localvoice::speak_wav(text) {
            Some(wav) => return Some(wav),
            None => crate::log_warn!("The local voice could not speak, trying the next one"),
        }
    }
    if let Some(natural) = &voice.natural {
        match natural_wav(text, natural) {
            Some(wav) => return Some(wav),
            None => crate::log_info!("The natural voice is not available, the system voice speaks"),
        }
    }
    synthesize(text).ok()
}

fn start(text: &str, volume_pct: u8, done: Option<isize>, voice: Voice) {
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
        let started = Instant::now();
        let (sender, parts) = mpsc::sync_channel::<Option<Vec<u8>>>(2);
        let pieces = chunks(&text, CHUNK_CHARS);
        let maker = voice.clone();
        std::thread::spawn(move || {
            unsafe {
                let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            }
            for piece in pieces {
                if GENERATION.load(Ordering::SeqCst) != mine {
                    break;
                }
                if sender.send(render(&piece, &maker)).is_err() {
                    break;
                }
            }
        });
        let mut first = true;
        for part in parts {
            if GENERATION.load(Ordering::SeqCst) != mine {
                break;
            }
            let Some(mut wav) = part.and_then(|w| normalize_wav(&w)) else {
                crate::log_warn!("A piece of speech was not a sound that can be played safely, skipped");
                continue;
            };
            if first {
                first = false;
                crate::log_info!("The voice starts after {} ms", started.elapsed().as_millis());
            }
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

const CHUNK_CHARS: usize = 170;

fn sentences(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut current = String::new();
    for (i, &c) in chars.iter().enumerate() {
        current.push(c);
        let ends = matches!(c, '.' | '!' | '?' | '\u{2026}' | '\n') || (c == ';' && chars.get(i + 1) == Some(&' '));
        let boundary = chars.get(i + 1).is_none_or(|n| n.is_whitespace());
        if ends && boundary {
            out.push(std::mem::take(&mut current).trim().to_string());
        }
    }
    out.push(current.trim().to_string());
    out.retain(|s| s.chars().any(char::is_alphanumeric));
    out
}

fn hard_split(sentence: &str, max: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest: Vec<char> = sentence.chars().collect();
    while rest.len() > max {
        let window = &rest[..max];
        let cut = window
            .iter()
            .rposition(|c| matches!(c, ',' | ';' | ':'))
            .or_else(|| window.iter().rposition(|c| c.is_whitespace()))
            .map_or(max, |at| at + 1);
        let head: String = rest[..cut].iter().collect();
        out.push(head.trim().to_string());
        rest = rest[cut..].to_vec();
        while rest.first().is_some_and(|c| c.is_whitespace()) {
            rest.remove(0);
        }
    }
    if !rest.is_empty() {
        out.push(rest.iter().collect());
    }
    out
}

pub fn chunks(text: &str, max: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();
    for (i, sentence) in sentences(text).into_iter().enumerate() {
        let parts = if sentence.chars().count() > max { hard_split(&sentence, max) } else { vec![sentence] };
        for part in parts {
            let joined = current.chars().count() + 1 + part.chars().count();
            if !current.is_empty() && (i == 0 || joined > max) {
                out.push(std::mem::take(&mut current));
            }
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(&part);
        }
        if i == 0 && !current.is_empty() {
            out.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
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
    fn the_elevenlabs_request_follows_its_documented_shape() {
        let natural = Natural {
            provider: crate::services::assistant::TtsProvider::Eleven,
            base: "https://api.elevenlabs.io".into(),
            model: "eleven_multilingual_v2".into(),
            voice: "21m00Tcm4TlvDq8ikWAM".into(),
        };
        let body: serde_json::Value = serde_json::from_str(&eleven_request("Привет", &natural)).unwrap();
        assert_eq!(body["text"], "Привет");
        assert_eq!(body["model_id"], "eleven_multilingual_v2");
        assert_eq!(
            eleven_url("https://api.elevenlabs.io", "21m00Tcm4TlvDq8ikWAM"),
            "https://api.elevenlabs.io/v1/text-to-speech/21m00Tcm4TlvDq8ikWAM?output_format=wav_24000"
        );
        assert_eq!(
            eleven_url("https://api.us.elevenlabs.io/v1/", "abc"),
            "https://api.us.elevenlabs.io/v1/text-to-speech/abc?output_format=wav_24000",
            "a base typed with /v1 does not double it"
        );
    }

    fn streamed_wav(declared: u32, extra: &[u8], pcm: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&u32::MAX.to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&24_000u32.to_le_bytes());
        out.extend_from_slice(&48_000u32.to_le_bytes());
        out.extend_from_slice(&2u16.to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(extra);
        out.extend_from_slice(b"data");
        out.extend_from_slice(&declared.to_le_bytes());
        out.extend_from_slice(pcm);
        out
    }

    #[test]
    fn a_streamed_wav_with_unknown_sizes_is_rewritten_with_the_real_ones() {
        let pcm = vec![7u8; 24_000];
        for declared in [u32::MAX, 0, 24_000, 999_999_999] {
            let fixed = as_wav(streamed_wav(declared, b"", &pcm)).expect("repaired");
            assert_eq!(fixed.len(), 44 + 24_000, "declared {declared}");
            assert_eq!(u32::from_le_bytes(fixed[4..8].try_into().unwrap()) as usize, fixed.len() - 8);
            assert_eq!(u32::from_le_bytes(fixed[40..44].try_into().unwrap()), 24_000);
            assert_eq!(u32::from_le_bytes(fixed[24..28].try_into().unwrap()), 24_000);
        }
        let mut extra = b"LIST".to_vec();
        extra.extend_from_slice(&5u32.to_le_bytes());
        extra.extend_from_slice(b"abcde\0");
        let with_list = as_wav(streamed_wav(u32::MAX, &extra, &pcm)).expect("a LIST chunk is skipped");
        assert_eq!(with_list.len(), 44 + 24_000);
        let odd = as_wav(streamed_wav(u32::MAX, b"", &pcm[..24_001 - 1 - 1])).expect("an odd tail is cut");
        assert_eq!(odd.len() % 2, 0);
    }

    #[test]
    fn a_wav_that_cannot_be_played_safely_is_refused() {
        assert!(as_wav(b"RIFF".to_vec()).is_none());
        assert!(as_wav(streamed_wav(u32::MAX, b"", &[1u8; 10])).is_none(), "too short to be speech");
        let mut float = streamed_wav(u32::MAX, b"", &[1u8; 24_000]);
        float[20] = 3;
        assert!(as_wav(float).is_none(), "only 16-bit integer sound is played");
        let mut no_data = streamed_wav(0, b"", &[]);
        no_data.truncate(36);
        assert!(as_wav(no_data).is_none());
    }

    #[test]
    fn the_fish_audio_request_follows_its_documented_shape() {
        let mut natural = Natural {
            provider: crate::services::assistant::TtsProvider::Fish,
            base: "https://api.fish.audio".into(),
            model: "s2.1-pro".into(),
            voice: "default".into(),
        };
        let body: serde_json::Value = serde_json::from_str(&fish_request("Привет", &natural)).unwrap();
        assert_eq!(body["text"], "Привет");
        assert_eq!(body["format"], "wav");
        assert_eq!(body["sample_rate"], 24000);
        assert!(body.get("reference_id").is_none(), "the default voice sends no reference");
        natural.voice = "0123456789abcdef0123456789abcdef".into();
        let body: serde_json::Value = serde_json::from_str(&fish_request("Hi", &natural)).unwrap();
        assert_eq!(body["reference_id"], "0123456789abcdef0123456789abcdef");
        assert_eq!(fish_url("https://api.fish.audio"), "https://api.fish.audio/v1/tts");
        assert_eq!(fish_url("https://api.fish.audio/v1/"), "https://api.fish.audio/v1/tts");
    }

    #[test]
    fn the_first_sentence_goes_alone_so_the_sound_starts_early() {
        let text = "Привет, я тут. Сегодня в Иваново облачно, около трёх градусов. Завтра снег! К выходным потеплеет.";
        let parts = chunks(text, 80);
        assert_eq!(parts[0], "Привет, я тут.");
        assert_eq!(parts.join(" "), text);
        assert!(parts.iter().all(|p| p.chars().count() <= 80), "{parts:?}");
        assert!(parts.len() >= 3);
    }

    #[test]
    fn numbers_with_a_dot_and_abbreviation_free_text_are_not_cut_in_the_middle() {
        let parts = chunks("Температура 3.5 градуса. Ветер 2.1 метра в секунду.", 200);
        assert_eq!(parts, ["Температура 3.5 градуса.", "Ветер 2.1 метра в секунду."]);
    }

    #[test]
    fn a_very_long_sentence_is_cut_at_commas_then_at_spaces() {
        let sentence = format!("{}.", ["слово"; 60].join(" "));
        let parts = chunks(&sentence, 50);
        assert!(parts.len() > 3 && parts.iter().all(|p| p.chars().count() <= 50), "{parts:?}");
        assert_eq!(parts.join(" "), sentence);
        let with_commas = chunks("раз два три, четыре пять шесть, семь восемь девять, десять", 25);
        assert!(with_commas.iter().all(|p| p.chars().count() <= 25), "{with_commas:?}");
        assert!(with_commas[0].ends_with(','));
    }

    #[test]
    fn empty_noise_and_punctuation_make_no_pieces() {
        assert!(chunks("", 100).is_empty());
        assert!(chunks("  ...  ?! ", 100).is_empty());
        assert_eq!(chunks("Без точки", 100), ["Без точки"]);
        assert_eq!(chunks("Одно.", 100), ["Одно."]);
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
        let new = Natural {
            provider: crate::services::assistant::TtsProvider::OpenAi,
            base: "https://api.openai.com/v1".into(),
            model: "gpt-4o-mini-tts".into(),
            voice: "nova".into(),
        };
        let body: serde_json::Value = serde_json::from_str(&natural_request("Привет", &new)).unwrap();
        assert_eq!(body["input"], "Привет");
        assert_eq!(body["voice"], "nova");
        assert_eq!(body["response_format"], "pcm");
        assert!(body["instructions"].is_string());
        let old = Natural {
            provider: crate::services::assistant::TtsProvider::OpenAi,
            base: "https://api.openai.com/v1".into(),
            model: "tts-1".into(),
            voice: "alloy".into(),
        };
        let body: serde_json::Value = serde_json::from_str(&natural_request("Hi", &old)).unwrap();
        assert!(body.get("instructions").is_none());
    }

    #[test]
    #[ignore = "needs an audio device and an installed voice"]
    fn speaks_aloud() {
        let wav = synthesize("Привет").expect("a voice synthesized the text");
        assert!(data_chunk(&wav).is_some());
        speak("Привет, это остров", 60, Voice::default());
        std::thread::sleep(std::time::Duration::from_secs(3));
    }
}
