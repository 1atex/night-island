use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use serde::{Deserialize, Serialize};

use super::desktop_dock::DockSettings;
use super::settings::Settings;
use super::top_panel::TopPanelSettings;

pub const MAX_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(usize)]
pub enum Scope {
    Island,
    Dock,
    Top,
}

impl Scope {
    pub fn filename(self) -> &'static str {
        match self {
            Self::Island => "Night Island.json",
            Self::Dock => "Night Island Dock.json",
            Self::Top => "Night Island Top Panel.json",
        }
    }
}

#[derive(Serialize)]
struct Profile {
    format: String,
    version: u32,
    #[serde(flatten)]
    content: Content,
}

#[derive(Serialize)]
#[serde(tag = "scope", content = "settings", rename_all = "snake_case")]
enum Content {
    Island(Box<Settings>),
    Dock(DockSettings),
    Top(TopPanelSettings),
}

pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let (temp, mut file) = loop {
        let temp = path.with_file_name(format!(
            ".night-island-{}-{}.tmp",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        match OpenOptions::new().write(true).create_new(true).open(&temp) {
            Ok(file) => break (temp, file),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    };
    let result = file.write_all(bytes);
    drop(file);
    let result = result.and_then(|_| fs::rename(&temp, path));
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}

pub fn export(path: &Path, settings: &Settings, scope: Scope) -> io::Result<()> {
    let content = match scope {
        Scope::Island => Content::Island(Box::new(settings.clone())),
        Scope::Dock => Content::Dock(settings.desktop_dock.clone()),
        Scope::Top => Content::Top(settings.top_panel.clone()),
    };
    let profile = Profile { format: "night-island".into(), version: 1, content };
    let bytes = serde_json::to_vec_pretty(&profile)?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(io::Error::other("Profile is too large"));
    }
    write_atomic(path, &bytes)
}

pub struct Import(Content);

impl Import {
    pub fn read(path: &Path, scope: Scope) -> Result<Self, &'static str> {
        let mut bytes = Vec::new();
        File::open(path)
            .and_then(|file| file.take(MAX_BYTES + 1).read_to_end(&mut bytes))
            .map_err(|_| "Could not read the settings file")?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err("Profile is too large");
        }
        let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| "Invalid settings file")?;
        if value.get("format").and_then(|v| v.as_str()) != Some("night-island") {
            return Err("Choose an exported Night Island profile");
        }
        if value.get("version").and_then(|v| v.as_u64()) != Some(1) {
            return Err("Unsupported profile version");
        }
        let body = &value["settings"];
        let content = match value["scope"].as_str() {
            Some("island")
                if body.get("desktop_dock").is_some()
                    && body.get("top_panel").is_some()
                    && body.get("placement").is_some() =>
            {
                Settings::deserialize(body).map(|s| Content::Island(Box::new(s)))
            }
            Some("dock") if body.get("enabled").is_some() && body.get("pins").is_some() => {
                DockSettings::deserialize(body).map(Content::Dock)
            }
            Some("top") if body.get("enabled").is_some() && body.get("entrance").is_some() => {
                TopPanelSettings::deserialize(body).map(Content::Top)
            }
            _ => return Err("Invalid settings file"),
        }
        .map_err(|_| "Invalid settings file")?;
        let content = match (scope, content) {
            (Scope::Dock, Content::Island(all)) => Content::Dock(all.desktop_dock),
            (Scope::Top, Content::Island(all)) => Content::Top(all.top_panel),
            (Scope::Island, content @ Content::Island(_))
            | (Scope::Dock, content @ Content::Dock(_))
            | (Scope::Top, content @ Content::Top(_)) => content,
            _ => return Err("This profile belongs to another settings section"),
        };
        Ok(Self(content))
    }

    pub fn apply(self, current: &Settings) -> Settings {
        match self.0 {
            Content::Island(mut full) => {
                full.tour_revision = current.tour_revision;
                full.sanitize();
                *full
            }
            Content::Dock(mut dock) => {
                dock.sanitize();
                Settings { desktop_dock: dock, ..current.clone() }
            }
            Content::Top(mut top) => {
                top.sanitize();
                Settings { top_panel: top, ..current.clone() }
            }
        }
    }
}

pub fn backup_path() -> io::Result<PathBuf> {
    Ok(Settings::path()
        .ok_or_else(|| io::Error::other("Profile folder unavailable"))?
        .with_file_name("before-import.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scoped_import_keeps_unrelated_settings_and_accepts_full_snapshots() {
        let dir = std::env::temp_dir().join(format!("night-island-profile-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("profile.json");
        let mut source = Settings::default();
        source.desktop_dock.icon_size = 66;
        source.top_panel.volume = false;
        source.vpn_provider = super::super::settings::VpnProvider::Svoi;
        source.desktop_dock.pins[0].name = "Проводник".into();
        source.home_widget_sizes.insert(9, crate::data::home_widgets::WidgetSize::Tall4);
        let current = Settings { tour_revision: 42, ..Settings::default() };
        for scope in [Scope::Dock, Scope::Top, Scope::Island] {
            export(&path, &source, scope).unwrap();
            let restored = Import::read(&path, scope).unwrap().apply(&current);
            assert_eq!(
                restored.desktop_dock,
                if scope == Scope::Top { current.desktop_dock.clone() } else { source.desktop_dock.clone() }
            );
            assert_eq!(
                restored.top_panel,
                if scope == Scope::Dock { current.top_panel.clone() } else { source.top_panel.clone() }
            );
            assert_eq!(restored.tour_revision, 42);
            if scope == Scope::Island {
                assert_eq!(restored.home_widget_sizes, source.home_widget_sizes);
                assert_eq!(restored.vpn_provider, source.vpn_provider);
                let dock_only = Import::read(&path, Scope::Dock).unwrap().apply(&current);
                assert_eq!(dock_only.top_panel, current.top_panel);
                assert_eq!(dock_only.desktop_dock, source.desktop_dock);
            } else {
                assert!(Import::read(&path, Scope::Island).is_err());
            }
        }
        for invalid in [
            r#"{}"#,
            r#"{"format":"night-island","version":1,"scope":"dock","settings":{}}"#,
            r#"{"format":"night-island","version":2,"scope":"dock","settings":{"pins":[],"enabled":true}}"#,
        ] {
            fs::write(&path, invalid).unwrap();
            assert!(Import::read(&path, Scope::Dock).is_err());
        }
        fs::remove_file(path).unwrap();
        fs::remove_dir(dir).unwrap();
    }
}
