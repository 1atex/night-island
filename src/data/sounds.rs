use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SoundPreset {
    #[default]
    Classic,
    Glass,
}

impl SoundPreset {
    pub const ALL: [Self; 2] = [Self::Classic, Self::Glass];
    pub fn name(self) -> &'static str {
        match self {
            Self::Classic => "Original sounds",
            Self::Glass => "Glass · iPhone style",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SoundSettings {
    pub enabled: bool,
    pub preset: SoundPreset,
    pub volume: u8,
    pub custom: BTreeMap<String, String>,
}

impl Default for SoundSettings {
    fn default() -> Self {
        Self { enabled: true, preset: SoundPreset::Classic, volume: 100, custom: BTreeMap::new() }
    }
}

impl SoundSettings {
    pub fn sanitize(&mut self) {
        self.volume = self.volume.min(100);
        self.custom.retain(|key, path| {
            matches!(key.as_str(), "heard" | "done" | "error" | "reminder")
                && !path.is_empty()
                && path.len() <= 4096
                && !path.chars().any(char::is_control)
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn old_profiles_keep_their_assistant_and_original_sounds() {
        let settings: crate::data::settings::Settings =
            serde_json::from_str(r#"{"assistant":{"volume":37,"sounds":false},"music_eq":false}"#).unwrap();
        assert_eq!(settings.sounds.preset, SoundPreset::Classic);
        assert_eq!(settings.sounds.volume, 100);
        assert_eq!(settings.assistant.volume, 37);
        assert!(!settings.assistant.sounds && !settings.music_eq);
        assert!(settings.live_activities);
        let mut sounds = SoundSettings { volume: 255, ..SoundSettings::default() };
        sounds.custom.insert("heard".into(), "C:/sound.wav".into());
        sounds.custom.insert("unknown".into(), "C:/unknown.wav".into());
        sounds.sanitize();
        assert_eq!(sounds.volume, 100);
        assert_eq!(sounds.custom.len(), 1);
        assert_eq!(serde_json::from_str::<SoundSettings>(&serde_json::to_string(&sounds).unwrap()).unwrap(), sounds);
    }
}
