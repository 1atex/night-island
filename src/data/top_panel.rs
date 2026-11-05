use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TopPanelSettings {
    pub enabled: bool,
    pub application: bool,
    pub auto_hide: bool,
    pub tray: bool,
    pub wifi: bool,
    pub bluetooth: bool,
    pub volume: bool,
    pub battery: bool,
    pub hide_fullscreen: bool,
    pub reserve_space: bool,
    pub clear_window_controls: bool,
    pub microphone: bool,
    pub keyboard: bool,
    pub workspaces: bool,
    pub animations: bool,
}

impl Default for TopPanelSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            application: true,
            auto_hide: true,
            tray: true,
            wifi: true,
            bluetooth: true,
            volume: true,
            battery: true,
            hide_fullscreen: true,
            reserve_space: true,
            clear_window_controls: true,
            microphone: false,
            keyboard: false,
            workspaces: false,
            animations: true,
        }
    }
}

impl TopPanelSettings {
    pub fn has_blocks(&self) -> bool {
        self.application
            || self.tray
            || self.wifi
            || self.bluetooth
            || self.volume
            || self.battery
            || self.microphone
            || self.keyboard
            || self.workspaces
    }
}
