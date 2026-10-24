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
    let (x, y, _, h) = pill;
    let size = 7.0 * scale;
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
    let (x, y, _, h) = pill;
    let any = ui.privacy.mic || ui.mic_muted == Some(true) || ui.privacy.cam;
    let left = x + (15.0 + ui.char_inset) * scale + if any { 12.0 * scale } else { 0.0 };
    let size = 14.0 * scale;
    Some((left, y + h / 2.0 - size / 2.0, left + size, y + h / 2.0 + size / 2.0))
}

pub fn hit_mic_dot(pill: Rect, scale: f32, ui: &UiState, px: f32, py: f32) -> bool {
    indicators(pill, scale, ui).0.is_some_and(|(l, t, r, b)| {
        let pad = 8.0 * scale;
        px >= l - pad && px < r + pad && py >= t - pad && py < b + pad
    })
}

pub(in crate::view) fn paint_indicators(p: &Painter, pill: Rect, scale: f32, ui: &UiState) -> Result<()> {
    let (mic, cam) = indicators(pill, scale, ui);
    if let Some(r) = mic {
        let color = if ui.mic_muted == Some(true) { rgba(1.0, 0.3, 0.3, 1.0) } else { rgba(1.0, 0.62, 0.2, 1.0) };
        if ui.hover_head == Some(HeadHit::Mic) {
            let grow = 2.5 * scale;
            p.fill_round_rect(
                (r.0 - grow, r.1 - grow, r.2 + grow, r.3 + grow),
                3.5 * scale + grow,
                rgba(1.0, 1.0, 1.0, 0.18),
            )?;
        }
        p.fill_round_rect(r, 3.5 * scale, color)?;
    }
    if let Some(r) = cam {
        p.fill_round_rect(r, 3.5 * scale, rgba(0.3, 0.85, 0.45, 1.0))?;
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
    ui.cfg.music_lyrics && ui.lyric.lines.is_some()
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

fn agent_label(ui: &UiState) -> (String, windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F) {
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
    let cover = (music && cover_visible(ui)).then_some(COVER_SLOT);
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

pub fn paint_collapsed(p: &Painter, pill: Rect, scale: f32, ui: &UiState) -> Result<()> {
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
            weather_icon(p, cx, y + h / 2.0, 10.5 * scale, wx.sky, wx.night)?;
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
            let side = width.min(h - 10.0 * scale);
            let top = y + (h - side) / 2.0;
            p.image((x + off, top, x + off + side, top + side), 6.0 * scale, cover.size, &cover.bgra)?;
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
