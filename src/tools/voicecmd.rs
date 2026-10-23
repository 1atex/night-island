use serde_json::Value;

pub const MAX_WAKE_CHARS: usize = 24;
pub const MAX_WAKES: usize = 5;

pub fn wake_words(setting: &str) -> Vec<String> {
    setting
        .split(',')
        .map(|w| w.chars().filter(|c| !c.is_control()).collect::<String>().trim().to_string())
        .filter(|w| !w.is_empty() && w.chars().count() <= MAX_WAKE_CHARS)
        .take(MAX_WAKES)
        .collect()
}

pub fn clean_wake_setting(text: &str) -> Option<String> {
    let parts: Vec<&str> = text.split(',').map(str::trim).filter(|w| !w.is_empty()).collect();
    let ok = parts.len() <= MAX_WAKES
        && parts.iter().all(|w| w.chars().count() <= MAX_WAKE_CHARS && !w.chars().any(char::is_control));
    ok.then(|| parts.join(", "))
}

pub fn strip_wake(text: &str, wakes: &[String]) -> Option<String> {
    let text = text.trim_start();
    for wake in wakes {
        let mut rest = text.chars();
        let matched = wake.chars().all(|w| rest.next().is_some_and(|t| t.to_lowercase().eq(w.to_lowercase())));
        if !matched {
            continue;
        }
        let tail = rest.as_str();
        if tail.chars().next().is_some_and(char::is_alphanumeric) {
            continue;
        }
        let command = tail.trim_start_matches(|c: char| c.is_whitespace() || ",.:;!?-—–".contains(c));
        return Some(command.trim().to_string());
    }
    None
}

pub const SYSTEM_PROMPT: &str = "You turn a spoken command for a desktop assistant into ONE JSON object and reply with \
that object only, no other text. Pick exactly one of:\n\
{\"action\":\"media\",\"do\":\"play_pause\"|\"next\"|\"previous\"}\n\
{\"action\":\"control\",\"do\":\"mic\"|\"wifi\"|\"bluetooth\"|\"theme\"|\"lock\"}\n\
{\"action\":\"remind\",\"phrase\":\"<when and what, e.g. in 10 min call mom | 18:30 standup | tomorrow 9:00 report>\"}\n\
{\"action\":\"answer\",\"text\":\"<a short answer in the language of the command, at most two sentences>\"}\n\
{\"action\":\"unknown\"}\n\
A timer is a reminder: \"set a timer for 5 minutes\" is {\"action\":\"remind\",\"phrase\":\"in 5 min timer\"}. \
\"Mic\" toggles the microphone mute. Questions are answered with \"answer\". Anything else, \
and anything about deleting, shutting down, restarting or changing files, is {\"action\":\"unknown\"}.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Media {
    PlayPause,
    Next,
    Previous,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Toggle {
    Mic,
    WiFi,
    Bluetooth,
    Theme,
    Lock,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Media(Media),
    Control(Toggle),
    Remind(String),
    Answer(String),
    Unknown,
}

const MAX_PHRASE_CHARS: usize = 300;
const MAX_ANSWER_CHARS: usize = 300;

fn json_object(reply: &str) -> Option<&str> {
    let start = reply.find('{')?;
    let end = reply.rfind('}')?;
    (end > start).then(|| &reply[start..=end])
}

pub fn parse(reply: &str) -> Command {
    let Some(value) = json_object(reply).and_then(|j| serde_json::from_str::<Value>(j).ok()) else {
        return Command::Unknown;
    };
    let text = |key: &str, max: usize| -> Option<String> {
        let t: String = value[key].as_str()?.chars().filter(|c| *c == '\n' || !c.is_control()).collect();
        let t: String = t.trim().chars().take(max).collect();
        (!t.is_empty()).then_some(t)
    };
    let what = value["do"].as_str().unwrap_or("");
    match value["action"].as_str().unwrap_or("") {
        "media" => match what {
            "play_pause" => Command::Media(Media::PlayPause),
            "next" => Command::Media(Media::Next),
            "previous" => Command::Media(Media::Previous),
            _ => Command::Unknown,
        },
        "control" => match what {
            "mic" => Command::Control(Toggle::Mic),
            "wifi" => Command::Control(Toggle::WiFi),
            "bluetooth" => Command::Control(Toggle::Bluetooth),
            "theme" => Command::Control(Toggle::Theme),
            "lock" => Command::Control(Toggle::Lock),
            _ => Command::Unknown,
        },
        "remind" => text("phrase", MAX_PHRASE_CHARS).map_or(Command::Unknown, Command::Remind),
        "answer" => text("text", MAX_ANSWER_CHARS).map_or(Command::Unknown, Command::Answer),
        _ => Command::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wakes(s: &str) -> Vec<String> {
        wake_words(s)
    }

    #[test]
    fn the_call_word_is_found_in_any_case_and_dropped_with_its_punctuation() {
        let w = wakes("Island");
        assert_eq!(strip_wake("Island, set a timer for 5 minutes.", &w).as_deref(), Some("set a timer for 5 minutes."));
        assert_eq!(strip_wake("  island - next track", &w).as_deref(), Some("next track"));
        assert_eq!(strip_wake("ISLAND", &w).as_deref(), Some(""), "the word alone is a call without a command");
        assert_eq!(strip_wake("Islander, hi", &w), None, "only whole words");
        assert_eq!(strip_wake("on the island, ", &w), None, "the word must come first");
    }

    #[test]
    fn several_call_words_and_other_alphabets_work() {
        let w = wakes("Island, Айленд ,, ");
        assert_eq!(w, ["Island", "Айленд"]);
        assert_eq!(strip_wake("айленд, включи следующий трек", &w).as_deref(), Some("включи следующий трек"));
        assert_eq!(strip_wake("Айленды тут", &w), None);
        assert!(wakes("").is_empty() && strip_wake("Island hi", &[]).is_none());
    }

    #[test]
    fn the_setting_is_checked_before_it_is_saved() {
        assert_eq!(clean_wake_setting("  Island ,Айленд ,").as_deref(), Some("Island, Айленд"));
        assert_eq!(clean_wake_setting("").as_deref(), Some(""), "empty switches commands off");
        assert!(clean_wake_setting(&"x".repeat(MAX_WAKE_CHARS + 1)).is_none());
        assert!(clean_wake_setting("a,b,c,d,e,f").is_none());
        assert_eq!(wake_words(&"x".repeat(100)).len(), 0);
    }

    #[test]
    fn a_reply_becomes_one_of_the_known_commands() {
        assert_eq!(parse(r#"{"action":"media","do":"next"}"#), Command::Media(Media::Next));
        assert_eq!(parse("```json\n{\"action\":\"control\",\"do\":\"mic\"}\n```"), Command::Control(Toggle::Mic));
        assert_eq!(parse(r#"{"action":"remind","phrase":"in 5 min timer"}"#), Command::Remind("in 5 min timer".into()));
        assert_eq!(parse(r#"Sure! {"action":"answer","text":"Paris."}"#), Command::Answer("Paris.".into()));
    }

    #[test]
    fn nothing_dangerous_or_unknown_gets_through() {
        for reply in [
            r#"{"action":"control","do":"shutdown"}"#,
            r#"{"action":"control","do":"restart"}"#,
            r#"{"action":"shell","cmd":"del /s C:\\"}"#,
            r#"{"action":"media","do":"format"}"#,
            r#"{"action":"remind"}"#,
            r#"{"action":"remind","phrase":"   "}"#,
            "no json here",
            "{broken",
            "",
        ] {
            assert_eq!(parse(reply), Command::Unknown, "{reply}");
        }
    }

    #[test]
    fn long_answers_are_cut() {
        let reply = format!(r#"{{"action":"answer","text":"{}"}}"#, "я".repeat(1000));
        let Command::Answer(t) = parse(&reply) else { panic!("not an answer") };
        assert_eq!(t.chars().count(), MAX_ANSWER_CHARS);
    }
}
