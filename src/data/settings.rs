use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::data::keys::{Combo, Hotkey};
use crate::ui::characters::Character;

const MAX_LYRICS_APPS: usize = 20;

const MAX_FILE_BYTES: u64 = 64 * 1024;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum Placement {
    #[default]
    Floating,
    Docked,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    Ru,
    En,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum Edge {
    #[default]
    Top,
    Bottom,
    Left,
    Right,
}

impl Edge {
    pub fn name(self) -> &'static str {
        match self {
            Edge::Top => "Top edge",
            Edge::Bottom => "Bottom edge",
            Edge::Left => "Left edge",
            Edge::Right => "Right edge",
        }
    }

    pub fn step(self, forward: bool) -> Self {
        const ALL: [Edge; 4] = [Edge::Top, Edge::Right, Edge::Bottom, Edge::Left];
        let i = ALL.iter().position(|e| *e == self).unwrap_or(0);
        ALL[if forward { (i + 1) % 4 } else { (i + 3) % 4 }]
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum RecordFormat {
    #[default]
    Mp4,
    Gif,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum Style {
    Flat,
    #[default]
    Liquid,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum BatteryStyle {
    Hidden,
    Icon,
    #[default]
    Percent,
}

impl BatteryStyle {
    pub fn name(self) -> &'static str {
        match self {
            BatteryStyle::Hidden => "Hidden",
            BatteryStyle::Icon => "Icon only",
            BatteryStyle::Percent => "With percent",
        }
    }

    pub fn step(self, forward: bool) -> Self {
        const ALL: [BatteryStyle; 3] = [BatteryStyle::Hidden, BatteryStyle::Icon, BatteryStyle::Percent];
        let i = ALL.iter().position(|v| *v == self).unwrap_or(0);
        ALL[if forward { (i + 1) % 3 } else { (i + 2) % 3 }]
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Settings {
    pub language: Language,
    pub weather: bool,
    pub weather_city: String,
    pub weather_lat: Option<f64>,
    pub weather_lon: Option<f64>,
    pub pill_opacity: f32,
    pub pill_blur: bool,
    pub agy_auto: bool,
    pub discord_overlay: bool,
    pub overlay_pos: Option<[i32; 2]>,
    pub panel_hue: f32,
    pub panel_sat: f32,
    pub panel_val: f32,
    pub accent_hue: f32,
    pub accent_sat: f32,
    pub accent_val: f32,
    pub placement: Placement,
    pub dock_edge: Edge,
    pub dock_offset: f32,
    pub float_pos: Option<[i32; 2]>,
    pub pill_weather: bool,
    pub pill_battery: BatteryStyle,
    pub music_eq: bool,
    pub music_title: bool,
    pub music_lyrics: bool,
    pub dev_tools: bool,
    pub shared_lyrics: bool,
    pub update_check: bool,
    pub update_beta: bool,
    pub ai: AiSettings,
    pub hidden_tabs: Vec<String>,
    pub notify: NotifySettings,
    pub spike_color: bool,
    pub record_format: RecordFormat,
    pub record_fps: u32,
    pub record_system_audio: bool,
    pub record_mic: bool,
    pub character: Character,
    pub edge_spikes: bool,
    pub style: Style,
    pub capture_hidden: bool,
    pub capture_self_hidden: bool,
    pub game_mode: bool,
    pub auto_hide: bool,
    pub acrylic: bool,
    pub scale_percent: u32,
    pub tint_open: f32,
    pub liquid_reach: f32,
    pub liquid_strength: f32,
    pub liquid_softness: f32,
    pub open_delay_ms: u32,
    pub close_delay_ms: u32,
    pub open_speed: f32,
    pub close_speed: f32,
    pub keys: BTreeMap<String, Combo>,
    pub hide_pill_secs: u32,
    pub lyrics_apps: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: Language::Ru,
            weather: true,
            weather_city: String::new(),
            weather_lat: None,
            weather_lon: None,
            pill_opacity: 1.0,
            pill_blur: false,
            agy_auto: false,
            discord_overlay: false,
            overlay_pos: None,
            panel_hue: 0.0,
            panel_sat: 0.0,
            panel_val: 0.0,
            accent_hue: 231.0,
            accent_sat: 0.35,
            accent_val: 1.0,
            placement: Placement::Floating,
            dock_edge: Edge::Top,
            dock_offset: 0.5,
            float_pos: None,
            pill_weather: true,
            pill_battery: BatteryStyle::Percent,
            music_eq: true,
            music_title: false,
            music_lyrics: false,
            dev_tools: false,
            shared_lyrics: false,
            update_check: true,
            update_beta: false,
            ai: AiSettings::default(),
            hidden_tabs: vec!["AGY".to_string()],
            notify: NotifySettings::default(),
            spike_color: true,
            record_format: RecordFormat::Mp4,
            record_fps: 30,
            record_system_audio: true,
            record_mic: false,
            character: Character::None,
            edge_spikes: true,
            style: Style::Liquid,
            capture_hidden: false,
            capture_self_hidden: true,
            game_mode: true,
            auto_hide: false,
            acrylic: true,
            scale_percent: 100,
            tint_open: 0.6,
            liquid_reach: 160.0,
            liquid_strength: 24.0,
            liquid_softness: 0.55,
            open_delay_ms: 220,
            close_delay_ms: 400,
            open_speed: 1.0,
            close_speed: 1.0,
            keys: BTreeMap::new(),
            hide_pill_secs: 7,
            lyrics_apps: Vec::new(),
        }
    }
}

fn clamp_f(v: f32, lo: f32, hi: f32, default: f32) -> f32 {
    if v.is_finite() { v.clamp(lo, hi) } else { default }
}

fn clean_app_name(name: &str) -> String {
    name.chars().filter(|c| !c.is_control()).take(40).collect::<String>().trim().to_uppercase()
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub enum AiProvider {
    #[default]
    Claude,
    OpenAi,
    Gemini,
}

impl AiProvider {
    pub fn id(self) -> &'static str {
        match self {
            AiProvider::Claude => "claude",
            AiProvider::OpenAi => "openai",
            AiProvider::Gemini => "gemini",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            AiProvider::Claude => "Claude",
            AiProvider::OpenAi => "OpenAI-compatible",
            AiProvider::Gemini => "Gemini",
        }
    }

    pub fn step(self, forward: bool) -> Self {
        const ALL: [AiProvider; 3] = [AiProvider::Claude, AiProvider::OpenAi, AiProvider::Gemini];
        let at = ALL.iter().position(|p| *p == self).unwrap_or(0);
        ALL[(at + if forward { 1 } else { ALL.len() - 1 }) % ALL.len()]
    }

    pub fn default_model(self) -> &'static str {
        match self {
            AiProvider::Claude => "claude-haiku-5-5",
            AiProvider::OpenAi => "gpt-4o-mini",
            AiProvider::Gemini => "gemini-2.0-flash",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(default)]
pub struct AiSettings {
    pub provider: AiProvider,
    pub claude_model: String,
    pub openai_model: String,
    pub openai_base: String,
    pub gemini_model: String,
}

impl Default for AiSettings {
    fn default() -> Self {
        Self {
            provider: AiProvider::Claude,
            claude_model: AiProvider::Claude.default_model().into(),
            openai_model: AiProvider::OpenAi.default_model().into(),
            openai_base: "https://api.openai.com/v1".into(),
            gemini_model: AiProvider::Gemini.default_model().into(),
        }
    }
}

impl AiSettings {
    pub fn model(&self, provider: AiProvider) -> &str {
        match provider {
            AiProvider::Claude => &self.claude_model,
            AiProvider::OpenAi => &self.openai_model,
            AiProvider::Gemini => &self.gemini_model,
        }
    }

    pub fn set_model(&mut self, provider: AiProvider, model: &str) {
        let model = model.trim().to_string();
        match provider {
            AiProvider::Claude => self.claude_model = model,
            AiProvider::OpenAi => self.openai_model = model,
            AiProvider::Gemini => self.gemini_model = model,
        }
    }

    pub fn sanitize(&mut self) {
        use crate::services::ai::{clean_base, valid_model};
        let d = AiSettings::default();
        for provider in [AiProvider::Claude, AiProvider::OpenAi, AiProvider::Gemini] {
            if !valid_model(self.model(provider), provider) {
                self.set_model(provider, d.model(provider));
            }
        }
        self.openai_base = clean_base(&self.openai_base).unwrap_or(d.openai_base);
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Notice {
    Mic,
    Download,
    Capture,
    Recording,
    Agy,
    Integrations,
    Error,
    Clipboard,
    LockKeys,
    Battery,
    Ai,
    Update,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(default)]
pub struct NotifySettings {
    pub mic: bool,
    pub downloads: bool,
    pub capture: bool,
    pub recording: bool,
    pub agy: bool,
    pub integrations: bool,
    pub errors: bool,
    pub clipboard: bool,
    pub clipboard_text: bool,
    pub lock_keys: bool,
    pub battery: bool,
    pub ai: bool,
    pub update: bool,
}

impl Default for NotifySettings {
    fn default() -> Self {
        Self {
            mic: true,
            downloads: true,
            capture: true,
            recording: true,
            agy: true,
            integrations: true,
            errors: true,
            clipboard: false,
            clipboard_text: false,
            lock_keys: false,
            battery: false,
            ai: true,
            update: true,
        }
    }
}

impl NotifySettings {
    pub fn on(&self, notice: Notice) -> bool {
        match notice {
            Notice::Mic => self.mic,
            Notice::Download => self.downloads,
            Notice::Capture => self.capture,
            Notice::Recording => self.recording,
            Notice::Agy => self.agy,
            Notice::Integrations => self.integrations,
            Notice::Error => self.errors,
            Notice::Clipboard => self.clipboard,
            Notice::LockKeys => self.lock_keys,
            Notice::Battery => self.battery,
            Notice::Ai => self.ai,
            Notice::Update => self.update,
        }
    }
}

pub const HIDEABLE_TABS: [&str; 10] =
    ["Clipboard", "Shelf", "Text", "Note", "Reminders", "Weather", "Record", "AGY", "Controls", "Theme"];

impl Settings {
    pub fn tab_hidden(&self, name: &str) -> bool {
        self.hidden_tabs.iter().any(|t| t == name)
    }

    pub fn set_tab_hidden(&mut self, name: &str, hidden: bool) {
        let Some(known) = HIDEABLE_TABS.iter().find(|t| **t == name) else { return };
        self.hidden_tabs.retain(|t| t != known);
        if hidden {
            self.hidden_tabs.push((*known).to_string());
        }
    }

    pub fn combo(&self, hotkey: Hotkey) -> Combo {
        self.keys.get(hotkey.key()).copied().unwrap_or_else(|| hotkey.default_combo())
    }

    pub fn set_combo(&mut self, hotkey: Hotkey, combo: Combo) {
        if combo == hotkey.default_combo() {
            self.keys.remove(hotkey.key());
        } else {
            self.keys.insert(hotkey.key().to_string(), combo);
        }
    }

    pub fn hotkey_using(&self, hotkey: Hotkey, combo: Combo) -> Option<Hotkey> {
        Hotkey::ALL.into_iter().find(|h| *h != hotkey && self.combo(*h) == combo)
    }

    pub fn lyrics_app_allowed(&self, source: &str) -> bool {
        self.lyrics_apps.is_empty() || self.lyrics_apps.iter().any(|a| a.eq_ignore_ascii_case(source))
    }

    pub fn add_lyrics_app(&mut self, source: &str) -> bool {
        let name = clean_app_name(source);
        if name.is_empty() || self.lyrics_apps.len() >= MAX_LYRICS_APPS || self.lyrics_apps.contains(&name) {
            return false;
        }
        self.lyrics_apps.push(name);
        true
    }

    fn sanitize_keys(&mut self) {
        let known: Vec<&str> = Hotkey::ALL.iter().map(|h| h.key()).collect();
        self.keys.retain(|name, combo| known.contains(&name.as_str()) && combo.valid());
        let mut seen: Vec<Combo> = Vec::new();
        for h in Hotkey::ALL {
            let combo = self.combo(h);
            if seen.contains(&combo) {
                self.keys.remove(h.key());
                if seen.contains(&self.combo(h)) {
                    self.keys.clear();
                    return;
                }
            }
            seen.push(self.combo(h));
        }
    }

    fn path() -> Option<PathBuf> {
        let dir = PathBuf::from(std::env::var_os("APPDATA")?).join("NightIsland");
        Some(dir.join("settings.json"))
    }

    pub fn load() -> Self {
        let file = Self::path().and_then(|p| fs::File::open(p).ok());
        let existed = file.is_some();
        let parsed = file.and_then(|f| {
            let mut buf = String::new();
            f.take(MAX_FILE_BYTES).read_to_string(&mut buf).ok()?;
            serde_json::from_str::<Settings>(&buf).ok()
        });
        if existed && parsed.is_none() {
            crate::log_warn!("settings.json is unreadable, using defaults");
        }
        let mut s = parsed.unwrap_or_default();
        s.sanitize();
        s
    }

    pub fn sanitize(&mut self) {
        let d = Settings::default();
        self.scale_percent = self.scale_percent.clamp(80, 200);
        let mut seen: Vec<String> = Vec::new();
        for t in &self.hidden_tabs {
            if HIDEABLE_TABS.contains(&t.as_str()) && !seen.contains(t) {
                seen.push(t.clone());
            }
        }
        self.hidden_tabs = seen;
        self.ai.sanitize();
        self.record_fps = self.record_fps.clamp(10, 60);
        self.pill_opacity = clamp_f(self.pill_opacity, 0.2, 1.0, 1.0);
        let valid = |v: Option<f64>, lim: f64| v.filter(|x| x.is_finite() && x.abs() <= lim);
        self.weather_lat = valid(self.weather_lat, 90.0);
        self.weather_lon = valid(self.weather_lon, 180.0);
        if self.weather_lat.is_none() || self.weather_lon.is_none() {
            self.weather_lat = None;
            self.weather_lon = None;
        }
        self.weather_city =
            self.weather_city.chars().filter(|c| !c.is_control()).take(60).collect::<String>().trim().to_string();
        self.dock_offset = clamp_f(self.dock_offset, 0.0, 1.0, 0.5);
        self.float_pos = self.float_pos.map(|[x, y]| [x.clamp(-20_000, 20_000), y.clamp(-20_000, 20_000)]);
        self.overlay_pos = self.overlay_pos.map(|[x, y]| [x.clamp(-20_000, 20_000), y.clamp(-20_000, 20_000)]);
        self.panel_hue = clamp_f(self.panel_hue, 0.0, 360.0, d.panel_hue);
        self.panel_sat = clamp_f(self.panel_sat, 0.0, 1.0, d.panel_sat);
        self.panel_val = clamp_f(self.panel_val, 0.0, 0.3, d.panel_val);
        self.accent_hue = clamp_f(self.accent_hue, 0.0, 360.0, d.accent_hue);
        self.accent_sat = clamp_f(self.accent_sat, 0.0, 1.0, d.accent_sat);
        self.accent_val = clamp_f(self.accent_val, 0.6, 1.0, d.accent_val);
        self.tint_open = clamp_f(self.tint_open, 0.2, 1.0, d.tint_open);
        self.liquid_reach = clamp_f(self.liquid_reach, 60.0, 300.0, d.liquid_reach);
        self.liquid_strength = clamp_f(self.liquid_strength, 6.0, 40.0, d.liquid_strength);
        self.liquid_softness = clamp_f(self.liquid_softness, 0.0, 1.0, d.liquid_softness);
        self.open_delay_ms = self.open_delay_ms.clamp(0, 1000);
        self.close_delay_ms = self.close_delay_ms.clamp(0, 3000);
        self.open_speed = clamp_f(self.open_speed, 0.4, 2.5, d.open_speed);
        self.close_speed = clamp_f(self.close_speed, 0.4, 2.5, d.close_speed);
        self.hide_pill_secs = self.hide_pill_secs.clamp(5, 10);
        self.sanitize_keys();
        let mut apps: Vec<String> = Vec::new();
        for a in &self.lyrics_apps {
            let name = clean_app_name(a);
            if !name.is_empty() && !apps.contains(&name) && apps.len() < MAX_LYRICS_APPS {
                apps.push(name);
            }
        }
        self.lyrics_apps = apps;
    }

    pub fn save(&self) {
        let Some(path) = Self::path() else { return };
        let Some(dir) = path.parent() else { return };
        if fs::create_dir_all(dir).is_err() {
            return;
        }
        let Ok(json) = serde_json::to_string_pretty(self) else { return };
        let tmp = path.with_extension("json.tmp");
        if fs::write(&tmp, json).is_err() || fs::rename(&tmp, &path).is_err() {
            crate::log_warn!("could not save settings.json");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notices_keep_their_old_defaults_and_read_partial_files() {
        let n = NotifySettings::default();
        assert!(n.on(Notice::Mic) && n.on(Notice::Download) && n.on(Notice::Error));
        assert!(!n.on(Notice::Clipboard) && !n.on(Notice::LockKeys) && !n.on(Notice::Battery) && !n.clipboard_text);
        let old: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(old.notify, n);
        let partial: Settings = serde_json::from_str(r#"{"notify":{"lock_keys":true,"mic":false}}"#).unwrap();
        assert!(partial.notify.lock_keys && !partial.notify.mic && partial.notify.downloads);
    }

    #[test]
    fn hostile_ai_settings_fall_back_to_defaults() {
        let json = r#"{"ai":{"provider":"gemini","gemini_model":"../x?y","claude_model":"claude-3","openai_base":"http://evil.example/v1"}}"#;
        let mut s: Settings = serde_json::from_str(json).unwrap();
        s.sanitize();
        assert_eq!(s.ai.provider, AiProvider::Gemini);
        assert_eq!(s.ai.gemini_model, AiProvider::Gemini.default_model());
        assert_eq!(s.ai.claude_model, "claude-3");
        assert_eq!(s.ai.openai_base, "https://api.openai.com/v1");
        assert_eq!(AiProvider::Claude.step(true), AiProvider::OpenAi);
        assert_eq!(AiProvider::Claude.step(false), AiProvider::Gemini);
    }

    #[test]
    fn hidden_tabs_are_validated_and_toggle() {
        let mut s: Settings = serde_json::from_str(r#"{"hidden_tabs":["AGY","AGY","Home","evil","Note"]}"#).unwrap();
        s.sanitize();
        assert_eq!(s.hidden_tabs, ["AGY", "Note"]);
        assert!(Settings::default().tab_hidden("AGY"), "AGY starts hidden");
        s.set_tab_hidden("AGY", false);
        s.set_tab_hidden("Home", true);
        assert_eq!(s.hidden_tabs, ["Note"]);
    }

    #[test]
    fn hotkeys_default_change_and_survive_a_hostile_file() {
        let mut s = Settings::default();
        assert_eq!(s.combo(Hotkey::HidePill), Hotkey::HidePill.default_combo());
        let c = Combo::new(crate::data::keys::CTRL | crate::data::keys::SHIFT, 'H' as u32);
        s.set_combo(Hotkey::HidePill, c);
        assert_eq!(s.combo(Hotkey::HidePill), c);
        assert_eq!(s.hotkey_using(Hotkey::Mic, c), Some(Hotkey::HidePill));
        s.set_combo(Hotkey::HidePill, Hotkey::HidePill.default_combo());
        assert!(s.keys.is_empty());

        let json = r#"{"keys":{"toggle":{"mods":0,"vk":69},"bogus":{"mods":2,"vk":69},
            "mic":{"mods":2,"vk":69},"hide_pill":{"mods":2,"vk":70}},"hide_pill_secs":99}"#;
        let mut s: Settings = serde_json::from_str(json).unwrap();
        s.sanitize();
        assert_eq!(s.combo(Hotkey::Toggle), Hotkey::Toggle.default_combo());
        assert!(!s.keys.contains_key("bogus"));
        assert_eq!(s.combo(Hotkey::Mic), Combo::new(2, 69));
        assert_eq!(s.combo(Hotkey::HidePill), Combo::new(2, 70));
        assert_eq!(s.hide_pill_secs, 10);

        let json = r#"{"keys":{"mic":{"mods":2,"vk":69}}}"#;
        let mut s: Settings = serde_json::from_str(json).unwrap();
        s.sanitize();
        assert_eq!(s.combo(Hotkey::Mic), Hotkey::Mic.default_combo());
    }

    #[test]
    fn trusted_lyric_players() {
        let mut s = Settings::default();
        assert!(s.lyrics_app_allowed("SPOTIFY") && s.lyrics_app_allowed(""));
        assert!(s.add_lyrics_app(" spotify "));
        assert!(!s.add_lyrics_app("SPOTIFY") && !s.add_lyrics_app("  "));
        assert!(s.lyrics_app_allowed("Spotify") && !s.lyrics_app_allowed("CHROME") && !s.lyrics_app_allowed(""));
        let mut s: Settings = serde_json::from_str(r#"{"lyrics_apps":["a","A","","b\u0007"]}"#).unwrap();
        s.sanitize();
        assert_eq!(s.lyrics_apps, ["A", "B"]);
    }

    #[test]
    fn hostile_values_are_clamped() {
        let json = r#"{"scale_percent":99999,"tint_open":1e30,"liquid_reach":-5,"open_speed":0,"open_delay_ms":4000000,"placement":"docked"}"#;
        let mut s: Settings = serde_json::from_str(json).unwrap();
        s.sanitize();
        assert_eq!(s.scale_percent, 200);
        assert_eq!(s.tint_open, 1.0);
        assert_eq!(s.liquid_reach, 60.0);
        assert_eq!(s.open_speed, 0.4);
        assert_eq!(s.open_delay_ms, 1000);
        assert_eq!(s.placement, Placement::Docked);
    }

    #[test]
    fn old_files_and_garbage_still_load() {
        let s: Settings = serde_json::from_str(r#"{"liquid":true,"auto_hide":true}"#).unwrap();
        assert!(s.auto_hide);
        assert_eq!(s.style, Style::Liquid);
        assert!(serde_json::from_str::<Settings>("not json").is_err());
    }
}

#[cfg(test)]
mod real_file {
    #[test]
    #[ignore]
    fn load_real() {
        println!("{:?}", super::Settings::load());
    }
}
