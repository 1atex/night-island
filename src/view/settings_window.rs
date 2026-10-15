use crate::view::*;

pub struct Group {
    pub glyph: char,
    pub name: &'static str,
    pub(in crate::view) rows: &'static [(Sid, &'static str, Kind)],
}

const GENERAL: [(Sid, &str, Kind); 9] = [
    (Sid::Language, "Language", Kind::Segment(["Русский", "English"])),
    (Sid::Weather, "Weather", Kind::Toggle),
    (Sid::Autostart, "Start with Windows", Kind::Toggle),
    (Sid::GameMode, "Game mode", Kind::Toggle),
    (Sid::AutoHide, "Auto-hide", Kind::Toggle),
    (Sid::Capture, "Hide in screenshots", Kind::Toggle),
    (Sid::CaptureSelf, "Hide island in my captures", Kind::Toggle),
    (Sid::AgyAuto, "AGY auto-approve", Kind::Toggle),
    (Sid::DevTools, "Developer tools", Kind::Toggle),
];

const ISLAND: [(Sid, &str, Kind); 9] = [
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

const PILL: [(Sid, &str, Kind); 7] = [
    (Sid::PillWeather, "Weather on the pill", Kind::Toggle),
    (Sid::PillBattery, "Battery on the pill", Kind::Cycle),
    (Sid::MusicEq, "Equalizer", Kind::Toggle),
    (Sid::MusicTitle, "Track title", Kind::Toggle),
    (Sid::MusicLyrics, "Synced lyrics", Kind::Toggle),
    (Sid::Spikes, "Edge spikes", Kind::Toggle),
    (Sid::SpikeColor, "Spike tips in track colour", Kind::Toggle),
];

const NOTIFICATIONS: [(Sid, &str, Kind); 11] = [
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
];

const INTEGRATIONS: [(Sid, &str, Kind); 1] = [(Sid::OpenIntegrations, "Integrations", Kind::Button)];

const ABOUT: [(Sid, &str, Kind); 1] = [(Sid::OpenLogs, "Logs folder", Kind::Button)];

pub const GROUPS: [Group; 8] = [
    Group { glyph: '\u{E713}', name: "General", rows: &GENERAL },
    Group { glyph: '\u{E80F}', name: "Island", rows: &ISLAND },
    Group { glyph: '\u{E790}', name: "Appearance", rows: &APPEARANCE },
    Group { glyph: '\u{E8D6}', name: "Pill and music", rows: &PILL },
    Group { glyph: '\u{EA8F}', name: "Notifications", rows: &NOTIFICATIONS },
    Group { glyph: '\u{E71D}', name: "Tabs", rows: &ROWS_TABS_ONLY },
    Group { glyph: '\u{E71B}', name: "Integrations", rows: &INTEGRATIONS },
    Group { glyph: '\u{E946}', name: "About", rows: &ABOUT },
];

const ROWS_TABS_ONLY: [(Sid, &str, Kind); 10] = [
    (Sid::TabClipboard, "Clipboard", Kind::Toggle),
    (Sid::TabShelf, "Shelf", Kind::Toggle),
    (Sid::TabText, "Text", Kind::Toggle),
    (Sid::TabNote, "Note", Kind::Toggle),
    (Sid::TabReminders, "Reminders", Kind::Toggle),
    (Sid::TabWeather, "Weather", Kind::Toggle),
    (Sid::TabRecord, "Record", Kind::Toggle),
    (Sid::TabAgy, "AGY", Kind::Toggle),
    (Sid::TabControls, "Controls", Kind::Toggle),
    (Sid::TabTheme, "Theme", Kind::Toggle),
];

pub const SW_SIZE: (f32, f32) = (940.0, 620.0);

const SIDEBAR_W: f32 = 220.0;
const PAD: f32 = 28.0;
const SEARCH_H: f32 = 38.0;
const HEADING_H: f32 = 44.0;
const ROW_H: f32 = 46.0;
const CTL_W: f32 = 300.0;
const GROUP_H: f32 = 42.0;

#[derive(Default)]
pub struct SwUi {
    pub group: usize,
    pub scroll: f32,
    pub search: String,
    pub hover: Option<SwHit>,
    pub drag: Option<Sid>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SwHit {
    Group(usize),
    Row(Sid, SetHit),
}

enum Item {
    Heading(&'static str),
    Row(Sid, &'static str, Kind),
}

impl Item {
    fn height(&self) -> f32 {
        match self {
            Item::Heading(_) => HEADING_H,
            Item::Row(..) => ROW_H,
        }
    }
}

fn matches(label: &str, query: &str) -> bool {
    let q = query.trim().to_lowercase();
    q.is_empty() || label.to_lowercase().contains(&q) || crate::ui::i18n::tr(label).to_lowercase().contains(&q)
}

fn items(ui: &UiState) -> Vec<Item> {
    let mut out = Vec::new();
    if ui.sw.search.trim().is_empty() {
        let group = &GROUPS[ui.sw.group.min(GROUPS.len() - 1)];
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
        let Item::Row(id, _, kind) = item else { continue };
        let (row, ctl) = row_rects(&l, scale, top);
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
    let result = (|| {
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
                    let (row, ctl) = row_rects(&l, scale, top);
                    if matches!(ui.sw.hover, Some(SwHit::Row(h, _)) if h == id) {
                        p.fill_round_rect(
                            (row.0 - 8.0 * scale, row.1 + 2.0 * scale, row.2 + 8.0 * scale, row.3 - 2.0 * scale),
                            10.0 * scale,
                            rgba(1.0, 1.0, 1.0, 0.04),
                        )?;
                    }
                    p.text(
                        label,
                        (row.0, row.1, ctl.0 - 12.0 * scale, row.3),
                        14.0 * scale,
                        DWRITE_FONT_WEIGHT_NORMAL,
                        DWRITE_TEXT_ALIGNMENT_LEADING,
                        rgba(1.0, 1.0, 1.0, 0.9),
                    )?;
                    paint_control(p, scale, &ui.cfg, id, kind, ctl)?;
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
    fn every_group_row_is_unique_and_groups_are_not_empty() {
        let mut seen = Vec::new();
        for g in &GROUPS {
            assert!(!g.rows.is_empty());
            for (id, ..) in g.rows {
                assert!(!seen.contains(id), "{id:?} listed twice");
                seen.push(*id);
            }
        }
    }
}
