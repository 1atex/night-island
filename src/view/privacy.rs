use crate::data::usage::{UseKind, UseRow};
use crate::view::*;

#[derive(Default)]
pub struct PrivacyUi {
    pub rows: Vec<UseRow>,
    pub scroll: usize,
    pub clear_armed: bool,
}

const ROW_H: f32 = 36.0;
const ROW_GAP: f32 = 4.0;
const TOP: f32 = 94.0;

pub fn privacy_rows(pill: Rect, scale: f32) -> usize {
    let avail = pill.3 / scale - TOP - 16.0;
    (((avail + ROW_GAP) / (ROW_H + ROW_GAP)).floor().max(1.0)) as usize
}

fn side(pill: Rect, scale: f32) -> (f32, f32) {
    (pill.0 + (SIDE_PAD - 4.0) * scale, pill.0 + pill.2 - (SIDE_PAD - 4.0) * scale)
}

pub fn usage_clear_button(pill: Rect, scale: f32) -> Rect {
    let (_, r) = side(pill, scale);
    (r - 92.0 * scale, pill.1 + 58.0 * scale, r, pill.1 + 86.0 * scale)
}

fn row_rect(pill: Rect, scale: f32, i: usize) -> Rect {
    let (l, r) = side(pill, scale);
    let top = pill.1 + (TOP + i as f32 * (ROW_H + ROW_GAP)) * scale;
    (l, top, r, top + ROW_H * scale)
}

pub fn duration_text(secs: i64) -> String {
    let secs = secs.max(0);
    let ru = crate::ui::i18n::is_ru();
    let (h, m, s) = (if ru { "ч" } else { "h" }, if ru { "мин" } else { "min" }, if ru { "с" } else { "s" });
    match secs {
        0..=59 => format!("{secs} {s}"),
        60..=3599 => format!("{} {m}", secs / 60),
        _ => format!("{} {h} {} {m}", secs / 3600, secs / 60 % 60),
    }
}

fn when_text(unix: i64) -> String {
    use chrono::{Local, TimeZone};
    let Some(at) = Local.timestamp_opt(unix, 0).single() else { return String::new() };
    let today = Local::now().date_naive();
    let day = at.date_naive();
    let time = at.format("%H:%M").to_string();
    if day == today {
        time
    } else if today.pred_opt() == Some(day) {
        format!("{} {time}", crate::ui::i18n::tr("Yesterday"))
    } else {
        format!("{} {time}", at.format("%d.%m"))
    }
}

fn kind_look(kind: UseKind) -> (char, windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F) {
    match kind {
        UseKind::Mic => ('\u{E720}', rgba(1.0, 0.7, 0.3, 1.0)),
        UseKind::Cam => ('\u{E722}', rgba(0.4, 0.9, 0.55, 1.0)),
        UseKind::Screen => ('\u{E7F4}', rgba(0.55, 0.75, 1.0, 1.0)),
    }
}

pub(in crate::view) fn paint_privacy(p: &Painter, pill: Rect, scale: f32, ui: &UiState) -> Result<()> {
    let (l, r) = side(pill, scale);
    let y = pill.1;
    p.text(
        "Microphone, camera and screen",
        (l + 2.0 * scale, y + 56.0 * scale, r - 100.0 * scale, y + 74.0 * scale),
        14.0 * scale,
        DWRITE_FONT_WEIGHT_BOLD,
        DWRITE_TEXT_ALIGNMENT_LEADING,
        WHITE,
    )?;
    p.text(
        "Who used them, and when",
        (l + 2.0 * scale, y + 72.0 * scale, r - 100.0 * scale, y + 88.0 * scale),
        11.0 * scale,
        DWRITE_FONT_WEIGHT_NORMAL,
        DWRITE_TEXT_ALIGNMENT_LEADING,
        rgba(1.0, 1.0, 1.0, 0.5),
    )?;
    paint_clear_button(p, pill, scale, ui.privacy_log.clear_armed, ui.home.hover_act == Some(1300))?;
    if ui.privacy_log.rows.is_empty() {
        return p.text(
            "Nothing yet: no program has used them since the log began",
            (pill.0, y + 120.0 * scale, pill.0 + pill.2, y + 300.0 * scale),
            14.0 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_CENTER,
            rgba(1.0, 1.0, 1.0, 0.45),
        );
    }
    for (i, row) in ui.privacy_log.rows.iter().skip(ui.privacy_log.scroll).take(privacy_rows(pill, scale)).enumerate() {
        let rect = row_rect(pill, scale, i);
        let live = row.end.is_none();
        p.fill_round_rect(rect, 10.0 * scale, rgba(1.0, 1.0, 1.0, if live { 0.1 } else { 0.05 }))?;
        let (glyph, color) = kind_look(row.kind);
        p.glyph(glyph, (rect.0 + 6.0 * scale, rect.1, rect.0 + 34.0 * scale, rect.3), 15.0 * scale, color)?;
        p.text(
            &row.app,
            (rect.0 + 38.0 * scale, rect.1, rect.0 + 250.0 * scale, rect.3),
            13.5 * scale,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            WHITE,
        )?;
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
        let (text, text_color) = match row.end {
            Some(end) => {
                (format!("{} · {}", when_text(row.start), duration_text(end - row.start)), rgba(1.0, 1.0, 1.0, 0.6))
            }
            None => (
                format!("{} · {}", crate::ui::i18n::tr("now"), duration_text(now - row.start)),
                rgba(1.0, 0.72, 0.35, 1.0),
            ),
        };
        p.text(
            &text,
            (rect.0 + 250.0 * scale, rect.1, rect.2 - 12.0 * scale, rect.3),
            12.5 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_TRAILING,
            text_color,
        )?;
    }
    Ok(())
}

pub(in crate::view) fn paint_clear_button(p: &Painter, pill: Rect, scale: f32, armed: bool, hover: bool) -> Result<()> {
    let rect = usage_clear_button(pill, scale);
    let fill = if armed { rgba(1.0, 0.35, 0.35, 0.28) } else { rgba(1.0, 1.0, 1.0, if hover { 0.16 } else { 0.08 }) };
    p.fill_round_rect(rect, 12.0 * scale, fill)?;
    p.text(
        if armed { "Sure?" } else { "Clear" },
        rect,
        12.5 * scale,
        DWRITE_FONT_WEIGHT_SEMI_BOLD,
        DWRITE_TEXT_ALIGNMENT_CENTER,
        WHITE,
    )
}
