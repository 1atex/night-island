use windows::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows::Win32::Media::Audio::*;
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject};
use windows::core::PSTR;

#[repr(C, align(8))]
struct Header(WAVEHDR);

pub(super) struct Wave {
    pub samples: Vec<i16>,
    pub rate: u32,
    pub channels: u16,
}

pub(super) fn decode(bytes: &[u8]) -> Option<Wave> {
    if bytes.len() < 44 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }
    let u16_at = |n| Some(u16::from_le_bytes(bytes.get(n..n + 2)?.try_into().ok()?));
    let u32_at = |n| Some(u32::from_le_bytes(bytes.get(n..n + 4)?.try_into().ok()?));
    let end = usize::try_from(u32_at(4)?).ok()?.checked_add(8)?;
    if end > bytes.len() || end < 44 {
        return None;
    }
    let (mut format, mut data) = (None, None);
    let mut at = 12usize;
    while at.checked_add(8)? <= end {
        let size = usize::try_from(u32_at(at + 4)?).ok()?;
        let from = at + 8;
        let to = from.checked_add(size)?;
        if to > end {
            return None;
        }
        match bytes.get(at..at + 4)? {
            b"fmt " if size >= 16 => {
                if format.is_some() || u16_at(from)? != 1 || u16_at(from + 14)? != 16 {
                    return None;
                }
                let (channels, rate) = (u16_at(from + 2)?, u32_at(from + 4)?);
                if !(1..=2).contains(&channels)
                    || !(8000..=96000).contains(&rate)
                    || u16_at(from + 12)? != channels * 2
                    || u32_at(from + 8)? != rate * u32::from(channels) * 2
                {
                    return None;
                }
                format = Some((channels, rate));
            }
            b"data" => {
                if data.is_some() {
                    return None;
                }
                data = Some(from..to);
            }
            _ => {}
        }
        at = to.checked_add(size % 2)?;
    }
    let (channels, rate) = format?;
    let data = bytes.get(data?)?;
    if data.is_empty()
        || data.len() % (usize::from(channels) * 2) != 0
        || data.len() as u64 > u64::from(rate) * u64::from(channels) * 2 * 15
    {
        return None;
    }
    Some(Wave { samples: data.chunks_exact(2).map(|v| i16::from_le_bytes([v[0], v[1]])).collect(), rate, channels })
}

pub(super) fn play(mut wave: Wave, volume: u8, current: impl Fn() -> bool) -> bool {
    if volume == 0 || !current() {
        return true;
    }
    for sample in &mut wave.samples {
        *sample = (i32::from(*sample) * i32::from(volume.min(100)) / 100) as i16;
    }
    unsafe {
        let Ok(event) = CreateEventW(None, false, false, None) else { return false };
        let mut device = HWAVEOUT::default();
        let format = WAVEFORMATEX {
            wFormatTag: 1,
            nChannels: wave.channels,
            nSamplesPerSec: wave.rate,
            nAvgBytesPerSec: wave.rate * u32::from(wave.channels) * 2,
            nBlockAlign: wave.channels * 2,
            wBitsPerSample: 16,
            cbSize: 0,
        };
        if waveOutOpen(Some(&mut device), WAVE_MAPPER, &format, Some(event.0 as usize), None, CALLBACK_EVENT) != 0 {
            let _ = CloseHandle(event);
            return false;
        }
        let mut header = Box::new(Header(WAVEHDR {
            lpData: PSTR(wave.samples.as_mut_ptr().cast()),
            dwBufferLength: (wave.samples.len() * 2) as u32,
            ..Default::default()
        }));
        let size = size_of::<WAVEHDR>() as u32;
        let mut written = false;
        if waveOutPrepareHeader(device, &mut header.0, size) == 0 {
            if waveOutWrite(device, &mut header.0, size) == 0 {
                written = true;
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(17);
                while std::ptr::read_volatile(std::ptr::addr_of!(header.0.dwFlags)) & WHDR_DONE == 0
                    && std::time::Instant::now() < deadline
                    && current()
                {
                    let status = WaitForSingleObject(event, 32);
                    if status != WAIT_OBJECT_0 && status != WAIT_TIMEOUT {
                        break;
                    }
                }
            }
            let _ = waveOutReset(device);
            let _ = waveOutUnprepareHeader(device, &mut header.0, size);
        }
        let closed = waveOutClose(device) == 0;
        let _ = CloseHandle(event);
        written && closed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "Checks independent audio output with a silent buffer and cancellation"]
    fn native_sound_output_cancels_without_waiting_for_the_whole_file() {
        let start = std::time::Instant::now();
        let wave = Wave { samples: vec![0; 16000 * 5], rate: 16000, channels: 1 };
        assert!(play(wave, 100, || start.elapsed().as_millis() < 60));
        assert!(start.elapsed().as_millis() < 1000);
    }
}
