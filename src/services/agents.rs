use std::ffi::c_void;
use std::io::Read;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::Storage::FileSystem::{
    FILE_NOTIFY_CHANGE_FILE_NAME, FILE_NOTIFY_CHANGE_LAST_WRITE, FindCloseChangeNotification,
    FindFirstChangeNotificationW, FindNextChangeNotification,
};
use windows::Win32::System::Threading::{INFINITE, WaitForSingleObject};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};
use windows::core::HSTRING;

pub const WM_AGENT: u32 = WM_APP + 28;

const MARKER: &str = "--agent-event";
const STALE_HOURS: i64 = 24;
const WORKING_STALE_MIN: i64 = 45;
const DONE_KEEP_MIN: i64 = 30;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Agent {
    Claude,
    Codex,
}

impl Agent {
    pub const ALL: [Agent; 2] = [Agent::Claude, Agent::Codex];

    pub fn name(self) -> &'static str {
        match self {
            Agent::Claude => "Claude Code",
            Agent::Codex => "Codex CLI",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Agent::Claude => "claude",
            Agent::Codex => "codex",
        }
    }

    fn from_key(key: &str) -> Option<Agent> {
        Self::ALL.into_iter().find(|a| a.key() == key)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum State {
    Working,
    Waiting,
    Done,
}

impl State {
    fn key(self) -> &'static str {
        match self {
            State::Working => "working",
            State::Waiting => "waiting",
            State::Done => "done",
        }
    }

    fn from_key(key: &str) -> Option<State> {
        [State::Working, State::Waiting, State::Done].into_iter().find(|s| s.key() == key)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Session {
    pub agent: Agent,
    pub id: String,
    pub state: State,
    pub project: String,
    pub at: i64,
}

fn unix_now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

fn dir() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("APPDATA")?).join("NightIsland").join("agents"))
}

fn safe_id(id: &str) -> String {
    id.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_')).take(64).collect()
}

fn project_name(cwd: &str) -> String {
    let cwd = cwd.trim_end_matches(['\\', '/']);
    cwd.rsplit(['\\', '/']).next().unwrap_or(cwd).chars().filter(|c| !c.is_control()).take(40).collect()
}

pub fn handle_cli() -> bool {
    let args: Vec<String> = std::env::args().collect();
    let Some(at) = args.iter().position(|a| a == MARKER) else { return false };
    let (Some(agent), Some(state)) = (args.get(at + 1), args.get(at + 2)) else { return true };
    let Some(agent) = Agent::from_key(agent) else { return true };
    let payload: Value = match agent {
        Agent::Claude => {
            let mut text = String::new();
            let _ = std::io::stdin().take(64 * 1024).read_to_string(&mut text);
            serde_json::from_str(&text).unwrap_or(Value::Null)
        }
        Agent::Codex => args.get(at + 3).and_then(|a| serde_json::from_str(a).ok()).unwrap_or(Value::Null),
    };
    let id = ["session_id", "thread-id", "thread_id"].iter().find_map(|k| payload[*k].as_str()).unwrap_or("main");
    let cwd = payload["cwd"].as_str().unwrap_or("");
    record(agent, State::from_key(state), id, cwd);
    true
}

fn record(agent: Agent, state: Option<State>, id: &str, cwd: &str) {
    let Some(dir) = dir() else { return };
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(format!("{}-{}.json", agent.key(), safe_id(id)));
    let Some(state) = state else {
        let _ = std::fs::remove_file(path);
        return;
    };
    let known = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok());
    let cwd = if cwd.is_empty() { known.as_ref().and_then(|v| v["cwd"].as_str()).unwrap_or("") } else { cwd };
    let body = json!({"state": state.key(), "cwd": cwd, "at": unix_now()}).to_string();
    let tmp = path.with_extension("tmp");
    if std::fs::write(&tmp, body).is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
}

pub fn sessions() -> Vec<Session> {
    let Some(dir) = dir() else { return Vec::new() };
    let Ok(entries) = std::fs::read_dir(&dir) else { return Vec::new() };
    let now = unix_now();
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Some((agent, id)) = name.split_once('-') else { continue };
        let Some(agent) = Agent::from_key(agent) else { continue };
        let value = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok());
        let Some(value) = value else { continue };
        let (Some(state), Some(at)) = (value["state"].as_str().and_then(State::from_key), value["at"].as_i64()) else {
            continue;
        };
        let age_min = (now - at) / 60;
        let dead = (now - at) > STALE_HOURS * 3600
            || (state == State::Working && age_min > WORKING_STALE_MIN)
            || (state == State::Done && age_min > DONE_KEEP_MIN);
        if dead {
            if (now - at) > STALE_HOURS * 3600 {
                let _ = std::fs::remove_file(&path);
            }
            continue;
        }
        out.push(Session {
            agent,
            id: id.to_string(),
            state,
            project: project_name(value["cwd"].as_str().unwrap_or("")),
            at,
        });
    }
    out.sort_by(|a, b| b.at.cmp(&a.at).then_with(|| a.id.cmp(&b.id)));
    out
}

pub fn watch(hwnd: HWND) {
    let Some(dir) = dir() else { return };
    let _ = std::fs::create_dir_all(&dir);
    let raw = hwnd.0 as isize;
    std::thread::spawn(move || unsafe {
        let Ok(handle) = FindFirstChangeNotificationW(
            &HSTRING::from(dir.to_string_lossy().as_ref()),
            false,
            FILE_NOTIFY_CHANGE_FILE_NAME | FILE_NOTIFY_CHANGE_LAST_WRITE,
        ) else {
            return;
        };
        loop {
            WaitForSingleObject(handle, INFINITE);
            std::thread::sleep(Duration::from_millis(120));
            if FindNextChangeNotification(handle).is_err() {
                break;
            }
            let _ = PostMessageW(Some(HWND(raw as *mut c_void)), WM_AGENT, WPARAM(0), LPARAM(0));
        }
        let _ = FindCloseChangeNotification(handle);
    });
}

fn home() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").map(PathBuf::from)
}

fn command_for(agent: Agent, state: &str) -> Option<String> {
    let exe = std::env::current_exe().ok()?.to_string_lossy().replace('\\', "/");
    Some(format!("\"{exe}\" {MARKER} {} {state}", agent.key()))
}

fn claude_settings() -> Option<PathBuf> {
    Some(home()?.join(".claude").join("settings.json"))
}

fn codex_config() -> Option<PathBuf> {
    Some(home()?.join(".codex").join("config.toml"))
}

const CLAUDE_EVENTS: [(&str, &str); 5] = [
    ("UserPromptSubmit", "working"),
    ("PreToolUse", "working"),
    ("Notification", "waiting"),
    ("Stop", "done"),
    ("SessionEnd", "end"),
];

fn is_ours(entry: &Value) -> bool {
    entry["hooks"]
        .as_array()
        .is_some_and(|hooks| hooks.iter().any(|h| h["command"].as_str().is_some_and(|c| c.contains(MARKER))))
}

pub type Failure = &'static str;

pub fn connected(agent: Agent) -> bool {
    match agent {
        Agent::Claude => claude_settings()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .is_some_and(|v| {
                v["hooks"].as_object().is_some_and(|h| h.values().filter_map(Value::as_array).flatten().any(is_ours))
            }),
        Agent::Codex => codex_config()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .is_some_and(|t| t.lines().any(|l| l.trim_start().starts_with("notify") && l.contains(MARKER))),
    }
}

pub fn connect(agent: Agent) -> Result<(), Failure> {
    match agent {
        Agent::Claude => connect_claude(),
        Agent::Codex => connect_codex(),
    }
}

pub fn disconnect(agent: Agent) -> Result<(), Failure> {
    match agent {
        Agent::Claude => disconnect_claude(),
        Agent::Codex => disconnect_codex(),
    }
}

fn back_up(path: &std::path::Path) {
    let backup =
        path.with_extension(format!("{}.night-island.bak", path.extension().and_then(|e| e.to_str()).unwrap_or("")));
    if path.exists() && !backup.exists() {
        let _ = std::fs::copy(path, backup);
    }
}

fn read_claude_settings(path: &std::path::Path) -> Result<Value, Failure> {
    match std::fs::read_to_string(path) {
        Ok(text) if text.trim().is_empty() => Ok(json!({})),
        Ok(text) => serde_json::from_str(&text).map_err(|_| "The Claude Code settings file is not valid JSON"),
        Err(_) => Ok(json!({})),
    }
}

fn write_claude_settings(path: &std::path::Path, value: &Value) -> Result<(), Failure> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let text = serde_json::to_string_pretty(value).map_err(|_| "Could not write the settings")?;
    std::fs::write(path, text).map_err(|_| "Could not write the Claude Code settings file")
}

fn connect_claude() -> Result<(), Failure> {
    let path = claude_settings().ok_or("The user folder was not found")?;
    let mut settings = read_claude_settings(&path)?;
    back_up(&path);
    if !settings.is_object() {
        return Err("The Claude Code settings file is not valid JSON");
    }
    let hooks = settings.as_object_mut().ok_or("The Claude Code settings file is not valid JSON")?;
    let hooks = hooks.entry("hooks").or_insert_with(|| json!({}));
    let hooks = hooks.as_object_mut().ok_or("The hooks in the Claude Code settings are not an object")?;
    for (event, state) in CLAUDE_EVENTS {
        let command = command_for(Agent::Claude, state).ok_or("The program path is unknown")?;
        let list = hooks.entry(event).or_insert_with(|| json!([]));
        let list = list.as_array_mut().ok_or("A hook list in the Claude Code settings is not a list")?;
        list.retain(|e| !is_ours(e));
        list.push(json!({"hooks": [{"type": "command", "command": command}]}));
    }
    write_claude_settings(&path, &settings)
}

fn disconnect_claude() -> Result<(), Failure> {
    let path = claude_settings().ok_or("The user folder was not found")?;
    let mut settings = read_claude_settings(&path)?;
    if let Some(hooks) = settings["hooks"].as_object_mut() {
        for list in hooks.values_mut() {
            if let Some(list) = list.as_array_mut() {
                list.retain(|e| !is_ours(e));
            }
        }
        hooks.retain(|_, list| list.as_array().is_none_or(|l| !l.is_empty()));
    }
    if settings["hooks"].as_object().is_some_and(|h| h.is_empty())
        && let Some(root) = settings.as_object_mut()
    {
        root.remove("hooks");
    }
    write_claude_settings(&path, &settings)
}

fn connect_codex() -> Result<(), Failure> {
    let path = codex_config().ok_or("The user folder was not found")?;
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let has_notify = text.lines().any(|l| l.trim_start().starts_with("notify") && !l.contains(MARKER));
    if has_notify {
        return Err("Codex already has a notify command of its own");
    }
    back_up(&path);
    let command = command_for(Agent::Codex, "done").ok_or("The program path is unknown")?;
    let exe = command.split('"').nth(1).unwrap_or("");
    let line = format!("notify = [{}, \"{MARKER}\", \"codex\", \"done\"]", json!(exe));
    let mut lines: Vec<&str> =
        text.lines().filter(|l| !(l.trim_start().starts_with("notify") && l.contains(MARKER))).collect();
    lines.insert(0, &line);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(&path, lines.join("\n") + "\n").map_err(|_| "Could not write the Codex configuration")
}

fn disconnect_codex() -> Result<(), Failure> {
    let path = codex_config().ok_or("The user folder was not found")?;
    let Ok(text) = std::fs::read_to_string(&path) else { return Ok(()) };
    let kept: Vec<&str> =
        text.lines().filter(|l| !(l.trim_start().starts_with("notify") && l.contains(MARKER))).collect();
    std::fs::write(&path, kept.join("\n") + "\n").map_err(|_| "Could not write the Codex configuration")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_ids_are_made_safe() {
        assert_eq!(safe_id("ab12-CD_3/../x"), "ab12-CD_3x");
        assert_eq!(project_name(r"C:\Users\me\code\night-island\"), "night-island");
        assert_eq!(project_name("/home/me/app"), "app");
        assert_eq!(project_name(""), "");
    }

    #[test]
    fn our_hook_entries_are_recognised() {
        let ours =
            json!({"hooks": [{"type": "command", "command": "\"C:/x/night-island.exe\" --agent-event claude done"}]});
        let theirs = json!({"hooks": [{"type": "command", "command": "echo hi"}]});
        assert!(is_ours(&ours));
        assert!(!is_ours(&theirs));
        assert!(!is_ours(&json!({})));
    }

    #[test]
    fn states_round_trip() {
        for s in [State::Working, State::Waiting, State::Done] {
            assert_eq!(State::from_key(s.key()), Some(s));
        }
        assert_eq!(Agent::from_key("claude"), Some(Agent::Claude));
        assert_eq!(Agent::from_key("nope"), None);
    }
}
