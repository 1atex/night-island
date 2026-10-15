use crate::view::*;

#[derive(Default)]
pub struct RemindUi {
    pub input: TextEdit,
    pub status: String,
    pub list: Vec<(i64, String, String)>,
    pub hover: Option<RemHit>,
    pub alert: Option<(i64, String)>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RemHit {
    Row(usize),
    Done(usize),
}

pub(in crate::view) const REM_ROW_H: f32 = 42.0;

pub(in crate::view) const REM_ROWS_TOP: f32 = 124.0;

pub(in crate::view) fn remind_left(pill: Rect, scale: f32) -> f32 {
    pill.0 + SIDE_PAD * scale
}

pub fn remind_frame(pill: Rect, scale: f32) -> (Rect, Rect) {
    let (x, y, w, _) = pill;
    let frame =
        (remind_left(pill, scale), y + LIST_TOP * scale, x + w - SIDE_PAD * scale, y + (LIST_TOP + 34.0) * scale);
    let inner = (frame.0 + 12.0 * scale, frame.1, frame.2 - 10.0 * scale, frame.3);
    (frame, inner)
}

pub fn remind_rows(pill: Rect, scale: f32) -> usize {
    let avail = pill.3 / scale - REM_ROWS_TOP - LIST_BOTTOM_PAD;
    ((avail / (REM_ROW_H + 4.0)).floor() as usize).max(1)
}

pub(in crate::view) fn remind_row(pill: Rect, scale: f32, i: usize) -> Rect {
    let (x, y, w, _) = pill;
    let top = y + (REM_ROWS_TOP + i as f32 * (REM_ROW_H + 4.0)) * scale;
    (remind_left(pill, scale), top, x + w - SIDE_PAD * scale, top + REM_ROW_H * scale)
}

pub(in crate::view) fn remind_done_button(row: Rect, scale: f32) -> Rect {
    let size = 30.0 * scale;
    (
        row.2 - size - 6.0 * scale,
        row.1 + (row.3 - row.1 - size) / 2.0,
        row.2 - 6.0 * scale,
        row.1 + (row.3 - row.1 + size) / 2.0,
    )
}

pub fn hit_remind(pill: Rect, scale: f32, px: f32, py: f32, count: usize) -> Option<RemHit> {
    for i in 0..count.min(remind_rows(pill, scale)) {
        let row = remind_row(pill, scale, i);
        if inside(remind_done_button(row, scale), px, py) {
            return Some(RemHit::Done(i));
        }
        if inside(row, px, py) {
            return Some(RemHit::Row(i));
        }
    }
    None
}

pub(in crate::view) fn paint_reminders(p: &Painter, pill: Rect, scale: f32, ui: &UiState) -> Result<()> {
    let (frame, inner) = remind_frame(pill, scale);
    let focused = ui.editing;
    p.fill_round_rect(frame, 11.0 * scale, rgba(1.0, 1.0, 1.0, 0.06))?;
    if focused {
        p.stroke_round_rect(frame, 11.0 * scale, 1.2 * scale, acc(0.7))?;
    }
    let text = ui.remind.input.text();
    if text.is_empty() {
        p.text(
            "Remind me…  e.g. in 30 min call mom",
            inner,
            12.5 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            rgba(1.0, 1.0, 1.0, 0.4),
        )?;
    }
    if let Ok(layout) = crate::ui::gfx::make_layout(p.dwrite(), &text, 100_000.0, 13.5 * scale) {
        let (_, _, line_h) = edit::caret_xy(&layout, 0);
        let top = inner.1 + ((inner.3 - inner.1) - line_h) / 2.0;
        p.push_clip(inner);
        p.draw_layout(inner.0, top, &layout, WHITE)?;
        if focused && ui.caret_on {
            let (cx, cy, ch) = edit::caret_xy(&layout, ui.remind.input.utf16_at(ui.remind.input.caret));
            p.fill_rect((inner.0 + cx, top + cy, inner.0 + cx + 1.6 * scale, top + cy + ch), WHITE)?;
        }
        p.pop_clip();
    }

    let status = if ui.remind.status.is_empty() {
        "tomorrow 9:00 report  ·  18:30 call  ·  через 2 часа чай"
    } else {
        ui.remind.status.as_str()
    };
    p.text(
        status,
        (frame.0 + 2.0 * scale, frame.3 + 4.0 * scale, frame.2, frame.3 + 24.0 * scale),
        11.0 * scale,
        DWRITE_FONT_WEIGHT_NORMAL,
        DWRITE_TEXT_ALIGNMENT_LEADING,
        if ui.remind.status.is_empty() { rgba(1.0, 1.0, 1.0, 0.35) } else { acc(1.0) },
    )?;

    for (i, (_, label, when)) in ui.remind.list.iter().take(remind_rows(pill, scale)).enumerate() {
        let row = remind_row(pill, scale, i);
        let hovered = matches!(ui.remind.hover, Some(RemHit::Row(n) | RemHit::Done(n)) if n == i);
        p.fill_round_rect(row, 10.0 * scale, rgba(1.0, 1.0, 1.0, if hovered { 0.09 } else { 0.045 }))?;
        let right = if hovered { remind_done_button(row, scale).0 } else { row.2 - 8.0 * scale };
        p.text(
            label,
            (row.0 + 10.0 * scale, row.1 + 3.0 * scale, right - 4.0 * scale, row.1 + 24.0 * scale),
            13.0 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            WHITE,
        )?;
        p.text(
            when,
            (row.0 + 10.0 * scale, row.1 + 22.0 * scale, right - 4.0 * scale, row.3 - 3.0 * scale),
            11.0 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            acc(0.9),
        )?;
        if hovered {
            let b = remind_done_button(row, scale);
            let on_button = ui.remind.hover == Some(RemHit::Done(i));
            if on_button {
                p.fill_round_rect(b, 8.0 * scale, rgba(0.4, 0.9, 0.55, 0.25))?;
            }
            p.glyph('\u{E73E}', b, 14.0 * scale, rgba(0.5, 0.95, 0.6, 1.0))?;
        }
    }
    Ok(())
}

pub const ALERT_BUTTONS: [&str; 3] = ["Done", "Snooze 10 min", "Snooze 1 hour"];

pub(in crate::view) fn alert_button(pill: Rect, scale: f32, i: usize) -> Rect {
    let (x, y, w, h) = pill;
    let (bw, bh, gap) = (170.0 * scale, 40.0 * scale, 14.0 * scale);
    let total = 3.0 * bw + 2.0 * gap;
    let left = x + (w - total) / 2.0 + i as f32 * (bw + gap);
    (left, y + h - 76.0 * scale, left + bw, y + h - 76.0 * scale + bh)
}

pub fn hit_alert(pill: Rect, scale: f32, px: f32, py: f32) -> Option<usize> {
    (0..3).find(|&i| inside(alert_button(pill, scale, i), px, py))
}

pub(in crate::view) fn paint_alert(p: &Painter, pill: Rect, scale: f32, ui: &UiState) -> Result<()> {
    let Some((_, message)) = &ui.remind.alert else { return Ok(()) };
    let (x, y, w, _) = pill;
    p.glyph('\u{EA8F}', (x, y + 24.0 * scale, x + w, y + 80.0 * scale), 34.0 * scale, acc(1.0))?;
    p.text(
        "Reminder",
        (x, y + 82.0 * scale, x + w, y + 106.0 * scale),
        13.0 * scale,
        DWRITE_FONT_WEIGHT_SEMI_BOLD,
        DWRITE_TEXT_ALIGNMENT_CENTER,
        acc(1.0),
    )?;
    let area = (x + 70.0 * scale, y + 112.0 * scale, x + w - 70.0 * scale, y + 196.0 * scale);
    if let Ok(layout) = crate::ui::gfx::make_layout(p.dwrite(), message, area.2 - area.0, 22.0 * scale) {
        p.push_clip(area);
        p.draw_layout(area.0, area.1, &layout, WHITE)?;
        p.pop_clip();
    }
    for (i, label) in ALERT_BUTTONS.iter().enumerate() {
        let r = alert_button(pill, scale, i);
        let strong = i == 0;
        p.fill_round_rect(r, 12.0 * scale, acc(if strong { 0.55 } else { 0.18 }))?;
        p.text(label, r, 13.5 * scale, DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_TEXT_ALIGNMENT_CENTER, WHITE)?;
    }
    Ok(())
}
