use std::collections::HashMap;
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, WPARAM};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_NOTIFY, KEY_SET_VALUE, REG_DWORD, REG_NOTIFY_CHANGE_LAST_SET, REG_SZ, REG_VALUE_TYPE,
    RRF_RT_REG_DWORD, RRF_RT_REG_SZ, RegCloseKey, RegGetValueW, RegNotifyChangeKeyValue, RegOpenKeyExW, RegSetValueExW,
};
use windows::Win32::System::Threading::{CreateEventW, INFINITE, WaitForSingleObject};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, SHOW_WINDOW_CMD, SW_HIDE, SW_SHOWNORMAL, WM_APP};
use windows::core::{HSTRING, PCWSTR, w};

pub const WM_STEAM: u32 = WM_APP + 29;

const KEY: &str = r"Software\Valve\Steam";
const NOT_GAMES: [u32; 6] = [228_980, 250_820, 1_070_560, 1_391_110, 1_628_350, 1_493_710];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Update {
    #[default]
    None,
    Needed,
    Running,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Game {
    pub id: u32,
    pub name: String,
    pub update: Update,
}

pub(super) fn steam_path() -> Option<PathBuf> {
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

const UPDATE_REQUIRED: u32 = 2;
const FULLY_INSTALLED: u32 = 4;
const UPDATE_RUNNING: u32 = 256;
const UPDATE_PAUSED: u32 = 512;
const DOWNLOADING: u32 = 1_048_576;

fn update_state(flags: u32, to_download: u64, downloaded: u64) -> Update {
    if flags & (UPDATE_RUNNING | DOWNLOADING) != 0 {
        Update::Running
    } else if flags & (UPDATE_REQUIRED | UPDATE_PAUSED) != 0
        || (flags & FULLY_INSTALLED != 0 && to_download > downloaded)
    {
        Update::Needed
    } else {
        Update::None
    }
}

fn parse_manifest(acf: &str) -> Option<Game> {
    let (mut id, mut name) = (None, None);
    let (mut flags, mut to_download, mut downloaded) = (0u32, 0u64, 0u64);
    for line in acf.lines() {
        let q = quoted(line);
        if q.len() != 2 {
            continue;
        }
        match q[0].as_str() {
            "appid" => id = q[1].parse::<u32>().ok(),
            "name" => name = Some(q[1].chars().filter(|c| !c.is_control()).take(80).collect::<String>()),
            "StateFlags" => flags = q[1].parse().unwrap_or(0),
            "BytesToDownload" => to_download = q[1].parse().unwrap_or(0),
            "BytesDownloaded" => downloaded = q[1].parse().unwrap_or(0),
            _ => {}
        }
    }
    let (id, name) = (id?, name?);
    (!name.trim().is_empty() && !NOT_GAMES.contains(&id)).then_some(Game {
        id,
        name,
        update: update_state(flags, to_download, downloaded),
    })
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

pub fn refresh() {
    if let Ok(mut cache) = CACHE.lock() {
        *cache = None;
    }
}

pub fn installed() -> Vec<Game> {
    if let Ok(cache) = CACHE.lock()
        && let Some((at, games)) = cache.as_ref()
        && at.elapsed() < Duration::from_secs(30)
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

pub fn update_url(id: u32) -> String {
    format!("steam://install/{id}")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Online,
    Away,
    Invisible,
    Offline,
}

impl Status {
    pub const ALL: [Status; 4] = [Status::Online, Status::Away, Status::Invisible, Status::Offline];

    pub fn label(self) -> &'static str {
        match self {
            Status::Online => "Online",
            Status::Away => "Away",
            Status::Invisible => "Invisible",
            Status::Offline => "Offline",
        }
    }

    pub fn url(self) -> &'static str {
        match self {
            Status::Online => "steam://friends/status/online",
            Status::Away => "steam://friends/status/away",
            Status::Invisible => "steam://friends/status/invisible",
            Status::Offline => "steam://friends/status/offline",
        }
    }

    fn from_state(state: u64) -> Option<Status> {
        match state {
            0 => Some(Status::Offline),
            1 | 2 | 4..=6 => Some(Status::Online),
            3 => Some(Status::Away),
            7 => Some(Status::Invisible),
            _ => None,
        }
    }
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VdfValue {
    Text(String),
    Block(Vdf),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Vdf {
    pub entries: Vec<(String, VdfValue)>,
}

impl Vdf {
    pub fn get(&self, key: &str) -> Option<&VdfValue> {
        self.entries.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v)
    }

    pub fn text(&self, key: &str) -> Option<&str> {
        match self.get(key)? {
            VdfValue::Text(t) => Some(t),
            VdfValue::Block(_) => None,
        }
    }

    pub fn block(&self, key: &str) -> Option<&Vdf> {
        match self.get(key)? {
            VdfValue::Block(b) => Some(b),
            VdfValue::Text(_) => None,
        }
    }

    pub fn path(&self, keys: &[&str]) -> Option<&Vdf> {
        keys.iter().try_fold(self, |block, key| block.block(key))
    }
}

enum Token {
    Str(String),
    Open,
    Close,
}

fn tokens(text: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' => out.push(Token::Open),
            '}' => out.push(Token::Close),
            '"' => {
                let mut s = String::new();
                while let Some(c) = chars.next() {
                    match c {
                        '"' => break,
                        '\\' => match chars.next() {
                            Some('n') => s.push('\n'),
                            Some('t') => s.push('\t'),
                            Some(other) => s.push(other),
                            None => break,
                        },
                        c => s.push(c),
                    }
                }
                out.push(Token::Str(s));
            }
            '/' if chars.peek() == Some(&'/') => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    out
}

pub fn parse_vdf(text: &str) -> Vdf {
    const MAX_DEPTH: usize = 24;
    let tokens = tokens(text);
    let mut stack: Vec<(String, Vdf)> = vec![(String::new(), Vdf::default())];
    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i] {
            Token::Str(key) => match tokens.get(i + 1) {
                Some(Token::Str(value)) => {
                    if let Some((_, top)) = stack.last_mut() {
                        top.entries.push((key.clone(), VdfValue::Text(value.clone())));
                    }
                    i += 2;
                }
                Some(Token::Open) if stack.len() < MAX_DEPTH => {
                    stack.push((key.clone(), Vdf::default()));
                    i += 2;
                }
                _ => i += 1,
            },
            Token::Close => {
                if stack.len() > 1
                    && let Some((key, block)) = stack.pop()
                    && let Some((_, parent)) = stack.last_mut()
                {
                    parent.entries.push((key, VdfValue::Block(block)));
                }
                i += 1;
            }
            Token::Open => i += 1,
        }
    }
    while stack.len() > 1 {
        if let Some((key, block)) = stack.pop()
            && let Some((_, parent)) = stack.last_mut()
        {
            parent.entries.push((key, VdfValue::Block(block)));
        }
    }
    stack.pop().map(|(_, root)| root).unwrap_or_default()
}

const STEAM_ID_BASE: u64 = 76_561_197_960_265_728;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Account {
    pub id64: u64,
    pub name: String,
    pub persona: String,
    pub remembered: bool,
    pub last: i64,
}

impl Account {
    pub fn account_id(&self) -> u32 {
        (self.id64.saturating_sub(STEAM_ID_BASE)) as u32
    }
}

fn valid_login(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

fn parse_accounts(text: &str) -> Vec<Account> {
    let root = parse_vdf(text);
    let Some(users) = root.block("users") else { return Vec::new() };
    let mut out: Vec<Account> = users
        .entries
        .iter()
        .filter_map(|(id, value)| {
            let VdfValue::Block(user) = value else { return None };
            let id64 = id.parse::<u64>().ok().filter(|v| *v > STEAM_ID_BASE)?;
            let name = user.text("AccountName").filter(|n| valid_login(n))?.to_string();
            let persona: String =
                user.text("PersonaName").unwrap_or(&name).chars().filter(|c| !c.is_control()).take(32).collect();
            Some(Account {
                id64,
                name,
                persona,
                remembered: user.text("RememberPassword") == Some("1"),
                last: user.text("Timestamp").and_then(|t| t.parse().ok()).unwrap_or(0),
            })
        })
        .collect();
    out.sort_by(|a, b| b.last.cmp(&a.last).then_with(|| a.name.cmp(&b.name)));
    out
}

fn login_users_path() -> Option<PathBuf> {
    Some(steam_path()?.join("config").join("loginusers.vdf"))
}

pub fn accounts() -> Vec<Account> {
    login_users_path().and_then(|p| std::fs::read_to_string(p).ok()).map(|t| parse_accounts(&t)).unwrap_or_default()
}

pub fn active_account_id() -> Option<u32> {
    let mut value = 0u32;
    let mut size = size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            &HSTRING::from(r"Software\Valve\Steam\ActiveProcess"),
            w!("ActiveUser"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut value as *mut u32 as *mut c_void),
            Some(&mut size),
        )
    };
    (status.is_ok() && value != 0).then_some(value)
}

fn mark_next_login(text: &str, id64: u64, now: i64) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len() + 2);
    let target = id64.to_string();
    let mut block: Option<String> = None;
    let mut had_auto_login = false;
    for (i, line) in lines.iter().enumerate() {
        let plain = line.trim_end_matches('\r');
        let q = quoted(plain);
        let next_is_open = lines.get(i + 1).is_some_and(|n| n.trim() == "{");
        if q.len() == 1 && next_is_open && q[0].len() >= 10 && q[0].chars().all(|c| c.is_ascii_digit()) {
            block = Some(q[0].clone());
            had_auto_login = false;
        } else if plain.trim() == "}" && block.is_some() {
            if block.as_deref() == Some(target.as_str()) && !had_auto_login {
                let indent: String = plain.chars().take_while(|c| c.is_whitespace()).collect();
                out.push(format!("{indent}\t\"AutoLogin\"\t\t\"1\""));
            }
            block = None;
        } else if let (Some(current), 2) = (&block, q.len()) {
            let mine = *current == target;
            let new = match q[0].as_str() {
                "MostRecent" => Some(if mine { "1".to_string() } else { "0".to_string() }),
                "AutoLogin" => {
                    had_auto_login = true;
                    Some(if mine { "1".to_string() } else { "0".to_string() })
                }
                "Timestamp" | "timestamp" if mine => Some(now.to_string()),
                _ => None,
            };
            if let Some(value) = new {
                let indent: String = plain.chars().take_while(|c| c.is_whitespace()).collect();
                let cr = if line.ends_with('\r') { "\r" } else { "" };
                out.push(format!("{indent}\"{}\"\t\t\"{value}\"{cr}", q[0]));
                continue;
            }
        }
        out.push((*line).to_string());
    }
    out.join("\n")
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Played {
    pub last: i64,
    pub minutes: u32,
}

#[derive(Default)]
pub struct LocalConfig {
    pub played: HashMap<u32, Played>,
    pub status: Option<Status>,
}

pub fn local_config(account_id: u32) -> LocalConfig {
    const MAX_BYTES: u64 = 16 * 1024 * 1024;
    let Some(path) =
        steam_path().map(|p| p.join("userdata").join(account_id.to_string()).join("config").join("localconfig.vdf"))
    else {
        return LocalConfig::default();
    };
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > MAX_BYTES) {
        return LocalConfig::default();
    }
    let Ok(text) = std::fs::read_to_string(path) else { return LocalConfig::default() };
    LocalConfig { played: parse_played(&text), status: parse_status(&text, account_id) }
}

fn parse_status(text: &str, account_id: u32) -> Option<Status> {
    let key = format!("\"FriendStoreLocalPrefs_{account_id}\"");
    let line = text.lines().find(|l| l.contains(&key))?;
    let json: serde_json::Value = serde_json::from_str(quoted(line).get(1)?).ok()?;
    Status::from_state(json.get("ePersonaState")?.as_u64()?)
}

fn parse_played(text: &str) -> HashMap<u32, Played> {
    let root = parse_vdf(text);
    let Some(apps) = root.path(&["UserLocalConfigStore", "Software", "Valve", "Steam", "apps"]) else {
        return HashMap::new();
    };
    apps.entries
        .iter()
        .filter_map(|(id, value)| {
            let VdfValue::Block(app) = value else { return None };
            let id = id.parse::<u32>().ok()?;
            let last = app.text("LastPlayed").and_then(|t| t.parse().ok()).unwrap_or(0);
            let minutes = app.text("Playtime").and_then(|t| t.parse().ok()).unwrap_or(0);
            (last > 0 || minutes > 0).then_some((id, Played { last, minutes }))
        })
        .collect()
}

pub const WM_STEAM_SWITCH: u32 = WM_APP + 33;

static SWITCH_RESULT: Mutex<Option<Result<String, &'static str>>> = Mutex::new(None);
static SWITCHING: AtomicBool = AtomicBool::new(false);

pub fn switching() -> bool {
    SWITCHING.load(Ordering::SeqCst)
}

pub fn take_switch_result() -> Option<Result<String, &'static str>> {
    SWITCH_RESULT.lock().ok()?.take()
}

fn steam_exe() -> Option<PathBuf> {
    let exe = steam_path()?.join("steam.exe");
    exe.is_file().then_some(exe)
}

pub fn client_installed() -> bool {
    static FOUND: OnceLock<bool> = OnceLock::new();
    *FOUND.get_or_init(|| steam_exe().is_some())
}

fn process_running(name: &str) -> bool {
    unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else { return false };
        let mut entry = PROCESSENTRY32W { dwSize: size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut ok = Process32FirstW(snap, &mut entry).is_ok();
        let mut found = false;
        while ok && !found {
            let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
            found = String::from_utf16_lossy(&entry.szExeFile[..len]).eq_ignore_ascii_case(name);
            ok = Process32NextW(snap, &mut entry).is_ok();
        }
        let _ = CloseHandle(snap);
        found
    }
}

pub fn is_running() -> bool {
    process_running("steam.exe")
}

fn shell_run(exe: &Path, args: PCWSTR, show: SHOW_WINDOW_CMD) {
    unsafe {
        ShellExecuteW(None, w!("open"), &HSTRING::from(exe.to_string_lossy().as_ref()), args, PCWSTR::null(), show);
    }
}

fn set_registry(name: PCWSTR, kind: REG_VALUE_TYPE, data: &[u8]) -> bool {
    unsafe {
        let mut key = HKEY::default();
        if RegOpenKeyExW(HKEY_CURRENT_USER, &HSTRING::from(KEY), None, KEY_SET_VALUE, &mut key).is_err() {
            return false;
        }
        let ok = RegSetValueExW(key, name, None, kind, Some(data)).is_ok();
        let _ = RegCloseKey(key);
        ok
    }
}

pub fn switch_account(hwnd: HWND, login: &str) -> bool {
    if SWITCHING.swap(true, Ordering::SeqCst) {
        return false;
    }
    let (raw, login) = (hwnd.0 as isize, login.to_string());
    std::thread::spawn(move || {
        let result = switch_now(&login);
        if let Ok(mut slot) = SWITCH_RESULT.lock() {
            *slot = Some(result);
        }
        SWITCHING.store(false, Ordering::SeqCst);
        unsafe {
            let _ = PostMessageW(Some(HWND(raw as *mut c_void)), WM_STEAM_SWITCH, WPARAM(0), LPARAM(0));
        }
    });
    true
}

fn switch_now(login: &str) -> Result<String, &'static str> {
    let account = accounts().into_iter().find(|a| a.name == login).ok_or("That account is not known to Steam")?;
    if !account.remembered {
        return Err("Steam does not remember that account: sign in to it once in Steam");
    }
    let exe = steam_exe().ok_or("Steam was not found")?;
    if process_running("steam.exe") {
        shell_run(&exe, w!("-shutdown"), SW_HIDE);
        let start = Instant::now();
        while process_running("steam.exe") {
            if start.elapsed() > Duration::from_secs(40) {
                return Err("Steam did not close");
            }
            std::thread::sleep(Duration::from_millis(300));
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    let name16: Vec<u8> = login.encode_utf16().chain(std::iter::once(0)).flat_map(u16::to_le_bytes).collect();
    if !set_registry(w!("AutoLoginUser"), REG_SZ, &name16)
        || !set_registry(w!("RememberPassword"), REG_DWORD, &1u32.to_le_bytes())
    {
        return Err("Could not change Steam's sign-in setting");
    }
    if let Some(path) = login_users_path()
        && let Ok(text) = std::fs::read_to_string(&path)
    {
        let backup = path.with_extension("vdf.night-island.bak");
        if !backup.exists() {
            let _ = std::fs::copy(&path, &backup);
        }
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
        let _ = std::fs::write(&path, mark_next_login(&text, account.id64, now));
    }
    shell_run(&exe, PCWSTR::null(), SW_SHOWNORMAL);
    Ok(account.persona)
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
        assert_eq!(parse_manifest(acf), Some(Game { id: 620, name: "Portal 2".into(), update: Update::None }));
        let redist =
            "\"AppState\"\n{\n\t\"appid\"\t\t\"228980\"\n\t\"name\"\t\t\"Steamworks Common Redistributables\"\n}";
        assert_eq!(parse_manifest(redist), None);
        assert_eq!(launch_url(620), "steam://rungameid/620");
    }
}

#[cfg(test)]
mod account_tests {
    use super::*;

    const LOGINUSERS: &str = "\"users\"\n{\n\t\"76561199197613739\"\n\t{\n\t\t\"AccountName\"\t\t\"main_acc\"\n\t\t\"PersonaName\"\t\t\"Main\"\n\t\t\"RememberPassword\"\t\t\"1\"\n\t\t\"AutoLogin\"\t\t\"1\"\n\t\t\"timestamp\"\t\t\"1792830951\"\n\t}\n\t\"76561198983762385\"\n\t{\n\t\t\"AccountName\"\t\t\"alt-acc\"\n\t\t\"PersonaName\"\t\t\"Alt\"\n\t\t\"RememberPassword\"\t\t\"1\"\n\t\t\"AutoLogin\"\t\t\"0\"\n\t\t\"timestamp\"\t\t\"1792785149\"\n\t}\n\t\"76561199598295706\"\n\t{\n\t\t\"AccountName\"\t\t\"bad name!\"\n\t\t\"RememberPassword\"\t\t\"0\"\n\t}\n}\n";

    #[test]
    fn the_vdf_tree_is_read() {
        let v = parse_vdf("// c\n\"a\"\n{\n\t\"b\"\t\"1\"\n\t\"C\"\n\t{\n\t\t\"d\"\t\"x\\\"y\"\n\t}\n}");
        assert_eq!(v.path(&["a"]).and_then(|a| a.text("B")), Some("1"));
        assert_eq!(v.path(&["a", "c"]).and_then(|c| c.text("d")), Some("x\"y"));
        assert!(v.path(&["a", "nope"]).is_none());
        let broken = parse_vdf("\"a\" { \"b\" { \"c\"");
        assert!(broken.path(&["a", "b"]).is_some());
        assert_eq!(parse_vdf(&"\"k\" {".repeat(500)).entries.len(), 1);
        assert_eq!(parse_vdf("").entries.len(), 0);
    }

    #[test]
    fn accounts_come_from_loginusers_and_odd_names_are_dropped() {
        let accounts = parse_accounts(LOGINUSERS);
        assert_eq!(accounts.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(), ["main_acc", "alt-acc"]);
        assert_eq!(accounts[0].persona, "Main");
        assert!(accounts.iter().all(|a| a.remembered));
        assert_eq!(accounts[0].account_id(), (76_561_199_197_613_739u64 - STEAM_ID_BASE) as u32);
    }

    #[test]
    fn the_next_login_is_marked_without_touching_the_rest() {
        let out = mark_next_login(LOGINUSERS, 76_561_198_983_762_385, 1_800_000_000);
        let accounts = parse_accounts(&out);
        assert_eq!(accounts[0].name, "alt-acc", "the chosen account is now the latest");
        let root = parse_vdf(&out);
        let users = root.block("users").unwrap();
        assert_eq!(users.block("76561198983762385").unwrap().text("AutoLogin"), Some("1"));
        assert_eq!(users.block("76561199197613739").unwrap().text("AutoLogin"), Some("0"));
        assert_eq!(users.block("76561199197613739").unwrap().text("AccountName"), Some("main_acc"));
        assert_eq!(out.lines().count(), LOGINUSERS.lines().count(), "no line was added or lost");
        let third = mark_next_login(LOGINUSERS, 76_561_199_598_295_706, 5);
        assert_eq!(
            parse_vdf(&third).block("users").unwrap().block("76561199598295706").unwrap().text("AutoLogin"),
            Some("1")
        );
    }

    #[test]
    fn play_times_come_from_the_apps_block() {
        let text = "\"UserLocalConfigStore\"\n{\n\t\"Software\"\n\t{\n\t\t\"Valve\"\n\t\t{\n\t\t\t\"Steam\"\n\t\t\t{\n\t\t\t\t\"apps\"\n\t\t\t\t{\n\t\t\t\t\t\"570\"\n\t\t\t\t\t{\n\t\t\t\t\t\t\"LastPlayed\"\t\t\"1791022549\"\n\t\t\t\t\t\t\"Playtime\"\t\t\"115872\"\n\t\t\t\t\t}\n\t\t\t\t\t\"7\"\n\t\t\t\t\t{\n\t\t\t\t\t\t\"cloud\"\n\t\t\t\t\t\t{\n\t\t\t\t\t\t}\n\t\t\t\t\t}\n\t\t\t\t}\n\t\t\t}\n\t\t}\n\t}\n}";
        let played = parse_played(text);
        assert_eq!(played.get(&570), Some(&Played { last: 1_791_022_549, minutes: 115_872 }));
        assert!(!played.contains_key(&7), "a game never played has nothing to show");
    }

    #[test]
    fn updates_come_from_the_manifest() {
        let manifest = |flags: &str, to: &str, done: &str| {
            format!(
                "\"AppState\"\n{{\n\t\"appid\"\t\t\"10\"\n\t\"name\"\t\t\"G\"\n\t\"StateFlags\"\t\t\"{flags}\"\n\t\"BytesToDownload\"\t\t\"{to}\"\n\t\"BytesDownloaded\"\t\t\"{done}\"\n}}"
            )
        };
        let state = |m: String| parse_manifest(&m).unwrap().update;
        assert_eq!(state(manifest("4", "900", "900")), Update::None);
        assert_eq!(state(manifest("6", "900", "0")), Update::Needed);
        assert_eq!(state(manifest("4", "900", "100")), Update::Needed);
        assert_eq!(state(manifest("1048582", "900", "100")), Update::Running);
        assert_eq!(state(manifest("260", "900", "100")), Update::Running);
    }

    #[test]
    fn the_status_comes_from_the_stored_prefs() {
        let text = "\t\t\"FriendStoreLocalPrefs_42\"\t\t\"{\\\"ePersonaState\\\":7,\\\"x\\\":\\\"\\\"}\"\n";
        assert_eq!(parse_status(text, 42), Some(Status::Invisible));
        assert_eq!(parse_status(text, 4), None);
        assert_eq!(Status::from_state(3), Some(Status::Away));
        assert_eq!(update_url(7), "steam://install/7");
    }

    #[test]
    #[ignore = "reads this computer"]
    fn reads_this_steam() {
        let accounts = accounts();
        println!(
            "{} accounts, {} remembered, active {:?}",
            accounts.len(),
            accounts.iter().filter(|a| a.remembered).count(),
            active_account_id()
        );
        let id = active_account_id().or_else(|| accounts.first().map(Account::account_id)).expect("an account");
        let played = local_config(id).played;
        println!("{} games with play time, {} installed, running: {}", played.len(), installed().len(), is_running());
        assert!(!accounts.is_empty());
    }
}
