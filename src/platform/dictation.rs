use std::ffi::c_void;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::Media::Audio::{
    AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM, AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY,
    IAudioCaptureClient, IAudioClient, IMMDeviceEnumerator, MMDeviceEnumerator, WAVEFORMATEX, eCapture, eConsole,
};
use windows::Win32::System::Com::{CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};
use windows::core::Result;

pub const WM_VOICE: u32 = WM_APP + 25;

const RATE: u32 = 16_000;
pub const MAX_SECS: usize = 120;
const MIN_SAMPLES: usize = RATE as usize * 6 / 10;
const SILENCE_PEAK: i16 = 120;
const AUDCLNT_BUFFERFLAGS_SILENT: u32 = 0x2;

static RUNNING: AtomicBool = AtomicBool::new(false);
static WANTED: AtomicBool = AtomicBool::new(false);
static RESULT: Mutex<Option<std::result::Result<Vec<u8>, &'static str>>> = Mutex::new(None);

pub fn active() -> bool {
    RUNNING.load(Ordering::SeqCst)
}

pub fn start(hwnd: HWND) -> bool {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return false;
    }
    WANTED.store(true, Ordering::SeqCst);
    let raw = hwnd.0 as isize;
    std::thread::spawn(move || {
        let outcome = match capture() {
            Ok(samples) => finish(&samples),
            Err(_) => Err("The microphone could not be opened"),
        };
        if let Ok(mut r) = RESULT.lock() {
            *r = Some(outcome);
        }
        RUNNING.store(false, Ordering::SeqCst);
        unsafe {
            let _ = PostMessageW(Some(HWND(raw as *mut c_void)), WM_VOICE, WPARAM(0), LPARAM(0));
        }
    });
    true
}

pub fn stop() {
    WANTED.store(false, Ordering::SeqCst);
}

pub fn take() -> Option<std::result::Result<Vec<u8>, &'static str>> {
    RESULT.lock().ok()?.take()
}

fn finish(samples: &[i16]) -> std::result::Result<Vec<u8>, &'static str> {
    if samples.len() < MIN_SAMPLES {
        return Err("Too short, nothing was recorded");
    }
    if samples.iter().map(|s| s.saturating_abs()).max().unwrap_or(0) < SILENCE_PEAK {
        return Err("Nothing was heard (is the microphone muted?)");
    }
    Ok(wav(samples))
}

pub fn wav(samples: &[i16]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + samples.len() * 2);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&(RATE * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

fn capture() -> Result<Vec<i16>> {
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let device = enumerator.GetDefaultAudioEndpoint(eCapture, eConsole)?;
        let client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;
        let format = WAVEFORMATEX {
            wFormatTag: 1,
            nChannels: 1,
            nSamplesPerSec: RATE,
            nAvgBytesPerSec: RATE * 2,
            nBlockAlign: 2,
            wBitsPerSample: 16,
            cbSize: 0,
        };
        client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY,
            2_000_000,
            0,
            &format,
            None,
        )?;
        let capture: IAudioCaptureClient = client.GetService()?;
        client.Start()?;
        let mut samples: Vec<i16> = Vec::with_capacity(RATE as usize * 10);
        while WANTED.load(Ordering::SeqCst) && samples.len() < RATE as usize * MAX_SECS {
            std::thread::sleep(Duration::from_millis(30));
            while capture.GetNextPacketSize().unwrap_or(0) > 0 {
                let (mut data, mut frames, mut flags) = (std::ptr::null_mut(), 0u32, 0u32);
                if capture.GetBuffer(&mut data, &mut frames, &mut flags, None, None).is_err() {
                    break;
                }
                let n = frames as usize;
                if flags & AUDCLNT_BUFFERFLAGS_SILENT != 0 || data.is_null() {
                    samples.resize(samples.len() + n, 0);
                } else {
                    samples.extend((0..n).map(|i| (data as *const i16).add(i).read_unaligned()));
                }
                let _ = capture.ReleaseBuffer(frames);
            }
        }
        let _ = client.Stop();
        Ok(samples)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wav_file_has_a_correct_header() {
        let file = wav(&[0, 1, -1, 300]);
        assert_eq!(&file[..4], b"RIFF");
        assert_eq!(&file[8..16], b"WAVEfmt ");
        assert_eq!(u32::from_le_bytes(file[24..28].try_into().unwrap()), RATE);
        assert_eq!(u32::from_le_bytes(file[40..44].try_into().unwrap()), 8);
        assert_eq!(file.len(), 44 + 8);
        assert_eq!(u32::from_le_bytes(file[4..8].try_into().unwrap()) as usize, file.len() - 8);
    }

    #[test]
    #[ignore = "needs a microphone"]
    fn records_the_default_microphone() {
        WANTED.store(true, Ordering::SeqCst);
        let stopper = std::thread::spawn(|| {
            std::thread::sleep(Duration::from_millis(1500));
            WANTED.store(false, Ordering::SeqCst);
        });
        let samples = capture().expect("the microphone opened");
        stopper.join().unwrap();
        let peak = samples.iter().map(|s| s.saturating_abs()).max().unwrap_or(0);
        println!("{} samples ({:.2} s), peak {peak}", samples.len(), samples.len() as f32 / RATE as f32);
        assert!(samples.len() > RATE as usize);
    }

    #[test]
    fn short_and_silent_recordings_are_refused() {
        assert!(finish(&[5000; 100]).is_err());
        assert!(finish(&vec![3; MIN_SAMPLES * 2]).is_err());
        assert!(finish(&vec![4000; MIN_SAMPLES * 2]).is_ok());
    }
}
