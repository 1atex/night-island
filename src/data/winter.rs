#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct WinterSettings {
    pub enabled: bool,
    pub dense: bool,
    pub economy: bool,
    pub windows: bool,
    pub accumulate: bool,
    pub interactive: bool,
    pub island_cap: bool,
    pub island_shake: bool,
    pub starter_cover: bool,
}

impl Default for WinterSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            dense: false,
            economy: false,
            windows: true,
            accumulate: true,
            interactive: true,
            island_cap: true,
            island_shake: true,
            starter_cover: true,
        }
    }
}
