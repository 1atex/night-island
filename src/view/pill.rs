use crate::view::*;

pub struct Clock {
    pub hh_mm: String,
    pub date: String,
    pub ms_to_next_minute: u32,
}

pub fn clock() -> Clock {
    let t = unsafe { GetLocalTime() };
    Clock {
        hh_mm: format!("{:02}:{:02}", t.wHour, t.wMinute),
        date: format!("{:02}.{:02}.{}", t.wDay, t.wMonth, t.wYear),
        ms_to_next_minute: (60 - t.wSecond as u32) * 1000 - t.wMilliseconds as u32 + 50,
    }
}

pub fn battery() -> Option<(u8, bool)> {
    let mut s = SYSTEM_POWER_STATUS::default();
    unsafe { GetSystemPowerStatus(&mut s).ok()? };
    const NO_BATTERY: u8 = 128;
    const UNKNOWN: u8 = 255;
    if s.BatteryFlag & NO_BATTERY != 0 || s.BatteryLifePercent == UNKNOWN {
        return None;
    }
    Some((s.BatteryLifePercent, s.ACLineStatus == 1))
}

pub(in crate::view) fn paint_battery(
    p: &Painter,
    cx: f32,
    cy: f32,
    scale: f32,
    percent: u8,
    charging: bool,
) -> Result<()> {
    let (bw, bh) = (22.0 * scale, 11.0 * scale);
    let (x, y) = (cx - bw / 2.0, cy - bh / 2.0);
    let (r, g, b) = if charging {
        GREEN
    } else if percent <= 15 {
        RED
    } else {
        (1.0, 1.0, 1.0)
    };
    p.stroke_round_rect((x, y, x + bw, y + bh), 3.0 * scale, 1.2 * scale, rgba(1.0, 1.0, 1.0, 0.7))?;
    p.fill_round_rect(
        (x + bw + 1.5 * scale, cy - 2.5 * scale, x + bw + 3.5 * scale, cy + 2.5 * scale),
        1.0 * scale,
        rgba(1.0, 1.0, 1.0, 0.7),
    )?;
    let inset = 2.0 * scale;
    let fill_w = ((bw - 2.0 * inset) * percent as f32 / 100.0).max(1.5 * scale);
    p.fill_round_rect((x + inset, y + inset, x + inset + fill_w, y + bh - inset), 1.5 * scale, rgba(r, g, b, 1.0))
}

pub fn indicators(pill: Rect, scale: f32, ui: &UiState) -> (Option<Rect>, Option<Rect>) {
    let (x, y, w, h) = pill;
    let size = 7.0 * scale;
    if ui.cfg.vertical() {
        let left = x + (w - size) / 2.0;
        let mut top = y + (16.0 + ui.char_inset) * scale;
        let mut next = |on: bool| {
            on.then(|| {
                let r = (left, top, left + size, top + size);
                top += 15.0 * scale;
                r
            })
        };
        let mic = next(ui.privacy.mic || ui.mic_muted == Some(true));
        let cam = next(ui.privacy.cam);
        return (mic, cam);
    }
    let gap = 2.5 * scale;
    let left = x + (15.0 + ui.char_inset) * scale;
    let mic_on = ui.privacy.mic || ui.mic_muted == Some(true);
    let cam_on = ui.privacy.cam;
    let both = mic_on && cam_on;
    let mid = y + h / 2.0;
    let at = |top: f32| (left, top, left + size, top + size);
    let mic = mic_on.then(|| at(if both { mid - gap / 2.0 - size } else { mid - size / 2.0 }));
    let cam = cam_on.then(|| at(if both { mid + gap / 2.0 } else { mid - size / 2.0 }));
    (mic, cam)
}

pub fn indicators_width(ui: &UiState, scale: f32) -> f32 {
    let any = ui.privacy.mic || ui.mic_muted == Some(true) || ui.privacy.cam;
    (if any { 12.0 } else { 0.0 } + if vpn_visible(ui) { 19.0 } else { 0.0 }) * scale
}

pub fn vpn_visible(ui: &UiState) -> bool {
    ui.cfg.vpn_pill && (ui.vpn == Some(true) || ui.vpn_busy)
}

pub fn vpn_button(ui: &UiState) -> bool {
    ui.cfg.vpn_pill && ui.vpn.is_some()
}

pub const VPN_GLYPH: char = '\u{EA18}';

pub fn vpn_color(ui: &UiState) -> windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F {
    match (ui.vpn, ui.vpn_busy) {
        (_, true) => rgba(1.0, 0.75, 0.3, 1.0),
        (Some(true), _) => rgba(0.4, 0.9, 0.55, 1.0),
        _ => rgba(1.0, 1.0, 1.0, 0.45),
    }
}

pub fn vpn_rect(pill: Rect, scale: f32, ui: &UiState) -> Option<Rect> {
    if !vpn_visible(ui) {
        return None;
    }
    let (x, y, w, h) = pill;
    let any = ui.privacy.mic || ui.mic_muted == Some(true) || ui.privacy.cam;
    let size = 14.0 * scale;
    if ui.cfg.vertical() {
        let dots = usize::from(ui.privacy.mic || ui.mic_muted == Some(true)) + usize::from(ui.privacy.cam);
        let top = y + (16.0 + ui.char_inset + dots as f32 * 15.0) * scale;
        return Some((x + (w - size) / 2.0, top, x + (w + size) / 2.0, top + size));
    }
    let left = x + (15.0 + ui.char_inset) * scale + if any { 12.0 * scale } else { 0.0 };
    Some((left, y + h / 2.0 - size / 2.0, left + size, y + h / 2.0 + size / 2.0))
}

pub fn hit_mic_dot(pill: Rect, scale: f32, ui: &UiState, px: f32, py: f32) -> bool {
    indicators(pill, scale, ui).0.is_some_and(|(l, t, r, b)| {
        let pad = 8.0 * scale;
        px >= l - pad && px < r + pad && py >= t - pad && py < b + pad
    })
}

pub(in crate::view) fn mic_look(ui: &UiState) -> (char, windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F) {
    if ui.mic_muted == Some(true) {
        ('\u{F781}', rgba(1.0, 0.3, 0.3, 1.0))
    } else {
        ('\u{E720}', rgba(1.0, 0.62, 0.2, 1.0))
    }
}

pub(in crate::view) fn paint_indicators(p: &Painter, pill: Rect, scale: f32, ui: &UiState) -> Result<()> {
    let (mic, cam) = indicators(pill, scale, ui);
    if let Some(r) = mic {
        let (glyph, color) = mic_look(ui);
        if ui.hover_head == Some(HeadHit::Mic) {
            let grow = 2.5 * scale;
            p.fill_round_rect(
                (r.0 - grow, r.1 - grow, r.2 + grow, r.3 + grow),
                3.5 * scale + grow,
                rgba(1.0, 1.0, 1.0, 0.18),
            )?;
        }
        let (cx, cy) = ((r.0 + r.2) / 2.0, (r.1 + r.3) / 2.0);
        let half = 6.0 * scale;
        p.glyph(glyph, (cx - half, cy - half, cx + half, cy + half), 11.0 * scale, color)?;
    }
    if let Some(r) = cam {
        let (cx, cy) = ((r.0 + r.2) / 2.0, (r.1 + r.3) / 2.0);
        let half = 6.0 * scale;
        p.glyph('\u{E714}', (cx - half, cy - half, cx + half, cy + half), 11.0 * scale, rgba(0.3, 0.85, 0.45, 1.0))?;
    }
    if let Some(r) = vpn_rect(pill, scale, ui) {
        p.glyph(VPN_GLYPH, r, 12.0 * scale, vpn_color(ui))?;
    }
    Ok(())
}

#[derive(Default)]
pub struct LyricUi {
    pub key: Option<(String, String)>,
    pub lines: Option<crate::services::lyrics::Lines>,
    pub idx: Option<usize>,
    pub shown: [String; 5],
    pub slide: f32,
    pub slide_start: Option<std::time::Instant>,
    pub width: f32,
}

pub fn lyric_visible(ui: &UiState) -> bool {
    ui.cfg.music_lyrics && ui.lyric.lines.is_some() && !ui.cfg.vertical()
}

pub fn cover_aspect(ui: &UiState) -> f32 {
    match (&ui.media.cover, ui.cfg.cover_mode) {
        (Some(c), crate::data::settings::CoverMode::Fit) => {
            (c.full_w.max(1) as f32 / c.full_h.max(1) as f32).clamp(0.5, 1.8)
        }
        _ => 1.0,
    }
}

pub fn cover_visible(ui: &UiState) -> bool {
    ui.cfg.music_cover && ui.media.cover.is_some()
}

pub fn music_pill(ui: &UiState) -> bool {
    ui.media.playing
        && !ui.media.title.is_empty()
        && (ui.cfg.music_eq || ui.cfg.music_title || cover_visible(ui) || lyric_visible(ui))
        && ui.notice.is_none()
        && ui.volume.is_none()
        && ui.downloading.is_none()
}

pub fn row_pill(ui: &UiState) -> bool {
    music_pill(ui)
        || ((ui.recording_secs.is_some() || pill_metrics_on(&ui.cfg) || agents_active(ui))
            && ui.notice.is_none()
            && ui.volume.is_none()
            && ui.downloading.is_none())
}

pub(in crate::view) fn rec_slot(secs: u32) -> f32 {
    if secs >= 3600 { 76.0 } else { 58.0 }
}

pub(in crate::view) fn rec_time(secs: u32) -> String {
    if secs >= 3600 {
        format!("{}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60)
    } else {
        format!("{:02}:{:02}", secs / 60, secs % 60)
    }
}

pub(in crate::view) const COVER_SLOT: f32 = 26.0;

pub(in crate::view) const WEATHER_SLOT: f32 = 62.0;

pub(in crate::view) const BARS_SLOT: f32 = 63.0;

pub(in crate::view) const TITLE_SLOT: f32 = 124.0;

pub(in crate::view) const LYRIC_SLOT: f32 = 176.0;

pub const LYRIC_SLOT_MAX: f32 = 320.0;

pub const LYRIC_PITCH: f32 = 19.0;

pub fn agents_active(ui: &UiState) -> bool {
    use crate::services::agents::State;
    ui.agents.iter().any(|s| matches!(s.state, State::Working | State::Waiting))
}

pub(in crate::view) fn agent_label(ui: &UiState) -> (String, windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F) {
    use crate::services::agents::State;
    let active: Vec<_> = ui.agents.iter().filter(|s| matches!(s.state, State::Working | State::Waiting)).collect();
    let waiting = active.iter().any(|s| s.state == State::Waiting);
    let color = if waiting { rgba(1.0, 0.75, 0.3, 1.0) } else { rgba(0.45, 0.8, 1.0, 1.0) };
    let text = match active.as_slice() {
        [one] => one.agent.short().to_string(),
        many => format!("×{}", many.len()),
    };
    (if waiting { format!("{text} !") } else { text }, color)
}

pub(in crate::view) const AGENT_SLOT: f32 = 78.0;

pub(in crate::view) const METRICS_CHAR_W: f32 = 6.6;

pub(in crate::view) const CLOCK_SLOT: f32 = 54.0;

pub(in crate::view) const SLOT_GAP: f32 = 8.0;

pub(in crate::view) fn battery_slot(ui: &UiState) -> Option<f32> {
    battery()?;
    match ui.cfg.pill_battery {
        BatteryStyle::Hidden => None,
        BatteryStyle::Icon => Some(30.0),
        BatteryStyle::Percent => Some(78.0),
    }
}

pub struct MusicSlots {
    pub cover: Option<(f32, f32)>,
    pub weather: Option<(f32, f32)>,
    pub bars: Option<(f32, f32)>,
    pub title: Option<(f32, f32)>,
    pub lyric: Option<(f32, f32)>,
    pub rec: Option<(f32, f32)>,
    pub metrics: Option<(f32, f32)>,
    pub agent: Option<(f32, f32)>,
    pub clock: (f32, f32),
    pub battery: Option<(f32, f32)>,
    pub width: f32,
}

pub fn music_slots(ui: &UiState, scale: f32, pill_w: f32) -> MusicSlots {
    let shift = ui.char_inset + indicators_width(ui, 1.0);
    let weather = (ui.cfg.pill_weather && ui.weather.is_some()).then_some(WEATHER_SLOT);
    let music = music_pill(ui);
    let cover = (music && cover_visible(ui)).then_some(COVER_SLOT * cover_aspect(ui));
    let bars = (music && ui.cfg.music_eq).then_some(BARS_SLOT);
    let title = (music && ui.cfg.music_title).then_some(TITLE_SLOT);
    let lyric = (music && lyric_visible(ui)).then_some(ui.lyric.width.max(LYRIC_SLOT));
    let rec = ui.recording_secs.map(rec_slot);
    let agent = agents_active(ui).then_some(AGENT_SLOT);
    let metrics = pill_metrics_on(&ui.cfg).then(|| pill_chars(&ui.cfg.metrics_pill) as f32 * METRICS_CHAR_W + 4.0);
    let battery = battery_slot(ui);
    let widths = [cover, weather, bars, title, lyric, rec, agent, metrics, Some(CLOCK_SLOT), battery];
    let count = widths.iter().flatten().count() as f32;
    let content: f32 = widths.iter().flatten().sum::<f32>() + SLOT_GAP * (count - 1.0).max(0.0);
    let width = (32.0 + shift + content).max(200.0);
    let total = if pill_w > 0.0 { pill_w / scale } else { width };
    let mut x = 16.0 + shift + (total - 32.0 - shift - content).max(0.0) / 2.0;
    let mut place = |w: Option<f32>| {
        w.map(|w| {
            let slot = (x * scale, w * scale);
            x += w + SLOT_GAP;
            slot
        })
    };
    let cover = place(cover);
    let weather = place(weather);
    let bars = place(bars);
    let title = place(title);
    let lyric = place(lyric);
    let rec = place(rec);
    let agent = place(agent);
    let metrics = place(metrics);
    let clock = place(Some(CLOCK_SLOT)).unwrap_or((0.0, 0.0));
    let battery = place(battery);
    MusicSlots { cover, weather, bars, title, lyric, rec, agent, metrics, clock, battery, width }
}

pub(in crate::view) fn downloading_label(name: &str) -> String {
    let shown = name.trim_end_matches(|c: char| c != '.').trim_end_matches('.');
    format!("Downloading {}", if shown.is_empty() { name } else { shown })
}

pub fn collapsed_width(ui: &UiState, base: f32) -> f32 {
    if ui.cfg.vertical() {
        return vertical_length(ui);
    }
    let chars = |s: &str| crate::ui::i18n::tr(s).chars().count() as f32;
    if let Some((_, text)) = &ui.notice {
        return (ui.char_inset + 40.0 + chars(text) * 7.6 + 20.0).clamp(base, 460.0);
    }
    if ui.volume.is_some() {
        return base;
    }
    if let Some(name) = &ui.downloading {
        let shift = ui.char_inset + indicators_width(ui, 1.0);
        return (shift + 40.0 + chars(&downloading_label(name)) * 6.9 + 18.0).clamp(base, 420.0);
    }
    if row_pill(ui) {
        return music_slots(ui, 1.0, 0.0).width.max(base);
    }
    idle_width(ui).max(base)
}

pub(in crate::view) const IDLE_CLOCK_HALF: f32 = 24.0;

pub(in crate::view) fn idle_width(ui: &UiState) -> f32 {
    let shift = ui.char_inset + indicators_width(ui, 1.0);
    let left = if ui.cfg.pill_weather && ui.weather.is_some() { 68.0 + shift } else { 14.0 + shift };
    let right = match battery().map(|_| ui.cfg.pill_battery) {
        Some(BatteryStyle::Percent) => 75.0,
        Some(BatteryStyle::Icon) => 46.0,
        _ => 14.0,
    };
    2.0 * (left.max(right) + IDLE_CLOCK_HALF)
}

fn paint_lyrics(p: &Painter, slot: Rect, scale: f32, lyric: &LyricUi) -> Result<()> {
    let (l, t, r, b) = slot;
    let (mid, pitch) = ((t + b) / 2.0, LYRIC_PITCH * scale);
    p.push_clip(slot);
    let result = (|| {
        for (i, line) in lyric.shown.iter().enumerate() {
            if line.is_empty() {
                continue;
            }
            let pos = (i as f32 - 2.0) + lyric.slide;
            let d = pos.abs();
            let alpha = if d <= 1.0 { 1.0 - 0.72 * d } else { 0.28 * (2.0 - d).max(0.0) };
            if alpha <= 0.01 {
                continue;
            }
            let cy = mid + pos * pitch;
            p.text(
                line,
                (l, cy - pitch / 2.0, r, cy + pitch / 2.0),
                12.5 * scale,
                if d < 0.5 { DWRITE_FONT_WEIGHT_SEMI_BOLD } else { DWRITE_FONT_WEIGHT_NORMAL },
                DWRITE_TEXT_ALIGNMENT_LEADING,
                rgba(1.0, 1.0, 1.0, alpha),
            )?;
        }
        Ok(())
    })();
    p.pop_clip();
    result
}

pub fn progress_strip_on(ui: &UiState) -> bool {
    ui.cfg.music_progress && music_pill(ui) && crate::platform::media::progress().is_some()
}

pub(in crate::view) fn paint_progress_strip(p: &Painter, pill: Rect, scale: f32, ui: &UiState) -> Result<()> {
    let Some((pos, len)) = crate::platform::media::progress().filter(|_| progress_strip_on(ui)) else { return Ok(()) };
    if ui.cfg.progress_style != crate::data::settings::ProgressStyle::Strip {
        return paint_progress_outline(p, pill, scale, ui, (pos / len).clamp(0.0, 1.0));
    }
    let (x, y, w, h) = pill;
    let thick = 3.0 * scale;
    let r = h / 2.0;
    let rise = r - thick;
    let inset = (r - (r * r - rise * rise).max(0.0).sqrt()).max(0.0) + 1.0 * scale;
    let (left, right) = (x + inset, x + w - inset);
    let top = y + h - thick - 0.5 * scale;
    p.fill_round_rect((left, top, right, top + thick), thick / 2.0, rgba(1.0, 1.0, 1.0, 0.1))?;
    let filled = left + (right - left) * (pos / len).clamp(0.0, 1.0);
    let base = ui.media.accent.unwrap_or_else(crate::ui::theme::accent_u8);
    const STEPS: usize = 40;
    let step = (right - left) / STEPS as f32;
    for i in 0..STEPS {
        let a = left + i as f32 * step;
        if a >= filled {
            break;
        }
        let (r, g, b) = crate::platform::media::gradient(base, i as f32 / (STEPS - 1) as f32);
        let c = rgba(f32::from(r) / 255.0, f32::from(g) / 255.0, f32::from(b) / 255.0, 1.0);
        p.fill_rect((a, top, (a + step + 0.5).min(filled), top + thick), c)?;
    }
    Ok(())
}

fn paint_progress_outline(p: &Painter, pill: Rect, scale: f32, ui: &UiState, fraction: f32) -> Result<()> {
    use crate::data::settings::ProgressStyle;
    let (x, y, w, h) = pill;
    let inset = 1.8 * scale;
    let r = h / 2.0 - inset;
    let straight = (w - h).max(0.0);
    let arc = std::f32::consts::PI * r;
    let perimeter = 2.0 * straight + 2.0 * arc;
    let (cx, cy) = (x + w / 2.0, y + h / 2.0);
    let half = straight / 2.0;
    let point = |at: f32| -> (f32, f32) {
        let at = at.rem_euclid(perimeter);
        if at < half {
            (cx + at, cy + r)
        } else if at < half + arc {
            let a = (at - half) / r;
            (cx + half + r * a.sin(), cy + r * a.cos())
        } else if at < half + arc + straight {
            (cx + half - (at - half - arc), cy - r)
        } else if at < half + 2.0 * arc + straight {
            let a = std::f32::consts::PI + (at - half - arc - straight) / r;
            (cx - half + r * a.sin(), cy + r * a.cos())
        } else {
            (cx - half + (at - half - 2.0 * arc - straight), cy + r)
        }
    };
    let base = ui.media.accent.unwrap_or_else(crate::ui::theme::accent_u8);
    let tone = |t: f32| {
        let (r, g, b) = crate::platform::media::gradient(base, t.clamp(0.0, 1.0));
        rgba(f32::from(r) / 255.0, f32::from(g) / 255.0, f32::from(b) / 255.0, 1.0)
    };
    let thick = 2.6 * scale;
    let steps = ((perimeter / (2.0 * scale)) as usize).clamp(64, 600);
    let faint = rgba(1.0, 1.0, 1.0, 0.1);
    let track = (x + inset, y + inset, x + w - inset, y + h - inset);
    p.stroke_round_rect(track, r, thick, faint)?;
    let left_middle = half + arc + straight + arc / 2.0;
    let bottom_start = half + 2.0 * arc + straight;
    for i in 0..steps {
        let (s0, s1) = (perimeter * i as f32 / steps as f32, perimeter * (i + 1) as f32 / steps as f32);
        let mid = (s0 + s1) / 2.0;
        let colour = match ui.cfg.progress_style {
            ProgressStyle::OutlineFromLeft => {
                let away = (mid - left_middle).rem_euclid(perimeter);
                let t = away.min(perimeter - away) / (perimeter / 2.0);
                (t <= fraction).then(|| tone(t))
            }
            _ => {
                let along = (mid + perimeter - bottom_start).rem_euclid(perimeter);
                let on_bottom = straight > 1.0 && along < straight;
                let t = ((point(mid).0 - x) / w.max(1.0)).clamp(0.0, 1.0);
                (!on_bottom || along / straight < fraction).then(|| tone(t))
            }
        };
        let Some(colour) = colour else { continue };
        let (a, b) = (point(s0), point(s1 + 0.5 * scale));
        p.line(a, b, thick, colour)?;
        p.fill_ellipse(b.0, b.1, thick / 2.0, thick / 2.0, colour)?;
    }
    Ok(())
}

pub fn pill_weather_visible(ui: &UiState) -> bool {
    ui.cfg.pill_weather
        && ui.weather.is_some()
        && ui.notice.is_none()
        && ui.volume.is_none()
        && ui.downloading.is_none()
}

pub fn pill_weather_rect(pill: Rect, scale: f32, ui: &UiState) -> Option<Rect> {
    if !pill_weather_visible(ui) {
        return None;
    }
    let (x, y, w, h) = pill;
    let (cx, cy, r) = if ui.cfg.vertical() {
        let (off, _) = vertical_slots(ui, scale, h).weather?;
        (x + w / 2.0, y + off + 10.0 * scale, 10.0 * scale)
    } else {
        let cx = if row_pill(ui) {
            x + music_slots(ui, scale, w).weather?.0 + 10.0 * scale
        } else {
            x + (24.0 + ui.char_inset) * scale + indicators_width(ui, scale)
        };
        (cx, y + h / 2.0, 10.5 * scale)
    };
    Some((cx - r, cy - r, cx + r, cy + r))
}

pub fn paint_pill_weather_icon(p: &Painter, pill: Rect, scale: f32, ui: &UiState) -> Result<()> {
    if let (Some((left, top, right, bottom)), Some(wx)) = (pill_weather_rect(pill, scale, ui), &ui.weather) {
        animated_weather_icon(p, (left + right) / 2.0, (top + bottom) / 2.0, (right - left) / 2.0, wx.sky, wx.night)?;
    }
    Ok(())
}

pub fn paint_collapsed(p: &Painter, pill: Rect, scale: f32, ui: &UiState) -> Result<()> {
    if ui.cfg.vertical() {
        return paint_collapsed_vertical(p, pill, scale, ui);
    }
    paint_collapsed_body(p, pill, scale, ui)?;
    paint_progress_strip(p, pill, scale, ui)
}

fn paint_collapsed_body(p: &Painter, pill: Rect, scale: f32, ui: &UiState) -> Result<()> {
    let (x, y, w, h) = pill;
    let inset = ui.char_inset * scale;
    if let Some((glyph, text)) = &ui.notice {
        p.glyph(*glyph, (x + 10.0 * scale + inset, y, x + 40.0 * scale + inset, y + h), 16.0 * scale, WHITE)?;
        return p.text(
            text,
            (x + 40.0 * scale + inset, y, x + w - 10.0 * scale, y + h),
            13.0 * scale,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            WHITE,
        );
    }
    paint_indicators(p, pill, scale, ui)?;
    if let Some(level) = ui.volume {
        let icon = if level <= 0.001 { '\u{E74F}' } else { '\u{E767}' };
        p.glyph(icon, (x + 10.0 * scale + inset, y, x + 40.0 * scale + inset, y + h), 16.0 * scale, WHITE)?;
        let (tl, tr) = (x + 46.0 * scale + inset, x + w - 52.0 * scale);
        let cy = y + h / 2.0;
        p.fill_round_rect((tl, cy - 3.0 * scale, tr, cy + 3.0 * scale), 3.0 * scale, rgba(1.0, 1.0, 1.0, 0.22))?;
        p.fill_round_rect(
            (tl, cy - 3.0 * scale, tl + (tr - tl) * level.clamp(0.0, 1.0), cy + 3.0 * scale),
            3.0 * scale,
            WHITE,
        )?;
        return p.text(
            &format!("{}%", (level * 100.0).round() as u32),
            (tr + 4.0 * scale, y, x + w - 8.0 * scale, y + h),
            12.5 * scale,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_TRAILING,
            WHITE,
        );
    }
    if let Some(name) = &ui.downloading {
        let shift = indicators_width(ui, scale) + inset;
        p.glyph('\u{E896}', (x + 10.0 * scale + shift, y, x + 38.0 * scale + shift, y + h), 15.0 * scale, acc(1.0))?;
        let label = downloading_label(name);
        return p.text(
            &label,
            (x + 40.0 * scale + shift, y, x + w - 14.0 * scale, y + h),
            12.5 * scale,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            WHITE,
        );
    }
    let battery_info = battery();
    let paint_pill_battery = |left: f32, right: f32| -> Result<()> {
        let Some((percent, charging)) = battery_info else { return Ok(()) };
        match ui.cfg.pill_battery {
            BatteryStyle::Hidden => Ok(()),
            BatteryStyle::Icon => {
                paint_battery(p, (left + right) / 2.0 - 2.0 * scale, y + h / 2.0, scale, percent, charging)
            }
            BatteryStyle::Percent => {
                p.text(
                    &format!("{percent}%"),
                    (left, y, right, y + h),
                    12.0 * scale,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_TEXT_ALIGNMENT_TRAILING,
                    rgba(1.0, 1.0, 1.0, DIM),
                )?;
                paint_battery(p, left + 14.0 * scale, y + h / 2.0, scale, percent, charging)
            }
        }
    };
    let weather_at = |cx: f32| -> Result<()> {
        if let (true, Some(wx)) = (ui.cfg.pill_weather, &ui.weather) {
            paint_pill_weather_icon(p, pill, scale, ui)?;
            p.text(
                &format!("{}°", wx.temp.round() as i32),
                (cx + 14.0 * scale, y, cx + 48.0 * scale, y + h),
                12.5 * scale,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                rgba(1.0, 1.0, 1.0, 0.85),
            )?;
        }
        Ok(())
    };
    if row_pill(ui) {
        let slots = music_slots(ui, scale, w);
        if let (Some((off, width)), Some(cover)) = (slots.cover, &ui.media.cover) {
            let side = (COVER_SLOT * scale).min(h - 10.0 * scale);
            let top = y + (h - side) / 2.0;
            let wide = width.min(side * cover_aspect(ui));
            p.cover((x + off, top, x + off + wide, top + side), 6.0 * scale, cover, ui.cfg.cover_mode)?;
        }
        if let Some((off, _)) = slots.weather {
            weather_at(x + off + 10.0 * scale)?;
        }
        if let Some((off, width)) = slots.title {
            p.text(
                &ui.media.title,
                (x + off, y, x + off + width, y + h),
                12.5 * scale,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                WHITE,
            )?;
        }
        if let Some((off, width)) = slots.lyric {
            paint_lyrics(p, (x + off, y, x + off + width, y + h), scale, &ui.lyric)?;
        }
        if let (Some((off, width)), Some(secs)) = (slots.rec, ui.recording_secs) {
            let cy = y + h / 2.0;
            let alpha = if secs % 2 == 0 { 1.0 } else { 0.45 };
            let r = 4.5 * scale;
            let dot_cx = x + off + r + 1.0 * scale;
            p.fill_ellipse(dot_cx, cy, r * 1.8, r * 1.8, rgba(1.0, 0.27, 0.27, 0.18 * alpha))?;
            p.fill_ellipse(dot_cx, cy, r, r, rgba(1.0, 0.27, 0.27, alpha))?;
            p.text(
                &rec_time(secs),
                (dot_cx + r + 6.0 * scale, y, x + off + width, y + h),
                13.0 * scale,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                rgba(1.0, 0.55, 0.55, 1.0),
            )?;
        }
        if let Some((off, width)) = slots.agent {
            let (label, color) = agent_label(ui);
            let cy = y + h / 2.0;
            p.fill_ellipse(x + off + 5.0 * scale, cy, 8.0 * scale, 8.0 * scale, rgba(color.r, color.g, color.b, 0.2))?;
            p.fill_ellipse(x + off + 5.0 * scale, cy, 4.0 * scale, 4.0 * scale, color)?;
            p.text(
                &label,
                (x + off + 15.0 * scale, y, x + off + width, y + h),
                12.0 * scale,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                color,
            )?;
        }
        if let Some((off, width)) = slots.metrics {
            p.text(
                &ui.metrics.pill,
                (x + off, y, x + off + width, y + h),
                11.5 * scale,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                rgba(1.0, 1.0, 1.0, 0.85),
            )?;
        }
        p.text(
            &clock().hh_mm,
            (x + slots.clock.0, y, x + slots.clock.0 + slots.clock.1, y + h),
            15.0 * scale,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_CENTER,
            WHITE,
        )?;
        if let Some((off, width)) = slots.battery {
            paint_pill_battery(x + off, x + off + width)?;
        }
        return Ok(());
    }
    p.text(
        &clock().hh_mm,
        (x, y, x + w, y + h),
        15.0 * scale,
        DWRITE_FONT_WEIGHT_SEMI_BOLD,
        DWRITE_TEXT_ALIGNMENT_CENTER,
        WHITE,
    )?;
    weather_at(x + 24.0 * scale + inset + indicators_width(ui, scale))?;
    match ui.cfg.pill_battery {
        BatteryStyle::Hidden => {}
        BatteryStyle::Icon => paint_pill_battery(x + w - 46.0 * scale, x + w - 16.0 * scale)?,
        BatteryStyle::Percent => paint_pill_battery(x + w - 16.0 * scale - 62.0 * scale, x + w - 16.0 * scale)?,
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::settings::Edge;
    use crate::services::weather::Weather;

    fn weather_ui() -> UiState {
        let mut ui = UiState::new();
        ui.cfg.weather = true;
        ui.cfg.pill_weather = true;
        ui.weather = Some(Weather {
            place: "Test".into(),
            temp: 12.0,
            wind: 2.0,
            humidity: 50.0,
            sky: Sky::Rain,
            night: false,
            hourly: Vec::new(),
            daily: Vec::new(),
            city_missing: false,
        });
        ui
    }

    #[test]
    fn weather_animation_regions_fit_both_pill_orientations_at_different_scales() {
        let mut ui = weather_ui();
        for scale in [1.0, 1.25, 2.0] {
            for edge in Edge::ALL {
                ui.cfg.placement = Placement::Docked;
                ui.cfg.dock_edge = edge;
                let (w, h) = if ui.cfg.vertical() { (42.0, 500.0) } else { (500.0, 42.0) };
                let pill = (17.25, 8.75, w * scale, h * scale);
                let rect = pill_weather_rect(pill, scale, &ui).unwrap();
                assert!(rect.0 >= pill.0 && rect.1 >= pill.1);
                assert!(rect.2 <= pill.0 + pill.2 && rect.3 <= pill.1 + pill.3);
                let expected = if ui.cfg.vertical() { 20.0 } else { 21.0 } * scale;
                assert!((rect.2 - rect.0 - expected).abs() < 0.001);
                assert!((rect.3 - rect.1 - expected).abs() < 0.001);
            }
        }
    }

    #[test]
    fn temporary_overlays_suspend_pill_weather_drawing() {
        let mut ui = weather_ui();
        let pill = (0.0, 0.0, 500.0, 500.0);
        assert!(pill_weather_rect(pill, 1.0, &ui).is_some());
        ui.notice = Some(('!', "Notice".into()));
        assert!(pill_weather_rect(pill, 1.0, &ui).is_none());
        ui.notice = None;
        ui.volume = Some(0.5);
        assert!(pill_weather_rect(pill, 1.0, &ui).is_none());
        ui.volume = None;
        ui.downloading = Some("file.zip".into());
        assert!(pill_weather_rect(pill, 1.0, &ui).is_none());
        ui.downloading = None;
        assert!(pill_weather_rect(pill, 1.0, &ui).is_some());
        ui.cfg.pill_weather = false;
        assert!(pill_weather_rect(pill, 1.0, &ui).is_none());
        ui.cfg.pill_weather = true;
        ui.weather = None;
        assert!(pill_weather_rect(pill, 1.0, &ui).is_none());
    }
}
