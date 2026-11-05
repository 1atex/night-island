use std::path::PathBuf;

use super::perms::Cap;
use super::{shell, web};

pub const MAX_FILE_CHARS: usize = 20_000;
const MAX_LISTED: usize = 120;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Tool {
    Cmd(String),
    PowerShell(String),
    Fetch(String),
    Search(String),
    ReadFile(String),
    WriteFile { path: String, text: String, append: bool },
}

impl Tool {
    pub fn cap(&self) -> Cap {
        match self {
            Tool::Cmd(_) | Tool::PowerShell(_) => Cap::Shell,
            Tool::Fetch(_) => Cap::Browser,
            Tool::Search(_) => Cap::WebSearch,
            Tool::ReadFile(_) => Cap::FilesRead,
            Tool::WriteFile { .. } => Cap::FilesWrite,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Tool::Cmd(_) => "shell",
            Tool::PowerShell(_) => "powershell",
            Tool::Fetch(_) => "fetch",
            Tool::Search(_) => "search",
            Tool::ReadFile(_) => "read_file",
            Tool::WriteFile { .. } => "write_file",
        }
    }

    pub fn detail(&self) -> &str {
        match self {
            Tool::Cmd(t) | Tool::PowerShell(t) | Tool::Fetch(t) | Tool::Search(t) | Tool::ReadFile(t) => t,
            Tool::WriteFile { path, .. } => path,
        }
    }

    pub fn script(&self) -> bool {
        matches!(self, Tool::Cmd(_) | Tool::PowerShell(_))
    }

    pub fn looks_only(&self) -> bool {
        matches!(self, Tool::Cmd(c) if shell::classify(c) == shell::Class::Look)
    }

    pub fn external(&self) -> bool {
        match self {
            Tool::Fetch(_) | Tool::Search(_) | Tool::ReadFile(_) => true,
            Tool::Cmd(t) | Tool::PowerShell(t) => t.to_ascii_lowercase().contains("http"),
            Tool::WriteFile { .. } => false,
        }
    }
}

fn resolve(path: &str) -> Result<PathBuf, String> {
    let path = path.trim().trim_matches('"');
    if path.is_empty() || path.contains('\0') {
        return Err("The path is empty".into());
    }
    let home = PathBuf::from(std::env::var_os("USERPROFILE").unwrap_or_else(|| ".".into()));
    let expanded = match path.strip_prefix('~') {
        Some(rest) => home.join(rest.trim_start_matches(['\\', '/'])),
        None => PathBuf::from(path),
    };
    Ok(if expanded.is_absolute() { expanded } else { home.join(expanded) })
}

fn read_file(path: &str) -> Result<String, String> {
    let path = resolve(path)?;
    let meta = std::fs::metadata(&path).map_err(|_| "That path does not exist or cannot be read".to_string())?;
    if meta.is_dir() {
        let mut names: Vec<String> = std::fs::read_dir(&path)
            .map_err(|_| "That folder cannot be read".to_string())?
            .flatten()
            .map(|e| {
                let dir = e.file_type().is_ok_and(|t| t.is_dir());
                format!("{}{}", e.file_name().to_string_lossy(), if dir { "\\" } else { "" })
            })
            .collect();
        names.sort_unstable_by_key(|n| n.to_lowercase());
        let total = names.len();
        names.truncate(MAX_LISTED);
        let mut out = names.join("\n");
        if total > MAX_LISTED {
            out.push_str(&format!("\n[{} more]", total - MAX_LISTED));
        }
        return Ok(if out.is_empty() { "The folder is empty".into() } else { out });
    }
    let mut buf = Vec::new();
    std::io::Read::read_to_end(
        &mut std::io::Read::take(std::fs::File::open(&path).map_err(|_| "That file cannot be opened")?, 200_000),
        &mut buf,
    )
    .map_err(|_| "That file cannot be read")?;
    if buf.contains(&0) {
        return Err("That file is not text".into());
    }
    Ok(shell::clip_output(&String::from_utf8_lossy(&buf)))
}

fn write_file(path: &str, text: &str, append: bool) -> Result<String, String> {
    let path = resolve(path)?;
    if text.chars().count() > MAX_FILE_CHARS {
        return Err("The text is too long for one file write".into());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|_| "The folder could not be created".to_string())?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(append)
        .truncate(!append)
        .open(&path)
        .map_err(|_| "The file could not be written".to_string())?;
    std::io::Write::write_all(&mut file, text.as_bytes()).map_err(|_| "The file could not be written".to_string())?;
    Ok(format!("Saved {}", path.display()))
}

pub fn run(tool: &Tool) -> Result<String, String> {
    match tool {
        Tool::Cmd(command) => shell::run(command),
        Tool::PowerShell(script) => shell::run_powershell(script),
        Tool::Fetch(url) => web::fetch(url),
        Tool::Search(query) => web::search(query),
        Tool::ReadFile(path) => read_file(path),
        Tool::WriteFile { path, text, append } => write_file(path, text, *append),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ni-tools-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn every_tool_maps_to_a_capability_and_a_name() {
        let all = [
            (Tool::Cmd("dir".into()), Cap::Shell, "shell"),
            (Tool::PowerShell("ls".into()), Cap::Shell, "powershell"),
            (Tool::Fetch("https://a.b".into()), Cap::Browser, "fetch"),
            (Tool::Search("x".into()), Cap::WebSearch, "search"),
            (Tool::ReadFile("a".into()), Cap::FilesRead, "read_file"),
            (Tool::WriteFile { path: "a".into(), text: String::new(), append: false }, Cap::FilesWrite, "write_file"),
        ];
        for (tool, cap, name) in all {
            assert_eq!((tool.cap(), tool.name()), (cap, name));
        }
        assert!(Tool::Cmd("dir".into()).looks_only() && !Tool::Cmd("mkdir x".into()).looks_only());
        assert!(!Tool::PowerShell("ls".into()).looks_only());
    }

    #[test]
    fn what_may_carry_outside_text_is_marked() {
        assert!(Tool::Fetch("https://a.b".into()).external() && Tool::Search("x".into()).external());
        assert!(Tool::PowerShell("iwr https://x.y | select Content".into()).external());
        assert!(!Tool::PowerShell("Get-ChildItem".into()).external());
        assert!(!Tool::WriteFile { path: "a".into(), text: "http".into(), append: false }.external());
    }

    #[test]
    fn files_are_written_appended_and_read_back() {
        let dir = scratch("rw");
        let file = dir.join("deep").join("notes.txt");
        let path = file.to_string_lossy().into_owned();
        assert!(write_file(&path, "one\n", false).unwrap().starts_with("Saved"));
        write_file(&path, "two\n", true).unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "one\ntwo\n");
        write_file(&path, "fresh", false).unwrap();
        assert_eq!(read_file(&path).unwrap(), "fresh");
        let listing = read_file(&dir.to_string_lossy()).unwrap();
        assert_eq!(listing, "deep\\");
        assert!(write_file(&path, &"x".repeat(MAX_FILE_CHARS + 1), false).is_err());
        assert!(read_file(&dir.join("missing").to_string_lossy()).is_err());
        assert!(resolve("   ").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn binary_files_are_refused() {
        let dir = scratch("bin");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("a.bin");
        std::fs::write(&file, [1u8, 0, 2]).unwrap();
        assert!(read_file(&file.to_string_lossy()).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn relative_paths_and_tilde_start_in_the_user_folder() {
        let home = PathBuf::from(std::env::var_os("USERPROFILE").unwrap_or_else(|| ".".into()));
        assert_eq!(resolve("~/Docs/a.txt").unwrap(), home.join("Docs/a.txt"));
        assert_eq!(resolve("Docs\\a.txt").unwrap(), home.join("Docs\\a.txt"));
    }
}
