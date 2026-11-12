use serde::{Deserialize, Serialize};
use std::sync::{Mutex, OnceLock, mpsc};
use std::time::{Duration, Instant};
use windows::Media::Playback::MediaPlayer;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoiceMusic {
    Unchanged,
    #[default]
    Lower,
    Pause,
}

impl VoiceMusic {
    pub const ALL: [Self; 3] = [Self::Unchanged, Self::Lower, Self::Pause];

    pub fn name(self) -> &'static str {
        match self {
            Self::Unchanged => "Keep music volume",
            Self::Lower => "Lower music volume",
            Self::Pause => "Pause music",
        }
    }

    pub fn step(self, forward: bool) -> Self {
        Self::ALL[(self as usize + if forward { 1 } else { 2 }) % Self::ALL.len()]
    }

    fn gain(self) -> f64 {
        if self == Self::Lower { 0.25 } else { 1.0 }
    }
}

struct Output {
    player: MediaPlayer,
    base: f64,
    gain: f64,
    start_gain: f64,
    changed: Instant,
    wants_play: bool,
    paused_by_voice: bool,
}

struct Focus {
    mode: VoiceMusic,
    output: Option<Output>,
}

static FOCUS: Mutex<Focus> = Mutex::new(Focus { mode: VoiceMusic::Unchanged, output: None });

pub fn set(mode: VoiceMusic) {
    let mut focus = FOCUS.lock().unwrap_or_else(|e| e.into_inner());
    if focus.mode == mode {
        return;
    }
    focus.mode = mode;
    let Some(out) = focus.output.as_mut() else { return };
    out.start_gain = out.gain;
    out.changed = Instant::now();
    drop(focus);
    let _ = worker().try_send(());
}

fn worker() -> &'static mpsc::SyncSender<()> {
    static TX: OnceLock<mpsc::SyncSender<()>> = OnceLock::new();
    TX.get_or_init(|| {
        let (tx, rx) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize};
            let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
            while rx.recv().is_ok() {
                while frame() {
                    std::thread::sleep(Duration::from_millis(16));
                }
            }
        });
        tx
    })
}

fn frame() -> bool {
    let mut focus = FOCUS.lock().unwrap_or_else(|e| e.into_inner());
    let mode = focus.mode;
    let Some(out) = focus.output.as_mut() else { return false };
    let pause = mode == VoiceMusic::Pause && out.wants_play;
    if pause != out.paused_by_voice {
        if pause {
            let _ = out.player.Pause();
        } else if out.wants_play {
            let _ = out.player.Play();
        }
        out.paused_by_voice = pause;
    }
    let t = (out.changed.elapsed().as_secs_f64() / 0.24).min(1.0);
    let target = mode.gain();
    if out.gain == target {
        return false;
    }
    out.gain = if t >= 1.0 { target } else { out.start_gain + (target - out.start_gain) * t * t * (3.0 - 2.0 * t) };
    let _ = out.player.SetVolume(out.base * out.gain);
    t < 1.0
}

pub(super) fn install(player: &MediaPlayer, volume: u8) -> windows::core::Result<()> {
    let mut focus = FOCUS.lock().unwrap_or_else(|e| e.into_inner());
    let gain = focus.mode.gain();
    let base = f64::from(volume.min(100)) / 100.0;
    let pause = focus.mode == VoiceMusic::Pause;
    player.SetVolume(base * gain)?;
    if !pause {
        player.Play()?;
    }
    focus.output = Some(Output {
        player: player.clone(),
        base,
        gain,
        start_gain: gain,
        changed: Instant::now(),
        wants_play: true,
        paused_by_voice: pause,
    });
    Ok(())
}

pub(super) fn volume(player: &MediaPlayer, volume: u8) -> windows::core::Result<()> {
    let mut focus = FOCUS.lock().unwrap_or_else(|e| e.into_inner());
    let base = f64::from(volume.min(100)) / 100.0;
    if let Some(out) = focus.output.as_mut().filter(|out| out.player == *player) {
        player.SetVolume(base * out.gain)?;
        out.base = base;
    }
    Ok(())
}

pub(super) fn playback(player: &MediaPlayer, playing: bool) {
    let mut focus = FOCUS.lock().unwrap_or_else(|e| e.into_inner());
    let pause = focus.mode == VoiceMusic::Pause;
    if let Some(out) = focus.output.as_mut().filter(|out| out.player == *player) {
        out.wants_play = playing;
        out.paused_by_voice = playing && pause;
        if playing && !pause {
            let _ = player.Play();
        } else {
            let _ = player.Pause();
        }
    }
}

pub(super) fn remove(player: &MediaPlayer) {
    let mut focus = FOCUS.lock().unwrap_or_else(|e| e.into_inner());
    if focus.output.as_ref().is_some_and(|out| out.player == *player) {
        focus.output = None;
    }
}
