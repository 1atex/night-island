use crate::data::keys::Hotkey;
use crate::services::{discord, spotify};
use crate::view::*;

pub struct Group {
    pub glyph: char,
    pub name: &'static str,
    pub(in crate::view) rows: &'static [(Sid, &'static str, Kind)],
    pub(in crate::view) custom: Option<fn(&UiState) -> Vec<Item>>,
}

const GENERAL: [(Sid, &str, Kind); 9] = [
    (Sid::Language, "Language", Kind::Segment(["Русский", "English"])),
    (Sid::Weather, "Weather", Kind::Toggle),
    (Sid::Autostart, "Start with Windows", Kind::Toggle),
    (Sid::GameMode, "Game mode", Kind::Toggle),
    (Sid::AutoHide, "Auto-hide", Kind::Toggle),
    (Sid::Capture, "Hide in screenshots", Kind::Toggle),
    (Sid::CaptureSelf, "Hide island in my captures", Kind::Toggle),
    (Sid::DevTools, "Developer tools", Kind::Toggle),
    (Sid::ActivityTrack, "Track activity (time in programs)", Kind::Toggle),
];

const ISLAND: [(Sid, &str, Kind); 14] = [
    (Sid::GhostMode, "Hotkey makes the pill a ghost", Kind::Toggle),
    (Sid::GhostOpacity, "Ghost visibility", Kind::Slider),
    (Sid::GhostThrough, "Ghost ignores the mouse", Kind::Toggle),
    (Sid::GhostHold, "Ghost only while held", Kind::Toggle),
    (Sid::HidePillSecs, "Hide the pill for", Kind::Slider),
    (Sid::Placement, "Placement", Kind::Segment(["Floating", "Docked"])),
    (Sid::Edge, "Edge", Kind::Cycle),
    (Sid::Style, "Shape", Kind::Segment(["Flat", "Liquid"])),
    (Sid::Character, "Character", Kind::Cycle),
    (Sid::Size, "Size", Kind::Slider),
    (Sid::OpenDelay, "Open delay", Kind::Slider),
    (Sid::CloseDelay, "Close delay", Kind::Slider),
    (Sid::OpenSpeed, "Open speed", Kind::Slider),
    (Sid::CloseSpeed, "Close speed", Kind::Slider),
];

const APPEARANCE: [(Sid, &str, Kind); 4] = [
    (Sid::Acrylic, "Acrylic blur", Kind::Toggle),
    (Sid::LiquidReach, "Liquid reach", Kind::Slider),
    (Sid::LiquidStrength, "Liquid depth", Kind::Slider),
    (Sid::LiquidSoftness, "Liquid softness", Kind::Slider),
];

const PILL: [(Sid, &str, Kind); 10] = [
    (Sid::VpnPill, "VPN shield and button (Happ)", Kind::Toggle),
    (Sid::PillWeather, "Weather on the pill", Kind::Toggle),
    (Sid::PillBattery, "Battery on the pill", Kind::Cycle),
    (Sid::MusicEq, "Equalizer", Kind::Toggle),
    (Sid::MusicTitle, "Track title", Kind::Toggle),
    (Sid::MusicCover, "Cover art on the pill", Kind::Toggle),
    (Sid::MusicLyrics, "Synced lyrics", Kind::Toggle),
    (Sid::SharedLyrics, "Shared lyrics cache", Kind::Toggle),
    (Sid::Spikes, "Edge spikes", Kind::Toggle),
    (Sid::SpikeColor, "Spike tips in track colour", Kind::Toggle),
];

const NOTIFICATIONS: [(Sid, &str, Kind); 16] = [
    (Sid::NotifyMic, "Microphone mute", Kind::Toggle),
    (Sid::NotifyDownloads, "Finished downloads", Kind::Toggle),
    (Sid::NotifyCapture, "Screenshots, text and QR", Kind::Toggle),
    (Sid::NotifyRecording, "Screen recording", Kind::Toggle),
    (Sid::NotifyAgy, "AGY", Kind::Toggle),
    (Sid::NotifyIntegrations, "Discord and Spotify", Kind::Toggle),
    (Sid::NotifyErrors, "Errors and limits", Kind::Toggle),
    (Sid::NotifyClipboard, "Copied to the clipboard", Kind::Toggle),
    (Sid::NotifyClipboardText, "Show the copied text", Kind::Toggle),
    (Sid::NotifyLockKeys, "Caps, Num and Scroll Lock", Kind::Toggle),
    (Sid::NotifyBattery, "Charger and low battery", Kind::Toggle),
    (Sid::NotifyAi, "AI answers", Kind::Toggle),
    (Sid::NotifyUpdate, "New version available", Kind::Toggle),
    (Sid::NotifyPrivacy, "Screen capture by other programs", Kind::Toggle),
    (Sid::NotifyAgents, "Coding agents (waiting, finished)", Kind::Toggle),
    (Sid::NotifySteam, "Steam games (started, ended)", Kind::Toggle),
];

const AI: [(Sid, &str, Kind); 3] = [
    (Sid::AgyAuto, "AGY auto-approve", Kind::Toggle),
    (Sid::VoicePaste, "Paste dictated text", Kind::Toggle),
    (Sid::VoiceSeparate, "Separate speech service", Kind::Toggle),
];

const ACTIVITY: [(Sid, &str, Kind); 1] = [(Sid::ActivityAi, "Sort unknown programs with AI", Kind::Toggle)];

const AI_PROVIDER: (Sid, &str, Kind) = (Sid::AiProvider, "Service", Kind::Cycle);

const ABOUT: [(Sid, &str, Kind); 3] = [
    (Sid::UpdateCheck, "Check for updates", Kind::Toggle),
    (Sid::UpdateBeta, "Receive beta versions", Kind::Toggle),
    (Sid::OpenLogs, "Logs folder", Kind::Button),
];

const ROWS_TABS_ONLY: [(Sid, &str, Kind); 15] = [
    (Sid::TabClipboard, "Clipboard", Kind::Toggle),
    (Sid::TabShelf, "Shelf", Kind::Toggle),
    (Sid::TabText, "Text", Kind::Toggle),
    (Sid::TabNote, "Note", Kind::Toggle),
    (Sid::TabReminders, "Reminders", Kind::Toggle),
    (Sid::TabWeather, "Weather", Kind::Toggle),
    (Sid::TabRecord, "Record", Kind::Toggle),
    (Sid::TabAgy, "AGY", Kind::Toggle),
    (Sid::TabChat, "Chat", Kind::Toggle),
    (Sid::TabPrivacy, "Privacy", Kind::Toggle),
    (Sid::TabActivity, "Activity", Kind::Toggle),
    (Sid::TabSteam, "Steam", Kind::Toggle),
    (Sid::TabSystem, "System", Kind::Toggle),
    (Sid::TabControls, "Controls", Kind::Toggle),
    (Sid::TabTheme, "Theme", Kind::Toggle),
];

pub const GROUPS: [Group; 13] = [
    Group { glyph: '\u{E713}', name: "General", rows: &GENERAL, custom: None },
    Group { glyph: '\u{E80F}', name: "Island", rows: &ISLAND, custom: None },
    Group { glyph: '\u{E790}', name: "Appearance", rows: &APPEARANCE, custom: None },
    Group { glyph: '\u{E8D6}', name: "Pill and music", rows: &PILL, custom: Some(pill_items) },
    Group { glyph: '\u{E9D9}', name: "System", rows: &[], custom: Some(metrics_items) },
    Group { glyph: '\u{EA8F}', name: "Notifications", rows: &NOTIFICATIONS, custom: None },
    Group { glyph: '\u{E71D}', name: "Tabs", rows: &ROWS_TABS_ONLY, custom: None },
    Group { glyph: '\u{E765}', name: "Keys", rows: &[], custom: Some(key_items) },
    Group { glyph: '\u{E71B}', name: "Integrations", rows: &[], custom: Some(integration_items) },
    Group { glyph: '\u{E9D9}', name: "Activity", rows: &ACTIVITY, custom: Some(activity_items) },
    Group { glyph: '\u{E99A}', name: "AI", rows: &AI, custom: Some(ai_items) },
    Group { glyph: '\u{E756}', name: "Agents", rows: &[], custom: Some(agent_items) },
    Group { glyph: '\u{E946}', name: "About", rows: &ABOUT, custom: Some(about_items) },
];

pub const SW_SIZE: (f32, f32) = (940.0, 620.0);

const SIDEBAR_W: f32 = 220.0;
const PAD: f32 = 28.0;
const SEARCH_H: f32 = 38.0;
const HEADING_H: f32 = 44.0;
const ROW_H: f32 = 46.0;
const SERVICE_H: f32 = 56.0;
const NOTE_H: f32 = 19.0;
const CTL_W: f32 = 300.0;
const GROUP_H: f32 = 38.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Field {
    DiscordId,
    DiscordSecret,
    SpotifyId,
    AiKey,
    AiModel,
    AiBase,
    VoiceKey,
    VoiceModel,
    VoiceBase,
    WakeWord,
    GroupName,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Act {
    DiscordMain,
    DiscordMute,
    DiscordDeaf,
    DiscordOverlay,
    SpotifyMain,
    SpotifySaveId,
    AiSave,
    AiRemoveKey,
    VoiceSave,
    VoiceRemoveKey,
    WakeSave,
    GroupAdd,
    GroupDelete(i64),
    UpdateCheck,
    UpdateDownload,
    UpdateInstall,
    Rebind(Hotkey),
    KeysReset,
    LyricsAppAdd,
    LyricsAppRemove(usize),
    Metric(usize),
    Agent(usize),
}

#[derive(Default)]
pub struct SwUi {
    pub group: usize,
    pub scroll: f32,
    pub search: String,
    pub hover: Option<SwHit>,
    pub drag: Option<Sid>,
    pub field: Option<Field>,
    pub caret_on: bool,
    pub capturing: Option<Hotkey>,
    pub quiet_sys: bool,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SwHit {
    Group(usize),
    Row(Sid, SetHit),
    Field(Field),
    Act(Act),
}

pub(in crate::view) enum Item {
    Heading(&'static str),
    Row(Sid, &'static str, Kind),
    Service { name: &'static str, glyph: char, status: String, live: bool },
    Field(Field, &'static str, &'static str),
    Button(Act, String, bool),
    Pair(Act, String, String, bool),
    Note(&'static str),
    Text(String),
}

impl Item {
    fn height(&self) -> f32 {
        match self {
            Item::Heading(_) => HEADING_H,
            Item::Row(..) | Item::Field(..) | Item::Button(..) | Item::Pair(..) => ROW_H,
            Item::Service { .. } => SERVICE_H,
            Item::Note(_) | Item::Text(_) => NOTE_H,
        }
    }
}

fn discord_status_text(d: &discord::State) -> (String, bool) {
    use discord::Status as D;
    let text = match &d.status {
        D::NoConfig => "Needs a Client ID".to_string(),
        D::Idle => "Not connected".to_string(),
        D::NotRunning => "Not running".to_string(),
        D::Authorizing => "Approve in Discord…".to_string(),
        D::Connected if d.channel.is_empty() => "Not in a voice channel".to_string(),
        D::Connected => d.channel.clone(),
        D::Error(why) => why.clone(),
    };
    (text, d.status == D::Connected)
}

fn spotify_status_text(s: &spotify::State) -> (String, bool) {
    use spotify::Status as S;
    let text = match &s.status {
        S::NoClientId => "Needs a Client ID".to_string(),
        S::Disconnected => "Not connected".to_string(),
        S::Connecting => "Waiting for the browser…".to_string(),
        S::Connected => "Connected".to_string(),
        S::Error(why) => why.clone(),
    };
    (text, s.status == S::Connected)
}

fn integration_items(ui: &UiState) -> Vec<Item> {
    use discord::Status as D;
    let mut out = vec![Item::Heading("Integrations")];

    let (status, live) = discord_status_text(&ui.discord);
    out.push(Item::Service { name: "Discord", glyph: '\u{E8F2}', status, live });
    match &ui.discord.status {
        D::NoConfig => {
            out.push(Item::Field(Field::DiscordId, "Client ID", "Client ID"));
            out.push(Item::Field(Field::DiscordSecret, "Client Secret", "Client Secret"));
            out.push(Item::Note("discord.com/developers: New Application, OAuth2: Redirect http://127.0.0.1,"));
            out.push(Item::Note("App Testers: add yourself. The credentials stay on this computer."));
            out.push(Item::Button(Act::DiscordMain, "Save".into(), discord_fields_ok(ui)));
        }
        D::Idle | D::Error(_) => {
            out.push(Item::Note("Discord must be running; you will see an Authorize prompt inside it."));
            out.push(Item::Button(Act::DiscordMain, "Connect".into(), true));
        }
        D::Authorizing => out.push(Item::Button(Act::DiscordMain, "Approve in Discord…".into(), false)),
        D::NotRunning => out.push(Item::Button(Act::DiscordMain, "Disconnect".into(), true)),
        D::Connected => {
            let mute = if ui.discord.self_mute { "Unmute the microphone" } else { "Mute the microphone" };
            let deaf = if ui.discord.self_deaf { "Undeafen" } else { "Deafen" };
            let overlay = if ui.cfg.discord_overlay { "Hide the call overlay" } else { "Show the call overlay" };
            out.push(Item::Button(Act::DiscordMute, mute.into(), true));
            out.push(Item::Button(Act::DiscordDeaf, deaf.into(), true));
            out.push(Item::Button(Act::DiscordOverlay, overlay.into(), true));
            out.push(Item::Button(Act::DiscordMain, "Disconnect".into(), true));
        }
    }

    let (status, live) = spotify_status_text(&ui.spotify);
    out.push(Item::Service { name: "Spotify", glyph: '\u{E8D6}', status, live });
    out.push(Item::Note("Shuffle, repeat and likes for the playing track."));
    match &ui.spotify.status {
        spotify::Status::Connecting => {
            out.push(Item::Button(Act::SpotifyMain, "Waiting for the browser…".into(), false));
        }
        spotify::Status::Connected => out.push(Item::Button(Act::SpotifyMain, "Disconnect".into(), true)),
        _ => out.push(Item::Button(Act::SpotifyMain, "Connect".into(), true)),
    }
    out.push(Item::Field(Field::SpotifyId, "Own Client ID", "Optional: your Spotify app"));
    out.push(Item::Button(Act::SpotifySaveId, "Save the Client ID".into(), true));
    out
}

fn agent_items(ui: &UiState) -> Vec<Item> {
    use crate::services::agents::{self, Agent, State};
    let mut out = vec![Item::Heading("Coding agents")];
    out.push(Item::Note("The island shows when an agent works, waits for you or finishes."));
    out.push(Item::Note("Nothing is changed in their settings until you connect."));
    for (i, agent) in Agent::ALL.into_iter().enumerate() {
        let state = if agents::connected(agent) { "Disconnect" } else { "Connect" };
        out.push(Item::Pair(Act::Agent(i), agent.name().into(), state.into(), false));
    }
    out.push(Item::Note("Claude Code, Gemini CLI, Cursor, Copilot CLI: hooks in their settings files."));
    out.push(Item::Note("Codex CLI: its notify command (finished only); a command it already has keeps running."));
    out.push(Item::Note("Perplexity has no hooks or command line to connect to."));
    out.push(Item::Heading("Sessions"));
    if ui.agents.is_empty() {
        out.push(Item::Note("No sessions heard from yet."));
    }
    let tr = |s: &str| crate::ui::i18n::tr(s).into_owned();
    for s in ui.agents.iter().take(8) {
        let word = match s.state {
            State::Working => tr("working"),
            State::Waiting => tr("waiting for you"),
            State::Done => tr("finished"),
        };
        out.push(Item::Text(format!("{} · {} — {word}", s.agent.name(), s.project)));
    }
    out
}

fn metrics_items(ui: &UiState) -> Vec<Item> {
    use crate::platform::metrics::Metric;
    let mut out = vec![Item::Heading("System"), Item::Row(Sid::MetricsInterval, "Read every", Kind::Slider)];
    out.push(Item::Heading("Figures"));
    for (i, m) in Metric::ALL.into_iter().enumerate() {
        let on_tab = ui.cfg.metrics_tab.iter().any(|k| k == m.key());
        let on_pill = ui.cfg.metrics_pill.iter().any(|k| k == m.key());
        let state = match (on_tab, on_pill) {
            (false, false) => "Hidden",
            (true, false) => "On the tab",
            (false, true) => "On the pill",
            (true, true) => "Tab and pill",
        };
        out.push(Item::Pair(Act::Metric(i), m.label().into(), state.into(), false));
    }
    out.push(Item::Note("Click a figure to move it: hidden, the System tab, the pill, or both."));
    out.push(Item::Note("Nothing is read while no figure is on screen."));
    out
}

fn pill_items(ui: &UiState) -> Vec<Item> {
    let mut out = vec![Item::Heading("Pill and music")];
    out.extend(PILL.iter().map(|&(id, label, kind)| Item::Row(id, label, kind)));
    out.push(Item::Heading("Players for lyrics"));
    let apps = &ui.cfg.lyrics_apps;
    if apps.is_empty() {
        out.push(Item::Note("Any player. Add one to take lyrics only from the players you trust."));
    } else {
        out.push(Item::Note("Lyrics are looked up only while one of these plays."));
    }
    for (i, app) in apps.iter().enumerate() {
        out.push(Item::Pair(Act::LyricsAppRemove(i), app.clone(), "Remove".into(), false));
    }
    let source = &ui.media.source;
    let can_add = !source.is_empty() && !apps.iter().any(|a| a.eq_ignore_ascii_case(source));
    let label = if source.is_empty() {
        "Add the player (start playing first)".into()
    } else {
        format!("Add the player {source}")
    };
    out.push(Item::Button(Act::LyricsAppAdd, label, can_add));
    out
}

fn key_items(ui: &UiState) -> Vec<Item> {
    let mut out = vec![Item::Heading("Keys")];
    for h in Hotkey::ALL {
        let capturing = ui.sw.capturing == Some(h);
        let text = if capturing { "Press the new keys…".to_string() } else { ui.cfg.combo(h).text() };
        out.push(Item::Pair(Act::Rebind(h), h.label().into(), text, capturing));
    }
    out.push(Item::Note("Click a key, then press the new combination with Ctrl, Alt, Shift or Win."));
    out.push(Item::Note("Esc cancels, Delete brings the default back."));
    out.push(Item::Button(Act::KeysReset, "Reset all keys".into(), !ui.cfg.keys.is_empty()));
    out
}

fn about_items(ui: &UiState) -> Vec<Item> {
    use crate::services::update::{self, Status as U};
    let mut out = vec![Item::Heading("About"), Item::Text(format!("Night Island {}", update::CURRENT))];
    let line = match &ui.update {
        U::Unknown | U::NotConfigured if !update::configured() => {
            "Updates are off: this build has no release key".to_string()
        }
        U::Unknown | U::NotConfigured => "Not checked yet".to_string(),
        U::Checking => "Checking…".to_string(),
        U::UpToDate => "You have the latest version".to_string(),
        U::Available(f) => format!("Version {} is available", f.version),
        U::Downloading(f) => format!("Downloading version {}…", f.version),
        U::Ready(f, _) => format!("Version {} is downloaded and checked", f.version),
        U::Failed(why) => why.clone(),
    };
    out.push(Item::Text(line));
    let busy = matches!(ui.update, U::Checking | U::Downloading(_));
    out.push(Item::Button(Act::UpdateCheck, "Check now".into(), update::configured() && !busy));
    match &ui.update {
        U::Available(f) => {
            if let Some(first) = f.notes.lines().next().filter(|l| !l.trim().is_empty()) {
                out.push(Item::Text(first.chars().take(110).collect()));
            }
            out.push(Item::Button(Act::UpdateDownload, format!("Download version {}", f.version), true));
        }
        U::Ready(f, _) => {
            out.push(Item::Button(Act::UpdateInstall, format!("Install version {} and restart", f.version), true));
        }
        _ => {}
    }
    for &(id, label, kind) in &ABOUT {
        out.push(Item::Row(id, label, kind));
    }
    out
}

fn activity_items(ui: &UiState) -> Vec<Item> {
    let mut out = vec![Item::Heading("Activity")];
    for &(id, label, kind) in &ACTIVITY {
        out.push(Item::Row(id, label, kind));
    }
    out.push(Item::Note("Off by default. Programs are grouped by a built-in table first, and by your choices."));
    out.push(Item::Note("On: the file names of programs nothing else knows (nothing more) go to your AI service."));
    out.push(Item::Text(format!(
        "{} {} / {}",
        crate::ui::i18n::tr("AI requests today:"),
        ui.activity.ai_used,
        crate::services::ai::SORT_REQUESTS_PER_DAY
    )));
    out.push(Item::Heading("Groups"));
    for group in &ui.activity.groups {
        if group.builtin {
            out.push(Item::Text(group.name.clone()));
        } else {
            out.push(Item::Pair(Act::GroupDelete(group.id), group.name.clone(), "Delete".into(), false));
        }
    }
    out.push(Item::Field(Field::GroupName, "New group", "Name"));
    out.push(Item::Button(Act::GroupAdd, "Add group".into(), ui.activity.groups.len() < 12));
    out.push(Item::Note("Open a group in the Activity tab and click a program to move it to the next group."));
    out
}

fn ai_items(ui: &UiState) -> Vec<Item> {
    use crate::data::settings::AiProvider;
    let provider = ui.cfg.ai.provider;
    let saved = ui.int.ai_key_saved;
    let (id, label, kind) = AI_PROVIDER;
    let mut out = vec![Item::Heading("AI"), Item::Row(id, label, kind)];
    out.push(Item::Service {
        name: provider.name(),
        glyph: '\u{E99A}',
        status: if saved { "Key saved".into() } else { "No key yet".into() },
        live: saved,
    });
    out.push(Item::Field(
        Field::AiKey,
        "API key",
        if saved { "Saved: type a new one to replace it" } else { "Paste the key here" },
    ));
    out.push(Item::Field(Field::AiModel, "Model", provider.default_model()));
    if provider == AiProvider::OpenAi {
        out.push(Item::Field(Field::AiBase, "Server address", "https://api.openai.com/v1"));
    }
    out.push(Item::Button(Act::AiSave, "Save".into(), true));
    if saved {
        out.push(Item::Button(Act::AiRemoveKey, "Remove the saved key".into(), true));
    }
    out.push(Item::Note("Ask in the command bar (Alt+Space): start with ?, for example  ? what is DPAPI"));
    out.push(Item::Note("The key is stored encrypted for your Windows account and is never shown again."));
    out.push(Item::Note("OpenAI-compatible: OpenAI, OpenRouter, or a local server such as LM Studio."));
    for &(id, label, kind) in &AI {
        out.push(Item::Row(id, label, kind));
    }
    out.push(Item::Field(Field::WakeWord, "Voice command name", "Island, Айленд"));
    out.push(Item::Button(Act::WakeSave, "Save command name".into(), true));
    out.push(Item::Note(
        "Dictate the name first: \"Island, next track\", \"Island, remind me in 10 min\". Empty: off.",
    ));
    if ui.cfg.ai.voice_separate {
        let saved = ui.int.voice_key_saved;
        let shared = ui.int.voice_key_shared;
        out.push(Item::Field(
            Field::VoiceKey,
            "Speech API key",
            if saved {
                "Saved: type a new one to replace it"
            } else if shared {
                "Same server: the key of the chat service is used"
            } else {
                "Paste the key here"
            },
        ));
        out.push(Item::Field(Field::VoiceModel, "Speech model", "whisper-1"));
        out.push(Item::Field(Field::VoiceBase, "Speech server address", "https://api.openai.com/v1"));
        out.push(Item::Button(Act::VoiceSave, "Save speech service".into(), true));
        if saved {
            out.push(Item::Button(Act::VoiceRemoveKey, "Remove the speech key".into(), true));
        }
        out.push(Item::Note("Any OpenAI-compatible /audio/transcriptions server: OpenAI, Groq, your own Whisper."));
    }
    out
}

fn matches(label: &str, query: &str) -> bool {
    let q = query.trim().to_lowercase();
    q.is_empty() || label.to_lowercase().contains(&q) || crate::ui::i18n::tr(label).to_lowercase().contains(&q)
}

pub(in crate::view) fn items(ui: &UiState) -> Vec<Item> {
    let mut out = Vec::new();
    if ui.sw.search.trim().is_empty() {
        let group = &GROUPS[ui.sw.group.min(GROUPS.len() - 1)];
        if let Some(build) = group.custom {
            return build(ui);
        }
        out.push(Item::Heading(group.name));
        out.extend(group.rows.iter().map(|&(id, label, kind)| Item::Row(id, label, kind)));
        return out;
    }
    for group in &GROUPS {
        let found: Vec<_> = group.rows.iter().filter(|r| matches(r.1, &ui.sw.search)).collect();
        if found.is_empty() {
            continue;
        }
        out.push(Item::Heading(group.name));
        out.extend(found.into_iter().map(|&(id, label, kind)| Item::Row(id, label, kind)));
    }
    out
}

struct Layout {
    sidebar: Rect,
    search: Rect,
    content: Rect,
}

fn layout(size: (f32, f32), scale: f32) -> Layout {
    let (w, h) = size;
    let left = (SIDEBAR_W + PAD) * scale;
    Layout {
        sidebar: (0.0, 0.0, SIDEBAR_W * scale, h),
        search: (left, 24.0 * scale, w - PAD * scale, (24.0 + SEARCH_H) * scale),
        content: (left, (24.0 + SEARCH_H + 14.0) * scale, w - PAD * scale, h - 12.0 * scale),
    }
}

fn group_rect(l: &Layout, scale: f32, i: usize) -> Rect {
    let top = (24.0 + 52.0) * scale + i as f32 * GROUP_H * scale;
    (l.sidebar.0 + 12.0 * scale, top, l.sidebar.2 - 12.0 * scale, top + (GROUP_H - 4.0) * scale)
}

fn placed(size: (f32, f32), scale: f32, ui: &UiState) -> Vec<(Item, f32)> {
    let l = layout(size, scale);
    let mut y = l.content.1 - ui.sw.scroll;
    items(ui)
        .into_iter()
        .map(|item| {
            let top = y;
            y += item.height() * scale;
            (item, top)
        })
        .collect()
}

pub fn max_scroll(size: (f32, f32), scale: f32, ui: &UiState) -> f32 {
    let l = layout(size, scale);
    let total: f32 = items(ui).iter().map(Item::height).sum::<f32>() * scale;
    (total - (l.content.3 - l.content.1)).max(0.0)
}

fn row_rects(l: &Layout, scale: f32, top: f32) -> (Rect, Rect) {
    let row = (l.content.0, top, l.content.2, top + ROW_H * scale);
    let ctl = (row.2 - CTL_W * scale, top + 7.0 * scale, row.2, top + (ROW_H - 7.0) * scale);
    (row, ctl)
}

pub fn sw_hit(size: (f32, f32), scale: f32, ui: &UiState, px: f32, py: f32) -> Option<SwHit> {
    let l = layout(size, scale);
    if let Some(i) = (0..GROUPS.len()).find(|&i| inside(group_rect(&l, scale, i), px, py)) {
        return Some(SwHit::Group(i));
    }
    if !inside(l.content, px, py) {
        return None;
    }
    for (item, top) in placed(size, scale, ui) {
        let (row, ctl) = row_rects(&l, scale, top);
        match item {
            Item::Row(id, _, kind) => {
                if !inside(row, px, py) || px < ctl.0 - 6.0 * scale {
                    continue;
                }
                let hit = match kind {
                    Kind::Segment(opts) => {
                        let t = ((px - ctl.0) / (ctl.2 - ctl.0)).clamp(0.0, 0.999);
                        SetHit::Option((t * opts.len() as f32) as usize)
                    }
                    Kind::Cycle => SetHit::Option(usize::from(px > (ctl.0 + ctl.2) / 2.0)),
                    Kind::Slider => SetHit::Slider,
                    Kind::Toggle => SetHit::Toggle,
                    Kind::Button => SetHit::Button,
                };
                return Some(SwHit::Row(id, hit));
            }
            Item::Field(field, ..) if inside(ctl, px, py) => return Some(SwHit::Field(field)),
            Item::Button(act, _, enabled) if enabled && inside(ctl, px, py) => return Some(SwHit::Act(act)),
            Item::Pair(act, ..) if inside(ctl, px, py) => return Some(SwHit::Act(act)),
            _ => {}
        }
    }
    None
}

pub fn sw_slider_track(size: (f32, f32), scale: f32, ui: &UiState, id: Sid) -> Option<(f32, f32)> {
    let l = layout(size, scale);
    let top = placed(size, scale, ui).into_iter().find_map(|(item, top)| match item {
        Item::Row(row_id, ..) if row_id == id => Some(top),
        _ => None,
    })?;
    let (_, ctl) = row_rects(&l, scale, top);
    Some((ctl.0 + 4.0 * scale, ctl.2 - 54.0 * scale))
}

pub fn sw_field_rects(size: (f32, f32), scale: f32, ui: &UiState, field: Field) -> Option<(Rect, Rect)> {
    let l = layout(size, scale);
    let top = placed(size, scale, ui).into_iter().find_map(|(item, top)| match item {
        Item::Field(f, ..) if f == field => Some(top),
        _ => None,
    })?;
    let (_, ctl) = row_rects(&l, scale, top);
    Some((ctl, (ctl.0 + 12.0 * scale, ctl.1, ctl.2 - 10.0 * scale, ctl.3)))
}

pub fn sw_next_field(ui: &UiState, field: Field) -> Option<Field> {
    let fields: Vec<Field> = items(ui)
        .into_iter()
        .filter_map(|i| match i {
            Item::Field(f, ..) => Some(f),
            _ => None,
        })
        .collect();
    let at = fields.iter().position(|f| *f == field)?;
    fields.get((at + 1) % fields.len()).copied()
}

pub fn field_edit(ui: &UiState, field: Field) -> &TextEdit {
    match field {
        Field::DiscordId => &ui.int.id,
        Field::DiscordSecret => &ui.int.secret,
        Field::SpotifyId => &ui.int.spotify_id,
        Field::AiKey => &ui.int.ai_key,
        Field::AiModel => &ui.int.ai_model,
        Field::AiBase => &ui.int.ai_base,
        Field::VoiceKey => &ui.int.voice_key,
        Field::VoiceModel => &ui.int.voice_model,
        Field::VoiceBase => &ui.int.voice_base,
        Field::WakeWord => &ui.int.wake,
        Field::GroupName => &ui.int.group_name,
    }
}

pub fn paint_settings_window(p: &Painter, size: (f32, f32), scale: f32, ui: &UiState) -> Result<()> {
    let l = layout(size, scale);
    let (pr, pg, pb) = crate::ui::theme::panel();
    p.fill_rect((0.0, 0.0, size.0, size.1), rgba(pr, pg, pb, 1.0))?;
    p.fill_rect(l.sidebar, rgba(1.0, 1.0, 1.0, 0.045))?;

    p.text(
        "Settings",
        (l.sidebar.0 + 22.0 * scale, 22.0 * scale, l.sidebar.2, 56.0 * scale),
        17.0 * scale,
        DWRITE_FONT_WEIGHT_SEMI_BOLD,
        DWRITE_TEXT_ALIGNMENT_LEADING,
        WHITE,
    )?;
    let searching = !ui.sw.search.trim().is_empty();
    for (i, group) in GROUPS.iter().enumerate() {
        let r = group_rect(&l, scale, i);
        let selected = i == ui.sw.group && !searching;
        if selected {
            p.fill_round_rect(r, 10.0 * scale, acc(0.22))?;
        } else if ui.sw.hover == Some(SwHit::Group(i)) {
            p.fill_round_rect(r, 10.0 * scale, rgba(1.0, 1.0, 1.0, 0.07))?;
        }
        let alpha = if selected { 1.0 } else { 0.7 };
        p.glyph(
            group.glyph,
            (r.0 + 6.0 * scale, r.1, r.0 + 38.0 * scale, r.3),
            16.0 * scale,
            rgba(1.0, 1.0, 1.0, alpha),
        )?;
        p.text(
            group.name,
            (r.0 + 42.0 * scale, r.1, r.2, r.3),
            13.5 * scale,
            if selected { DWRITE_FONT_WEIGHT_SEMI_BOLD } else { DWRITE_FONT_WEIGHT_NORMAL },
            DWRITE_TEXT_ALIGNMENT_LEADING,
            rgba(1.0, 1.0, 1.0, alpha),
        )?;
    }

    p.fill_round_rect(l.search, 11.0 * scale, rgba(1.0, 1.0, 1.0, 0.07))?;
    p.glyph(
        '\u{E721}',
        (l.search.0 + 4.0 * scale, l.search.1, l.search.0 + 38.0 * scale, l.search.3),
        14.0 * scale,
        rgba(1.0, 1.0, 1.0, 0.6),
    )?;
    let (text, alpha) = if ui.sw.search.is_empty() { ("Search settings", 0.45) } else { (ui.sw.search.as_str(), 1.0) };
    p.text(
        text,
        (l.search.0 + 40.0 * scale, l.search.1, l.search.2 - 12.0 * scale, l.search.3),
        13.5 * scale,
        DWRITE_FONT_WEIGHT_NORMAL,
        DWRITE_TEXT_ALIGNMENT_LEADING,
        rgba(1.0, 1.0, 1.0, alpha),
    )?;

    p.push_clip(l.content);
    let result = (|| -> Result<()> {
        let placed = placed(size, scale, ui);
        if placed.is_empty() {
            return p.text(
                "Nothing found",
                (l.content.0, l.content.1, l.content.2, l.content.1 + 40.0 * scale),
                14.0 * scale,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                rgba(1.0, 1.0, 1.0, 0.5),
            );
        }
        for (item, top) in placed {
            if top > l.content.3 || top + item.height() * scale < l.content.1 {
                continue;
            }
            let (row, ctl) = row_rects(&l, scale, top);
            let label_area = (row.0, row.1, ctl.0 - 12.0 * scale, row.3);
            match item {
                Item::Heading(name) => {
                    p.text(
                        name,
                        (l.content.0, top, l.content.2, top + HEADING_H * scale),
                        18.0 * scale,
                        DWRITE_FONT_WEIGHT_SEMI_BOLD,
                        DWRITE_TEXT_ALIGNMENT_LEADING,
                        WHITE,
                    )?;
                }
                Item::Row(id, label, kind) => {
                    if matches!(ui.sw.hover, Some(SwHit::Row(h, _)) if h == id) {
                        p.fill_round_rect(
                            (row.0 - 8.0 * scale, row.1 + 2.0 * scale, row.2 + 8.0 * scale, row.3 - 2.0 * scale),
                            10.0 * scale,
                            rgba(1.0, 1.0, 1.0, 0.04),
                        )?;
                    }
                    p.text(
                        label,
                        label_area,
                        14.0 * scale,
                        DWRITE_FONT_WEIGHT_NORMAL,
                        DWRITE_TEXT_ALIGNMENT_LEADING,
                        rgba(1.0, 1.0, 1.0, 0.9),
                    )?;
                    paint_control(p, scale, &ui.cfg, id, kind, ctl)?;
                }
                Item::Service { name, glyph, status, live } => {
                    let d = 38.0 * scale;
                    let circle = (row.0, top + 8.0 * scale, row.0 + d, top + 8.0 * scale + d);
                    let colour = match name {
                        "Discord" => rgba(0.35, 0.4, 0.95, 1.0),
                        "Spotify" => rgba(0.12, 0.84, 0.38, 1.0),
                        _ => acc(0.9),
                    };
                    p.fill_round_rect(circle, d / 2.0, colour)?;
                    p.glyph(glyph, circle, 17.0 * scale, WHITE)?;
                    p.text(
                        name,
                        (circle.2 + 12.0 * scale, circle.1 - 2.0 * scale, row.2, circle.1 + 20.0 * scale),
                        15.0 * scale,
                        DWRITE_FONT_WEIGHT_BOLD,
                        DWRITE_TEXT_ALIGNMENT_LEADING,
                        WHITE,
                    )?;
                    let status_colour = if live { rgba(0.4, 0.9, 0.55, 1.0) } else { rgba(1.0, 1.0, 1.0, 0.6) };
                    p.text(
                        &status,
                        (circle.2 + 12.0 * scale, circle.1 + 19.0 * scale, row.2, circle.3 + 2.0 * scale),
                        12.5 * scale,
                        DWRITE_FONT_WEIGHT_NORMAL,
                        DWRITE_TEXT_ALIGNMENT_LEADING,
                        status_colour,
                    )?;
                }
                Item::Field(field, label, hint) => {
                    p.text(
                        label,
                        label_area,
                        14.0 * scale,
                        DWRITE_FONT_WEIGHT_NORMAL,
                        DWRITE_TEXT_ALIGNMENT_LEADING,
                        rgba(1.0, 1.0, 1.0, 0.9),
                    )?;
                    let inner = (ctl.0 + 12.0 * scale, ctl.1, ctl.2 - 10.0 * scale, ctl.3);
                    paint_line_edit(
                        p,
                        ctl,
                        inner,
                        field_edit(ui, field),
                        matches!(field, Field::DiscordSecret | Field::AiKey | Field::VoiceKey),
                        hint,
                        ui.sw.field == Some(field),
                        ui.sw.caret_on,
                        scale,
                    )?;
                }
                Item::Button(act, label, enabled) => {
                    let hovered = enabled && ui.sw.hover == Some(SwHit::Act(act));
                    let fill = if hovered {
                        0.2
                    } else if enabled {
                        0.12
                    } else {
                        0.05
                    };
                    p.fill_round_rect(ctl, 11.0 * scale, rgba(1.0, 1.0, 1.0, fill))?;
                    p.text(
                        &label,
                        ctl,
                        13.5 * scale,
                        DWRITE_FONT_WEIGHT_SEMI_BOLD,
                        DWRITE_TEXT_ALIGNMENT_CENTER,
                        rgba(1.0, 1.0, 1.0, if enabled { 1.0 } else { 0.4 }),
                    )?;
                }
                Item::Pair(act, label, button, active) => {
                    p.text(
                        &label,
                        label_area,
                        14.0 * scale,
                        DWRITE_FONT_WEIGHT_NORMAL,
                        DWRITE_TEXT_ALIGNMENT_LEADING,
                        rgba(1.0, 1.0, 1.0, 0.9),
                    )?;
                    if active {
                        p.fill_round_rect(ctl, 11.0 * scale, acc(0.45))?;
                    } else {
                        let fill = if ui.sw.hover == Some(SwHit::Act(act)) { 0.2 } else { 0.12 };
                        p.fill_round_rect(ctl, 11.0 * scale, rgba(1.0, 1.0, 1.0, fill))?;
                    }
                    p.text(
                        &button,
                        ctl,
                        13.5 * scale,
                        DWRITE_FONT_WEIGHT_SEMI_BOLD,
                        DWRITE_TEXT_ALIGNMENT_CENTER,
                        WHITE,
                    )?;
                }
                Item::Text(text) => {
                    p.text(
                        &text,
                        (l.content.0, top, l.content.2, top + NOTE_H * scale),
                        12.5 * scale,
                        DWRITE_FONT_WEIGHT_NORMAL,
                        DWRITE_TEXT_ALIGNMENT_LEADING,
                        rgba(1.0, 1.0, 1.0, 0.75),
                    )?;
                }
                Item::Note(text) => {
                    p.text(
                        text,
                        (l.content.0, top, l.content.2, top + NOTE_H * scale),
                        11.5 * scale,
                        DWRITE_FONT_WEIGHT_NORMAL,
                        DWRITE_TEXT_ALIGNMENT_LEADING,
                        rgba(1.0, 1.0, 1.0, 0.5),
                    )?;
                }
            }
        }
        Ok(())
    })();
    p.pop_clip();
    result?;

    let max = max_scroll(size, scale, ui);
    if max > 0.0 {
        let (track_top, track_h) = (l.content.1, l.content.3 - l.content.1);
        let total = track_h + max;
        let bar_h = (track_h * track_h / total).max(24.0 * scale);
        let y = track_top + (track_h - bar_h) * (ui.sw.scroll / max);
        p.fill_round_rect(
            (size.0 - 12.0 * scale, y, size.0 - 8.0 * scale, y + bar_h),
            2.0 * scale,
            rgba(1.0, 1.0, 1.0, 0.25),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_finds_rows_in_both_languages() {
        assert!(matches("Synced lyrics", "lyr"));
        assert!(matches("Synced lyrics", "  "));
        assert!(!matches("Synced lyrics", "battery"));
    }

    #[test]
    fn every_group_row_is_unique_and_groups_have_content() {
        let mut seen = Vec::new();
        for g in &GROUPS {
            assert!(!g.rows.is_empty() || g.custom.is_some(), "{} is empty", g.name);
            for (id, ..) in g.rows {
                assert!(!seen.contains(id), "{id:?} listed twice");
                seen.push(*id);
            }
        }
    }

    #[test]
    fn the_integration_group_follows_the_services_state() {
        let mut ui = UiState::new();
        ui.sw.group = GROUPS.iter().position(|g| g.name == "Integrations").unwrap();
        ui.discord.status = discord::Status::NoConfig;
        let discord_fields = |ui: &UiState| {
            items(ui).iter().filter(|i| matches!(i, Item::Field(Field::DiscordId | Field::DiscordSecret, ..))).count()
        };
        assert_eq!(discord_fields(&ui), 2);
        let save_enabled = |ui: &UiState| {
            items(ui).iter().any(|i| matches!(i, Item::Button(Act::DiscordMain, label, true) if label == "Save"))
        };
        assert!(!save_enabled(&ui));
        ui.int.id.set_text("123456789012345678");
        ui.int.secret.set_text("secret");
        assert!(save_enabled(&ui));
        ui.discord.status = discord::Status::Connected;
        assert_eq!(discord_fields(&ui), 0);
        assert!(items(&ui).iter().any(|i| matches!(i, Item::Button(Act::DiscordMute, ..))));
    }
}
