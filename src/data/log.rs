use std::fmt::Arguments;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU8, Ordering};

const MAX_BYTES: u64 = 1_000_000;
const MAX_LINE_CHARS: usize = 1000;
const FILE_NAME: &str = "night-island.log";

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Level {
    Error = 0,
    Warn = 1,
    Info = 2,
    Debug = 3,
}

impl Level {
    fn tag(self) -> &'static str {
        match self {
            Level::Error => "ERROR",
            Level::Warn => "WARN ",
            Level::Info => "INFO ",
            Level::Debug => "DEBUG",
        }
    }

    fn parse(s: &str) -> Option<Level> {
        match s.trim().to_ascii_lowercase().as_str() {
            "error" => Some(Level::Error),
            "warn" => Some(Level::Warn),
            "info" => Some(Level::Info),
            "debug" => Some(Level::Debug),
            _ => None,
        }
    }
}

struct Logger {
    file: File,
    size: u64,
    path: PathBuf,
}

impl Logger {
    fn open(path: PathBuf) -> Option<Logger> {
        fs::create_dir_all(path.parent()?).ok()?;
        let size = fs::metadata(&path).map_or(0, |m| m.len());
        let mut logger = Logger { file: OpenOptions::new().create(true).append(true).open(&path).ok()?, size, path };
        if size >= MAX_BYTES {
            logger.rotate();
        }
        Some(logger)
    }

    fn rotate(&mut self) {
        let old = self.path.with_extension("log.1");
        let _ = fs::remove_file(&old);
        if fs::rename(&self.path, &old).is_err() {
            let _ = fs::remove_file(&self.path);
        }
        if let Ok(file) = OpenOptions::new().create(true).append(true).open(&self.path) {
            self.file = file;
            self.size = 0;
        }
    }

    fn write_line(&mut self, line: &str) {
        if self.size >= MAX_BYTES {
            self.rotate();
        }
        if self.file.write_all(line.as_bytes()).is_ok() {
            self.size += line.len() as u64;
        }
    }
}

static LOGGER: Mutex<Option<Logger>> = Mutex::new(None);
static LEVEL: AtomicU8 = AtomicU8::new(Level::Info as u8);

pub fn dir() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("APPDATA")?).join("NightIsland").join("logs"))
}

pub fn init() {
    if let Some(level) = std::env::var("NI_LOG").ok().as_deref().and_then(Level::parse) {
        LEVEL.store(level as u8, Ordering::Relaxed);
    }
    let Some(logger) = dir().and_then(|d| Logger::open(d.join(FILE_NAME))) else { return };
    if let Ok(mut g) = LOGGER.lock() {
        *g = Some(logger);
    }
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let place = info.location().map_or_else(String::new, |l| format!(" at {}:{}", l.file(), l.line()));
        let what = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| (*s).to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_default();
        write(Level::Error, "panic", format_args!("{what}{place}"));
        previous(info);
    }));
}

pub fn enabled(level: Level) -> bool {
    level as u8 <= LEVEL.load(Ordering::Relaxed)
}

pub fn write(level: Level, module: &str, args: Arguments) {
    if !enabled(level) {
        return;
    }
    let stamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    let line = format_line(&stamp.to_string(), level, module, &args.to_string());
    if let Ok(mut g) = LOGGER.lock()
        && let Some(logger) = g.as_mut()
    {
        logger.write_line(&line);
    }
}

fn format_line(stamp: &str, level: Level, module: &str, message: &str) -> String {
    let clean: String = message.chars().map(|c| if c.is_control() { ' ' } else { c }).take(MAX_LINE_CHARS).collect();
    format!("{stamp} {} {module}: {clean}\n", level.tag())
}

pub fn short(module_path: &str) -> &str {
    module_path.rsplit("::").next().unwrap_or(module_path)
}

#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => { $crate::data::log::write($crate::data::log::Level::Error, $crate::data::log::short(module_path!()), format_args!($($arg)*)) };
}

#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => { $crate::data::log::write($crate::data::log::Level::Warn, $crate::data::log::short(module_path!()), format_args!($($arg)*)) };
}

#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => { $crate::data::log::write($crate::data::log::Level::Info, $crate::data::log::short(module_path!()), format_args!($($arg)*)) };
}

#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => { $crate::data::log::write($crate::data::log::Level::Debug, $crate::data::log::short(module_path!()), format_args!($($arg)*)) };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ni-log-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn lines_are_single_and_bounded() {
        let line = format_line("T", Level::Warn, "m", "a\nb\r\tc\u{0}d");
        assert_eq!(line, "T WARN  m: a b  c d\n");
        let long = format_line("T", Level::Info, "m", &"x".repeat(5000));
        assert!(long.chars().count() < MAX_LINE_CHARS + 30);
        assert_eq!(long.matches('\n').count(), 1);
    }

    #[test]
    fn levels_parse_and_order() {
        assert_eq!(Level::parse(" DEBUG "), Some(Level::Debug));
        assert_eq!(Level::parse("nope"), None);
        assert!(Level::Error < Level::Debug);
    }

    #[test]
    fn short_module_name() {
        assert_eq!(short("night_island::services::lyrics"), "lyrics");
        assert_eq!(short("main"), "main");
    }

    #[test]
    fn rotates_when_full() {
        let dir = temp_dir("rotate");
        let path = dir.join(FILE_NAME);
        let mut l = Logger::open(path.clone()).unwrap();
        l.write_line("first\n");
        l.size = MAX_BYTES;
        l.write_line("second\n");
        assert_eq!(fs::read_to_string(&path).unwrap(), "second\n");
        assert_eq!(fs::read_to_string(path.with_extension("log.1")).unwrap(), "first\n");
        let _ = fs::remove_dir_all(&dir);
    }
}
