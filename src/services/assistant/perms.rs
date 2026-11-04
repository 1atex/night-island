use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Cap {
    Media,
    Controls,
    Open,
    Reminders,
    Memory,
    WebSearch,
    FilesRead,
    FilesWrite,
    Shell,
    Browser,
    Power,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Off,
    Ask,
    Auto,
}

pub const ALL: [Cap; 11] = [
    Cap::Media,
    Cap::Controls,
    Cap::Open,
    Cap::Reminders,
    Cap::Memory,
    Cap::WebSearch,
    Cap::FilesRead,
    Cap::FilesWrite,
    Cap::Shell,
    Cap::Browser,
    Cap::Power,
];

impl Cap {
    pub fn key(self) -> &'static str {
        match self {
            Cap::Media => "media",
            Cap::Controls => "controls",
            Cap::Open => "open",
            Cap::Reminders => "reminders",
            Cap::Memory => "memory",
            Cap::WebSearch => "web_search",
            Cap::FilesRead => "files_read",
            Cap::FilesWrite => "files_write",
            Cap::Shell => "shell",
            Cap::Browser => "browser",
            Cap::Power => "power",
        }
    }

    pub fn default_level(self) -> Level {
        match self {
            Cap::Media | Cap::Controls | Cap::Open | Cap::Reminders | Cap::Memory => Level::Auto,
            Cap::WebSearch | Cap::Power => Level::Ask,
            Cap::FilesRead | Cap::FilesWrite | Cap::Shell | Cap::Browser => Level::Off,
        }
    }

    pub fn dangerous(self) -> bool {
        matches!(self, Cap::FilesWrite | Cap::Shell | Cap::Browser | Cap::Power)
    }

    pub fn exposes_outside_text(self) -> bool {
        matches!(self, Cap::WebSearch | Cap::FilesRead | Cap::Browser | Cap::Shell)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(default)]
pub struct Permissions {
    pub enabled: bool,
    pub levels: Vec<(Cap, Level)>,
}

impl Default for Permissions {
    fn default() -> Self {
        Self { enabled: false, levels: ALL.iter().map(|&c| (c, c.default_level())).collect() }
    }
}

impl Permissions {
    pub fn configured(&self, cap: Cap) -> Level {
        self.levels.iter().find(|(c, _)| *c == cap).map_or(cap.default_level(), |(_, l)| *l)
    }

    pub fn level(&self, cap: Cap) -> Level {
        if self.enabled { self.configured(cap) } else { Level::Off }
    }

    pub fn set(&mut self, cap: Cap, level: Level) {
        match self.levels.iter_mut().find(|(c, _)| *c == cap) {
            Some(slot) => slot.1 = level,
            None => self.levels.push((cap, level)),
        }
    }

    pub fn sanitize(&mut self) {
        let mut seen = Vec::new();
        self.levels.retain(|(c, _)| {
            let fresh = !seen.contains(c);
            seen.push(*c);
            fresh
        });
        for cap in ALL {
            if !self.levels.iter().any(|(c, _)| *c == cap) {
                self.levels.push((cap, cap.default_level()));
            }
        }
    }

    pub fn decide(&self, cap: Cap, tainted: bool) -> Level {
        let level = self.level(cap);
        if tainted && cap.dangerous() && level == Level::Auto { Level::Ask } else { level }
    }

    pub fn command_level(&self, cap: Cap, tainted: bool) -> Level {
        if self.enabled {
            return self.decide(cap, tainted);
        }
        match cap {
            Cap::Media | Cap::Controls | Cap::Open | Cap::Reminders => Level::Auto,
            Cap::Power => Level::Ask,
            _ => Level::Off,
        }
    }

    pub fn commands_offered(&self) -> Vec<Cap> {
        ALL.iter().copied().filter(|&c| self.command_level(c, false) != Level::Off).collect()
    }

    pub fn tools_offered(&self) -> Vec<Cap> {
        ALL.iter().copied().filter(|&c| self.level(c) != Level::Off).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on() -> Permissions {
        Permissions { enabled: true, ..Permissions::default() }
    }

    #[test]
    fn the_master_switch_turns_everything_off() {
        let p = Permissions::default();
        assert!(ALL.iter().all(|&c| p.level(c) == Level::Off));
        assert!(p.tools_offered().is_empty());
    }

    #[test]
    fn with_the_assistant_off_the_old_dictation_commands_still_work() {
        let p = Permissions::default();
        assert_eq!(p.command_level(Cap::Media, false), Level::Auto);
        assert_eq!(p.command_level(Cap::Power, false), Level::Ask);
        assert_eq!(p.command_level(Cap::Memory, false), Level::Off);
        assert_eq!(p.command_level(Cap::Shell, false), Level::Off);
        assert!(!p.commands_offered().contains(&Cap::Memory));
        assert!(on().commands_offered().contains(&Cap::Memory));
    }

    #[test]
    fn risky_things_are_off_by_default() {
        let p = on();
        assert_eq!(p.level(Cap::Shell), Level::Off);
        assert_eq!(p.level(Cap::FilesWrite), Level::Off);
        assert_eq!(p.level(Cap::Browser), Level::Off);
        assert_eq!(p.level(Cap::Power), Level::Ask);
        assert_eq!(p.level(Cap::Media), Level::Auto);
        assert!(!p.tools_offered().contains(&Cap::Shell));
    }

    #[test]
    fn text_from_outside_downgrades_dangerous_auto_to_ask() {
        let mut p = on();
        p.set(Cap::Shell, Level::Auto);
        p.set(Cap::Media, Level::Auto);
        assert_eq!(p.decide(Cap::Shell, false), Level::Auto);
        assert_eq!(p.decide(Cap::Shell, true), Level::Ask);
        assert_eq!(p.decide(Cap::Media, true), Level::Auto);
        p.set(Cap::Shell, Level::Off);
        assert_eq!(p.decide(Cap::Shell, true), Level::Off);
    }

    #[test]
    fn sanitize_drops_duplicates_and_fills_the_gaps() {
        let mut p = Permissions { enabled: true, levels: vec![(Cap::Shell, Level::Auto), (Cap::Shell, Level::Off)] };
        p.sanitize();
        assert_eq!(p.levels.len(), ALL.len());
        assert_eq!(p.level(Cap::Shell), Level::Auto);
    }

    #[test]
    fn keys_are_unique() {
        let mut keys: Vec<&str> = ALL.iter().map(|c| c.key()).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), ALL.len());
    }

    #[test]
    fn settings_survive_a_json_round_trip() {
        let mut p = on();
        p.set(Cap::Shell, Level::Ask);
        let back: Permissions = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        assert_eq!(back, p);
        let empty: Permissions = serde_json::from_str("{}").unwrap();
        assert!(!empty.enabled);
    }
}
