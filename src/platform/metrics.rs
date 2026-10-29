use std::ffi::c_void;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use windows::Win32::Foundation::{FILETIME, HWND, LPARAM, WPARAM};
use windows::Win32::NetworkManagement::IpHelper::{FreeMibTable, GetIfTable2, MIB_IF_TABLE2};
use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
use windows::Win32::System::Performance::{
    PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE, PDH_HCOUNTER, PDH_HQUERY, PdhAddEnglishCounterW, PdhCloseQuery,
    PdhCollectQueryData, PdhGetFormattedCounterArrayW, PdhOpenQueryW,
};
use windows::Win32::System::SystemInformation::{GetTickCount64, GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows::Win32::System::Threading::GetSystemTimes;
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};
use windows::core::{HSTRING, w};

pub const WM_METRICS: u32 = WM_APP + 27;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Metric {
    Cpu,
    Ram,
    Gpu,
    Disk,
    NetDown,
    NetUp,
    Uptime,
}

impl Metric {
    pub const ALL: [Metric; 7] =
        [Metric::Cpu, Metric::Ram, Metric::Gpu, Metric::Disk, Metric::NetDown, Metric::NetUp, Metric::Uptime];

    pub fn key(self) -> &'static str {
        match self {
            Metric::Cpu => "cpu",
            Metric::Ram => "ram",
            Metric::Gpu => "gpu",
            Metric::Disk => "disk",
            Metric::NetDown => "net_down",
            Metric::NetUp => "net_up",
            Metric::Uptime => "uptime",
        }
    }

    pub fn from_key(key: &str) -> Option<Metric> {
        Self::ALL.into_iter().find(|m| m.key() == key)
    }

    pub fn label(self) -> &'static str {
        match self {
            Metric::Cpu => "Processor",
            Metric::Ram => "Memory",
            Metric::Gpu => "Graphics card",
            Metric::Disk => "System disk",
            Metric::NetDown => "Download speed",
            Metric::NetUp => "Upload speed",
            Metric::Uptime => "Uptime",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            Metric::Cpu => "CPU",
            Metric::Ram => "RAM",
            Metric::Gpu => "GPU",
            Metric::Disk => "SSD",
            Metric::NetDown => "↓",
            Metric::NetUp => "↑",
            Metric::Uptime => "UP",
        }
    }

    pub fn is_percent(self) -> bool {
        matches!(self, Metric::Cpu | Metric::Ram | Metric::Gpu | Metric::Disk)
    }

    pub fn value(self, s: &Sample) -> Option<f32> {
        match self {
            Metric::Cpu => Some(s.cpu),
            Metric::Ram => Some(s.ram),
            Metric::Gpu => s.gpu,
            Metric::Disk => Some(s.disk),
            Metric::NetDown => Some(s.net_down),
            Metric::NetUp => Some(s.net_up),
            Metric::Uptime => Some(s.uptime as f32),
        }
    }

    pub fn text(self, s: &Sample) -> String {
        let Some(v) = self.value(s) else { return "—".into() };
        match self {
            Metric::NetDown | Metric::NetUp => speed_text(v),
            Metric::Uptime => uptime_text(s.uptime),
            _ => format!("{}%", v.round() as i32),
        }
    }

    pub fn detail(self, s: &Sample) -> Option<String> {
        const GB: f64 = 1_073_741_824.0;
        match self {
            Metric::Ram => Some(format!("{:.1} / {:.1} GB", s.ram_used as f64 / GB, s.ram_total as f64 / GB)),
            Metric::Disk => Some(format!("{:.0} / {:.0} GB", s.disk_used as f64 / GB, s.disk_total as f64 / GB)),
            _ => None,
        }
    }
}

pub fn speed_text(bytes_per_sec: f32) -> String {
    let b = bytes_per_sec.max(0.0);
    if b < 1000.0 {
        format!("{:.0} B/s", b)
    } else if b < 1_000_000.0 {
        format!("{:.0} KB/s", b / 1000.0)
    } else if b < 1_000_000_000.0 {
        format!("{:.1} MB/s", b / 1_000_000.0)
    } else {
        format!("{:.1} GB/s", b / 1_000_000_000.0)
    }
}

pub fn uptime_text(secs: u64) -> String {
    let (d, h, m) = (secs / 86_400, secs / 3600 % 24, secs / 60 % 60);
    if d > 0 {
        format!("{d} d {h} h")
    } else if h > 0 {
        format!("{h} h {m} min")
    } else {
        format!("{m} min")
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sample {
    pub cpu: f32,
    pub ram: f32,
    pub ram_used: u64,
    pub ram_total: u64,
    pub gpu: Option<f32>,
    pub disk: f32,
    pub disk_used: u64,
    pub disk_total: u64,
    pub net_down: f32,
    pub net_up: f32,
    pub uptime: u64,
}

static LATEST: Mutex<Option<Sample>> = Mutex::new(None);
static RUN: AtomicU64 = AtomicU64::new(0);

pub fn latest() -> Option<Sample> {
    LATEST.lock().ok().and_then(|g| g.clone())
}

static EVERY_MS: AtomicU64 = AtomicU64::new(2000);
static RUNNING: AtomicBool = AtomicBool::new(false);

pub fn start(hwnd: HWND, every: Duration) {
    EVERY_MS.store(every.as_millis().max(500) as u64, Ordering::SeqCst);
    if RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }
    let id = RUN.fetch_add(1, Ordering::SeqCst) + 1;
    let raw = hwnd.0 as isize;
    std::thread::spawn(move || {
        let mut reader = Reader::new();
        let _ = reader.read();
        let wait = |ms: u64| {
            let mut slept = 0;
            while slept < ms && RUN.load(Ordering::SeqCst) == id {
                std::thread::sleep(Duration::from_millis(100));
                slept += 100;
            }
        };
        wait(1000);
        while RUN.load(Ordering::SeqCst) == id {
            let sample = reader.read();
            if RUN.load(Ordering::SeqCst) != id {
                break;
            }
            if let Ok(mut g) = LATEST.lock() {
                *g = Some(sample);
            }
            unsafe {
                let _ = PostMessageW(Some(HWND(raw as *mut c_void)), WM_METRICS, WPARAM(0), LPARAM(0));
            }
            let mut slept = 0;
            while slept < EVERY_MS.load(Ordering::SeqCst) && RUN.load(Ordering::SeqCst) == id {
                std::thread::sleep(Duration::from_millis(100));
                slept += 100;
            }
        }
    });
}

pub fn stop() {
    RUN.fetch_add(1, Ordering::SeqCst);
    RUNNING.store(false, Ordering::SeqCst);
    if let Ok(mut g) = LATEST.lock() {
        *g = None;
    }
}

fn filetime(ft: FILETIME) -> u64 {
    (u64::from(ft.dwHighDateTime) << 32) | u64::from(ft.dwLowDateTime)
}

struct Reader {
    cpu: Option<(u64, u64, u64)>,
    net: Option<(u64, u64, std::time::Instant)>,
    gpu: Option<GpuCounter>,
}

impl Reader {
    fn new() -> Self {
        Self { cpu: None, net: None, gpu: GpuCounter::open() }
    }

    fn read(&mut self) -> Sample {
        let mut s = Sample { cpu: self.cpu(), uptime: unsafe { GetTickCount64() } / 1000, ..Sample::default() };
        let mut mem = MEMORYSTATUSEX { dwLength: size_of::<MEMORYSTATUSEX>() as u32, ..Default::default() };
        if unsafe { GlobalMemoryStatusEx(&mut mem) }.is_ok() {
            s.ram = mem.dwMemoryLoad as f32;
            s.ram_total = mem.ullTotalPhys;
            s.ram_used = mem.ullTotalPhys.saturating_sub(mem.ullAvailPhys);
        }
        let (used, total) = system_disk();
        s.disk_used = used;
        s.disk_total = total;
        s.disk = if total > 0 { used as f32 / total as f32 * 100.0 } else { 0.0 };
        (s.net_down, s.net_up) = self.network();
        s.gpu = self.gpu.as_ref().and_then(GpuCounter::read);
        s
    }

    fn cpu(&mut self) -> f32 {
        let (mut idle, mut kernel, mut user) = (FILETIME::default(), FILETIME::default(), FILETIME::default());
        if unsafe { GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)) }.is_err() {
            return 0.0;
        }
        let now = (filetime(idle), filetime(kernel), filetime(user));
        let share = self.cpu.map_or(0.0, |(i0, k0, u0)| {
            let total = (now.1 + now.2).saturating_sub(k0 + u0);
            let idle = now.0.saturating_sub(i0);
            if total == 0 { 0.0 } else { (total.saturating_sub(idle)) as f32 / total as f32 * 100.0 }
        });
        self.cpu = Some(now);
        share.clamp(0.0, 100.0)
    }

    fn network(&mut self) -> (f32, f32) {
        let (rx, tx) = adapter_octets();
        let now = std::time::Instant::now();
        let speeds = self.net.map_or((0.0, 0.0), |(rx0, tx0, at)| {
            let secs = now.duration_since(at).as_secs_f32().max(0.05);
            (rx.saturating_sub(rx0) as f32 / secs, tx.saturating_sub(tx0) as f32 / secs)
        });
        self.net = Some((rx, tx, now));
        speeds
    }
}

fn system_disk() -> (u64, u64) {
    let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
    let root = HSTRING::from(format!("{drive}\\"));
    let (mut free, mut total) = (0u64, 0u64);
    let ok = unsafe { GetDiskFreeSpaceExW(&root, Some(&mut free), Some(&mut total), None) }.is_ok();
    if ok { (total.saturating_sub(free), total) } else { (0, 0) }
}

fn adapter_octets() -> (u64, u64) {
    let (mut rx, mut tx) = (0u64, 0u64);
    unsafe {
        let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
        if GetIfTable2(&mut table).is_err() || table.is_null() {
            return (0, 0);
        }
        let rows = std::slice::from_raw_parts((*table).Table.as_ptr(), (*table).NumEntries as usize);
        for row in rows {
            let up = row.OperStatus.0 == 1;
            let loopback = row.Type == 24;
            let hardware = row.InterfaceAndOperStatusFlags._bitfield & 1 != 0;
            if up && !loopback && hardware {
                rx += row.InOctets;
                tx += row.OutOctets;
            }
        }
        FreeMibTable(table as *const c_void);
    }
    (rx, tx)
}

struct GpuCounter {
    query: PDH_HQUERY,
    counter: PDH_HCOUNTER,
}

unsafe impl Send for GpuCounter {}

impl GpuCounter {
    fn open() -> Option<Self> {
        unsafe {
            let mut query = PDH_HQUERY::default();
            if PdhOpenQueryW(None, 0, &mut query) != 0 {
                return None;
            }
            let mut counter = PDH_HCOUNTER::default();
            if PdhAddEnglishCounterW(query, w!(r"\GPU Engine(*)\Utilization Percentage"), 0, &mut counter) != 0 {
                let _ = PdhCloseQuery(query);
                return None;
            }
            PdhCollectQueryData(query);
            Some(Self { query, counter })
        }
    }

    fn read(&self) -> Option<f32> {
        const MORE_DATA: u32 = 0x800007D2;
        unsafe {
            if PdhCollectQueryData(self.query) != 0 {
                return None;
            }
            let (mut bytes, mut count) = (0u32, 0u32);
            let status = PdhGetFormattedCounterArrayW(self.counter, PDH_FMT_DOUBLE, &mut bytes, &mut count, None);
            if status != MORE_DATA || bytes == 0 {
                return None;
            }
            let mut storage = vec![0u64; (bytes as usize).div_ceil(8)];
            let items = storage.as_mut_ptr() as *mut PDH_FMT_COUNTERVALUE_ITEM_W;
            if PdhGetFormattedCounterArrayW(self.counter, PDH_FMT_DOUBLE, &mut bytes, &mut count, Some(items)) != 0 {
                return None;
            }
            let mut sums: Vec<(String, f64)> = Vec::new();
            for item in std::slice::from_raw_parts(items, count as usize) {
                let name = item.szName.to_string().unwrap_or_default();
                let engine = name.rsplit("engtype_").next().unwrap_or("").to_string();
                let value = item.FmtValue.Anonymous.doubleValue;
                match sums.iter_mut().find(|(e, _)| *e == engine) {
                    Some((_, total)) => *total += value,
                    None => sums.push((engine, value)),
                }
            }
            sums.iter()
                .map(|(_, v)| *v)
                .fold(None, |best: Option<f64>, v| Some(best.map_or(v, |b| b.max(v))))
                .map(|v| v.clamp(0.0, 100.0) as f32)
        }
    }
}

impl Drop for GpuCounter {
    fn drop(&mut self) {
        unsafe {
            let _ = PdhCloseQuery(self.query);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speeds_and_uptime_read_naturally() {
        assert_eq!(speed_text(850.0), "850 B/s");
        assert_eq!(speed_text(12_000.0), "12 KB/s");
        assert_eq!(speed_text(1_234_000.0), "1.2 MB/s");
        assert_eq!(uptime_text(42 * 60), "42 min");
        assert_eq!(uptime_text(5 * 3600 + 600), "5 h 10 min");
        assert_eq!(uptime_text(3 * 86_400 + 4 * 3600), "3 d 4 h");
    }

    #[test]
    fn the_settings_know_every_metric_key() {
        let keys: Vec<&str> = Metric::ALL.iter().map(|m| m.key()).collect();
        assert_eq!(keys, crate::data::settings::METRIC_KEYS);
        let mut s: crate::data::settings::Settings = serde_json::from_str(
            r#"{"metrics_tab":["cpu","cpu","bogus","ram"],"metrics_pill":["gpu"],"metrics_secs":99}"#,
        )
        .unwrap();
        s.sanitize();
        assert_eq!(
            (s.metrics_tab, s.metrics_pill, s.metrics_secs),
            (vec!["cpu".into(), "ram".into()], vec!["gpu".into()], 10)
        );
    }

    #[test]
    fn metrics_round_trip_through_their_keys() {
        for m in Metric::ALL {
            assert_eq!(Metric::from_key(m.key()), Some(m));
        }
        assert_eq!(Metric::from_key("bogus"), None);
        let s = Sample { cpu: 23.4, net_down: 1_500_000.0, ..Sample::default() };
        assert_eq!(Metric::Cpu.text(&s), "23%");
        assert_eq!(Metric::NetDown.text(&s), "1.5 MB/s");
        assert_eq!(Metric::Gpu.text(&s), "—");
    }

    #[test]
    #[ignore = "reads this computer"]
    fn reads_this_computer() {
        let mut reader = Reader::new();
        let _ = reader.read();
        std::thread::sleep(Duration::from_millis(1200));
        let s = reader.read();
        println!("{s:?}");
        assert!(s.ram > 0.0 && s.ram_total > 0 && s.disk_total > 0 && s.uptime > 0);
    }
}
