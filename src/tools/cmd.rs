use std::path::PathBuf;

use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::{HSTRING, PCWSTR, w};

use crate::platform::apps;
use crate::platform::steam;
use crate::tools::calc;
use crate::view::Ctl;

pub const MAX_ITEMS: usize = 5;

pub enum Action {
    Launch { launch: String, path: PathBuf },
    Copy(String),
    System(Ctl),
    Web(String),
    Ask(String),
    OpenAiSettings,
}

pub struct Item {
    pub icon: Option<std::sync::Arc<apps::Icon>>,
    pub glyph: char,
    pub title: String,
    pub hint: String,
    pub action: Action,
}

const SYSTEM: [(&str, Ctl); 20] = [
    ("mic", Ctl::Mic),
    ("microphone", Ctl::Mic),
    ("микрофон", Ctl::Mic),
    ("wifi", Ctl::WiFi),
    ("wi-fi", Ctl::WiFi),
    ("bluetooth", Ctl::Bluetooth),
    ("theme", Ctl::Theme),
    ("dark", Ctl::Theme),
    ("тема", Ctl::Theme),
    ("lock", Ctl::Lock),
    ("заблокировать", Ctl::Lock),
    ("sleep", Ctl::Sleep),
    ("сон", Ctl::Sleep),
    ("restart", Ctl::Restart),
    ("reboot", Ctl::Restart),
    ("перезагрузка", Ctl::Restart),
    ("shutdown", Ctl::PowerOff),
    ("shut down", Ctl::PowerOff),
    ("power off", Ctl::PowerOff),
    ("выключить", Ctl::PowerOff),
];

pub const FILE_PREFIX: char = '/';

pub const AI_PREFIX: char = '?';

const ANSWER_LINE: usize = 70;

fn plain_item(glyph: char, title: &str, hint: &str, action: Action) -> Item {
    Item { icon: None, glyph, title: title.to_string(), hint: hint.to_string(), action }
}

fn ai_items(question: &str) -> Vec<Item> {
    use crate::services::ai;
    let provider = ai::active_provider();
    let q = question.trim();
    if !ai::has_key(provider) {
        return vec![plain_item(
            '\u{E99A}',
            "AI is not set up",
            "Open Settings, AI: add a key",
            Action::OpenAiSettings,
        )];
    }
    if q.is_empty() {
        return vec![plain_item(
            '\u{E99A}',
            "Type your question after the ?",
            provider.name(),
            Action::Copy(String::new()),
        )];
    }
    vec![plain_item(
        '\u{E99A}',
        &format!("Ask {}: {q}", provider.name()),
        "AI · Enter to send",
        Action::Ask(q.to_string()),
    )]
}

pub fn ai_thinking_item() -> Item {
    plain_item('\u{E99A}', "Thinking…", crate::services::ai::active_provider().name(), Action::Copy(String::new()))
}

pub fn ai_answer_items(answer: Result<String, String>) -> Vec<Item> {
    match answer {
        Err(why) => vec![plain_item('\u{E783}', &why, "AI", Action::Copy(String::new()))],
        Ok(text) => {
            let lines = wrap(&text, ANSWER_LINE);
            let shown = lines.len().min(MAX_ITEMS);
            let mut items: Vec<Item> = lines
                .iter()
                .take(shown)
                .enumerate()
                .map(|(i, line)| {
                    let hint = if i == 0 { "AI answer · Enter to copy" } else { "" };
                    plain_item(if i == 0 { '\u{E99A}' } else { ' ' }, line, hint, Action::Copy(text.clone()))
                })
                .collect();
            if lines.len() > shown
                && let Some(last) = items.last_mut()
            {
                last.title.push('…');
            }
            items
        }
    }
}

fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.lines() {
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            let mut word = word;
            while word.chars().count() > width {
                if !line.is_empty() {
                    lines.push(std::mem::take(&mut line));
                }
                let cut = word.char_indices().nth(width).map_or(word.len(), |(i, _)| i);
                lines.push(word[..cut].to_string());
                word = &word[cut..];
            }
            let extra = usize::from(!line.is_empty());
            if line.chars().count() + extra + word.chars().count() > width {
                lines.push(std::mem::take(&mut line));
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
        if !line.is_empty() {
            lines.push(line);
        }
    }
    lines
}

pub fn is_file_search(query: &str) -> bool {
    query.trim().strip_prefix(FILE_PREFIX).is_some_and(|rest| !rest.trim().is_empty())
}

pub fn start_file_search(own: windows::Win32::Foundation::HWND, query: &str) {
    if let Some(rest) = query.trim().strip_prefix(FILE_PREFIX) {
        crate::services::everything::start(own, rest.trim(), MAX_ITEMS as u32);
    }
}

fn files(own: windows::Win32::Foundation::HWND, query: &str) -> Vec<Item> {
    let notice = |glyph, title: &str, hint: &str, action| Item {
        icon: None,
        glyph,
        title: title.to_string(),
        hint: hint.to_string(),
        action,
    };
    let q = query.trim();
    if q.is_empty() {
        return vec![notice(
            '\u{E721}',
            "Type to search files with Everything",
            "Everything",
            Action::Copy(String::new()),
        )];
    }
    start_file_search(own, &format!("{FILE_PREFIX}{q}"));
    vec![notice('\u{E721}', "Searching…", "Everything", Action::Copy(String::new()))]
}

pub fn file_items(
    answer: Result<Vec<crate::services::everything::Hit>, crate::services::everything::Error>,
) -> Vec<Item> {
    use crate::services::everything::Error;
    let notice = |glyph, title: &str, hint: &str, action| Item {
        icon: None,
        glyph,
        title: title.to_string(),
        hint: hint.to_string(),
        action,
    };
    match answer {
        Err(Error::NotRunning) => vec![notice(
            '\u{E783}',
            "Everything is not running",
            "Start Everything to search files",
            Action::Web("https://www.voidtools.com".into()),
        )],
        Err(Error::Failed) => {
            vec![notice('\u{E783}', "Everything did not answer", "Everything", Action::Copy(String::new()))]
        }
        Ok(hits) if hits.is_empty() => {
            vec![notice('\u{E721}', "Nothing found", "Everything", Action::Copy(String::new()))]
        }
        Ok(hits) => hits
            .into_iter()
            .take(MAX_ITEMS)
            .map(|h| {
                let path = PathBuf::from(&h.path);
                let name = path.file_name().map_or_else(|| h.path.clone(), |n| n.to_string_lossy().into_owned());
                let parent = path.parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
                let hint = if parent.chars().count() > 38 {
                    format!("…{}", parent.chars().skip(parent.chars().count() - 37).collect::<String>())
                } else {
                    parent
                };
                Item {
                    icon: apps::icon_for_path(&h.path),
                    glyph: if h.folder { '\u{E838}' } else { '\u{E8A5}' },
                    title: name,
                    hint,
                    action: Action::Launch { launch: h.path.clone(), path },
                }
            })
            .collect(),
    }
}

pub fn search(own: windows::Win32::Foundation::HWND, query: &str) -> Vec<Item> {
    let q = query.trim();
    if q.is_empty() {
        return Vec::new();
    }
    if let Some(rest) = q.strip_prefix(FILE_PREFIX) {
        return files(own, rest);
    }
    if let Some(rest) = q.strip_prefix(AI_PREFIX) {
        return ai_items(rest);
    }
    let mut items = Vec::new();

    if let Some(text) = calc::evaluate(q) {
        items.push(Item {
            icon: None,
            glyph: '\u{E8EF}',
            title: format!("= {text}"),
            hint: "Calculator · Enter to copy".into(),
            action: Action::Copy(text),
        });
    } else if let Some(text) = calc::convert(q) {
        let result = text.rsplit(" = ").next().unwrap_or(&text).to_string();
        items.push(Item {
            icon: None,
            glyph: '\u{E8EF}',
            title: text,
            hint: "Converter · Enter to copy".into(),
            action: Action::Copy(result),
        });
    }

    let lower = q.to_lowercase();
    if lower.chars().count() >= 2 {
        let mut seen: Vec<Ctl> = Vec::new();
        for (word, ctl) in SYSTEM {
            if word.starts_with(&lower) && !seen.contains(&ctl) {
                seen.push(ctl);
                let title = crate::view::CTLS.iter().find(|c| c.0 == ctl).map_or("", |c| c.2);
                items.push(Item {
                    icon: None,
                    glyph: '\u{E713}',
                    title: title.to_string(),
                    hint: "System".into(),
                    action: Action::System(ctl),
                });
            }
        }
    }

    for app in apps::find(q, MAX_ITEMS) {
        items.push(Item {
            icon: apps::icon(app),
            glyph: '\u{E71D}',
            title: app.name.clone(),
            hint: "App".into(),
            action: Action::Launch { launch: app.launch.clone(), path: app.path.clone() },
        });
    }

    for game in steam::find(q, 3) {
        items.push(Item {
            icon: None,
            glyph: '\u{E7FC}',
            title: game.name.clone(),
            hint: "Steam".into(),
            action: Action::Launch { launch: steam::launch_url(game.id), path: PathBuf::new() },
        });
    }

    items.truncate(MAX_ITEMS - 1);
    items.push(Item {
        icon: None,
        glyph: '\u{E774}',
        title: format!("Search the web for “{q}”"),
        hint: "Web".into(),
        action: Action::Web(search_url(q)),
    });
    items
}

fn search_url(query: &str) -> String {
    let mut url = String::from("https://www.google.com/search?q=");
    for b in query.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => url.push(b as char),
            _ => url.push_str(&format!("%{b:02X}")),
        }
    }
    url
}

pub fn shell_open(target: &str, verb: Option<&str>) {
    let target = HSTRING::from(target);
    let verb = verb.map(HSTRING::from);
    unsafe {
        ShellExecuteW(
            None,
            verb.as_ref().map_or(w!("open"), |v| PCWSTR(v.as_ptr())),
            &target,
            None,
            None,
            SW_SHOWNORMAL,
        );
    }
}

pub fn reveal(path: &std::path::Path) {
    let arg = format!("/select,\"{}\"", path.display());
    let _ = std::process::Command::new("explorer.exe").raw_arg(arg).spawn();
}

use std::os::windows::process::CommandExt;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_is_percent_encoded() {
        assert_eq!(search_url("a b&c=d"), "https://www.google.com/search?q=a%20b%26c%3Dd");
        assert_eq!(search_url("привет"), "https://www.google.com/search?q=%D0%BF%D1%80%D0%B8%D0%B2%D0%B5%D1%82");
    }

    #[test]
    fn calculator_comes_first_and_web_last() {
        let items = search(Default::default(), "2+2");
        assert!(matches!(items.first().map(|i| &i.action), Some(Action::Copy(t)) if t == "4"));
        assert!(matches!(items.last().map(|i| &i.action), Some(Action::Web(_))));
    }

    #[test]
    fn answers_wrap_to_rows_and_keep_the_whole_text_for_copying() {
        let lines = wrap("one two three four five six", 12);
        assert_eq!(lines, ["one two", "three four", "five six"]);
        assert_eq!(wrap("abcdefghij", 4), ["abcd", "efgh", "ij"]);
        assert_eq!(wrap("a\n\nb", 10), ["a", "b"]);
        let items = ai_answer_items(Ok("word ".repeat(200)));
        assert_eq!(items.len(), MAX_ITEMS);
        assert!(items.last().unwrap().title.ends_with('…'));
        assert!(items.iter().all(|i| matches!(&i.action, Action::Copy(t) if t.len() > 500)));
        assert!(matches!(&ai_answer_items(Err("no".into()))[0].action, Action::Copy(t) if t.is_empty()));
    }

    #[test]
    fn a_question_mark_goes_to_the_ai_rows() {
        let items = search(Default::default(), "? what is rust");
        assert_eq!(items.len(), 1);
        assert!(matches!(items[0].action, Action::OpenAiSettings | Action::Ask(_)));
    }

    #[test]
    fn system_commands_match_by_prefix() {
        let items = search(Default::default(), "loc");
        assert!(items.iter().any(|i| matches!(i.action, Action::System(Ctl::Lock))));
        assert!(search(Default::default(), "").is_empty());
    }
}
