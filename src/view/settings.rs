use crate::view::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sid {
    PillBlur,
    Autostart,
    PillOpacity,
    AgyAuto,
    PanelHue,
    PanelSat,
    PanelVal,
    AccentHue,
    AccentSat,
    Language,
    Weather,
    Placement,
    Edge,
    Style,
    Size,
    LiquidReach,
    LiquidStrength,
    LiquidSoftness,
    Tint,
    PillWeather,
    PillBattery,
    MusicEq,
    MusicTitle,
    MusicLyrics,
    TabClipboard,
    TabShelf,
    TabText,
    TabNote,
    TabReminders,
    TabWeather,
    TabRecord,
    TabAgy,
    TabControls,
    TabTheme,
    OpenIntegrations,
    NotifyMic,
    NotifyDownloads,
    NotifyCapture,
    NotifyRecording,
    NotifyAgy,
    NotifyIntegrations,
    NotifyErrors,
    NotifyClipboard,
    NotifyClipboardText,
    NotifyLockKeys,
    NotifyBattery,
    OpenLogs,
    DevTools,
    SpikeColor,
    Spikes,
    Character,
    RecFormat,
    RecFps,
    RecSystem,
    RecMic,
    OpenDelay,
    CloseDelay,
    OpenSpeed,
    CloseSpeed,
    AutoHide,
    Acrylic,
    Capture,
    CaptureSelf,
    GameMode,
}

#[derive(Clone, Copy)]
pub(in crate::view) enum Kind {
    Segment([&'static str; 2]),
    Cycle,
    Slider,
    Toggle,
    Button,
}

pub(in crate::view) fn tab_toggle_name(id: Sid) -> Option<&'static str> {
    Some(match id {
        Sid::TabClipboard => "Clipboard",
        Sid::TabShelf => "Shelf",
        Sid::TabText => "Text",
        Sid::TabNote => "Note",
        Sid::TabReminders => "Reminders",
        Sid::TabWeather => "Weather",
        Sid::TabRecord => "Record",
        Sid::TabAgy => "AGY",
        Sid::TabControls => "Controls",
        Sid::TabTheme => "Theme",
        _ => return None,
    })
}

pub(in crate::view) const RECORD_ROWS: [(Sid, &str, Kind); 4] = [
    (Sid::RecFormat, "Format", Kind::Segment(["MP4", "GIF"])),
    (Sid::RecFps, "Frame rate", Kind::Slider),
    (Sid::RecSystem, "System sound", Kind::Toggle),
    (Sid::RecMic, "Microphone", Kind::Toggle),
];

pub(in crate::view) const THEME_ROWS: [(Sid, &str, Kind); 8] = [
    (Sid::PanelHue, "Panel hue", Kind::Slider),
    (Sid::PanelSat, "Panel saturation", Kind::Slider),
    (Sid::PanelVal, "Panel brightness", Kind::Slider),
    (Sid::Tint, "Panel opacity", Kind::Slider),
    (Sid::PillOpacity, "Pill opacity", Kind::Slider),
    (Sid::PillBlur, "Pill blur", Kind::Toggle),
    (Sid::AccentHue, "Accent hue", Kind::Slider),
    (Sid::AccentSat, "Accent saturation", Kind::Slider),
];

pub(in crate::view) fn rows_for(tab: Tab) -> &'static [(Sid, &'static str, Kind)] {
    match tab {
        Tab::Record => &RECORD_ROWS,
        Tab::Theme => &THEME_ROWS,
        _ => &[],
    }
}

pub(in crate::view) const SET_ROW_H: f32 = 30.0;

pub(in crate::view) const SET_PER_COL: usize = 10;

pub(in crate::view) const SET_COL_GAP: f32 = 28.0;

pub(in crate::view) const SET_LABEL_W: f32 = 136.0;

pub(in crate::view) fn setting_row(pill: Rect, scale: f32, index: usize) -> (Rect, Rect) {
    let (x, y, w, _) = pill;
    let (side, gap) = (SIDE_PAD * scale, SET_COL_GAP * scale);
    let col_w = (w - 2.0 * side - gap) / 2.0;
    let (col, row) = ((index / SET_PER_COL) as f32, (index % SET_PER_COL) as f32);
    let left = x + side + col * (col_w + gap);
    let top = y + (LIST_TOP + 2.0) * scale + row * SET_ROW_H * scale;
    let rect = (left, top, left + col_w, top + SET_ROW_H * scale);
    let ctl = (left + SET_LABEL_W * scale, top + 4.0 * scale, left + col_w, top + (SET_ROW_H - 4.0) * scale);
    (rect, ctl)
}

pub(in crate::view) fn slider_range(id: Sid) -> (f32, f32, f32) {
    match id {
        Sid::Size => (80.0, 200.0, 5.0),
        Sid::LiquidReach => (60.0, 300.0, 5.0),
        Sid::LiquidStrength => (6.0, 40.0, 1.0),
        Sid::LiquidSoftness => (0.0, 1.0, 0.05),
        Sid::Tint => (0.2, 1.0, 0.05),
        Sid::PillOpacity => (0.2, 1.0, 0.05),
        Sid::PanelHue | Sid::AccentHue => (0.0, 360.0, 1.0),
        Sid::PanelSat | Sid::AccentSat => (0.0, 1.0, 0.01),
        Sid::PanelVal => (0.0, 0.3, 0.005),
        Sid::OpenDelay => (0.0, 1000.0, 10.0),
        Sid::CloseDelay => (0.0, 3000.0, 50.0),
        Sid::OpenSpeed | Sid::CloseSpeed => (0.4, 2.5, 0.1),
        Sid::RecFps => (10.0, 60.0, 5.0),
        _ => (0.0, 1.0, 0.01),
    }
}

pub(in crate::view) fn slider_value(id: Sid, c: &Settings) -> f32 {
    match id {
        Sid::Size => c.scale_percent as f32,
        Sid::LiquidReach => c.liquid_reach,
        Sid::LiquidStrength => c.liquid_strength,
        Sid::LiquidSoftness => c.liquid_softness,
        Sid::Tint => c.tint_open,
        Sid::PillOpacity => c.pill_opacity,
        Sid::PanelHue => c.panel_hue,
        Sid::PanelSat => c.panel_sat,
        Sid::PanelVal => c.panel_val,
        Sid::AccentHue => c.accent_hue,
        Sid::AccentSat => c.accent_sat,
        Sid::OpenDelay => c.open_delay_ms as f32,
        Sid::CloseDelay => c.close_delay_ms as f32,
        Sid::OpenSpeed => c.open_speed,
        Sid::CloseSpeed => c.close_speed,
        Sid::RecFps => c.record_fps as f32,
        _ => 0.0,
    }
}

pub(in crate::view) fn slider_text(id: Sid, v: f32) -> String {
    match id {
        Sid::Size => format!("{v:.0}%"),
        Sid::LiquidReach | Sid::LiquidStrength => format!("{v:.0} px"),
        Sid::LiquidSoftness | Sid::Tint | Sid::PillOpacity | Sid::PanelSat | Sid::AccentSat => {
            format!("{:.0}%", v * 100.0)
        }
        Sid::PanelHue | Sid::AccentHue => format!("{v:.0}°"),
        Sid::PanelVal => format!("{:.0}%", v / 0.3 * 100.0),
        Sid::OpenDelay | Sid::CloseDelay => format!("{v:.0} ms"),
        Sid::OpenSpeed | Sid::CloseSpeed => format!("{v:.1}×"),
        Sid::RecFps => format!("{v:.0} fps"),
        _ => String::new(),
    }
}

pub fn slider_apply(id: Sid, c: &mut Settings, norm: f32) {
    let (min, max, step) = slider_range(id);
    let raw = min + (max - min) * norm.clamp(0.0, 1.0);
    let v = (raw / step).round() * step;
    match id {
        Sid::Size => c.scale_percent = v as u32,
        Sid::LiquidReach => c.liquid_reach = v,
        Sid::LiquidStrength => c.liquid_strength = v,
        Sid::LiquidSoftness => c.liquid_softness = v,
        Sid::Tint => c.tint_open = v,
        Sid::PillOpacity => c.pill_opacity = v,
        Sid::PanelHue => c.panel_hue = v,
        Sid::PanelSat => c.panel_sat = v,
        Sid::PanelVal => c.panel_val = v,
        Sid::AccentHue => c.accent_hue = v,
        Sid::AccentSat => c.accent_sat = v,
        Sid::OpenDelay => c.open_delay_ms = v as u32,
        Sid::CloseDelay => c.close_delay_ms = v as u32,
        Sid::OpenSpeed => c.open_speed = v,
        Sid::CloseSpeed => c.close_speed = v,
        Sid::RecFps => c.record_fps = v as u32,
        _ => {}
    }
    c.sanitize();
}

pub fn toggle_get(id: Sid, c: &Settings) -> bool {
    toggle_value(id, c)
}

pub(in crate::view) fn toggle_value(id: Sid, c: &Settings) -> bool {
    if let Some(name) = tab_toggle_name(id) {
        return !c.tab_hidden(name);
    }
    match id {
        Sid::AutoHide => c.auto_hide,
        Sid::Acrylic => c.acrylic,
        Sid::Capture => c.capture_hidden,
        Sid::CaptureSelf => c.capture_self_hidden,
        Sid::GameMode => c.game_mode,
        Sid::Spikes => c.edge_spikes,
        Sid::PillWeather => c.pill_weather,
        Sid::MusicEq => c.music_eq,
        Sid::MusicTitle => c.music_title,
        Sid::MusicLyrics => c.music_lyrics,
        Sid::DevTools => c.dev_tools,
        Sid::NotifyMic => c.notify.mic,
        Sid::NotifyDownloads => c.notify.downloads,
        Sid::NotifyCapture => c.notify.capture,
        Sid::NotifyRecording => c.notify.recording,
        Sid::NotifyAgy => c.notify.agy,
        Sid::NotifyIntegrations => c.notify.integrations,
        Sid::NotifyErrors => c.notify.errors,
        Sid::NotifyClipboard => c.notify.clipboard,
        Sid::NotifyClipboardText => c.notify.clipboard_text,
        Sid::NotifyLockKeys => c.notify.lock_keys,
        Sid::NotifyBattery => c.notify.battery,
        Sid::SpikeColor => c.spike_color,
        Sid::PillBlur => c.pill_blur,
        Sid::Autostart => crate::platform::autostart::enabled(),
        Sid::Weather => c.weather,
        Sid::AgyAuto => c.agy_auto,
        Sid::RecSystem => c.record_system_audio,
        Sid::RecMic => c.record_mic,
        _ => false,
    }
}

pub fn toggle_set(id: Sid, c: &mut Settings, on: bool) {
    if let Some(name) = tab_toggle_name(id) {
        return c.set_tab_hidden(name, !on);
    }
    match id {
        Sid::AutoHide => c.auto_hide = on,
        Sid::Acrylic => c.acrylic = on,
        Sid::Capture => c.capture_hidden = on,
        Sid::CaptureSelf => c.capture_self_hidden = on,
        Sid::GameMode => c.game_mode = on,
        Sid::Spikes => c.edge_spikes = on,
        Sid::PillWeather => c.pill_weather = on,
        Sid::MusicEq => c.music_eq = on,
        Sid::MusicTitle => c.music_title = on,
        Sid::MusicLyrics => c.music_lyrics = on,
        Sid::DevTools => c.dev_tools = on,
        Sid::NotifyMic => c.notify.mic = on,
        Sid::NotifyDownloads => c.notify.downloads = on,
        Sid::NotifyCapture => c.notify.capture = on,
        Sid::NotifyRecording => c.notify.recording = on,
        Sid::NotifyAgy => c.notify.agy = on,
        Sid::NotifyIntegrations => c.notify.integrations = on,
        Sid::NotifyErrors => c.notify.errors = on,
        Sid::NotifyClipboard => c.notify.clipboard = on,
        Sid::NotifyClipboardText => c.notify.clipboard_text = on,
        Sid::NotifyLockKeys => c.notify.lock_keys = on,
        Sid::NotifyBattery => c.notify.battery = on,
        Sid::SpikeColor => c.spike_color = on,
        Sid::PillBlur => c.pill_blur = on,
        Sid::Autostart => crate::platform::autostart::set(on),
        Sid::Weather => c.weather = on,
        Sid::AgyAuto => c.agy_auto = on,
        Sid::RecSystem => c.record_system_audio = on,
        Sid::RecMic => c.record_mic = on,
        _ => {}
    }
}

pub(in crate::view) fn segment_value(id: Sid, c: &Settings) -> usize {
    match id {
        Sid::Language => usize::from(c.language == Language::En),
        Sid::Placement => usize::from(c.placement == Placement::Docked),
        Sid::Style => usize::from(c.style == Style::Liquid),
        Sid::RecFormat => usize::from(c.record_format == RecordFormat::Gif),
        _ => 0,
    }
}

pub fn cycle_set(id: Sid, c: &mut Settings, forward: bool) {
    match id {
        Sid::Character => c.character = c.character.step(forward),
        Sid::PillBattery => c.pill_battery = c.pill_battery.step(forward),
        Sid::Edge => {
            c.dock_edge = c.dock_edge.step(forward);
            c.dock_offset = 0.5;
        }
        _ => {}
    }
}

pub(in crate::view) fn cycle_name(id: Sid, c: &Settings) -> &'static str {
    match id {
        Sid::Character => c.character.name(),
        Sid::Edge => c.dock_edge.name(),
        Sid::PillBattery => c.pill_battery.name(),
        _ => "",
    }
}

pub fn segment_set(id: Sid, c: &mut Settings, option: usize) {
    match id {
        Sid::Language => c.language = if option == 1 { Language::En } else { Language::Ru },
        Sid::Placement => c.placement = if option == 1 { Placement::Docked } else { Placement::Floating },
        Sid::Style => c.style = if option == 1 { Style::Liquid } else { Style::Flat },
        Sid::RecFormat => c.record_format = if option == 1 { RecordFormat::Gif } else { RecordFormat::Mp4 },
        _ => {}
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SetHit {
    Option(usize),
    Slider,
    Toggle,
    Button,
}

pub fn slider_track(pill: Rect, scale: f32, id: Sid, tab: Tab) -> Option<(f32, f32)> {
    let index = rows_for(tab).iter().position(|r| r.0 == id)?;
    let (_, ctl) = setting_row(pill, scale, index);
    Some((ctl.0 + 4.0 * scale, ctl.2 - 54.0 * scale))
}

pub fn hit_setting(pill: Rect, scale: f32, px: f32, py: f32, tab: Tab) -> Option<(Sid, SetHit)> {
    for (i, (id, _, kind)) in rows_for(tab).iter().enumerate() {
        let (row, ctl) = setting_row(pill, scale, i);
        if !inside(row, px, py) || px < ctl.0 - 6.0 * scale {
            continue;
        }
        return Some((
            *id,
            match kind {
                Kind::Segment(opts) => {
                    let t = ((px - ctl.0) / (ctl.2 - ctl.0)).clamp(0.0, 0.999);
                    SetHit::Option((t * opts.len() as f32) as usize)
                }
                Kind::Cycle => SetHit::Option(usize::from(px > (ctl.0 + ctl.2) / 2.0)),
                Kind::Slider => SetHit::Slider,
                Kind::Toggle => SetHit::Toggle,
                Kind::Button => SetHit::Button,
            },
        ));
    }
    None
}

pub(in crate::view) fn theme_column(pill: Rect, scale: f32) -> (f32, f32, f32) {
    let (row, _) = setting_row(pill, scale, SET_PER_COL);
    (row.0, row.1, row.2)
}

pub(in crate::view) const PRESET_COLS: usize = 4;

pub(in crate::view) fn preset_tile(pill: Rect, scale: f32, i: usize) -> Rect {
    let (l, t, r) = theme_column(pill, scale);
    let gap = 6.0 * scale;
    let w = (r - l - (PRESET_COLS - 1) as f32 * gap) / PRESET_COLS as f32;
    let (col, row) = ((i % PRESET_COLS) as f32, (i / PRESET_COLS) as f32);
    let left = l + col * (w + gap);
    let top = t + 76.0 * scale + row * (30.0 * scale + gap);
    (left, top, left + w, top + 30.0 * scale)
}

pub(in crate::view) fn swatch(pill: Rect, scale: f32, row: usize, i: usize) -> Rect {
    let (l, t, r) = theme_column(pill, scale);
    let n = crate::ui::theme::PANEL_SWATCHES.len() as f32;
    let gap = 5.0 * scale;
    let d = ((r - l) - (n - 1.0) * gap) / n;
    let left = l + i as f32 * (d + gap);
    let top = t + (166.0 + row as f32 * 52.0) * scale;
    (left, top, left + d, top + d)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ThemeHit {
    Preset(usize),
    Panel(usize),
    Accent(usize),
}

pub fn hit_theme(pill: Rect, scale: f32, px: f32, py: f32) -> Option<ThemeHit> {
    if let Some(i) = (0..crate::ui::theme::PRESETS.len()).find(|&i| inside(preset_tile(pill, scale, i), px, py)) {
        return Some(ThemeHit::Preset(i));
    }
    let n = crate::ui::theme::PANEL_SWATCHES.len();
    for i in 0..n {
        if inside(swatch(pill, scale, 0, i), px, py) {
            return Some(ThemeHit::Panel(i));
        }
        if inside(swatch(pill, scale, 1, i), px, py) {
            return Some(ThemeHit::Accent(i));
        }
    }
    None
}

pub(in crate::view) fn paint_theme(p: &Painter, pill: Rect, scale: f32, ui: &UiState) -> Result<()> {
    paint_rows(p, pill, scale, ui, &THEME_ROWS)?;
    let (l, t, r) = theme_column(pill, scale);
    let (panel, accent) = crate::ui::theme::colors(&ui.cfg);
    let preview = (l, t, r, t + 58.0 * scale);
    p.fill_round_rect(preview, 29.0 * scale, rgba(panel.0, panel.1, panel.2, 1.0))?;
    p.stroke_round_rect(preview, 29.0 * scale, 1.0 * scale, rgba(1.0, 1.0, 1.0, 0.12))?;
    p.fill_ellipse(
        l + 28.0 * scale,
        t + 29.0 * scale,
        7.0 * scale,
        7.0 * scale,
        rgba(accent.0, accent.1, accent.2, 1.0),
    )?;
    p.text(
        "12:34",
        (l, t, r, t + 58.0 * scale),
        17.0 * scale,
        DWRITE_FONT_WEIGHT_SEMI_BOLD,
        DWRITE_TEXT_ALIGNMENT_CENTER,
        WHITE,
    )?;
    for k in 0..4 {
        let h = (8.0 + k as f32 * 4.0) * scale;
        let x = r - 56.0 * scale + k as f32 * 9.0 * scale;
        p.fill_round_rect(
            (x, t + 29.0 * scale - h / 2.0, x + 4.0 * scale, t + 29.0 * scale + h / 2.0),
            2.0 * scale,
            rgba(accent.0, accent.1, accent.2, 1.0),
        )?;
    }
    p.text(
        "Presets",
        (l, t + 58.0 * scale, r, t + 74.0 * scale),
        11.5 * scale,
        DWRITE_FONT_WEIGHT_NORMAL,
        DWRITE_TEXT_ALIGNMENT_LEADING,
        rgba(1.0, 1.0, 1.0, 0.55),
    )?;
    for (i, (name, _)) in crate::ui::theme::PRESETS.iter().enumerate() {
        let tile = preset_tile(pill, scale, i);
        let (pc, ac) = crate::ui::theme::preset_colors(i);
        let hover = ui.home.hover_act == Some(800 + i);
        p.fill_round_rect(tile, 10.0 * scale, rgba(pc.0, pc.1, pc.2, 1.0))?;
        p.stroke_round_rect(
            tile,
            10.0 * scale,
            if hover { 1.6 } else { 1.0 } * scale,
            rgba(1.0, 1.0, 1.0, if hover { 0.5 } else { 0.16 }),
        )?;
        p.fill_ellipse(
            tile.0 + 11.0 * scale,
            (tile.1 + tile.3) / 2.0,
            4.5 * scale,
            4.5 * scale,
            rgba(ac.0, ac.1, ac.2, 1.0),
        )?;
        p.text(
            name,
            (tile.0 + 20.0 * scale, tile.1, tile.2 - 2.0 * scale, tile.3),
            10.5 * scale,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            WHITE,
        )?;
    }
    let near = |a: f32, b: f32, eps: f32| (a - b).abs() <= eps;
    for (row, label, swatches) in
        [(0usize, "Panel", &crate::ui::theme::PANEL_SWATCHES), (1, "Accent", &crate::ui::theme::ACCENT_SWATCHES)]
    {
        let first = swatch(pill, scale, row, 0);
        p.text(
            label,
            (l, first.1 - 20.0 * scale, r, first.1 - 4.0 * scale),
            11.5 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            rgba(1.0, 1.0, 1.0, 0.55),
        )?;
        for (i, v) in swatches.iter().enumerate() {
            let rect = swatch(pill, scale, row, i);
            let (cr, cg, cb) = crate::ui::theme::hsv(v[0], v[1], v[2]);
            let hover = ui.home.hover_act == Some(820 + row * 20 + i);
            p.fill_round_rect(rect, (rect.2 - rect.0) / 2.0, rgba(cr, cg, cb, 1.0))?;
            let current = if row == 0 {
                near(ui.cfg.panel_hue, v[0], 1.0)
                    && near(ui.cfg.panel_sat, v[1], 0.03)
                    && near(ui.cfg.panel_val, v[2], 0.01)
            } else {
                near(ui.cfg.accent_hue, v[0], 1.0)
                    && near(ui.cfg.accent_sat, v[1], 0.03)
                    && near(ui.cfg.accent_val, v[2], 0.03)
            };
            let ring =
                if current { rgba(1.0, 1.0, 1.0, 0.95) } else { rgba(1.0, 1.0, 1.0, if hover { 0.6 } else { 0.22 }) };
            p.stroke_round_rect(rect, (rect.2 - rect.0) / 2.0, if current { 2.0 } else { 1.0 } * scale, ring)?;
        }
    }
    Ok(())
}

pub(in crate::view) fn track_color(id: Sid, t: f32, c: &Settings) -> Option<(f32, f32, f32)> {
    use crate::ui::theme::hsv;
    Some(match id {
        Sid::PanelHue => hsv(t * 360.0, c.panel_sat.max(0.7), 0.55),
        Sid::PanelSat => hsv(c.panel_hue, t, 0.4),
        Sid::PanelVal => hsv(c.panel_hue, c.panel_sat.max(0.5), t * 0.6),
        Sid::AccentHue => hsv(t * 360.0, c.accent_sat.max(0.5), 1.0),
        Sid::AccentSat => hsv(c.accent_hue, t, c.accent_val),
        _ => return None,
    })
}

pub(in crate::view) fn paint_rows(
    p: &Painter,
    pill: Rect,
    scale: f32,
    ui: &UiState,
    rows: &[(Sid, &str, Kind)],
) -> Result<()> {
    for (i, (id, label, kind)) in rows.iter().enumerate() {
        let (row, ctl) = setting_row(pill, scale, i);
        p.text(
            label,
            (row.0 + 2.0 * scale, row.1, row.0 + SET_LABEL_W * scale, row.3),
            13.0 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            rgba(1.0, 1.0, 1.0, 0.85),
        )?;
        paint_control(p, scale, &ui.cfg, *id, *kind, ctl)?;
    }
    Ok(())
}

pub(in crate::view) fn paint_control(
    p: &Painter,
    scale: f32,
    cfg: &Settings,
    id: Sid,
    kind: Kind,
    ctl: Rect,
) -> Result<()> {
    let accent = acc(1.0);
    let cy = (ctl.1 + ctl.3) / 2.0;
    match kind {
        Kind::Segment(opts) => {
            let selected = segment_value(id, cfg);
            p.fill_round_rect(ctl, 9.0 * scale, rgba(1.0, 1.0, 1.0, 0.07))?;
            let half = (ctl.2 - ctl.0) / 2.0;
            for (n, text) in opts.iter().enumerate() {
                let r = (ctl.0 + n as f32 * half, ctl.1, ctl.0 + (n + 1) as f32 * half, ctl.3);
                if n == selected {
                    p.fill_round_rect(
                        (r.0 + 2.0 * scale, r.1 + 2.0 * scale, r.2 - 2.0 * scale, r.3 - 2.0 * scale),
                        7.0 * scale,
                        acc(0.35),
                    )?;
                }
                let alpha = if n == selected { 1.0 } else { 0.6 };
                p.text(
                    text,
                    r,
                    12.0 * scale,
                    DWRITE_FONT_WEIGHT_SEMI_BOLD,
                    DWRITE_TEXT_ALIGNMENT_CENTER,
                    rgba(1.0, 1.0, 1.0, alpha),
                )?;
            }
        }
        Kind::Cycle => {
            p.fill_round_rect(ctl, 9.0 * scale, rgba(1.0, 1.0, 1.0, 0.07))?;
            let arrow = 26.0 * scale;
            p.glyph('\u{E76B}', (ctl.0, ctl.1, ctl.0 + arrow, ctl.3), 12.0 * scale, rgba(1.0, 1.0, 1.0, 0.75))?;
            p.glyph('\u{E76C}', (ctl.2 - arrow, ctl.1, ctl.2, ctl.3), 12.0 * scale, rgba(1.0, 1.0, 1.0, 0.75))?;
            p.text(
                cycle_name(id, cfg),
                (ctl.0 + arrow, ctl.1, ctl.2 - arrow, ctl.3),
                12.0 * scale,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_CENTER,
                WHITE,
            )?;
        }
        Kind::Slider => {
            let (tl, tr) = (ctl.0 + 4.0 * scale, ctl.2 - 54.0 * scale);
            let (min, max, _) = slider_range(id);
            let v = slider_value(id, cfg);
            let norm = ((v - min) / (max - min)).clamp(0.0, 1.0);
            let x = tl + (tr - tl) * norm;
            if track_color(id, 0.0, cfg).is_some() {
                const SEGMENTS: usize = 32;
                let seg = (tr - tl) / SEGMENTS as f32;
                for k in 0..SEGMENTS {
                    let (r, g, b) = track_color(id, (k as f32 + 0.5) / SEGMENTS as f32, cfg).unwrap_or((0.5, 0.5, 0.5));
                    let (left, right) = (tl + k as f32 * seg, tl + (k + 1) as f32 * seg + 0.6);
                    if k == 0 || k == SEGMENTS - 1 {
                        p.fill_round_rect(
                            (left, cy - 4.0 * scale, right, cy + 4.0 * scale),
                            4.0 * scale,
                            rgba(r, g, b, 1.0),
                        )?;
                    } else {
                        p.fill_rect((left, cy - 4.0 * scale, right, cy + 4.0 * scale), rgba(r, g, b, 1.0))?;
                    }
                }
            } else {
                p.fill_round_rect(
                    (tl, cy - 2.0 * scale, tr, cy + 2.0 * scale),
                    2.0 * scale,
                    rgba(1.0, 1.0, 1.0, 0.18),
                )?;
                p.fill_round_rect((tl, cy - 2.0 * scale, x, cy + 2.0 * scale), 2.0 * scale, accent)?;
            }
            p.fill_round_rect(
                (x - 7.0 * scale, cy - 7.0 * scale, x + 7.0 * scale, cy + 7.0 * scale),
                7.0 * scale,
                WHITE,
            )?;
            p.text(
                &slider_text(id, v),
                (tr + 10.0 * scale, ctl.1, ctl.2, ctl.3),
                12.0 * scale,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_TEXT_ALIGNMENT_TRAILING,
                rgba(1.0, 1.0, 1.0, 0.7),
            )?;
        }
        Kind::Button => {
            p.fill_round_rect(ctl, 9.0 * scale, rgba(1.0, 1.0, 1.0, 0.07))?;
            p.text(
                "Open",
                (ctl.0, ctl.1, ctl.2 - 26.0 * scale, ctl.3),
                12.0 * scale,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_CENTER,
                WHITE,
            )?;
            p.glyph('\u{E76C}', (ctl.2 - 26.0 * scale, ctl.1, ctl.2, ctl.3), 12.0 * scale, rgba(1.0, 1.0, 1.0, 0.75))?;
        }
        Kind::Toggle => {
            let on = toggle_value(id, cfg);
            let (w, h) = (40.0 * scale, 22.0 * scale);
            let track = (ctl.2 - w, cy - h / 2.0, ctl.2, cy + h / 2.0);
            p.fill_round_rect(track, h / 2.0, if on { acc(0.9) } else { rgba(1.0, 1.0, 1.0, 0.16) })?;
            let knob = 16.0 * scale;
            let kx = if on { track.2 - knob - 3.0 * scale } else { track.0 + 3.0 * scale };
            p.fill_round_rect((kx, cy - knob / 2.0, kx + knob, cy + knob / 2.0), knob / 2.0, WHITE)?;
        }
    }
    Ok(())
}
