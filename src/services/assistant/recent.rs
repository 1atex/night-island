use std::path::PathBuf;

use super::memory;

pub const MAX_ENTRIES: usize = 40;
const FIELD_CHARS: usize = 90;

pub fn path() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("APPDATA")?).join("NightIsland").join("assistant").join("recent.md"))
}

fn field(text: &str) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= FIELD_CHARS {
        flat
    } else {
        format!("{}…", flat.chars().take(FIELD_CHARS).collect::<String>())
    }
}

pub fn entry(when: &str, request: &str, did: &[String], said: &str) -> String {
    let mut line = format!("{when} asked: {}", field(request));
    if !did.is_empty() {
        let steps: Vec<String> = did.iter().take(4).map(|d| field(d)).collect();
        line.push_str(&format!(" | did: {}", steps.join("; ")));
    }
    if !said.trim().is_empty() {
        line.push_str(&format!(" | said: {}", field(said)));
    }
    line
}

pub fn push(all: &mut Vec<String>, line: &str) {
    if let Some(clean) = memory::clean(line) {
        all.push(clean);
    }
    let extra = all.len().saturating_sub(MAX_ENTRIES);
    all.drain(..extra);
}

pub fn load() -> Vec<String> {
    path().map(|p| memory::load_from(&p)).unwrap_or_default()
}

pub fn record(line: &str) {
    let Some(file) = path() else { return };
    let mut all = memory::load_from(&file);
    push(&mut all, line);
    let _ = memory::save_to(&file, &all);
}

pub fn prompt_block(all: &[String], budget_chars: usize) -> String {
    memory::prompt_block(all, budget_chars)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_entry_says_what_was_asked_done_and_said() {
        let line = entry("14:03", "turn the music on", &["open Spotify".into()], "Playing");
        assert_eq!(line, "14:03 asked: turn the music on | did: open Spotify | said: Playing");
        assert_eq!(entry("09:00", "hi", &[], ""), "09:00 asked: hi");
        let long = entry("09:00", &"w ".repeat(200), &[], "");
        assert!(long.chars().count() < 120 && long.ends_with('…'));
    }

    #[test]
    fn only_the_newest_entries_are_kept_and_the_prompt_takes_the_latest_that_fit() {
        let mut all = Vec::new();
        for i in 0..(MAX_ENTRIES + 5) {
            push(&mut all, &format!("10:{i:02} asked: request {i}"));
        }
        assert_eq!(all.len(), MAX_ENTRIES);
        assert!(all[0].contains("request 5"));
        let block = prompt_block(&all, 100);
        assert!(block.lines().count() >= 2 && block.trim_end().ends_with(&format!("request {}", MAX_ENTRIES + 4)));
        assert!(block.chars().count() <= 100);
        push(&mut all, "   ");
        assert_eq!(all.len(), MAX_ENTRIES);
    }
}
