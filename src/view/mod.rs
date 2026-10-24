use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FONT_WEIGHT_BOLD, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_TEXT_ALIGNMENT_CENTER,
    DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_TEXT_ALIGNMENT_TRAILING,
};
use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows::Win32::System::SystemInformation::GetLocalTime;
use windows::core::Result;

use crate::data::clips::{Clip, ShelfItem};
use crate::data::settings::{BatteryStyle, Language, Placement, RecordFormat, Settings, Style};
use crate::platform::media::Media;
use crate::platform::privacy::Privacy;
use crate::platform::system::Radios;
use crate::services::weather::Sky;
use crate::tools::cmd;
use crate::tools::edit::{self, TextEdit};
use crate::tools::texttools::ops;
use crate::ui::gfx::{Painter, WHITE, rgba};

mod activity;
mod agy;
mod chat;
mod clipboard;
mod command;
mod controls;
mod discord;
mod frame;
mod home;
mod integrations;
mod metrics;
mod note;
mod pill;
mod privacy;
mod record;
mod remind;
mod settings;
mod settings_window;
mod shelf;
mod steam;
mod text;
mod weather;
mod widgets;

use self::shelf::*;
pub use self::{
    activity::*, agy::*, chat::*, clipboard::*, command::*, controls::*, discord::*, frame::*, home::*,
    integrations::*, metrics::*, note::*, pill::*, privacy::*, record::*, remind::*, settings::*, settings_window::*,
    steam::*, text::*, weather::*, widgets::*,
};

pub type Rect = (f32, f32, f32, f32);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Home,
    Clipboard,
    Shelf,
    Text,
    Note,
    Remind,
    Weather,
    Record,
    Agy,
    Chat,
    Privacy,
    Activity,
    Steam,
    System,
    Controls,
    Theme,
    Settings,
}

pub const TABS: [(Tab, char, &str); 17] = [
    (Tab::Home, '\u{E80F}', "Home"),
    (Tab::Clipboard, '\u{E77F}', "Clipboard"),
    (Tab::Shelf, '\u{E838}', "Shelf"),
    (Tab::Text, '\u{E8E9}', "Text"),
    (Tab::Note, '\u{E70B}', "Note"),
    (Tab::Remind, '\u{E916}', "Reminders"),
    (Tab::Weather, '\u{F800}', "Weather"),
    (Tab::Record, '\u{E7C8}', "Record"),
    (Tab::Agy, '\u{E943}', "AGY"),
    (Tab::Chat, '\u{E8F2}', "Chat"),
    (Tab::Privacy, '\u{EA18}', "Privacy"),
    (Tab::Activity, '\u{E823}', "Activity"),
    (Tab::Steam, '\u{E7FC}', "Steam"),
    (Tab::System, '\u{E9D9}', "System"),
    (Tab::Controls, '\u{E9E9}', "Controls"),
    (Tab::Theme, '\u{E790}', "Theme"),
    (Tab::Settings, '\u{E713}', "Settings"),
];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ClipHit {
    Row(usize),
    Pin(usize),
    Delete(usize),
}

pub struct UiState {
    pub tab: Tab,
    pub hover_tab: Option<usize>,
    pub clip: ClipUi,
    pub shelf_scroll: usize,
    pub note: TextEdit,
    pub remind: RemindUi,
    pub cfg: Settings,
    pub text: TextUi,
    pub editing: bool,
    pub caret_on: bool,
    pub media: Media,
    pub lyric: LyricUi,
    pub update: crate::services::update::Status,
    pub notes: NotesUi,
    pub sw: SwUi,
    pub privacy: Privacy,
    pub mic_muted: Option<bool>,
    pub vpn: Option<bool>,
    pub vpn_busy: bool,
    pub notice: Option<(char, String)>,
    pub volume: Option<f32>,
    pub recording_secs: Option<u32>,
    pub downloading: Option<String>,
    pub char_inset: f32,
    pub cmd: CommandUi,
    pub radios: Radios,
    pub dark: bool,
    pub hover_ctl: Option<usize>,
    pub armed: Option<usize>,
    pub pinned: bool,
    pub home: HomeUi,
    pub hover_head: Option<HeadHit>,
    pub hand: bool,
    pub tip: Option<(String, (f32, f32))>,
    pub weather: Option<crate::services::weather::Weather>,
    pub spotify: crate::services::spotify::State,
    pub discord: crate::services::discord::State,
    pub int: IntegrationsUi,
    pub city: CityUi,
    pub agy: AgyUi,
    pub chat: ChatUi,
    pub privacy_log: PrivacyUi,
    pub activity: ActivityUi,
    pub steam: SteamUi,
    pub metrics: MetricsUi,
    pub agents: Vec<crate::services::agents::Session>,
}

impl UiState {
    pub fn new() -> Self {
        Self {
            tab: Tab::Home,
            agy: AgyUi::default(),
            chat: ChatUi::default(),
            privacy_log: PrivacyUi::default(),
            activity: ActivityUi::default(),
            steam: SteamUi::default(),
            metrics: MetricsUi::default(),
            agents: Vec::new(),
            cmd: CommandUi::default(),
            remind: RemindUi::default(),
            int: IntegrationsUi::default(),
            text: TextUi::default(),
            clip: ClipUi::default(),
            city: CityUi::default(),
            home: HomeUi::default(),
            hover_tab: None,
            shelf_scroll: 0,
            note: TextEdit::default(),
            cfg: Settings::default(),
            editing: false,
            caret_on: true,
            media: Media::default(),
            lyric: LyricUi::default(),
            update: crate::services::update::Status::default(),
            notes: NotesUi::default(),
            sw: SwUi::default(),
            privacy: Privacy::default(),
            mic_muted: None,
            vpn: None,
            vpn_busy: false,
            notice: None,
            volume: None,
            recording_secs: None,
            downloading: None,
            char_inset: 0.0,
            radios: Radios::default(),
            dark: true,
            hover_ctl: None,
            armed: None,
            pinned: false,
            hover_head: None,
            hand: false,
            tip: None,
            weather: None,
            spotify: crate::services::spotify::State::default(),
            discord: crate::services::discord::State::default(),
        }
    }
}

fn inside(r: Rect, px: f32, py: f32) -> bool {
    px >= r.0 && px < r.2 && py >= r.1 && py < r.3
}

const DIM: f32 = 0.6;

fn acc(alpha: f32) -> windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F {
    let (r, g, b) = crate::ui::theme::accent();
    rgba(r, g, b, alpha)
}

const GREEN: (f32, f32, f32) = (0.4, 0.85, 0.55);

const RED: (f32, f32, f32) = (1.0, 0.35, 0.35);
