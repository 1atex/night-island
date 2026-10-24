use serde_json::Value;

pub const BUILTIN: [(&str, u8); 7] =
    [("Development", 0), ("Communication", 1), ("Browsing", 2), ("Games", 3), ("Media", 4), ("Work", 5), ("Other", 6)];

pub const OTHER_ID: i64 = 7;

const KNOWN: [(&str, &[&str]); 6] = [
    (
        "Development",
        &[
            "code",
            "code-insiders",
            "devenv",
            "rider",
            "idea",
            "pycharm",
            "clion",
            "webstorm",
            "goland",
            "rustrover",
            "datagrip",
            "phpstorm",
            "androidstudio",
            "studio",
            "eclipse",
            "sublime_text",
            "notepad++",
            "windowsterminal",
            "wt",
            "powershell",
            "pwsh",
            "cmd",
            "conhost",
            "bash",
            "wsl",
            "mintty",
            "gitkraken",
            "githubdesktop",
            "docker desktop",
            "postman",
            "insomnia",
            "dbeaver",
            "ssms",
            "cursor",
            "windsurf",
            "zed",
            "claude",
            "unityhub",
            "unity",
            "unrealeditor",
            "godot",
            "gitextensions",
            "fork",
            "tortoisegitproc",
            "winmergeu",
        ],
    ),
    (
        "Communication",
        &[
            "discord",
            "slack",
            "teams",
            "ms-teams",
            "telegram",
            "whatsapp",
            "zoom",
            "skype",
            "outlook",
            "thunderbird",
            "signal",
            "viber",
            "element",
            "mattermost",
            "webex",
            "nightgram",
            "olk",
        ],
    ),
    (
        "Browsing",
        &[
            "chrome",
            "firefox",
            "msedge",
            "brave",
            "opera",
            "opera_gx",
            "vivaldi",
            "chromium",
            "arc",
            "tor",
            "iexplore",
            "browser",
            "zen",
            "librewolf",
            "waterfox",
            "yandex",
        ],
    ),
    (
        "Games",
        &[
            "steam",
            "epicgameslauncher",
            "galaxyclient",
            "riotclientservices",
            "riotclientux",
            "valorant",
            "league of legends",
            "leagueclient",
            "minecraft",
            "minecraftlauncher",
            "cs2",
            "dota2",
            "cyberpunk2077",
            "fortniteclient-win64-shipping",
            "eldenring",
            "gta5",
            "robloxplayerbeta",
            "battle.net",
            "origin",
            "eadesktop",
            "upc",
            "genshinimpact",
            "wow",
            "overwatch",
            "apex",
            "playnite",
            "heroic",
            "lutris",
            "xboxapp",
        ],
    ),
    (
        "Media",
        &[
            "spotify",
            "vlc",
            "mpv",
            "potplayer",
            "potplayermini",
            "wmplayer",
            "itunes",
            "musicbee",
            "foobar2000",
            "netflix",
            "applemusic",
            "yandexmusic",
            "tidal",
            "obs",
            "streamlabs",
            "audacity",
            "winamp",
            "kodi",
        ],
    ),
    (
        "Work",
        &[
            "winword",
            "excel",
            "powerpnt",
            "onenote",
            "notion",
            "obsidian",
            "acrobat",
            "acrord32",
            "photoshop",
            "illustrator",
            "premiere",
            "afterfx",
            "figma",
            "blender",
            "resolve",
            "soffice",
            "wps",
            "sumatrapdf",
            "notepad",
            "explorer",
            "totalcmd",
            "7zfm",
            "anki",
            "xmind",
            "miro",
            "lightroom",
            "indesign",
        ],
    ),
];

fn normal(app: &str) -> String {
    let lower = app.trim().to_lowercase();
    let lower = lower.strip_suffix("_x64").unwrap_or(&lower);
    lower.strip_suffix("64").filter(|rest| !rest.is_empty()).unwrap_or(lower).to_string()
}

pub fn builtin_group(app: &str) -> Option<&'static str> {
    let name = normal(app);
    let raw = app.trim().to_lowercase();
    KNOWN.iter().find(|(_, names)| names.iter().any(|n| *n == name || *n == raw)).map(|(group, _)| *group)
}

pub fn clean_app(app: &str) -> Option<String> {
    let name: String = app.trim().chars().take(40).collect();
    let ok = !name.is_empty()
        && name.chars().all(|c| c.is_alphanumeric() || matches!(c, '.' | '_' | '-' | '+' | ' '))
        && !name.starts_with(' ');
    ok.then(|| name.to_lowercase())
}

pub fn clean_group(name: &str) -> Option<String> {
    let name: String = name.chars().filter(|c| !c.is_control()).collect();
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    let name: String = name.chars().take(24).collect();
    (!name.is_empty()).then_some(name)
}

pub fn system_prompt(groups: &[String]) -> String {
    format!(
        "Sort desktop programs into groups by their file names. Reply with ONE JSON object that maps each \
         program name you are given to exactly one of these groups: {}. Use \"Other\" when you are not sure. \
         No other text.",
        groups.iter().map(|g| format!("\"{g}\"")).collect::<Vec<_>>().join(", ")
    )
}

pub fn parse_assignments(reply: &str, asked: &[String], groups: &[String]) -> Vec<(String, String)> {
    let (Some(start), Some(end)) = (reply.find('{'), reply.rfind('}')) else { return Vec::new() };
    if end <= start {
        return Vec::new();
    }
    let Ok(Value::Object(map)) = serde_json::from_str::<Value>(&reply[start..=end]) else { return Vec::new() };
    let mut out = Vec::new();
    for (name, group) in &map {
        let Some(app) = clean_app(name).filter(|a| asked.contains(a)) else { continue };
        let Some(group) = group.as_str().and_then(|g| groups.iter().find(|x| x.eq_ignore_ascii_case(g.trim()))) else {
            continue;
        };
        if !out.iter().any(|(a, _): &(String, String)| *a == app) {
            out.push((app, group.clone()));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn well_known_programs_are_grouped_by_their_whole_name() {
        assert_eq!(builtin_group("Code"), Some("Development"));
        assert_eq!(builtin_group("chrome"), Some("Browsing"));
        assert_eq!(builtin_group("Discord"), Some("Communication"));
        assert_eq!(builtin_group("rider64"), Some("Development"));
        assert_eq!(builtin_group("steam"), Some("Games"));
        assert_eq!(builtin_group("Spotify"), Some("Media"));
        assert_eq!(builtin_group("WINWORD"), Some("Work"));
    }

    #[test]
    fn pieces_of_words_do_not_count() {
        for name in ["search", "digital", "provide", "reader", "gitless", "teamviewer", "ealing", "xcodex"] {
            assert_eq!(builtin_group(name), None, "{name}");
        }
        assert_eq!(builtin_group(""), None);
    }

    #[test]
    fn names_for_requests_are_checked() {
        assert_eq!(clean_app("  Blender "), Some("blender".into()));
        assert_eq!(clean_app("my game.v2"), Some("my game.v2".into()));
        assert_eq!(clean_app("a\"b"), None, "quotes could break the request");
        assert_eq!(clean_app("a\nb"), None);
        assert_eq!(clean_app(""), None);
        assert_eq!(clean_app(&"x".repeat(100)).map(|s| s.len()), Some(40));
        assert_eq!(clean_group("  My   games \n"), Some("My games".into()));
        assert_eq!(clean_group(" \t"), None);
        assert_eq!(clean_group(&"я".repeat(60)).map(|s| s.chars().count()), Some(24));
    }

    #[test]
    fn only_asked_programs_and_known_groups_are_taken_from_a_reply() {
        let groups: Vec<String> = BUILTIN.iter().map(|(n, _)| n.to_string()).collect();
        let asked = vec!["blender".to_string(), "hades".to_string(), "weird".to_string()];
        let reply =
            "```json\n{\"Blender\":\"work\",\"hades\":\"Games\",\"weird\":\"Hacking\",\"injected\":\"Work\"}\n```";
        let got = parse_assignments(reply, &asked, &groups);
        assert_eq!(got, [("blender".to_string(), "Work".to_string()), ("hades".to_string(), "Games".to_string())]);
        assert!(parse_assignments("no json", &asked, &groups).is_empty());
        assert!(parse_assignments("{broken", &asked, &groups).is_empty());
        assert!(parse_assignments("", &asked, &groups).is_empty());
    }

    #[test]
    fn the_prompt_lists_the_groups() {
        let p = system_prompt(&["Work".to_string(), "My games".to_string()]);
        assert!(p.contains("\"Work\", \"My games\""));
    }

    #[test]
    fn the_table_has_no_duplicates_between_groups() {
        let mut seen = std::collections::HashSet::new();
        for (_, names) in KNOWN {
            for n in names {
                assert!(seen.insert(*n), "{n} is listed twice");
            }
        }
    }
}
