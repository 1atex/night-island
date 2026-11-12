use std::f32::consts::TAU;
use std::sync::OnceLock;

use crate::data::sounds::{SoundPreset, SoundSettings};
use std::sync::{
    Mutex, RwLock,
    atomic::{AtomicU32, Ordering},
    mpsc,
};

pub const WM_SOUND_PICK: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 105;
pub const WM_SOUND_READY: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 106;
type Imported = Result<(Cue, String), String>;
static IMPORTED: Mutex<Option<Imported>> = Mutex::new(None);
static GENERATION: AtomicU32 = AtomicU32::new(0);
static PENDING: Mutex<Option<(Cue, u8, SoundSettings, u32)>> = Mutex::new(None);

fn configuration() -> &'static RwLock<SoundSettings> {
    static CONFIG: OnceLock<RwLock<SoundSettings>> = OnceLock::new();
    CONFIG.get_or_init(|| RwLock::new(SoundSettings::default()))
}

pub fn configure(settings: &SoundSettings) {
    let mut current = configuration().write().unwrap_or_else(|e| e.into_inner());
    if *current != *settings {
        GENERATION.fetch_add(1, Ordering::Relaxed);
        *current = settings.clone();
    }
}

const RATE: u32 = 16_000;
const STEPS: usize = 11;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cue {
    Heard,
    Done,
    Error,
    Reminder,
    Notification,
    Summary,
}

impl Cue {
    pub const OPTIONS: [Self; 6] =
        [Self::Heard, Self::Done, Self::Error, Self::Reminder, Self::Notification, Self::Summary];
    pub fn key(self) -> &'static str {
        match self {
            Self::Heard => "heard",
            Self::Done => "done",
            Self::Error => "error",
            Self::Notification => "notification",
            Self::Summary => "summary",
            Self::Reminder => "reminder",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Heard => "Assistant is listening",
            Self::Done => "Assistant finished",
            Self::Error => "Assistant error",
            Self::Notification => "Incoming notification",
            Self::Summary => "Notification summary",
            Self::Reminder => "Timer and reminder",
        }
    }

    #[cfg(test)]
    const ALL: [Cue; 6] = Self::OPTIONS;

    fn index(self) -> usize {
        self as usize
    }

    fn notes(self) -> &'static [(f32, f32)] {
        match self {
            Cue::Heard => &[(659.25, 0.075), (987.77, 0.11)],
            Cue::Done => &[(987.77, 0.06), (659.25, 0.12)],
            Cue::Error => &[(329.63, 0.09), (261.63, 0.16)],
            Cue::Notification => &[(880.0, 0.09)],
            Cue::Summary => &[(659.25, 0.065), (880.0, 0.12)],
            Cue::Reminder => &[(784.0, 0.11), (987.77, 0.11), (1174.66, 0.14)],
        }
    }
}

fn note(freq: f32, secs: f32, out: &mut Vec<i16>, gain: f32) {
    let n = (RATE as f32 * secs) as usize;
    let attack = (RATE as f32 * 0.006) as usize;
    for i in 0..n {
        let t = i as f32 / RATE as f32;
        let rise = if i < attack { i as f32 / attack as f32 } else { 1.0 };
        let fall = (-5.5 * t / secs).exp();
        let tone =
            (TAU * freq * t).sin() * 0.78 + (TAU * freq * 2.0 * t).sin() * 0.16 + (TAU * freq * 3.0 * t).sin() * 0.06;
        out.push((tone * rise * fall * gain * 0.55 * f32::from(i16::MAX)) as i16);
    }
}

fn samples(cue: Cue, volume: f32) -> Vec<i16> {
    let mut out = Vec::new();
    for &(freq, secs) in cue.notes() {
        note(freq, secs, &mut out, volume.clamp(0.0, 1.0));
    }
    out.extend(std::iter::repeat_n(0, (RATE / 50) as usize));
    out
}

fn wav(samples: &[i16]) -> Vec<u8> {
    crate::platform::dictation::wav(samples)
}

fn cache() -> &'static [[OnceLock<Vec<u8>>; STEPS]; 6] {
    static CACHE: [[OnceLock<Vec<u8>>; STEPS]; 6] = [const { [const { OnceLock::new() }; STEPS] }; 6];
    &CACHE
}

fn step(volume_pct: u8) -> usize {
    (usize::from(volume_pct.min(100)) + 5) / 10
}

pub fn bytes(cue: Cue, volume_pct: u8) -> &'static [u8] {
    let s = step(volume_pct);
    cache()[cue.index()][s].get_or_init(|| wav(&samples(cue, s as f32 / 10.0)))
}

pub fn stop() {
    GENERATION.fetch_add(1, Ordering::Relaxed);
    PENDING.lock().unwrap_or_else(|e| e.into_inner()).take();
}

pub fn play(cue: Cue, volume_pct: u8) {
    let settings = configuration().read().unwrap_or_else(|e| e.into_inner()).clone();
    if !settings.enabled || settings.volume == 0 || volume_pct == 0 {
        return;
    }
    let generation = GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
    *PENDING.lock().unwrap_or_else(|e| e.into_inner()) = Some((cue, volume_pct, settings, generation));
    let _ = worker().try_send(());
}

fn worker() -> &'static mpsc::SyncSender<()> {
    static WORKER: OnceLock<mpsc::SyncSender<()>> = OnceLock::new();
    WORKER.get_or_init(|| {
        let (tx, rx) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let mut cache = std::collections::BTreeMap::<String, Vec<u8>>::new();
            while rx.recv().is_ok() {
                let job = PENDING.lock().unwrap_or_else(|e| e.into_inner()).take();
                let Some((cue, volume, settings, generation)) = job else { continue };
                let custom = settings.custom.get(cue.key()).and_then(|path| {
                    if !cache.contains_key(path) {
                        if cache.len() >= 4 {
                            cache.clear();
                        }
                        if let Ok(bytes) = read_custom(std::path::Path::new(path)) {
                            if cache.values().map(Vec::len).sum::<usize>() + bytes.len() > 6 * 1024 * 1024 {
                                cache.clear();
                            }
                            cache.insert(path.clone(), bytes);
                        }
                    }
                    cache.get(path).and_then(|bytes| super::sound_output::decode(bytes))
                });
                let wave = custom.or_else(|| {
                    super::sound_output::decode(match settings.preset {
                        SoundPreset::Classic => bytes(cue, 100),
                        SoundPreset::Glass => glass(cue),
                    })
                });
                if let Some(wave) = wave {
                    let volume = (u16::from(volume.min(100)) * u16::from(settings.volume) / 100) as u8;
                    super::sound_output::play(wave, volume, || GENERATION.load(Ordering::Relaxed) == generation);
                }
            }
        });
        tx
    })
}

fn glass(cue: Cue) -> &'static [u8] {
    static CACHE: [OnceLock<Vec<u8>>; 6] = [const { OnceLock::new() }; 6];
    CACHE[cue.index()].get_or_init(|| {
        let notes: &[f32] = match cue {
            Cue::Heard => &[880.0, 1318.51],
            Cue::Done => &[1318.51, 1046.5],
            Cue::Error => &[440.0, 392.0],
            Cue::Notification => &[1174.66],
            Cue::Summary => &[783.99, 1046.50],
            Cue::Reminder => &[1046.5, 1318.51, 1567.98],
        };
        let duration = if cue == Cue::Heard { 0.20 } else { notes.len() as f32 * 0.085 + 0.33 };
        let spacing = if cue == Cue::Heard { 0.045 } else { 0.085 };
        let mut out = vec![0i16; (RATE as f32 * duration) as usize];
        for (index, frequency) in notes.iter().enumerate() {
            let start = (index as f32 * RATE as f32 * spacing) as usize;
            for (i, sample) in out.iter_mut().skip(start).enumerate() {
                let t = i as f32 / RATE as f32;
                let envelope = (t / 0.009).min(1.0) * (-12.0 * t).exp() * (1.0 - t / 0.58).max(0.0).powi(2);
                let tone = (TAU * frequency * t).sin() * 0.7 + (TAU * frequency * 2.002 * t).sin() * 0.2;
                *sample = sample.saturating_add((tone * envelope * 0.32 * f32::from(i16::MAX)) as i16);
            }
        }
        let length = out.len();
        for (i, sample) in out.iter_mut().enumerate().skip(length.saturating_sub(320)) {
            *sample = (f32::from(*sample) * (length - i - 1) as f32 / 320.0) as i16;
        }
        wav(&out)
    })
}

fn read_custom(path: &std::path::Path) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let file = std::fs::File::open(path).map_err(|_| "Could not read the sound")?;
    let mut data = Vec::new();
    file.take(6 * 1024 * 1024 + 1).read_to_end(&mut data).map_err(|_| "Could not read the sound")?;
    if data.len() > 6 * 1024 * 1024 || super::sound_output::decode(&data).is_none() {
        return Err("Choose a PCM 16-bit WAV up to 15 seconds".into());
    }
    Ok(data)
}

pub fn import_async(cue: Cue, path: String, hwnd: windows::Win32::Foundation::HWND) {
    let owner = hwnd.0 as isize;
    std::thread::spawn(move || {
        let result = (|| {
            use sha2::{Digest, Sha256};
            let data = read_custom(std::path::Path::new(&path))?;
            let root = std::env::var_os("APPDATA").ok_or("Could not save the sound")?;
            let folder = std::path::PathBuf::from(root).join("NightIsland").join("sounds");
            std::fs::create_dir_all(&folder).map_err(|_| "Could not save the sound")?;
            let destination = folder.join(format!("{:x}.wav", Sha256::digest(&data)));
            let temporary = destination.with_extension("tmp");
            std::fs::write(&temporary, data).map_err(|_| "Could not save the sound")?;
            std::fs::rename(&temporary, &destination).map_err(|_| "Could not save the sound")?;
            Ok((cue, destination.to_string_lossy().into_owned()))
        })();
        *IMPORTED.lock().unwrap_or_else(|e| e.into_inner()) = Some(result);
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::PostMessageW(
                Some(windows::Win32::Foundation::HWND(owner as _)),
                WM_SOUND_READY,
                windows::Win32::Foundation::WPARAM(0),
                windows::Win32::Foundation::LPARAM(0),
            );
        }
    });
}

pub fn take_import() -> Option<Imported> {
    IMPORTED.lock().unwrap_or_else(|e| e.into_inner()).take()
}

#[cfg(test)]
pub fn all() -> [Cue; 6] {
    Cue::ALL
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peak(data: &[u8]) -> i16 {
        data[44..].chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]]).saturating_abs()).max().unwrap_or(0)
    }

    #[test]
    fn both_presets_decode_and_custom_wav_validation_rejects_corruption() {
        for cue in Cue::OPTIONS {
            for data in [bytes(cue, 100), glass(cue)] {
                let decoded = super::super::sound_output::decode(data).unwrap();
                assert!(!decoded.samples.is_empty());
                assert!(decoded.samples.iter().all(|v| *v != i16::MAX && *v != i16::MIN));
                assert_ne!(bytes(cue, 100), glass(cue));
                for length in [0, 4, 12, 30, data.len() - 1] {
                    assert!(super::super::sound_output::decode(&data[..length]).is_none());
                }
            }
        }
        let mut malformed = bytes(Cue::Done, 100).to_vec();
        malformed[32..34].copy_from_slice(&999u16.to_le_bytes());
        assert!(super::super::sound_output::decode(&malformed).is_none());
        let mut malformed = bytes(Cue::Done, 100).to_vec();
        malformed[40..44].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(super::super::sound_output::decode(&malformed).is_none());
        assert!(super::super::sound_output::decode(&wav(&vec![0; RATE as usize * 16])).is_none());
    }

    #[test]
    fn every_cue_is_a_short_audible_wav() {
        for cue in all() {
            let data = bytes(cue, 100);
            assert_eq!(&data[..4], b"RIFF");
            let secs = (data.len() - 44) as f32 / 2.0 / RATE as f32;
            assert!(secs > 0.1 && secs < 0.6, "{cue:?} lasts {secs}");
            assert!(peak(data) > 3000, "{cue:?} is too quiet");
        }
    }

    #[test]
    fn volume_scales_the_loudness_and_zero_is_silent() {
        let loud = peak(bytes(Cue::Heard, 100));
        let quiet = peak(bytes(Cue::Heard, 30));
        assert!(quiet < loud / 2 && quiet > 0);
        assert_eq!(step(0), 0);
        assert_eq!(step(100), 10);
        assert_eq!(step(250), 10);
    }

    #[test]
    fn a_cue_never_clips() {
        for cue in all() {
            assert!(peak(bytes(cue, 100)) < i16::MAX);
        }
    }
}
