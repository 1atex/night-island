use std::collections::VecDeque;
use std::ffi::c_void;
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{SyncSender, TrySendError, sync_channel};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{HMODULE, HWND, LPARAM, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_UNKNOWN;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::Graphics::Gdi::DeleteObject;
use windows::Win32::Media::Audio::*;
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::{CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx};
use windows::Win32::UI::WindowsAndMessaging::{
    CURSOR_SHOWING, CURSORINFO, DI_NORMAL, DrawIconEx, GetCursorInfo, GetIconInfo, ICONINFO, PostMessageW, WM_APP,
};
use windows::core::{Error, HSTRING, Interface, Result};

use crate::data::settings::RecordFormat;

pub const WM_RECORDED: u32 = WM_APP + 11;

const SAMPLE_RATE: u64 = 48_000;
const POOL: usize = 8;
const HEARTBEAT: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub format: RecordFormat,
    pub fps: u32,
    pub system_audio: bool,
    pub mic: bool,
}

pub enum Finished {
    Saved(PathBuf, Duration, Vec<&'static str>),
    Failed(String),
}

static RESULT: Mutex<Option<Finished>> = Mutex::new(None);
static PEAKS: [std::sync::atomic::AtomicU32; 2] = [const { std::sync::atomic::AtomicU32::new(0) }; 2];

pub fn take_result() -> Option<Finished> {
    RESULT.lock().ok().and_then(|mut g| g.take())
}

pub struct Recording {
    stop: Arc<AtomicBool>,
    pub started: Instant,
    _thread: JoinHandle<()>,
}

impl Recording {
    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

pub fn start(owner: HWND, region: RECT, path: PathBuf, opts: Options) -> Recording {
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let raw = owner.0 as isize;
    let started = Instant::now();
    let thread = std::thread::spawn(move || {
        let outcome = match run(region, &path, opts, &flag) {
            Ok((took, notes)) => Finished::Saved(path.clone(), took, notes),
            Err(e) => {
                let _ = std::fs::remove_file(&path);
                Finished::Failed(e.message())
            }
        };
        if let Ok(mut slot) = RESULT.lock() {
            *slot = Some(outcome);
        }
        unsafe {
            let _ = PostMessageW(Some(HWND(raw as *mut c_void)), WM_RECORDED, WPARAM(0), LPARAM(0));
        }
    });
    Recording { stop, started, _thread: thread }
}

pub fn default_path(format: RecordFormat) -> PathBuf {
    use windows::Win32::UI::Shell::{FOLDERID_Videos, KF_FLAG_CREATE, SHGetKnownFolderPath};
    let base = unsafe {
        SHGetKnownFolderPath(&FOLDERID_Videos, KF_FLAG_CREATE, None).ok().and_then(|p| {
            let s = p.to_string().ok();
            windows::Win32::System::Com::CoTaskMemFree(Some(p.0 as *const _));
            s
        })
    };
    let dir = base.map_or_else(std::env::temp_dir, PathBuf::from).join("NightIsland");
    let ext = if format == RecordFormat::Gif { "gif" } else { "mp4" };
    dir.join(format!("Recording_{}.{ext}", chrono::Local::now().format("%Y%m%d_%H%M%S")))
}

struct Agile<T>(T);
unsafe impl<T> Send for Agile<T> {}
unsafe impl<T> Sync for Agile<T> {}

struct Capture {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    dupl: IDXGIOutputDuplication,
    desktop: RECT,
    rel: RECT,
    w: u32,
    h: u32,
    clean: ID3D11Texture2D,
    pool: Vec<ID3D11Texture2D>,
    next: usize,
    staging: Option<ID3D11Texture2D>,
    last_cursor: (i32, i32, isize, bool),
    hot: (isize, POINT),
}

fn texture(device: &ID3D11Device, w: u32, h: u32, staging: bool) -> Result<ID3D11Texture2D> {
    let desc = D3D11_TEXTURE2D_DESC {
        Width: w,
        Height: h,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        Usage: if staging { D3D11_USAGE_STAGING } else { D3D11_USAGE_DEFAULT },
        BindFlags: if staging { 0 } else { (D3D11_BIND_SHADER_RESOURCE.0 | D3D11_BIND_RENDER_TARGET.0) as u32 },
        CPUAccessFlags: if staging { D3D11_CPU_ACCESS_READ.0 as u32 } else { 0 },
        MiscFlags: if staging { 0 } else { D3D11_RESOURCE_MISC_GDI_COMPATIBLE.0 as u32 },
    };
    let mut tex = None;
    unsafe { device.CreateTexture2D(&desc, None, Some(&mut tex))? };
    tex.ok_or_else(Error::empty)
}

impl Capture {
    fn new(region: RECT) -> Result<Self> {
        unsafe {
            let factory: IDXGIFactory1 = CreateDXGIFactory1()?;
            let (cx, cy) = ((region.left + region.right) / 2, (region.top + region.bottom) / 2);
            let mut found = None;
            let mut ai = 0;
            'search: while let Ok(adapter) = factory.EnumAdapters1(ai) {
                let mut oi = 0;
                while let Ok(output) = adapter.EnumOutputs(oi) {
                    let d = output.GetDesc()?.DesktopCoordinates;
                    if cx >= d.left && cx < d.right && cy >= d.top && cy < d.bottom {
                        found = Some((adapter.clone(), output, d));
                        break 'search;
                    }
                    oi += 1;
                }
                ai += 1;
            }
            let (adapter, output, mon) = found.ok_or_else(Error::empty)?;

            let left = region.left.max(mon.left) - mon.left;
            let top = region.top.max(mon.top) - mon.top;
            let right = region.right.min(mon.right) - mon.left;
            let bottom = region.bottom.min(mon.bottom) - mon.top;
            let (w, h) = (((right - left) & !1).max(2) as u32, ((bottom - top) & !1).max(2) as u32);
            let rel = RECT { left, top, right: left + w as i32, bottom: top + h as i32 };
            let desktop = RECT {
                left: mon.left + left,
                top: mon.top + top,
                right: mon.left + rel.right,
                bottom: mon.top + rel.bottom,
            };

            let (mut device, mut context) = (None, None);
            D3D11CreateDevice(
                &adapter.cast::<IDXGIAdapter>()?,
                D3D_DRIVER_TYPE_UNKNOWN,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT | D3D11_CREATE_DEVICE_VIDEO_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )?;
            let (device, context) = (device.ok_or_else(Error::empty)?, context.ok_or_else(Error::empty)?);
            let _ = device.cast::<ID3D11Multithread>()?.SetMultithreadProtected(true);

            let dupl = output.cast::<IDXGIOutput1>()?.DuplicateOutput(&device)?;
            let clean = texture(&device, w, h, false)?;
            let pool = (0..POOL).map(|_| texture(&device, w, h, false)).collect::<Result<Vec<_>>>()?;
            Ok(Self {
                device,
                context,
                dupl,
                desktop,
                rel,
                w,
                h,
                clean,
                pool,
                next: 0,
                staging: None,
                last_cursor: (i32::MIN, i32::MIN, 0, false),
                hot: (0, POINT::default()),
            })
        }
    }

    fn pump(&mut self, timeout_ms: u32) -> Result<bool> {
        unsafe {
            let mut info = DXGI_OUTDUPL_FRAME_INFO::default();
            let mut resource: Option<IDXGIResource> = None;
            match self.dupl.AcquireNextFrame(timeout_ms, &mut info, &mut resource) {
                Ok(()) => {}
                Err(e) if e.code() == DXGI_ERROR_WAIT_TIMEOUT => return Ok(false),
                Err(e) => return Err(e),
            }
            let changed = info.LastPresentTime != 0;
            if changed && let Some(res) = resource {
                let desktop: ID3D11Texture2D = res.cast()?;
                let region = D3D11_BOX {
                    left: self.rel.left as u32,
                    top: self.rel.top as u32,
                    front: 0,
                    right: self.rel.right as u32,
                    bottom: self.rel.bottom as u32,
                    back: 1,
                };
                self.context.CopySubresourceRegion(&self.clean, 0, 0, 0, 0, &desktop, 0, Some(&region));
            }
            self.dupl.ReleaseFrame()?;
            Ok(changed)
        }
    }

    fn cursor_changed(&mut self) -> bool {
        let mut ci = CURSORINFO { cbSize: size_of::<CURSORINFO>() as u32, ..Default::default() };
        let now = if unsafe { GetCursorInfo(&mut ci) }.is_ok() {
            (ci.ptScreenPos.x, ci.ptScreenPos.y, ci.hCursor.0 as isize, ci.flags.0 & CURSOR_SHOWING.0 != 0)
        } else {
            (0, 0, 0, false)
        };
        let inside = |x: i32, y: i32| {
            x >= self.desktop.left - 40
                && x < self.desktop.right + 40
                && y >= self.desktop.top - 40
                && y < self.desktop.bottom + 40
        };
        let changed =
            now != self.last_cursor && (inside(now.0, now.1) || inside(self.last_cursor.0, self.last_cursor.1));
        if changed {
            self.last_cursor = now;
        }
        changed
    }

    fn render(&mut self) -> Result<ID3D11Texture2D> {
        let out = self.pool[self.next].clone();
        self.next = (self.next + 1) % self.pool.len();
        unsafe { self.context.CopyResource(&out, &self.clean) };
        self.draw_cursor(&out);
        Ok(out)
    }

    fn draw_cursor(&mut self, tex: &ID3D11Texture2D) {
        let mut ci = CURSORINFO { cbSize: size_of::<CURSORINFO>() as u32, ..Default::default() };
        unsafe {
            if GetCursorInfo(&mut ci).is_err() || ci.flags.0 & CURSOR_SHOWING.0 == 0 {
                return;
            }
            let handle = ci.hCursor.0 as isize;
            if self.hot.0 != handle {
                let mut icon = ICONINFO::default();
                let mut hot = POINT::default();
                if GetIconInfo(ci.hCursor.into(), &mut icon).is_ok() {
                    hot = POINT { x: icon.xHotspot as i32, y: icon.yHotspot as i32 };
                    let _ = DeleteObject(icon.hbmMask.into());
                    let _ = DeleteObject(icon.hbmColor.into());
                }
                self.hot = (handle, hot);
            }
            let (x, y) = (
                ci.ptScreenPos.x - self.desktop.left - self.hot.1.x,
                ci.ptScreenPos.y - self.desktop.top - self.hot.1.y,
            );
            if x < -64 || y < -64 || x > self.w as i32 || y > self.h as i32 {
                return;
            }
            let Ok(surface) = tex.cast::<IDXGISurface1>() else { return };
            if let Ok(hdc) = surface.GetDC(false) {
                let _ = DrawIconEx(hdc, x, y, ci.hCursor.into(), 0, 0, 0, None, DI_NORMAL);
                let _ = surface.ReleaseDC(None);
            }
        }
    }

    fn readback(&mut self, tex: &ID3D11Texture2D) -> Result<Vec<u8>> {
        unsafe {
            if self.staging.is_none() {
                self.staging = Some(texture(&self.device, self.w, self.h, true)?);
            }
            let staging = self.staging.as_ref().ok_or_else(Error::empty)?;
            self.context.CopyResource(staging, tex);
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            self.context.Map(staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))?;
            let row = self.w as usize * 4;
            let mut out = vec![0u8; row * self.h as usize];
            for y in 0..self.h as usize {
                let src =
                    std::slice::from_raw_parts((mapped.pData as *const u8).add(y * mapped.RowPitch as usize), row);
                out[y * row..(y + 1) * row].copy_from_slice(src);
            }
            self.context.Unmap(staging, 0);
            Ok(out)
        }
    }
}

struct Ticker(Option<windows::Win32::Foundation::HANDLE>);

impl Ticker {
    fn new(interval: Duration) -> Self {
        use windows::Win32::System::Threading::{
            CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, CreateWaitableTimerExW, SetWaitableTimer, TIMER_ALL_ACCESS,
        };
        unsafe {
            let handle =
                CreateWaitableTimerExW(None, None, CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, TIMER_ALL_ACCESS.0).ok();
            let due = -((interval.as_nanos() / 100) as i64);
            let armed = handle.is_some_and(|h| {
                SetWaitableTimer(h, &due, interval.as_millis().max(1) as i32, None, None, false).is_ok()
            });
            Self(if armed { handle } else { None })
        }
    }

    fn wait(&self, fallback: Duration) {
        use windows::Win32::System::Threading::WaitForSingleObject;
        match self.0 {
            Some(h) => unsafe {
                WaitForSingleObject(h, (fallback.as_millis() * 4) as u32);
            },
            None => std::thread::sleep(fallback),
        }
    }
}

impl Drop for Ticker {
    fn drop(&mut self) {
        if let Some(h) = self.0 {
            unsafe {
                let _ = windows::Win32::Foundation::CloseHandle(h);
            }
        }
    }
}

fn capture_loop(
    region: RECT,
    fps: u32,
    stop: &AtomicBool,
    mut emit: impl FnMut(&mut Capture, &ID3D11Texture2D, Instant) -> Result<()>,
    cap: &mut Capture,
) -> Result<()> {
    let interval = Duration::from_micros(1_000_000 / u64::from(fps.max(1)));
    let ticker = Ticker::new(interval);
    let (mut last_out, mut dirty) = (None::<Instant>, true);
    let mut lost_since: Option<Instant> = None;
    while !stop.load(Ordering::SeqCst) {
        ticker.wait(interval);
        match cap.pump(0) {
            Ok(changed) => {
                lost_since = None;
                dirty |= changed;
            }
            Err(e) if e.code() == DXGI_ERROR_ACCESS_LOST => {
                let since = *lost_since.get_or_insert_with(Instant::now);
                if since.elapsed() > Duration::from_secs(10) {
                    return Err(e);
                }
                std::thread::sleep(Duration::from_millis(150));
                if let Ok(fresh) = Capture::new(region) {
                    *cap = fresh;
                    dirty = true;
                }
                continue;
            }
            Err(e) => return Err(e),
        }
        dirty |= cap.cursor_changed();
        let heartbeat = last_out.is_some_and(|t| t.elapsed() >= HEARTBEAT);
        if dirty || heartbeat {
            let frame = cap.render()?;
            emit(cap, &frame, Instant::now())?;
            last_out = Some(Instant::now());
            dirty = false;
        }
    }
    Ok(())
}

struct AudioIn {
    client: IAudioClient,
    capture: IAudioCaptureClient,
    queue: VecDeque<i16>,
    kind: usize,
}

fn open_audio(loopback: bool) -> Result<AudioIn> {
    unsafe {
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let device = enumerator.GetDefaultAudioEndpoint(if loopback { eRender } else { eCapture }, eConsole)?;
        let client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;
        let format = WAVEFORMATEX {
            wFormatTag: 1,
            nChannels: 2,
            nSamplesPerSec: SAMPLE_RATE as u32,
            nAvgBytesPerSec: SAMPLE_RATE as u32 * 4,
            nBlockAlign: 4,
            wBitsPerSample: 16,
            cbSize: 0,
        };
        const AUTOCONVERTPCM: u32 = 0x8000_0000;
        const SRC_DEFAULT_QUALITY: u32 = 0x0800_0000;
        let flags = AUTOCONVERTPCM | SRC_DEFAULT_QUALITY | if loopback { AUDCLNT_STREAMFLAGS_LOOPBACK } else { 0 };
        client.Initialize(AUDCLNT_SHAREMODE_SHARED, flags, 2_000_000, 0, &format, None)?;
        let capture: IAudioCaptureClient = client.GetService()?;
        client.Start()?;
        Ok(AudioIn {
            client,
            capture,
            queue: VecDeque::with_capacity(SAMPLE_RATE as usize),
            kind: usize::from(!loopback),
        })
    }
}

impl AudioIn {
    fn drain(&mut self) {
        unsafe {
            while self.capture.GetNextPacketSize().unwrap_or(0) > 0 {
                let (mut data, mut frames, mut flags) = (std::ptr::null_mut(), 0u32, 0u32);
                if self.capture.GetBuffer(&mut data, &mut frames, &mut flags, None, None).is_err() {
                    return;
                }
                let samples = frames as usize * 2;
                if flags & 0x2 != 0 || data.is_null() {
                    self.queue.extend(std::iter::repeat_n(0i16, samples));
                } else {
                    let block = std::slice::from_raw_parts(data as *const i16, samples);
                    let loudest = block.iter().map(|v| i32::from(*v).unsigned_abs()).max().unwrap_or(0);
                    PEAKS[self.kind].fetch_max(loudest, Ordering::Relaxed);
                    self.queue.extend(block.iter().copied());
                }
                let _ = self.capture.ReleaseBuffer(frames);
            }
            let max = SAMPLE_RATE as usize;
            if self.queue.len() > max {
                let excess = self.queue.len() - max / 4;
                self.queue.drain(..excess);
            }
        }
    }

    fn take(&mut self, samples: usize) -> impl Iterator<Item = i16> + '_ {
        (0..samples).map(|_| self.queue.pop_front().unwrap_or(0))
    }
}

fn mix(streams: &mut [AudioIn], frames: usize) -> Vec<i16> {
    let samples = frames * 2;
    let mut out = vec![0i32; samples];
    for s in streams.iter_mut() {
        for (o, v) in out.iter_mut().zip(s.take(samples)) {
            *o += i32::from(v);
        }
    }
    let gain = if streams.len() > 1 { 0.75 } else { 1.0 };
    out.into_iter().map(|v| (v as f32 * gain).clamp(-32768.0, 32767.0) as i16).collect()
}

fn audio_type(aac: bool) -> Result<IMFMediaType> {
    unsafe {
        let t = MFCreateMediaType()?;
        t.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
        t.SetGUID(&MF_MT_SUBTYPE, if aac { &MFAudioFormat_AAC } else { &MFAudioFormat_PCM })?;
        t.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 2)?;
        t.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, SAMPLE_RATE as u32)?;
        t.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16)?;
        if aac {
            t.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, 20_000)?;
            t.SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, 1)?;
            t.SetUINT32(&MF_MT_AAC_PAYLOAD_TYPE, 0)?;
            t.SetUINT32(&MF_MT_AAC_AUDIO_PROFILE_LEVEL_INDICATION, 0x29)?;
        } else {
            t.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, SAMPLE_RATE as u32 * 4)?;
            t.SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, 4)?;
        }
        Ok(t)
    }
}

fn memory_sample(bytes: &[u8], time_100ns: i64, duration_100ns: i64) -> Result<IMFSample> {
    unsafe {
        let buffer = MFCreateMemoryBuffer(bytes.len() as u32)?;
        let (mut dst, mut max) = (std::ptr::null_mut(), 0u32);
        buffer.Lock(&mut dst, Some(&mut max), None)?;
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), dst, bytes.len().min(max as usize));
        buffer.Unlock()?;
        buffer.SetCurrentLength(bytes.len() as u32)?;
        let sample = MFCreateSample()?;
        sample.AddBuffer(&buffer)?;
        sample.SetSampleTime(time_100ns)?;
        sample.SetSampleDuration(duration_100ns)?;
        Ok(sample)
    }
}

fn audio_thread(
    writer: Arc<Mutex<Agile<IMFSinkWriter>>>,
    stream: u32,
    mut sources: Vec<AudioIn>,
    start: Instant,
    stop: Arc<AtomicBool>,
) {
    let mut written: u64 = 0;
    let mut last = false;
    loop {
        std::thread::sleep(Duration::from_millis(25));
        for s in &mut sources {
            s.drain();
        }
        let target = (start.elapsed().as_secs_f64() * SAMPLE_RATE as f64) as u64;
        while target.saturating_sub(written) >= SAMPLE_RATE / 50 {
            let chunk = (target - written).min(SAMPLE_RATE / 5);
            let pcm = mix(&mut sources, chunk as usize);
            let bytes: Vec<u8> = pcm.iter().flat_map(|v| v.to_le_bytes()).collect();
            let time = (written * 10_000_000 / SAMPLE_RATE) as i64;
            let dur = (chunk * 10_000_000 / SAMPLE_RATE) as i64;
            if let Ok(sample) = memory_sample(&bytes, time, dur)
                && let Ok(w) = writer.lock()
            {
                let _ = unsafe { w.0.WriteSample(stream, &sample) };
            }
            written += chunk;
        }
        if last {
            break;
        }
        last = stop.load(Ordering::SeqCst);
    }
    for s in &sources {
        unsafe {
            let _ = s.client.Stop();
        }
    }
}

struct Mp4 {
    writer: Arc<Mutex<Agile<IMFSinkWriter>>>,
    video: u32,
    audio: Option<u32>,
}

fn video_type(
    subtype: windows::core::GUID,
    w: u32,
    h: u32,
    fps: u32,
    bitrate: Option<u32>,
    stride: bool,
) -> Result<IMFMediaType> {
    unsafe {
        let t = MFCreateMediaType()?;
        t.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        t.SetGUID(&MF_MT_SUBTYPE, &subtype)?;
        t.SetUINT64(&MF_MT_FRAME_SIZE, (u64::from(w) << 32) | u64::from(h))?;
        t.SetUINT64(&MF_MT_FRAME_RATE, (u64::from(fps) << 32) | 1)?;
        t.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, (1u64 << 32) | 1)?;
        t.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
        if let Some(b) = bitrate {
            t.SetUINT32(&MF_MT_AVG_BITRATE, b)?;
            t.SetUINT32(&MF_MT_MPEG2_PROFILE, 77)?;
        }
        if stride {
            t.SetUINT32(&MF_MT_DEFAULT_STRIDE, w * 4)?;
        }
        Ok(t)
    }
}

fn open_mp4(path: &Path, cap: &Capture, fps: u32, audio: bool, gpu: bool) -> Result<Mp4> {
    unsafe {
        let mut attrs: Option<IMFAttributes> = None;
        MFCreateAttributes(&mut attrs, 4)?;
        let attrs = attrs.ok_or_else(Error::empty)?;
        attrs.SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1)?;
        attrs.SetUINT32(&MF_SINK_WRITER_DISABLE_THROTTLING, 1)?;
        attrs.SetUINT32(&MF_LOW_LATENCY, 1)?;
        if gpu {
            let (mut token, mut manager) = (0u32, None::<IMFDXGIDeviceManager>);
            MFCreateDXGIDeviceManager(&mut token, &mut manager)?;
            let manager = manager.ok_or_else(Error::empty)?;
            manager.ResetDevice(&cap.device, token)?;
            attrs.SetUnknown(&MF_SINK_WRITER_D3D_MANAGER, &manager)?;
        }
        let writer = MFCreateSinkWriterFromURL(&HSTRING::from(path.as_os_str()), None, &attrs)?;
        let bitrate = (cap.w * cap.h / 14 * fps).clamp(1_500_000, 18_000_000);
        let video = writer.AddStream(&video_type(MFVideoFormat_H264, cap.w, cap.h, fps, Some(bitrate), false)?)?;
        let input = if gpu { MFVideoFormat_ARGB32 } else { MFVideoFormat_RGB32 };
        writer.SetInputMediaType(video, &video_type(input, cap.w, cap.h, fps, None, !gpu)?, None)?;
        let audio = if audio {
            let id = writer.AddStream(&audio_type(true)?)?;
            writer.SetInputMediaType(id, &audio_type(false)?, None)?;
            Some(id)
        } else {
            None
        };
        writer.BeginWriting()?;
        Ok(Mp4 { writer: Arc::new(Mutex::new(Agile(writer))), video, audio })
    }
}

fn mp4_attempt(
    region: RECT,
    path: &Path,
    opts: Options,
    stop: &Arc<AtomicBool>,
    gpu: bool,
    notes: &mut Vec<&'static str>,
) -> std::result::Result<Duration, (Error, u64)> {
    let mut cap = Capture::new(region).map_err(|e| (e, 0))?;
    for peak in &PEAKS {
        peak.store(0, Ordering::Relaxed);
    }
    let want_audio = opts.system_audio || opts.mic;
    let mut sources: Vec<AudioIn> = Vec::new();
    if opts.system_audio {
        match open_audio(true) {
            Ok(s) => sources.push(s),
            Err(_) => notes.push("no system sound device"),
        }
    }
    if opts.mic {
        match open_audio(false) {
            Ok(s) => sources.push(s),
            Err(_) => notes.push("mic unavailable"),
        }
    }
    let mp4 = open_mp4(path, &cap, opts.fps, want_audio && !sources.is_empty(), gpu).map_err(|e| (e, 0))?;

    let start = Instant::now();
    let audio_handle = mp4.audio.map(|stream| {
        let (writer, stop) = (mp4.writer.clone(), stop.clone());
        let sources = Agile(sources);
        std::thread::spawn(move || {
            let sources = sources;
            audio_thread(writer, stream, sources.0, start, stop);
        })
    });

    let mut frames = 0u64;
    let result = {
        let (writer, video) = (mp4.writer.clone(), mp4.video);
        let interval_ns = 10_000_000i64 / i64::from(opts.fps.max(1));
        capture_loop(
            region,
            opts.fps,
            stop,
            |cap, tex, now| unsafe {
                let sample = if gpu {
                    let buffer = MFCreateDXGISurfaceBuffer(&ID3D11Texture2D::IID, tex, 0, false)?;
                    buffer.SetCurrentLength(buffer.cast::<IMF2DBuffer>()?.GetContiguousLength()?)?;
                    let sample = MFCreateSample()?;
                    sample.AddBuffer(&buffer)?;
                    sample.SetSampleTime((now.duration_since(start).as_nanos() / 100) as i64)?;
                    sample.SetSampleDuration(interval_ns)?;
                    sample
                } else {
                    let pixels = cap.readback(tex)?;
                    memory_sample(&pixels, (now.duration_since(start).as_nanos() / 100) as i64, interval_ns)?
                };
                let guard = writer.lock().map_err(|_| Error::empty())?;
                guard.0.WriteSample(video, &sample)?;
                frames += 1;
                Ok(())
            },
            &mut cap,
        )
    };
    stop.store(true, Ordering::SeqCst);
    if let Some(h) = audio_handle {
        let _ = h.join();
    }
    let took = start.elapsed();
    let heard = |kind: usize| PEAKS[kind].load(Ordering::Relaxed);
    if opts.system_audio && !notes.contains(&"no system sound device") && heard(0) == 0 {
        notes.push("no system sound");
    }
    if opts.mic && !notes.contains(&"mic unavailable") && heard(1) < 40 {
        notes.push("mic was silent");
    }
    match result {
        Ok(()) => {
            let guard = mp4.writer.lock().map_err(|_| (Error::empty(), frames))?;
            unsafe { guard.0.Finalize() }.map_err(|e| (e, frames))?;
            Ok(took)
        }
        Err(e) => Err((e, frames)),
    }
}

fn run_mp4(
    region: RECT,
    path: &PathBuf,
    opts: Options,
    stop: &Arc<AtomicBool>,
) -> Result<(Duration, Vec<&'static str>)> {
    let mut notes = Vec::new();
    match mp4_attempt(region, path, opts, stop, true, &mut notes) {
        Ok(d) => return Ok((d, notes)),
        Err((_, frames)) if frames < 10 && !stop.load(Ordering::SeqCst) => {}
        Err((e, _)) => return Err(e),
    }
    let _ = std::fs::remove_file(path);
    notes.clear();
    notes.push("software encoder");
    mp4_attempt(region, path, opts, stop, false, &mut notes).map(|d| (d, notes)).map_err(|(e, _)| e)
}

const GIF_MAX_W: usize = 720;
const GIF_MAX_H: usize = 540;
const GIF_MAX_FPS: u32 = 20;
const GIF_QUEUE: usize = 4;
const PALETTE_ERROR: f32 = 11.0;

fn resize(bgra: &[u8], w: usize, h: usize, ow: usize, oh: usize) -> Vec<u8> {
    let k = (w / ow).min(h / oh).max(1);
    let (bw, bh) = ((w / k).max(1), (h / k).max(1));
    let mut boxed = vec![0u8; bw * bh * 4];
    if k == 1 {
        for (dst, src) in boxed.chunks_exact_mut(4).zip(bgra.chunks_exact(4)) {
            dst.copy_from_slice(&[src[2], src[1], src[0], 255]);
        }
    } else {
        let area = (k * k) as u32;
        for y in 0..bh {
            for x in 0..bw {
                let (mut r, mut g, mut b) = (0u32, 0u32, 0u32);
                for dy in 0..k {
                    let row = ((y * k + dy).min(h - 1) * w + x * k) * 4;
                    for dx in 0..k {
                        let i = row + dx.min(w - 1 - (x * k).min(w - 1)) * 4;
                        b += u32::from(bgra[i]);
                        g += u32::from(bgra[i + 1]);
                        r += u32::from(bgra[i + 2]);
                    }
                }
                let o = (y * bw + x) * 4;
                boxed[o..o + 4].copy_from_slice(&[(r / area) as u8, (g / area) as u8, (b / area) as u8, 255]);
            }
        }
    }
    if (bw, bh) == (ow, oh) {
        return boxed;
    }
    let axis = |n: usize, m: usize| -> Vec<(usize, usize, u32)> {
        (0..m)
            .map(|o| {
                let f = ((o as f32 + 0.5) * n as f32 / m as f32 - 0.5).max(0.0);
                let i0 = (f as usize).min(n - 1);
                (i0, (i0 + 1).min(n - 1), ((f - i0 as f32) * 256.0) as u32)
            })
            .collect()
    };
    let (xs, ys) = (axis(bw, ow), axis(bh, oh));
    let mut out = vec![255u8; ow * oh * 4];
    for (oy, &(y0, y1, wy)) in ys.iter().enumerate() {
        let (row0, row1) = (&boxed[y0 * bw * 4..(y0 + 1) * bw * 4], &boxed[y1 * bw * 4..(y1 + 1) * bw * 4]);
        let dst = &mut out[oy * ow * 4..(oy + 1) * ow * 4];
        for (px, &(x0, x1, wx)) in dst.chunks_exact_mut(4).zip(&xs) {
            for c in 0..3 {
                let top = u32::from(row0[x0 * 4 + c]) * (256 - wx) + u32::from(row0[x1 * 4 + c]) * wx;
                let bottom = u32::from(row1[x0 * 4 + c]) * (256 - wx) + u32::from(row1[x1 * 4 + c]) * wx;
                px[c] = ((top * (256 - wy) + bottom * wy) >> 16) as u8;
            }
        }
    }
    out
}

fn changed_box(prev: &[u8], next: &[u8], w: usize, h: usize) -> Option<(usize, usize, usize, usize)> {
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0usize, 0usize);
    for y in 0..h {
        let (a, b) = (&prev[y * w * 4..(y + 1) * w * 4], &next[y * w * 4..(y + 1) * w * 4]);
        if a == b {
            continue;
        }
        let first = a.chunks_exact(4).zip(b.chunks_exact(4)).position(|(p, q)| p != q).unwrap_or(0);
        let last = a.chunks_exact(4).zip(b.chunks_exact(4)).rposition(|(p, q)| p != q).unwrap_or(w - 1);
        x0 = x0.min(first);
        x1 = x1.max(last + 1);
        y0 = y0.min(y);
        y1 = y1.max(y + 1);
    }
    (x1 > x0 && y1 > y0).then_some((x0, y0, x1, y1))
}

struct Palette {
    rgb: Vec<u8>,
    quant: color_quant::NeuQuant,
    lut: Vec<u16>,
}

impl Palette {
    fn build(rgba: &[u8]) -> Self {
        let quant = color_quant::NeuQuant::new(30, 256, rgba);
        Self { rgb: quant.color_map_rgb(), quant, lut: vec![u16::MAX; 32 * 32 * 32] }
    }

    fn index(&mut self, r: u8, g: u8, b: u8) -> u8 {
        let key = (usize::from(r >> 3) << 10) | (usize::from(g >> 3) << 5) | usize::from(b >> 3);
        if self.lut[key] == u16::MAX {
            self.lut[key] = self.quant.index_of(&[r, g, b, 255]) as u16;
        }
        self.lut[key] as u8
    }

    fn map(&mut self, rgba: &[u8]) -> Vec<u8> {
        rgba.chunks_exact(4).map(|p| self.index(p[0], p[1], p[2])).collect()
    }

    fn error(&mut self, rgba: &[u8]) -> f32 {
        let step = (rgba.len() / 4 / 1500).max(1);
        let (mut sum, mut n) = (0u32, 0u32);
        for p in rgba.chunks_exact(4).step_by(step) {
            let i = usize::from(self.index(p[0], p[1], p[2])) * 3;
            sum += u32::from(p[0].abs_diff(self.rgb[i]))
                + u32::from(p[1].abs_diff(self.rgb[i + 1]))
                + u32::from(p[2].abs_diff(self.rgb[i + 2]));
            n += 3;
        }
        sum as f32 / n.max(1) as f32
    }
}

enum GifJob {
    Frame { rgba: Vec<u8>, at: Duration },
    Finish(Duration),
}

fn gif_writer(path: PathBuf, w: usize, h: usize, rx: std::sync::mpsc::Receiver<GifJob>) -> Result<()> {
    let file = File::create(&path).map_err(|_| Error::empty())?;
    let mut sink = Some(BufWriter::new(file));
    let mut enc: Option<gif::Encoder<BufWriter<File>>> = None;
    let mut palette: Option<Palette> = None;
    let mut prev: Option<Vec<u8>> = None;
    let mut pending: Option<(gif::Frame<'static>, Duration)> = None;
    fn flush(enc: &mut gif::Encoder<BufWriter<File>>, frame: Option<(gif::Frame<'static>, Duration)>, until: Duration) {
        if let Some((mut f, at)) = frame {
            f.delay = (until.saturating_sub(at).as_millis() / 10).clamp(2, 6000) as u16;
            let _ = enc.write_frame(&f);
        }
    }
    while let Ok(job) = rx.recv() {
        match job {
            GifJob::Frame { rgba, at } => {
                let region = match &prev {
                    None => Some((0, 0, w, h)),
                    Some(p) => changed_box(p, &rgba, w, h),
                };
                let Some((x0, y0, x1, y1)) = region else { continue };
                let (bw, bh) = (x1 - x0, y1 - y0);
                let mut crop = Vec::with_capacity(bw * bh * 4);
                for y in y0..y1 {
                    crop.extend_from_slice(&rgba[(y * w + x0) * 4..(y * w + x1) * 4]);
                }
                let mut local = None;
                let stale = palette.as_mut().map(|p| p.error(&crop) > PALETTE_ERROR);
                match stale {
                    None => palette = Some(Palette::build(&rgba)),
                    Some(true) => {
                        let fresh = Palette::build(&crop);
                        local = Some(fresh.rgb.clone());
                        palette = Some(fresh);
                    }
                    Some(false) => {}
                }
                let Some(pal) = palette.as_mut() else { continue };
                if enc.is_none() {
                    let Some(out) = sink.take() else { continue };
                    let mut e = gif::Encoder::new(out, w as u16, h as u16, &pal.rgb).map_err(|_| Error::empty())?;
                    let _ = e.set_repeat(gif::Repeat::Infinite);
                    enc = Some(e);
                }
                let Some(e) = enc.as_mut() else { continue };
                let mut frame = gif::Frame::from_indexed_pixels(bw as u16, bh as u16, pal.map(&crop), None);
                frame.palette = local;
                frame.left = x0 as u16;
                frame.top = y0 as u16;
                frame.dispose = gif::DisposalMethod::Keep;
                flush(e, pending.replace((frame, at)), at);
                prev = Some(rgba);
            }
            GifJob::Finish(end) => {
                if let Some(e) = enc.as_mut() {
                    flush(e, pending.take(), end.max(Duration::from_millis(100)));
                }
                break;
            }
        }
    }
    Ok(())
}

fn run_gif(region: RECT, path: &Path, opts: Options, stop: &Arc<AtomicBool>) -> Result<(Duration, Vec<&'static str>)> {
    let mut cap = Capture::new(region)?;
    let (src_w, src_h) = (cap.w as usize, cap.h as usize);
    let scale = (GIF_MAX_W as f32 / src_w as f32).min(GIF_MAX_H as f32 / src_h as f32).min(1.0);
    let (w, h) = (((src_w as f32 * scale).round() as usize).max(1), ((src_h as f32 * scale).round() as usize).max(1));
    let (tx, rx): (SyncSender<GifJob>, _) = sync_channel(GIF_QUEUE);
    let out_path = path.to_path_buf();
    let writer = std::thread::spawn(move || gif_writer(out_path, w, h, rx));

    let start = Instant::now();
    let fps = opts.fps.clamp(5, GIF_MAX_FPS);
    let result = capture_loop(
        region,
        fps,
        stop,
        |cap, tex, now| {
            let pixels = cap.readback(tex)?;
            let rgba = resize(&pixels, src_w, src_h, w, h);
            match tx.try_send(GifJob::Frame { rgba, at: now.duration_since(start) }) {
                Ok(()) | Err(TrySendError::Full(_)) => {}
                Err(TrySendError::Disconnected(_)) => return Err(Error::empty()),
            }
            Ok(())
        },
        &mut cap,
    );
    let took = start.elapsed();
    let _ = tx.send(GifJob::Finish(took));
    drop(tx);
    let written = writer.join().map_err(|_| Error::empty())?;
    let notes = if opts.system_audio || opts.mic { vec!["GIF is silent"] } else { Vec::new() };
    result.and(written).map(|()| (took, notes))
}

fn run(region: RECT, path: &PathBuf, opts: Options, stop: &Arc<AtomicBool>) -> Result<(Duration, Vec<&'static str>)> {
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
        MFStartup(MF_VERSION, MFSTARTUP_FULL)?;
    }
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    std::thread::sleep(Duration::from_millis(200));
    let outcome = match opts.format {
        RecordFormat::Mp4 => run_mp4(region, path, opts, stop),
        RecordFormat::Gif => run_gif(region, path, opts, stop),
    };
    unsafe {
        let _ = MFShutdown();
    }
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bgra(w: usize, h: usize, f: impl Fn(usize, usize) -> [u8; 3]) -> Vec<u8> {
        (0..w * h)
            .flat_map(|i| {
                let [b, g, r] = f(i % w, i / w);
                [b, g, r, 255]
            })
            .collect()
    }

    #[test]
    fn resize_averages_blocks_and_swaps_channels() {
        let src = bgra(4, 2, |x, _| if x < 2 { [0, 0, 0] } else { [0, 0, 255] });
        let out = resize(&src, 4, 2, 2, 1);
        assert_eq!(out.len(), 2 * 4);
        assert_eq!(&out[0..4], &[0, 0, 0, 255]);
        assert_eq!(&out[4..8], &[255, 0, 0, 255]);
    }

    #[test]
    fn resize_handles_non_integer_ratios_without_panicking() {
        let src = bgra(10, 7, |x, y| [(x * 20) as u8, (y * 30) as u8, 90]);
        for (ow, oh) in [(7, 5), (9, 6), (3, 2), (10, 7), (1, 1)] {
            assert_eq!(resize(&src, 10, 7, ow, oh).len(), ow * oh * 4);
        }
    }

    #[test]
    fn a_reused_palette_fits_its_own_colours_and_rejects_foreign_ones() {
        let red_blue: Vec<u8> =
            (0..64 * 64).flat_map(|i| if i % 2 == 0 { [250, 10, 10, 255] } else { [10, 10, 250, 255] }).collect();
        let mut pal = Palette::build(&red_blue);
        assert!(pal.error(&red_blue) < PALETTE_ERROR, "its own colours must fit");
        let green: Vec<u8> = std::iter::repeat_n([10u8, 240, 10, 255], 64 * 64).flatten().collect();
        assert!(pal.error(&green) > PALETTE_ERROR, "a new colour must trigger a refresh");
        let idx = pal.map(&red_blue);
        assert_eq!(idx.len(), 64 * 64);
        assert_ne!(idx[0], idx[1]);
    }

    #[test]
    fn changed_box_finds_exactly_what_moved() {
        let a = vec![0u8; 8 * 6 * 4];
        let mut b = a.clone();
        assert_eq!(changed_box(&a, &b, 8, 6), None);
        for (x, y) in [(2usize, 1usize), (5, 3)] {
            b[(y * 8 + x) * 4] = 9;
        }
        assert_eq!(changed_box(&a, &b, 8, 6), Some((2, 1, 6, 4)));
    }

    #[test]
    fn mixing_keeps_pace_with_silence_and_never_overflows() {
        let mut streams: Vec<AudioIn> = Vec::new();
        assert!(mix(&mut streams, 4).iter().all(|v| *v == 0));
        assert_eq!(mix(&mut streams, 4).len(), 8);
    }
}
