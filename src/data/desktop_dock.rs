use std::collections::HashSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

pub const MAX_PINS: usize = 32;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pin {
    pub name: String,
    pub target: String,
}

impl Pin {
    pub fn from_path(path: &str) -> Self {
        let name = Path::new(path).file_stem().unwrap_or_default().to_string_lossy().into_owned();
        Self { name, target: path.to_string() }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DockSettings {
    pub enabled: bool,
    pub auto_hide: bool,
    pub show_running: bool,
    pub hide_fullscreen: bool,
    pub economy: bool,
    pub icon_size: u32,
    pub zoom: f32,
    pub opacity: f32,
    pub margin: u32,
    pub monitor: String,
    pub pins: Vec<Pin>,
}

impl Default for DockSettings {
    fn default() -> Self {
        let windows = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into());
        Self {
            enabled: false,
            auto_hide: false,
            show_running: true,
            hide_fullscreen: true,
            economy: false,
            icon_size: 48,
            zoom: 1.3,
            opacity: 0.94,
            margin: 8,
            monitor: String::new(),
            pins: vec![Pin { name: "Explorer".into(), target: format!(r"{windows}\explorer.exe") }],
        }
    }
}

pub fn key(target: &str) -> String {
    target.trim().replace('/', "\\").to_lowercase()
}

impl DockSettings {
    pub fn sanitize(&mut self) {
        self.icon_size = self.icon_size.clamp(32, 72);
        self.zoom = if self.zoom.is_finite() { self.zoom.clamp(1.0, 1.7) } else { 1.3 };
        self.opacity = if self.opacity.is_finite() { self.opacity.clamp(0.4, 1.0) } else { 0.94 };
        self.margin = self.margin.min(40);
        self.monitor = self.monitor.chars().take(128).collect();
        let mut seen = HashSet::new();
        self.pins.retain(|p| {
            !p.target.trim().is_empty()
                && p.target.len() <= 2048
                && !p.target.contains(['\0', '\r', '\n'])
                && seen.insert(key(&p.target))
        });
        self.pins.truncate(MAX_PINS);
        let mut bytes = 0;
        self.pins.retain(|p| {
            bytes += p.target.len() + p.name.len().min(320);
            bytes <= 20 * 1024
        });
        for pin in &mut self.pins {
            pin.name = pin.name.chars().filter(|c| !c.is_control()).take(80).collect();
            if pin.name.is_empty() {
                pin.name = Pin::from_path(&pin.target).name;
            }
        }
    }

    pub fn add(&mut self, pin: Pin) -> bool {
        if self.pins.len() >= MAX_PINS || self.pins.iter().any(|p| key(&p.target) == key(&pin.target)) {
            return false;
        }
        let before = self.pins.len();
        self.pins.push(pin);
        self.sanitize();
        self.pins.len() > before
    }
}

pub fn import_mydock(bytes: &[u8]) -> Result<Vec<Pin>, &'static str> {
    if bytes.len() > 256 * 1024 {
        return Err("Dock configuration is too large");
    }
    let text = if bytes.starts_with(&[0xff, 0xfe]) {
        let wide: Vec<_> = bytes[2..].chunks_exact(2).map(|p| u16::from_le_bytes([p[0], p[1]])).collect();
        String::from_utf16(&wide).map_err(|_| "Unsupported dock configuration encoding")?
    } else {
        std::str::from_utf8(bytes).map_err(|_| "Unsupported dock configuration encoding")?.to_string()
    };
    let mut sections: Vec<(u32, String, String, bool)> = Vec::new();
    let mut current = None;
    for line in text.trim_start_matches('\u{feff}').lines().map(str::trim) {
        if line.starts_with('[') {
            current = line.strip_prefix("[ico").and_then(|s| s.strip_suffix(']')).and_then(|s| s.parse().ok());
            if let Some(id) = current {
                sections.push((id, String::new(), String::new(), false));
            }
        } else if current.is_some()
            && let Some((name, value)) = line.split_once('=')
            && let Some(row) = sections.last_mut()
        {
            match name.trim() {
                "tag" => row.1 = value.trim().to_string(),
                "filepath" => row.2 = value.trim().to_string(),
                "delimiter" => row.3 = value.trim() == "1",
                _ => {}
            }
        }
    }
    sections.sort_by_key(|s| s.0);
    let mut settings = DockSettings { pins: Vec::new(), ..Default::default() };
    for (_, name, target, separator) in sections {
        if !separator && !target.is_empty() {
            settings.add(Pin { name, target });
        }
    }
    if settings.pins.is_empty() { Err("No applications found in ico.ini") } else { Ok(settings.pins) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_unicode_and_numeric_order_without_separators_or_duplicates() {
        let ini = "[ico10]\ntag=Чат\nfilepath=C:\\chat.exe\n[ico2]\ntag=Browser\nfilepath=C:\\web.exe\n[ico3]\ndelimiter=1\n[ico11]\ntag=Duplicate\nfilepath=c:/chat.exe\n";
        let bytes: Vec<u8> = [0xff, 0xfe].into_iter().chain(ini.encode_utf16().flat_map(u16::to_le_bytes)).collect();
        let pins = import_mydock(&bytes).unwrap();
        assert_eq!(pins.len(), 2);
        assert_eq!(pins[0].name, "Browser");
        assert_eq!(pins[1].name, "Чат");
    }

    #[test]
    fn migration_is_opt_in_and_limits_persisted_input() {
        let mut cfg: DockSettings = serde_json::from_str("{}").unwrap();
        assert!(!cfg.enabled);
        cfg.zoom = f32::NAN;
        cfg.icon_size = u32::MAX;
        cfg.pins = (0..100).map(|i| Pin { name: "x".repeat(100), target: format!("C:\\{i}.exe") }).collect();
        cfg.sanitize();
        assert_eq!(cfg.zoom, 1.3);
        assert_eq!(cfg.icon_size, 72);
        assert_eq!(cfg.pins.len(), MAX_PINS);
        assert_eq!(cfg.pins[0].name.len(), 80);
        assert!(import_mydock(b"[normal]\nzoom=1").is_err());
    }
}
