use std::ffi::c_void;
use std::io::Read;
use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use base64::Engine;
use serde_json::Value;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};

use super::tools::{self, Tool};

pub const MAX_COMMAND_CHARS: usize = 4000;
pub const MAX_OUTPUT_CHARS: usize = 3000;
const TIMEOUT: Duration = Duration::from_secs(60);
const PIPE_GRACE: Duration = Duration::from_millis(400);
const MAX_PIPE_BYTES: usize = 256 * 1024;
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Look,
    Change,
}

const LOOK: [&str; 24] = [
    "dir",
    "echo",
    "ver",
    "whoami",
    "hostname",
    "systeminfo",
    "tasklist",
    "ipconfig",
    "where",
    "type",
    "tree",
    "vol",
    "getmac",
    "netstat",
    "ping",
    "nslookup",
    "tracert",
    "driverquery",
    "findstr",
    "find",
    "assoc",
    "ftype",
    "date",
    "time",
];

const METACHARS: &[char] = &['&', '|', '>', '<', '^', '%', ';', '`', '(', ')', '$', '!', '\n', '\r', '\0'];

fn verbs(command: &str) -> Vec<String> {
    command
        .split(|c: char| c.is_whitespace() || "&|;()<>^\"".contains(c))
        .filter(|t| !t.is_empty())
        .map(|t| {
            if t.starts_with(['/', '-']) {
                return t.to_lowercase();
            }
            let base = t.rsplit(['\\', '/']).next().unwrap_or(t).to_lowercase();
            base.strip_suffix(".exe")
                .or(base.strip_suffix(".cmd"))
                .or(base.strip_suffix(".bat"))
                .unwrap_or(&base)
                .to_string()
        })
        .collect()
}

pub fn classify(command: &str) -> Class {
    let command = command.trim();
    if command.is_empty() || command.contains(METACHARS) || command.chars().any(char::is_control) {
        return Class::Change;
    }
    let tokens = verbs(command);
    let Some(first) = tokens.first() else { return Class::Change };
    let args = &tokens[1..];
    let read_only = LOOK.contains(&first.as_str())
        && match first.as_str() {
            "ipconfig" => {
                !args.iter().any(|a| matches!(a.as_str(), "/release" | "/renew" | "/flushdns" | "/registerdns"))
            }
            "assoc" | "ftype" => !command.contains('='),
            "date" | "time" => args.first().is_some_and(|a| a == "/t"),
            "ping" => !args.iter().any(|a| a == "-t"),
            _ => true,
        };
    if read_only { Class::Look } else { Class::Change }
}

pub fn clip_output(raw: &str) -> String {
    let text: String = raw.chars().filter(|c| *c == '\n' || *c == '\t' || !c.is_control()).collect();
    let text = text.trim();
    if text.chars().count() <= MAX_OUTPUT_CHARS {
        return text.to_string();
    }
    let cut: String = text.chars().take(MAX_OUTPUT_CHARS).collect();
    format!("{cut}\n[cut]")
}

struct Drain {
    buf: Arc<Mutex<Vec<u8>>>,
    done: Arc<AtomicBool>,
}

fn drain<R: Read + Send + 'static>(pipe: Option<R>) -> Drain {
    let buf = Arc::new(Mutex::new(Vec::new()));
    let done = Arc::new(AtomicBool::new(false));
    let (into, flag) = (Arc::clone(&buf), Arc::clone(&done));
    std::thread::spawn(move || {
        if let Some(mut pipe) = pipe {
            let mut chunk = [0u8; 4096];
            while let Ok(n) = pipe.read(&mut chunk) {
                if n == 0 {
                    break;
                }
                let Ok(mut all) = into.lock() else { break };
                if all.len() < MAX_PIPE_BYTES {
                    all.extend_from_slice(&chunk[..n]);
                }
            }
        }
        flag.store(true, Ordering::SeqCst);
    });
    Drain { buf, done }
}

pub fn decode(bytes: &[u8]) -> String {
    let high: Vec<u8> = bytes.iter().skip(1).step_by(2).take(32).copied().collect();
    let wide = bytes.starts_with(&[0xFF, 0xFE])
        || (bytes.len() >= 4 && high.iter().filter(|b| **b < 0x10).count() * 10 >= high.len() * 9);
    if wide {
        let units: Vec<u16> = bytes.chunks_exact(2).map(|p| u16::from_le_bytes([p[0], p[1]])).collect();
        return String::from_utf16_lossy(&units).trim_start_matches('\u{feff}').to_string();
    }
    String::from_utf8_lossy(bytes).replace('\0', "")
}

impl Drain {
    fn text(&self) -> String {
        self.buf.lock().map(|b| decode(&b)).unwrap_or_default()
    }
}

fn kill_tree(child: &mut std::process::Child) {
    let _ = Command::new("taskkill")
        .args(["/F", "/T", "/PID", &child.id().to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .status();
    let _ = child.kill();
    let _ = child.wait();
}

fn collect(mut command: Command) -> Result<String, String> {
    let home = std::env::var_os("USERPROFILE").unwrap_or_else(|| ".".into());
    let mut child = command
        .current_dir(home)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|_| "The command line could not be started".to_string())?;
    let out = drain(child.stdout.take());
    let err = drain(child.stderr.take());
    let deadline = Instant::now() + TIMEOUT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(40)),
            _ => {
                kill_tree(&mut child);
                return Err(format!("The command did not finish in {} seconds", TIMEOUT.as_secs()));
            }
        }
    };
    let grace = Instant::now() + PIPE_GRACE;
    while !(out.done.load(Ordering::SeqCst) && err.done.load(Ordering::SeqCst)) && Instant::now() < grace {
        std::thread::sleep(Duration::from_millis(15));
    }
    let mut text = out.text();
    let errors = err.text();
    if !errors.trim().is_empty() {
        text.push('\n');
        text.push_str(&errors);
    }
    let mut text = clip_output(&text);
    if let Some(code) = status.code().filter(|c| *c != 0) {
        text.push_str(&format!("\n[exit code {code}]"));
    }
    Ok(text)
}

pub fn run(command: &str) -> Result<String, String> {
    let command = command.trim();
    if command.is_empty() || command.chars().count() > MAX_COMMAND_CHARS {
        return Err("The command is empty or too long".into());
    }
    let mut cmd = Command::new("cmd.exe");
    cmd.raw_arg(format!("/d /s /c \"chcp 65001>nul & {command}\""));
    collect(cmd)
}

static SCRIPT_COUNTER: AtomicU32 = AtomicU32::new(0);

const PRELUDE: &str = "try{[Console]::OutputEncoding=[System.Text.UTF8Encoding]::new($false);\
$OutputEncoding=[Console]::OutputEncoding}catch{};$ProgressPreference='SilentlyContinue';\n";

fn script_file() -> std::path::PathBuf {
    let n = SCRIPT_COUNTER.fetch_add(1, Ordering::SeqCst);
    std::env::temp_dir().join(format!("night-island-{}-{n}.ps1", std::process::id()))
}

pub fn run_powershell(script: &str) -> Result<String, String> {
    let script = script.trim();
    if script.is_empty() || script.chars().count() > MAX_COMMAND_CHARS {
        return Err("The script is empty or too long".into());
    }
    let path = script_file();
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice(PRELUDE.as_bytes());
    bytes.extend_from_slice(script.as_bytes());
    std::fs::write(&path, bytes).map_err(|_| "The script could not be prepared".to_string())?;
    let mut cmd = Command::new("powershell.exe");
    cmd.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File"]).arg(&path);
    let result = collect(cmd);
    let _ = std::fs::remove_file(&path);
    result
}

pub fn run_wsl(script: &str) -> Result<String, String> {
    let script = script.trim();
    if script.is_empty() || script.chars().count() > MAX_COMMAND_CHARS {
        return Err("The script is empty or too long".into());
    }
    let packed = base64::engine::general_purpose::STANDARD.encode(script.as_bytes());
    let mut cmd = Command::new("wsl.exe");
    cmd.args(["-e", "bash", "-c"]).arg(format!("echo {packed} | base64 -d | bash"));
    collect(cmd)
        .map_err(|why| if why.contains("could not be started") { "WSL could not be started".into() } else { why })
}

pub const REVIEW_PROMPT: &str = "You are a strict security reviewer for one Windows command line or PowerShell script \
that a voice assistant wants to run on the user's own computer. You see only the command, nothing else. Reply with ONE \
JSON object and nothing else: {\"safe\":true|false,\"reason\":\"<at most 12 words>\"}. Say safe only if the command \
does what a normal person asking an assistant would expect and cannot destroy or leak data, change security settings, \
install or download programs, run hidden code, disable protection, or touch anything outside the user's own files. \
Anything unclear, obfuscated or broader than it needs to be is not safe.";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Verdict {
    pub safe: bool,
    pub reason: String,
}

pub fn parse_verdict(reply: &str) -> Verdict {
    let unsafe_verdict = |why: &str| Verdict { safe: false, reason: why.into() };
    let (Some(start), Some(end)) = (reply.find('{'), reply.rfind('}')) else {
        return unsafe_verdict("The review gave no clear answer");
    };
    let Ok(value) = serde_json::from_str::<Value>(&reply[start..=end.max(start)]) else {
        return unsafe_verdict("The review gave no clear answer");
    };
    let reason: String = value["reason"].as_str().unwrap_or("").chars().filter(|c| !c.is_control()).take(120).collect();
    Verdict { safe: value["safe"].as_bool() == Some(true), reason }
}

#[derive(Clone, Debug)]
pub enum Outcome {
    Reviewed { tool: Tool, verdict: Verdict },
    Ran { tool: Tool, result: Result<String, String> },
}

pub const WM_SHELL: u32 = WM_APP + 82;

fn notify(raw: isize) {
    unsafe {
        let _ = PostMessageW(Some(HWND(raw as *mut c_void)), WM_SHELL, WPARAM(0), LPARAM(0));
    }
}

pub fn review(hwnd: HWND, tool: Tool) {
    let raw = hwnd.0 as isize;
    std::thread::spawn(move || {
        let verdict = match crate::services::ai::complete(REVIEW_PROMPT, tool.detail(), 1000) {
            Ok(reply) => parse_verdict(&reply),
            Err(_) => Verdict { safe: false, reason: "The review could not be done".into() },
        };
        publish(Outcome::Reviewed { tool, verdict });
        notify(raw);
    });
}

pub fn execute(hwnd: HWND, tool: Tool) {
    let raw = hwnd.0 as isize;
    std::thread::spawn(move || {
        let result = tools::run(&tool).map(|text| if text.is_empty() { "(no output)".to_string() } else { text });
        publish(Outcome::Ran { tool, result });
        notify(raw);
    });
}

static OUTCOME: Mutex<Option<Outcome>> = Mutex::new(None);

pub fn take_outcome() -> Option<Outcome> {
    OUTCOME.lock().ok()?.take()
}

pub fn publish(outcome: Outcome) {
    if let Ok(mut slot) = OUTCOME.lock() {
        *slot = Some(outcome);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_looking_commands_are_read_only() {
        for c in
            ["dir", "dir C:\\Users", "whoami", "ipconfig /all", "ver", "tasklist", "hostname", "date /t", "echo hi"]
        {
            assert_eq!(classify(c), Class::Look, "{c}");
        }
        assert_eq!(classify("C:\\Windows\\System32\\whoami.exe"), Class::Look);
    }

    #[test]
    fn anything_that_changes_something_is_a_change() {
        for c in [
            "mkdir foo",
            "copy a b",
            "del a.txt",
            "ipconfig /release",
            "ipconfig /flushdns",
            "echo hi > out.txt",
            "dir & mkdir x",
            "dir | more",
            "date",
            "ping -t 8.8.8.8",
            "assoc .x=y",
            "start notepad",
            "type a.txt > b.txt",
            "format C:",
            "powershell -c iex",
            "rd /s /q C:\\",
            "",
            "   ",
            "dir\u{7}",
        ] {
            assert_eq!(classify(c), Class::Change, "{c}");
        }
    }

    #[test]
    fn nothing_is_refused_by_name() {
        assert!(run("").is_err() && run_powershell("  ").is_err());
        assert!(run(&"a".repeat(MAX_COMMAND_CHARS + 1)).is_err());
        assert!(run_powershell(&"a".repeat(MAX_COMMAND_CHARS + 1)).is_err());
    }

    #[test]
    fn output_is_cleaned_and_cut() {
        assert_eq!(clip_output("  a\u{1b}b\n"), "ab");
        let long = clip_output(&"x".repeat(MAX_OUTPUT_CHARS * 2));
        assert!(long.ends_with("[cut]") && long.chars().count() < MAX_OUTPUT_CHARS + 10);
    }

    #[test]
    fn a_review_that_is_unclear_blocks_the_command() {
        assert!(parse_verdict("{\"safe\":true,\"reason\":\"lists a folder\"}").safe);
        assert!(!parse_verdict("{\"safe\":false,\"reason\":\"deletes\"}").safe);
        for bad in ["", "ok", "{broken", "{\"safe\":\"true\"}", "{\"safe\":1}", "{\"reason\":\"x\"}"] {
            let v = parse_verdict(bad);
            assert!(!v.safe, "{bad}");
        }
        assert!(parse_verdict("```json\n{\"safe\":true,\"reason\":\"fine\"}\n```").safe);
    }

    #[test]
    fn scripts_are_written_to_distinct_files() {
        assert_ne!(script_file(), script_file());
    }

    #[test]
    fn output_in_either_encoding_is_read() {
        assert_eq!(decode("héllo".as_bytes()), "héllo");
        assert_eq!(decode(b"a\0b"), "ab");
        let wide: Vec<u8> = "Нет дистрибутивов".encode_utf16().flat_map(u16::to_le_bytes).collect();
        assert_eq!(decode(&wide), "Нет дистрибутивов");
        let mut bom = vec![0xFF, 0xFE];
        bom.extend("ok".encode_utf16().flat_map(u16::to_le_bytes));
        assert_eq!(decode(&bom), "ok");
        assert_eq!(decode(b""), "");
    }

    #[test]
    #[ignore = "needs WSL with a distribution"]
    fn a_real_linux_command_runs_through_wsl() {
        let out = run_wsl("echo 'привет'\nuname -s").expect("ran");
        assert!(out.contains("привет") && out.contains("Linux"), "{out}");
    }

    #[test]
    #[ignore = "starts cmd.exe, which the wine test run does not have"]
    fn a_real_command_runs_and_the_output_is_utf8() {
        assert_eq!(run("echo hello").expect("ran"), "hello");
        let both = run("dir C:\\definitely-not-here").expect("ran");
        assert!(!both.is_empty());
        assert!(run("exit 3").expect("ran").contains("[exit code 3]"));
    }

    #[test]
    #[ignore = "starts powershell.exe, which the wine test run does not have"]
    fn a_real_script_runs_with_unicode_and_several_lines() {
        let out = run_powershell("$a = 'привет'\nforeach ($i in 1..2) { \"$a $i\" }\nWrite-Error 'oops'").expect("ran");
        assert!(out.contains("привет 1") && out.contains("привет 2") && out.contains("oops"), "{out}");
        assert!(run_powershell("Get-ChildItem C:\\definitely-not-here").expect("ran").contains("definitely-not-here"));
    }

    #[test]
    #[ignore = "starts cmd.exe, which the wine test run does not have"]
    fn a_program_left_running_does_not_hold_the_answer_back() {
        let started = Instant::now();
        let out = run("start \"\" /b ping -n 9 127.0.0.1 & echo started").expect("ran");
        assert!(out.contains("started"));
        assert!(started.elapsed() < Duration::from_secs(4), "{:?}", started.elapsed());
    }

    #[test]
    #[ignore = "starts cmd.exe, which the wine test run does not have"]
    fn a_slow_command_is_stopped() {
        let started = Instant::now();
        let result = run("ping -n 100 127.0.0.1");
        assert!(result.is_err());
        assert!(started.elapsed() < TIMEOUT + Duration::from_secs(10));
    }
}
