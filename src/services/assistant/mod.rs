pub mod memory;
pub mod perms;

use serde::{Deserialize, Serialize};

pub use perms::{Cap, Level, Permissions};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(default)]
pub struct Assistant {
    pub perms: Permissions,
    pub wake: bool,
    pub sounds: bool,
    pub speak: bool,
    pub volume: u8,
}

impl Default for Assistant {
    fn default() -> Self {
        Self { perms: Permissions::default(), wake: false, sounds: true, speak: true, volume: 70 }
    }
}

impl Assistant {
    pub fn sanitize(&mut self) {
        self.perms.sanitize();
        self.volume = self.volume.min(100);
    }

    pub fn cue_volume(&self) -> u8 {
        if self.sounds { self.volume } else { 0 }
    }

    pub fn voice_volume(&self) -> u8 {
        if self.speak { self.volume } else { 0 }
    }
}

impl Level {
    pub const ALL: [Level; 3] = [Level::Off, Level::Ask, Level::Auto];

    pub fn name(self) -> &'static str {
        match self {
            Level::Off => "Off",
            Level::Ask => "Ask first",
            Level::Auto => "Allowed",
        }
    }

    pub fn step(self, forward: bool) -> Level {
        let at = Level::ALL.iter().position(|l| *l == self).unwrap_or(0);
        let n = Level::ALL.len();
        Level::ALL[if forward { (at + 1) % n } else { (at + n - 1) % n }]
    }
}

impl Cap {
    pub fn title(self) -> &'static str {
        match self {
            Cap::Media => "Music and media keys",
            Cap::Controls => "Mic, Wi-Fi, Bluetooth, theme, lock",
            Cap::Open => "Open programs and games",
            Cap::Reminders => "Reminders and timers",
            Cap::Memory => "Remember things about you",
            Cap::WebSearch => "Search the internet",
            Cap::FilesRead => "Read and search files",
            Cap::FilesWrite => "Create and change files",
            Cap::Shell => "Command line",
            Cap::Browser => "Browser (open sites, read pages)",
            Cap::Power => "Shut down, restart, sleep",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_cycle_both_ways() {
        assert_eq!(Level::Off.step(true), Level::Ask);
        assert_eq!(Level::Auto.step(true), Level::Off);
        assert_eq!(Level::Off.step(false), Level::Auto);
    }

    #[test]
    fn muted_parts_have_zero_volume() {
        let mut a = Assistant::default();
        assert_eq!(a.cue_volume(), 70);
        a.sounds = false;
        a.speak = false;
        assert_eq!((a.cue_volume(), a.voice_volume()), (0, 0));
        a.volume = 250;
        a.sanitize();
        assert_eq!(a.volume, 100);
    }

    #[test]
    fn every_capability_has_a_title() {
        assert!(perms::ALL.iter().all(|c| !c.title().is_empty()));
    }
}
