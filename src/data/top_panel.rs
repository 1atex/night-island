use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TopPanelSettings {
    pub enabled: bool,
    pub application: bool,
    pub wifi: bool,
    pub bluetooth: bool,
    pub volume: bool,
    pub battery: bool,
    pub hide_fullscreen: bool,
    pub reserve_space: bool,
}

impl Default for TopPanelSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            application: true,
            wifi: true,
            bluetooth: true,
            volume: true,
            battery: true,
            hide_fullscreen: true,
            reserve_space: true,
        }
    }
}

impl TopPanelSettings {
    pub fn has_blocks(&self) -> bool {
        self.application || self.wifi || self.bluetooth || self.volume || self.battery
    }
}
