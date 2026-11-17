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
    pub physics: bool,
    pub plow: bool,
    pub island_plow: bool,
    pub plow_interval_secs: u32,
    pub plow_size: u32,
    pub plow_ground: bool,
    pub plow_snowballs: bool,
    pub window_shake: bool,
    pub inside_windows: bool,
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
            physics: true,
            plow: true,
            island_plow: true,
            plow_interval_secs: 90,
            plow_size: 100,
            plow_ground: true,
            plow_snowballs: true,
            window_shake: true,
            inside_windows: false,
        }
    }
}
