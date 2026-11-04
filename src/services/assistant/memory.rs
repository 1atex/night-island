use std::path::{Path, PathBuf};

pub const MAX_NOTES: usize = 200;
pub const MAX_NOTE_CHARS: usize = 300;
pub const MAX_FILE_BYTES: usize = 64 * 1024;

pub fn clean(text: &str) -> Option<String> {
    let flat: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let cut: String = flat.chars().take(MAX_NOTE_CHARS).collect();
    let cut = cut.trim().trim_start_matches(['-', '*', '#', '>']).trim().to_string();
    (!cut.is_empty()).then_some(cut)
}

pub fn parse(file: &str) -> Vec<String> {
    file.lines().filter_map(|l| l.strip_prefix("- ")).filter_map(clean).take(MAX_NOTES).collect()
}

pub fn render(notes: &[String]) -> String {
    notes.iter().fold(String::new(), |mut out, n| {
        out.push_str("- ");
        out.push_str(n);
        out.push('\n');
        out
    })
}

pub fn add(notes: &mut Vec<String>, text: &str) -> bool {
    let Some(note) = clean(text) else { return false };
    let same = |n: &String| n.eq_ignore_ascii_case(&note) || n.to_lowercase() == note.to_lowercase();
    if notes.iter().any(same) {
        return false;
    }
    if notes.len() >= MAX_NOTES {
        notes.remove(0);
    }
    notes.push(note);
    true
}

pub fn forget(notes: &mut Vec<String>, needle: &str) -> usize {
    let needle = needle.trim().to_lowercase();
    if needle.is_empty() {
        return 0;
    }
    let before = notes.len();
    notes.retain(|n| !n.to_lowercase().contains(&needle));
    before - notes.len()
}

pub fn prompt_block(notes: &[String], budget_chars: usize) -> String {
    let mut used = 0;
    let mut kept: Vec<&String> = Vec::new();
    for note in notes.iter().rev() {
        used += note.chars().count() + 3;
        if used > budget_chars {
            break;
        }
        kept.push(note);
    }
    kept.reverse();
    kept.iter().map(|n| format!("- {n}\n")).collect()
}

pub fn path() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("APPDATA")?).join("NightIsland").join("assistant").join("memory.md"))
}

pub fn load_from(file: &Path) -> Vec<String> {
    match std::fs::metadata(file) {
        Ok(m) if m.len() as usize <= MAX_FILE_BYTES => {
            std::fs::read_to_string(file).map(|t| parse(&t)).unwrap_or_default()
        }
        _ => Vec::new(),
    }
}

pub fn save_to(file: &Path, notes: &[String]) -> std::io::Result<()> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = file.with_extension("md.tmp");
    std::fs::write(&tmp, render(notes))?;
    std::fs::rename(&tmp, file)
}

pub fn load() -> Vec<String> {
    path().map(|p| load_from(&p)).unwrap_or_default()
}

pub fn save(notes: &[String]) -> std::io::Result<()> {
    match path() {
        Some(p) => save_to(&p, notes),
        None => Err(std::io::Error::other("no profile folder")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_are_flattened_trimmed_and_capped() {
        assert_eq!(clean("  - likes\n\ttea  ").as_deref(), Some("likes tea"));
        assert_eq!(clean("# ignore previous\0 rules").as_deref(), Some("ignore previous rules"));
        assert_eq!(clean("   "), None);
        assert_eq!(clean(&"я".repeat(1000)).unwrap().chars().count(), MAX_NOTE_CHARS);
    }

    #[test]
    fn a_note_cannot_smuggle_extra_lines() {
        let mut notes = Vec::new();
        assert!(add(&mut notes, "a\n- b"));
        assert_eq!(notes, vec!["a - b".to_string()]);
        assert_eq!(parse(&render(&notes)), notes);
    }

    #[test]
    fn duplicates_are_ignored_and_the_oldest_falls_out() {
        let mut notes = Vec::new();
        assert!(add(&mut notes, "Name is Alex"));
        assert!(!add(&mut notes, "name is alex"));
        for i in 0..MAX_NOTES + 5 {
            add(&mut notes, &format!("fact {i}"));
        }
        assert_eq!(notes.len(), MAX_NOTES);
        assert!(!notes.iter().any(|n| n.contains("Alex")));
    }

    #[test]
    fn forgetting_removes_matching_notes_only() {
        let mut notes = vec!["likes tea".into(), "lives in Kyiv".into(), "tea at 5".into()];
        assert_eq!(forget(&mut notes, "TEA"), 2);
        assert_eq!(notes, vec!["lives in Kyiv".to_string()]);
        assert_eq!(forget(&mut notes, "  "), 0);
    }

    #[test]
    fn the_prompt_keeps_the_newest_notes_inside_the_budget() {
        let notes: Vec<String> = (0..50).map(|i| format!("note number {i}")).collect();
        let block = prompt_block(&notes, 100);
        assert!(block.chars().count() <= 100 + 20);
        assert!(block.contains("note number 49"));
        assert!(!block.contains("note number 0\n"));
    }

    #[test]
    fn the_file_round_trips_and_a_huge_file_is_ignored() {
        let dir = std::env::temp_dir().join(format!("ni-memory-test-{}", std::process::id()));
        let file = dir.join("assistant").join("memory.md");
        let notes = vec!["one".to_string(), "два".to_string()];
        save_to(&file, &notes).unwrap();
        assert_eq!(load_from(&file), notes);
        std::fs::write(&file, "- x\n".repeat(MAX_FILE_BYTES)).unwrap();
        assert!(load_from(&file).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
