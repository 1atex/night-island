use std::ffi::c_void;
use std::io::Read;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use toml_edit::{Array, DocumentMut, value};
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
const CHAIN: &str = "--chain";
const STALE_HOURS: i64 = 24;
const WORKING_STALE_MIN: i64 = 45;
const DONE_KEEP_MIN: i64 = 30;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Agent {
    Claude,
    Codex,
    Gemini,
    Cursor,
    Copilot,
}

impl Agent {
    pub const ALL: [Agent; 5] = [Agent::Claude, Agent::Codex, Agent::Gemini, Agent::Cursor, Agent::Copilot];

    pub fn name(self) -> &'static str {
        match self {
            Agent::Claude => "Claude Code",
            Agent::Codex => "Codex CLI",
            Agent::Gemini => "Gemini CLI",
            Agent::Cursor => "Cursor",
            Agent::Copilot => "Copilot CLI",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            Agent::Claude => "Claude",
            Agent::Codex => "Codex",
            Agent::Gemini => "Gemini",
            Agent::Cursor => "Cursor",
            Agent::Copilot => "Copilot",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Agent::Claude => "claude",
            Agent::Codex => "codex",
            Agent::Gemini => "gemini",
            Agent::Cursor => "cursor",
            Agent::Copilot => "copilot",
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

fn stdin_json() -> Value {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut text = String::new();
        let _ = std::io::stdin().take(64 * 1024).read_to_string(&mut text);
        let _ = tx.send(text);
    });
    rx.recv_timeout(Duration::from_millis(1500)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null)
}

fn codex_args(args: &[String]) -> (Value, Vec<String>) {
    let json_at = args.len().checked_sub(1).filter(|&i| args[i].trim_start().starts_with('{'));
    let payload = json_at.and_then(|i| serde_json::from_str(&args[i]).ok()).unwrap_or(Value::Null);
    let end = json_at.unwrap_or(args.len());
    let chain = args[..end].iter().position(|a| a == CHAIN).map_or_else(Vec::new, |c| args[c + 1..end].to_vec());
    (payload, chain)
}

fn run_chain(chain: &[String], event: &Value) {
    use std::os::windows::process::CommandExt;
    let Some((program, rest)) = chain.split_first() else { return };
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let _ = std::process::Command::new(program)
        .args(rest)
        .arg(event.to_string())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
}

pub fn handle_cli() -> bool {
    let args: Vec<String> = std::env::args().collect();
    let Some(at) = args.iter().position(|a| a == MARKER) else { return false };
    let (Some(agent), Some(state)) = (args.get(at + 1), args.get(at + 2)) else { return true };
    let Some(agent) = Agent::from_key(agent) else { return true };
    let (payload, chain) = match agent {
        Agent::Codex => codex_args(&args[at + 3..]),
        _ => (stdin_json(), Vec::new()),
    };
    let from_env = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
    let id = ["session_id", "sessionId", "conversation_id", "thread-id", "thread_id"]
        .iter()
        .find_map(|k| payload[*k].as_str().map(str::to_string))
        .or_else(|| from_env("GEMINI_SESSION_ID"))
        .unwrap_or_else(|| "main".to_string());
    let cwd = payload["cwd"]
        .as_str()
        .or_else(|| payload["workspace_roots"][0].as_str())
        .map(str::to_string)
        .or_else(|| from_env("GEMINI_CWD"))
        .unwrap_or_default();
    record(agent, State::from_key(state), &id, &cwd);
    if agent == Agent::Codex {
        run_chain(&chain, &payload);
    }
    if agent == Agent::Cursor {
        println!("{{\"continue\": true}}");
    }
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

fn gemini_settings() -> Option<PathBuf> {
    Some(home()?.join(".gemini").join("settings.json"))
}

fn cursor_hooks() -> Option<PathBuf> {
    Some(home()?.join(".cursor").join("hooks.json"))
}

fn copilot_hooks() -> Option<PathBuf> {
    Some(home()?.join(".copilot").join("hooks").join("night-island.json"))
}

const CLAUDE_EVENTS: [(&str, &str); 5] = [
    ("UserPromptSubmit", "working"),
    ("PreToolUse", "working"),
    ("Notification", "waiting"),
    ("Stop", "done"),
    ("SessionEnd", "end"),
];
const GEMINI_EVENTS: [(&str, &str); 4] =
    [("BeforeAgent", "working"), ("Notification", "waiting"), ("AfterAgent", "done"), ("SessionEnd", "end")];
const CURSOR_EVENTS: [(&str, &str); 4] =
    [("beforeSubmitPrompt", "working"), ("preToolUse", "working"), ("stop", "done"), ("sessionEnd", "end")];
const COPILOT_EVENTS: [(&str, &str); 5] = [
    ("userPromptSubmitted", "working"),
    ("preToolUse", "working"),
    ("permissionRequest", "waiting"),
    ("agentStop", "done"),
    ("sessionEnd", "end"),
];

fn is_ours(entry: &Value) -> bool {
    let ours = |c: &Value| c.as_str().is_some_and(|c| c.contains(MARKER));
    ours(&entry["command"]) || entry["hooks"].as_array().is_some_and(|hooks| hooks.iter().any(|h| ours(&h["command"])))
}

pub type Failure = &'static str;

fn read_json_file(path: &std::path::Path, invalid: Failure) -> Result<Value, Failure> {
    match std::fs::read_to_string(path) {
        Ok(text) if text.trim().is_empty() => Ok(json!({})),
        Ok(text) => serde_json::from_str(&text).map_err(|_| invalid),
        Err(_) => Ok(json!({})),
    }
}

fn write_json_file(path: &std::path::Path, value: &Value, failed: Failure) -> Result<(), Failure> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let text = serde_json::to_string_pretty(value).map_err(|_| failed)?;
    std::fs::write(path, text).map_err(|_| failed)
}

struct HookFile {
    path: Option<PathBuf>,
    events: &'static [(&'static str, &'static str)],
    agent: Agent,
    nested: bool,
    invalid: Failure,
}

fn hook_file(agent: Agent) -> Option<HookFile> {
    Some(match agent {
        Agent::Claude => HookFile {
            path: claude_settings(),
            events: &CLAUDE_EVENTS,
            agent,
            nested: true,
            invalid: "The Claude Code settings file is not valid JSON",
        },
        Agent::Gemini => HookFile {
            path: gemini_settings(),
            events: &GEMINI_EVENTS,
            agent,
            nested: true,
            invalid: "The Gemini CLI settings file is not valid JSON",
        },
        Agent::Cursor => HookFile {
            path: cursor_hooks(),
            events: &CURSOR_EVENTS,
            agent,
            nested: false,
            invalid: "The Cursor hooks file is not valid JSON",
        },
        Agent::Codex | Agent::Copilot => return None,
    })
}

pub fn connected(agent: Agent) -> bool {
    match agent {
        Agent::Codex => codex_config()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|t| t.parse::<DocumentMut>().ok())
            .and_then(|doc| codex_notify(&doc))
            .is_some_and(|n| n.iter().any(|a| a == MARKER)),
        Agent::Copilot => copilot_hooks().is_some_and(|p| p.is_file()),
        _ => hook_file(agent)
            .and_then(|f| f.path)
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .is_some_and(|v| {
                v["hooks"].as_object().is_some_and(|h| h.values().filter_map(Value::as_array).flatten().any(is_ours))
            }),
    }
}

pub fn connect(agent: Agent) -> Result<(), Failure> {
    match agent {
        Agent::Codex => connect_codex(),
        Agent::Copilot => connect_copilot(),
        _ => connect_hooks(&hook_file(agent).ok_or("This agent has no hooks")?),
    }
}

pub fn disconnect(agent: Agent) -> Result<(), Failure> {
    match agent {
        Agent::Codex => disconnect_codex(),
        Agent::Copilot => {
            let path = copilot_hooks().ok_or("The user folder was not found")?;
            match std::fs::remove_file(path) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(_) => Err("Could not remove the Copilot CLI hooks file"),
            }
        }
        _ => disconnect_hooks(&hook_file(agent).ok_or("This agent has no hooks")?),
    }
}

fn back_up(path: &std::path::Path) {
    let backup =
        path.with_extension(format!("{}.night-island.bak", path.extension().and_then(|e| e.to_str()).unwrap_or("")));
    if path.exists() && !backup.exists() {
        let _ = std::fs::copy(path, backup);
    }
}

fn connect_hooks(file: &HookFile) -> Result<(), Failure> {
    let path = file.path.as_ref().ok_or("The user folder was not found")?;
    let mut settings = read_json_file(path, file.invalid)?;
    back_up(path);
    let root = settings.as_object_mut().ok_or(file.invalid)?;
    if !file.nested {
        root.entry("version").or_insert_with(|| json!(1));
    }
    let hooks = root.entry("hooks").or_insert_with(|| json!({}));
    let hooks = hooks.as_object_mut().ok_or("The hooks in the agent's settings are not an object")?;
    for (event, state) in file.events {
        let command = command_for(file.agent, state).ok_or("The program path is unknown")?;
        let list = hooks.entry(*event).or_insert_with(|| json!([]));
        let list = list.as_array_mut().ok_or("A hook list in the agent's settings is not a list")?;
        list.retain(|e| !is_ours(e));
        list.push(if file.nested {
            if file.agent == Agent::Gemini {
                json!({"matcher": "*", "hooks": [{"name": "night-island", "type": "command", "command": command}]})
            } else {
                json!({"hooks": [{"type": "command", "command": command}]})
            }
        } else {
            json!({"command": command})
        });
    }
    write_json_file(path, &settings, "Could not write the agent's settings file")
}

fn disconnect_hooks(file: &HookFile) -> Result<(), Failure> {
    let path = file.path.as_ref().ok_or("The user folder was not found")?;
    let mut settings = read_json_file(path, file.invalid)?;
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
    write_json_file(path, &settings, "Could not write the agent's settings file")
}

fn connect_copilot() -> Result<(), Failure> {
    let path = copilot_hooks().ok_or("The user folder was not found")?;
    let exe = std::env::current_exe().map_err(|_| "The program path is unknown")?.to_string_lossy().replace('\\', "/");
    let mut hooks = serde_json::Map::new();
    for (event, state) in COPILOT_EVENTS {
        hooks.insert(
            event.to_string(),
            json!([{"type": "command", "exec": exe, "args": [MARKER, "copilot", state], "timeoutSec": 10}]),
        );
    }
    write_json_file(&path, &json!({"version": 1, "hooks": hooks}), "Could not write the Copilot CLI hooks file")
}

fn codex_notify(doc: &DocumentMut) -> Option<Vec<String>> {
    doc.get("notify")?.as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
}

fn chained(notify: &[String]) -> Vec<String> {
    notify.iter().position(|a| a == CHAIN).map_or_else(Vec::new, |c| notify[c + 1..].to_vec())
}

fn string_array(items: &[String]) -> Array {
    let mut array = Array::new();
    for item in items {
        array.push(item.as_str());
    }
    array
}

fn read_codex_config(path: &std::path::Path) -> Result<DocumentMut, Failure> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    text.parse::<DocumentMut>().map_err(|_| "The Codex configuration is not valid TOML")
}

fn connect_codex() -> Result<(), Failure> {
    let path = codex_config().ok_or("The user folder was not found")?;
    let mut doc = read_codex_config(&path)?;
    if doc.get("notify").is_some_and(|n| !n.is_array()) {
        return Err("The notify setting of Codex has an unexpected form");
    }
    back_up(&path);
    let exe = std::env::current_exe().map_err(|_| "The program path is unknown")?.to_string_lossy().replace('\\', "/");
    let before = codex_notify(&doc).unwrap_or_default();
    let previous = if before.iter().any(|a| a == MARKER) { chained(&before) } else { before };
    let mut command = vec![exe, MARKER.to_string(), "codex".to_string(), "done".to_string()];
    if !previous.is_empty() {
        command.push(CHAIN.to_string());
        command.extend(previous);
    }
    doc["notify"] = value(string_array(&command));
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(&path, doc.to_string()).map_err(|_| "Could not write the Codex configuration")
}

fn disconnect_codex() -> Result<(), Failure> {
    let path = codex_config().ok_or("The user folder was not found")?;
    if !path.exists() {
        return Ok(());
    }
    let mut doc = read_codex_config(&path)?;
    let Some(now) = codex_notify(&doc).filter(|n| n.iter().any(|a| a == MARKER)) else { return Ok(()) };
    let old = chained(&now);
    if old.is_empty() {
        doc.remove("notify");
    } else {
        doc["notify"] = value(string_array(&old));
    }
    std::fs::write(&path, doc.to_string()).map_err(|_| "Could not write the Codex configuration")
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
    fn flat_and_nested_hook_entries_are_recognised() {
        assert!(is_ours(&json!({"command": "\"C:/x/night-island.exe\" --agent-event cursor stop"})));
        assert!(!is_ours(&json!({"command": "./hooks/on-stop.sh"})));
        assert!(is_ours(&json!({"matcher": "*", "hooks": [{"command": "x --agent-event gemini done"}]})));
    }

    #[test]
    fn codex_remembers_the_command_it_had_before() {
        let doc: DocumentMut =
            "model = \"x\"\nnotify = [\"a.exe\", \"turn-ended\"]\n[windows]\nsandbox = \"e\"\n".parse().unwrap();
        let before = codex_notify(&doc).unwrap();
        assert_eq!(before, ["a.exe", "turn-ended"]);
        let ours = vec![
            "n.exe".to_string(),
            MARKER.into(),
            "codex".into(),
            "done".into(),
            CHAIN.into(),
            "a.exe".into(),
            "turn-ended".into(),
        ];
        assert_eq!(chained(&ours), ["a.exe", "turn-ended"]);
        assert!(chained(&["n.exe".to_string(), MARKER.into(), "codex".into(), "done".into()]).is_empty());
    }

    #[test]
    fn codex_arguments_give_the_event_and_the_chained_command() {
        let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let (event, chain) =
            codex_args(&a(&["done", CHAIN, "a.exe", "turn-ended", "{\"cwd\":\"C:/p\",\"thread-id\":\"t1\"}"]));
        assert_eq!(event["cwd"], "C:/p");
        assert_eq!(chain, ["a.exe", "turn-ended"]);
        let (event, chain) = codex_args(&a(&["done", "{\"thread-id\":\"t1\"}"]));
        assert_eq!(event["thread-id"], "t1");
        assert!(chain.is_empty());
        let (event, chain) = codex_args(&a(&["done"]));
        assert!(event.is_null() && chain.is_empty());
    }

    #[test]
    fn toml_edits_keep_the_rest_of_the_file() {
        let text = "# my settings\nmodel = \"x\"\nnotify = [\"a.exe\", \"turn-ended\"]\n\n[windows]\nsandbox = \"elevated\" # keep\n";
        let mut doc: DocumentMut = text.parse().unwrap();
        doc["notify"] = value(string_array(&["n.exe".to_string(), MARKER.to_string()]));
        let out = doc.to_string();
        assert!(out.contains("# my settings") && out.contains("sandbox = \"elevated\" # keep"));
        assert!(out.find("notify").unwrap() < out.find("[windows]").unwrap(), "a top-level key stays above the tables");
        doc.remove("notify");
        assert!(!doc.to_string().contains("notify"));
    }

    #[test]
    #[ignore = "reads the user's Codex configuration"]
    fn shows_the_codex_change_on_the_real_configuration() {
        let path = codex_config().expect("a user folder");
        let mut doc = read_codex_config(&path).expect("valid TOML");
        let before = codex_notify(&doc).unwrap_or_default();
        let mut command = vec!["night-island.exe".to_string(), MARKER.into(), "codex".into(), "done".into()];
        if !before.is_empty() {
            command.push(CHAIN.into());
            command.extend(before.clone());
        }
        doc["notify"] = value(string_array(&command));
        let after = doc.to_string();
        let old = std::fs::read_to_string(&path).unwrap();
        let changed: Vec<_> = after.lines().filter(|l| !old.lines().any(|o| o == *l)).collect();
        println!("notify before: {before:?}");
        println!("changed lines: {changed:?}");
        assert_eq!(changed.len(), 1);
        assert_eq!(after.lines().count(), old.lines().count());
    }

    #[test]
    fn states_round_trip() {
        for s in [State::Working, State::Waiting, State::Done] {
            assert_eq!(State::from_key(s.key()), Some(s));
        }
        assert_eq!(Agent::from_key("claude"), Some(Agent::Claude));
        for agent in Agent::ALL {
            assert_eq!(Agent::from_key(agent.key()), Some(agent));
        }
        assert_eq!(Agent::from_key("nope"), None);
    }
}
