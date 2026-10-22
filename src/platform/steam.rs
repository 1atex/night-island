use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_NOTIFY, REG_NOTIFY_CHANGE_LAST_SET, RRF_RT_REG_DWORD, RRF_RT_REG_SZ, RegCloseKey,
    RegGetValueW, RegNotifyChangeKeyValue, RegOpenKeyExW,
};
use windows::Win32::System::Threading::{CreateEventW, INFINITE, WaitForSingleObject};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};
use windows::core::{HSTRING, w};

pub const WM_STEAM: u32 = WM_APP + 29;

const KEY: &str = r"Software\Valve\Steam";
const NOT_GAMES: [u32; 6] = [228_980, 250_820, 1_070_560, 1_391_110, 1_628_350, 1_493_710];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Game {
    pub id: u32,
    pub name: String,
}

fn steam_path() -> Option<PathBuf> {
    let mut buf = [0u16; 520];
    let mut size = (buf.len() * 2) as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            &HSTRING::from(KEY),
            w!("SteamPath"),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr() as *mut c_void),
            Some(&mut size),
        )
    };
    if status.is_err() {
        return None;
    }
    let len = (size as usize / 2).saturating_sub(1).min(buf.len());
    let path = PathBuf::from(String::from_utf16_lossy(&buf[..len]));
    path.is_dir().then_some(path)
}

pub fn running_app() -> Option<u32> {
    let mut value = 0u32;
    let mut size = size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            &HSTRING::from(KEY),
            w!("RunningAppID"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut value as *mut u32 as *mut c_void),
            Some(&mut size),
        )
    };
    (status.is_ok() && value != 0).then_some(value)
}

fn quoted(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c != '"' {
            continue;
        }
        let mut s = String::new();
        while let Some(c) = chars.next() {
            match c {
                '"' => break,
                '\\' => {
                    if let Some(next) = chars.next() {
                        s.push(next);
                    }
                }
                c => s.push(c),
            }
        }
        out.push(s);
    }
    out
}

fn library_paths(vdf: &str) -> Vec<PathBuf> {
    vdf.lines()
        .filter_map(|l| {
            let q = quoted(l);
            (q.len() == 2 && q[0] == "path").then(|| PathBuf::from(&q[1]))
        })
        .collect()
}

fn parse_manifest(acf: &str) -> Option<Game> {
    let (mut id, mut name) = (None, None);
    for line in acf.lines() {
        let q = quoted(line);
        if q.len() != 2 {
            continue;
        }
        match q[0].as_str() {
            "appid" => id = q[1].parse::<u32>().ok(),
            "name" => name = Some(q[1].chars().filter(|c| !c.is_control()).take(80).collect::<String>()),
            _ => {}
        }
    }
    let (id, name) = (id?, name?);
    (!name.trim().is_empty() && !NOT_GAMES.contains(&id)).then_some(Game { id, name })
}

fn games_in(library: &Path) -> Vec<Game> {
    let Ok(entries) = std::fs::read_dir(library.join("steamapps")) else { return Vec::new() };
    entries
        .flatten()
        .filter(|e| {
            let name = e.file_name();
            let name = name.to_string_lossy();
            name.starts_with("appmanifest_") && name.ends_with(".acf")
        })
        .filter_map(|e| std::fs::read_to_string(e.path()).ok())
        .filter_map(|text| parse_manifest(&text))
        .collect()
}

static CACHE: Mutex<Option<(Instant, Vec<Game>)>> = Mutex::new(None);

pub fn installed() -> Vec<Game> {
    if let Ok(cache) = CACHE.lock()
        && let Some((at, games)) = cache.as_ref()
        && at.elapsed() < Duration::from_secs(300)
    {
        return games.clone();
    }
    let mut libraries: Vec<PathBuf> = Vec::new();
    if let Some(steam) = steam_path() {
        let vdf = std::fs::read_to_string(steam.join("steamapps").join("libraryfolders.vdf")).unwrap_or_default();
        libraries.push(steam);
        for path in library_paths(&vdf) {
            if !libraries.contains(&path) {
                libraries.push(path);
            }
        }
    }
    let mut games: Vec<Game> = libraries.iter().flat_map(|l| games_in(l)).collect();
    games.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    games.dedup_by_key(|g| g.id);
    if let Ok(mut cache) = CACHE.lock() {
        *cache = Some((Instant::now(), games.clone()));
    }
    games
}

pub fn find(query: &str, limit: usize) -> Vec<Game> {
    let q = query.trim().to_lowercase();
    if q.chars().count() < 2 {
        return Vec::new();
    }
    let all = installed();
    let starts = all.iter().filter(|g| g.name.to_lowercase().starts_with(&q));
    let contains = all.iter().filter(|g| {
        let n = g.name.to_lowercase();
        !n.starts_with(&q) && n.contains(&q)
    });
    starts.chain(contains).take(limit).cloned().collect()
}

pub fn name_of(id: u32) -> Option<String> {
    installed().into_iter().find(|g| g.id == id).map(|g| g.name)
}

pub fn launch_url(id: u32) -> String {
    format!("steam://rungameid/{id}")
}

pub fn watch(hwnd: HWND) {
    let raw = hwnd.0 as isize;
    std::thread::spawn(move || unsafe {
        let mut key = HKEY::default();
        if RegOpenKeyExW(HKEY_CURRENT_USER, &HSTRING::from(KEY), None, KEY_NOTIFY, &mut key).is_err() {
            return;
        }
        let Ok(event) = CreateEventW(None, false, false, None) else { return };
        loop {
            if RegNotifyChangeKeyValue(key, false, REG_NOTIFY_CHANGE_LAST_SET, Some(event), true).is_err() {
                break;
            }
            WaitForSingleObject(event, INFINITE);
            std::thread::sleep(Duration::from_millis(300));
            let _ = PostMessageW(Some(HWND(raw as *mut c_void)), WM_STEAM, WPARAM(0), LPARAM(0));
        }
        let _ = RegCloseKey(key);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "reads this computer"]
    fn lists_installed_games() {
        let games = installed();
        println!("{} games, running: {:?}, first: {:?}", games.len(), running_app(), games.first());
    }

    #[test]
    fn valve_text_lines_are_read() {
        assert_eq!(quoted("\t\t\"name\"\t\t\"Half-Life 2\""), ["name", "Half-Life 2"]);
        assert_eq!(quoted(r#""path"  "D:\\SteamLibrary""#), ["path", r"D:\SteamLibrary"]);
        assert!(quoted("{").is_empty());
    }

    #[test]
    fn libraries_and_games_come_from_the_files() {
        let vdf = "\"libraryfolders\"\n{\n\t\"0\"\n\t{\n\t\t\"path\"\t\t\"C:\\\\Program Files (x86)\\\\Steam\"\n\t}\n\t\"1\"\n\t{\n\t\t\"path\"\t\t\"D:\\\\Games\"\n\t}\n}";
        assert_eq!(library_paths(vdf), [PathBuf::from(r"C:\Program Files (x86)\Steam"), PathBuf::from(r"D:\Games")]);
        let acf = "\"AppState\"\n{\n\t\"appid\"\t\t\"620\"\n\t\"name\"\t\t\"Portal 2\"\n\t\"StateFlags\"\t\t\"4\"\n}";
        assert_eq!(parse_manifest(acf), Some(Game { id: 620, name: "Portal 2".into() }));
        let redist =
            "\"AppState\"\n{\n\t\"appid\"\t\t\"228980\"\n\t\"name\"\t\t\"Steamworks Common Redistributables\"\n}";
        assert_eq!(parse_manifest(redist), None);
        assert_eq!(launch_url(620), "steam://rungameid/620");
    }
}
