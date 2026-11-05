pub mod agent;
pub mod env;
pub mod memory;
pub mod perms;
pub mod recent;
pub mod shell;
pub mod tools;
pub mod web;

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
    pub review: bool,
    pub ask_after_web: bool,
    pub follow_up: bool,
    pub natural_voice: bool,
    pub local_voice: bool,
    pub tts_eleven: bool,
    pub tts_base: String,
    pub tts_model: String,
    pub tts_voice: String,
    pub max_steps: u8,
}

pub const STEPS_RANGE: (u8, u8) = (3, 20);
pub const OPENAI_TTS: (&str, &str, &str) = ("https://api.openai.com/v1", "gpt-4o-mini-tts", "nova");
pub const ELEVEN_TTS: (&str, &str, &str) =
    ("https://api.elevenlabs.io", "eleven_multilingual_v2", "21m00Tcm4TlvDq8ikWAM");

impl Default for Assistant {
    fn default() -> Self {
        Self {
            perms: Permissions::default(),
            wake: false,
            sounds: true,
            speak: true,
            volume: 70,
            review: true,
            ask_after_web: false,
            follow_up: true,
            natural_voice: false,
            local_voice: false,
            tts_eleven: false,
            tts_base: OPENAI_TTS.0.into(),
            tts_model: OPENAI_TTS.1.into(),
            tts_voice: OPENAI_TTS.2.into(),
            max_steps: 10,
        }
    }
}

impl Assistant {
    pub fn set_eleven(&mut self, eleven: bool) {
        if self.tts_eleven == eleven {
            return;
        }
        let (base, model, voice) = if eleven { ELEVEN_TTS } else { OPENAI_TTS };
        self.tts_eleven = eleven;
        self.tts_base = base.into();
        self.tts_model = model.into();
        self.tts_voice = voice.into();
    }

    pub fn sanitize(&mut self) {
        self.perms.sanitize();
        self.volume = self.volume.min(100);
        self.max_steps = self.max_steps.clamp(STEPS_RANGE.0, STEPS_RANGE.1);
        let (base, model, voice) = if self.tts_eleven { ELEVEN_TTS } else { OPENAI_TTS };
        self.tts_base = crate::services::ai::clean_base(&self.tts_base).unwrap_or_else(|| base.to_string());
        if !crate::services::ai::valid_model(&self.tts_model, crate::data::settings::AiProvider::OpenAi) {
            self.tts_model = model.to_string();
        }
        let voice_ok = !self.tts_voice.is_empty()
            && self.tts_voice.len() <= 40
            && self.tts_voice.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
        if !voice_ok {
            self.tts_voice = voice.to_string();
        }
    }

    pub fn cue_volume(&self) -> u8 {
        if self.perms.enabled && self.sounds { self.volume } else { 0 }
    }

    pub fn voice_volume(&self) -> u8 {
        if self.perms.enabled && self.speak { self.volume } else { 0 }
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
            Cap::Controls => "Volume, mic, Wi-Fi, Bluetooth, theme, lock",
            Cap::Open => "Open programs and games",
            Cap::Reminders => "Reminders and timers",
            Cap::Memory => "Remember things about you",
            Cap::WebSearch => "Search the internet",
            Cap::FilesRead => "Read and search files",
            Cap::FilesWrite => "Create and change files",
            Cap::Shell => "Command line",
            Cap::Browser => "Browser (open sites, read pages)",
            Cap::Power => "Shut down, restart, sleep",
            Cap::Screen => "Look at the screen (preview, needs a model that sees images)",
            Cap::Island => "Island data: weather, music, Spotify, Discord, Steam, notes",
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
    fn the_natural_voice_settings_are_cleaned_and_the_follow_up_is_on_by_default() {
        let mut a = Assistant::default();
        assert!(a.follow_up && !a.natural_voice);
        a.tts_model = "bad model!".into();
        a.tts_voice = "../x".into();
        a.sanitize();
        let defaults = Assistant::default();
        assert_eq!((a.tts_model, a.tts_voice), (defaults.tts_model, defaults.tts_voice));
        let mut ok = Assistant { tts_model: "tts-1".into(), tts_voice: "alloy".into(), ..Assistant::default() };
        ok.sanitize();
        assert_eq!((ok.tts_model.as_str(), ok.tts_voice.as_str()), ("tts-1", "alloy"));
        let mut bad_base = Assistant { tts_base: "http://example.com/v1".into(), ..Assistant::default() };
        bad_base.sanitize();
        assert_eq!(bad_base.tts_base, "https://api.openai.com/v1", "plain http is only for this computer");
        let mut local = Assistant { tts_base: "http://localhost:8880/v1/".into(), ..Assistant::default() };
        local.sanitize();
        assert_eq!(local.tts_base, "http://localhost:8880/v1");
        let mut eleven = Assistant::default();
        eleven.set_eleven(true);
        assert_eq!(
            (eleven.tts_base.as_str(), eleven.tts_model.as_str(), eleven.tts_voice.as_str()),
            ("https://api.elevenlabs.io", "eleven_multilingual_v2", "21m00Tcm4TlvDq8ikWAM")
        );
        eleven.tts_voice = "bad voice!".into();
        eleven.sanitize();
        assert_eq!(eleven.tts_voice, "21m00Tcm4TlvDq8ikWAM", "a bad voice falls back to the provider's own");
        eleven.set_eleven(false);
        assert_eq!((eleven.tts_base.as_str(), eleven.tts_voice.as_str()), ("https://api.openai.com/v1", "nova"));
        let old: Assistant = serde_json::from_str(r#"{"wake":true}"#).unwrap();
        assert!(old.follow_up && old.tts_model == "gpt-4o-mini-tts" && old.max_steps == 10);
        for (given, kept) in [(0u8, 3u8), (2, 3), (7, 7), (200, 20)] {
            let mut a = Assistant { max_steps: given, ..Assistant::default() };
            a.sanitize();
            assert_eq!(a.max_steps, kept);
        }
    }

    #[test]
    fn muted_parts_have_zero_volume() {
        let mut a = Assistant::default();
        assert_eq!((a.cue_volume(), a.voice_volume()), (0, 0));
        a.perms.enabled = true;
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
