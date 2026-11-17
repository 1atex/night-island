#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct WinterSettings {
    pub enabled: bool,
    pub dense: bool,
    pub economy: bool,
    pub windows: bool,
    pub accumulate: bool,
    pub interactive: bool,
}

impl Default for WinterSettings {
    fn default() -> Self {
        Self { enabled: false, dense: false, economy: false, windows: true, accumulate: true, interactive: true }
    }
}
