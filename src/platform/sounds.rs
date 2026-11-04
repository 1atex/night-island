use std::f32::consts::TAU;
use std::sync::OnceLock;

use windows::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};
use windows::core::PCWSTR;

const RATE: u32 = 16_000;
const STEPS: usize = 11;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cue {
    Heard,
    Done,
    Error,
}

impl Cue {
    const ALL: [Cue; 3] = [Cue::Heard, Cue::Done, Cue::Error];

    fn index(self) -> usize {
        self as usize
    }

    fn notes(self) -> &'static [(f32, f32)] {
        match self {
            Cue::Heard => &[(659.25, 0.075), (987.77, 0.11)],
            Cue::Done => &[(987.77, 0.06), (659.25, 0.12)],
            Cue::Error => &[(329.63, 0.09), (261.63, 0.16)],
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

fn cache() -> &'static [[OnceLock<Vec<u8>>; STEPS]; 3] {
    static CACHE: [[OnceLock<Vec<u8>>; STEPS]; 3] = [const { [const { OnceLock::new() }; STEPS] }; 3];
    &CACHE
}

fn step(volume_pct: u8) -> usize {
    (usize::from(volume_pct.min(100)) + 5) / 10
}

pub fn bytes(cue: Cue, volume_pct: u8) -> &'static [u8] {
    let s = step(volume_pct);
    cache()[cue.index()][s].get_or_init(|| wav(&samples(cue, s as f32 / 10.0)))
}

pub fn play(cue: Cue, volume_pct: u8) {
    if volume_pct == 0 {
        return;
    }
    let data = bytes(cue, volume_pct);
    unsafe {
        let _ = PlaySoundW(PCWSTR(data.as_ptr().cast()), None, SND_MEMORY | SND_ASYNC | SND_NODEFAULT);
    }
}

pub fn all() -> [Cue; 3] {
    Cue::ALL
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peak(data: &[u8]) -> i16 {
        data[44..].chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]]).saturating_abs()).max().unwrap_or(0)
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
