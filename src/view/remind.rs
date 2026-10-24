use crate::tools::remind::QUICK;
use crate::view::*;

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Picker {
    pub hour: u32,
    pub minute: u32,
    pub tomorrow: bool,
}

#[derive(Default)]
pub struct RemindUi {
    pub input: TextEdit,
    pub status: String,
    pub list: Vec<(i64, String, String)>,
    pub scroll: usize,
    pub picker: Picker,
    pub hover: Option<RemHit>,
    pub alert: Option<(i64, String)>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RemHit {
    Row(usize),
    Done(usize),
    Chip(usize),
    Day(bool),
    HourDec,
    HourInc,
    MinDec,
    MinInc,
    HourBox,
    MinBox,
    Set,
}

pub(in crate::view) const REM_ROW_H: f32 = 36.0;

pub(in crate::view) const REM_ROW_GAP: f32 = 4.0;

const CHIP_TOP: f32 = LIST_TOP + 42.0;
const PICK_TOP: f32 = CHIP_TOP + 38.0;
pub(in crate::view) const REM_ROWS_TOP: f32 = PICK_TOP + 62.0;

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

fn chip_rect(pill: Rect, scale: f32, i: usize) -> Rect {
    let (x, y, w, _) = pill;
    let (left, right) = (remind_left(pill, scale), x + w - SIDE_PAD * scale);
    let gap = 6.0 * scale;
    let n = QUICK.len() as f32;
    let width = (right - left - (n - 1.0) * gap) / n;
    let l = left + i as f32 * (width + gap);
    (l, y + CHIP_TOP * scale, l + width, y + (CHIP_TOP + 30.0) * scale)
}

pub(in crate::view) struct PickerRects {
    pub today: Rect,
    pub tomorrow: Rect,
    pub hour_dec: Rect,
    pub hour_box: Rect,
    pub hour_inc: Rect,
    pub colon: Rect,
    pub min_dec: Rect,
    pub min_box: Rect,
    pub min_inc: Rect,
    pub set: Rect,
}

pub(in crate::view) fn picker_rects(pill: Rect, scale: f32) -> PickerRects {
    let (x, y, w, _) = pill;
    let (top, bottom) = (y + PICK_TOP * scale, y + (PICK_TOP + 34.0) * scale);
    let left = remind_left(pill, scale);
    let right = x + w - SIDE_PAD * scale;
    let s = scale;
    let seg_w = 78.0 * s;
    let (btn, boxw, gap) = (30.0 * s, 46.0 * s, 4.0 * s);
    let rect = |l: f32, w: f32| (l, top, l + w, bottom);
    let mut at = left + 2.0 * seg_w + 18.0 * s;
    let mut next = |w: f32| {
        let r = rect(at, w);
        at += w + gap;
        r
    };
    let hour_dec = next(btn);
    let hour_box = next(boxw);
    let hour_inc = next(btn);
    let colon = next(10.0 * s);
    let min_dec = next(btn);
    let min_box = next(boxw);
    let min_inc = next(btn);
    PickerRects {
        today: rect(left, seg_w),
        tomorrow: rect(left + seg_w, seg_w),
        hour_dec,
        hour_box,
        hour_inc,
        colon,
        min_dec,
        min_box,
        min_inc,
        set: (right - 120.0 * s, top, right, bottom),
    }
}

pub fn remind_rows(pill: Rect, scale: f32) -> usize {
    let avail = pill.3 / scale - REM_ROWS_TOP - LIST_BOTTOM_PAD;
    ((avail / (REM_ROW_H + REM_ROW_GAP)).floor() as usize).max(1)
}

pub(in crate::view) fn remind_row(pill: Rect, scale: f32, i: usize) -> Rect {
    let (x, y, w, _) = pill;
    let top = y + (REM_ROWS_TOP + i as f32 * (REM_ROW_H + REM_ROW_GAP)) * scale;
    (remind_left(pill, scale), top, x + w - SIDE_PAD * scale, top + REM_ROW_H * scale)
}

pub(in crate::view) fn remind_done_button(row: Rect, scale: f32) -> Rect {
    let size = 28.0 * scale;
    (
        row.2 - size - 6.0 * scale,
        row.1 + (row.3 - row.1 - size) / 2.0,
        row.2 - 6.0 * scale,
        row.1 + (row.3 - row.1 + size) / 2.0,
    )
}

pub fn hit_remind(pill: Rect, scale: f32, px: f32, py: f32, count: usize, scroll: usize) -> Option<RemHit> {
    if let Some(i) = (0..QUICK.len()).find(|&i| inside(chip_rect(pill, scale, i), px, py)) {
        return Some(RemHit::Chip(i));
    }
    let r = picker_rects(pill, scale);
    let controls = [
        (r.today, RemHit::Day(false)),
        (r.tomorrow, RemHit::Day(true)),
        (r.hour_dec, RemHit::HourDec),
        (r.hour_inc, RemHit::HourInc),
        (r.hour_box, RemHit::HourBox),
        (r.min_dec, RemHit::MinDec),
        (r.min_inc, RemHit::MinInc),
        (r.min_box, RemHit::MinBox),
        (r.set, RemHit::Set),
    ];
    if let Some((_, hit)) = controls.into_iter().find(|(rect, _)| inside(*rect, px, py)) {
        return Some(hit);
    }
    let visible = count.saturating_sub(scroll).min(remind_rows(pill, scale));
    for i in 0..visible {
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

fn paint_button(p: &Painter, r: Rect, scale: f32, hovered: bool, strong: bool, label: &str, size: f32) -> Result<()> {
    let fill = if strong {
        acc(if hovered { 0.75 } else { 0.55 })
    } else {
        rgba(1.0, 1.0, 1.0, if hovered { 0.14 } else { 0.07 })
    };
    p.fill_round_rect(r, 10.0 * scale, fill)?;
    p.text(label, r, size * scale, DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_TEXT_ALIGNMENT_CENTER, WHITE)
}

fn paint_picker(p: &Painter, pill: Rect, scale: f32, ui: &UiState) -> Result<()> {
    let hover = ui.remind.hover;
    for (i, (label, _)) in QUICK.iter().enumerate() {
        let r = chip_rect(pill, scale, i);
        paint_button(p, r, scale, hover == Some(RemHit::Chip(i)), false, label, 12.0)?;
    }
    let r = picker_rects(pill, scale);
    let pick = ui.remind.picker;
    p.fill_round_rect((r.today.0, r.today.1, r.tomorrow.2, r.tomorrow.3), 10.0 * scale, rgba(1.0, 1.0, 1.0, 0.07))?;
    for (rect, tomorrow, label) in [(r.today, false, "Today"), (r.tomorrow, true, "Tomorrow")] {
        let on = pick.tomorrow == tomorrow;
        let inner = (rect.0 + 2.0 * scale, rect.1 + 2.0 * scale, rect.2 - 2.0 * scale, rect.3 - 2.0 * scale);
        if on {
            p.fill_round_rect(inner, 8.0 * scale, acc(0.35))?;
        } else if hover == Some(RemHit::Day(tomorrow)) {
            p.fill_round_rect(inner, 8.0 * scale, rgba(1.0, 1.0, 1.0, 0.08))?;
        }
        let alpha = if on { 1.0 } else { 0.65 };
        p.text(
            label,
            rect,
            12.0 * scale,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_CENTER,
            rgba(1.0, 1.0, 1.0, alpha),
        )?;
    }
    for (dec, boxr, inc, value, hit_dec, hit_inc, hit_box) in [
        (r.hour_dec, r.hour_box, r.hour_inc, pick.hour, RemHit::HourDec, RemHit::HourInc, RemHit::HourBox),
        (r.min_dec, r.min_box, r.min_inc, pick.minute, RemHit::MinDec, RemHit::MinInc, RemHit::MinBox),
    ] {
        paint_button(p, dec, scale, hover == Some(hit_dec), false, "−", 16.0)?;
        paint_button(p, inc, scale, hover == Some(hit_inc), false, "+", 16.0)?;
        p.fill_round_rect(boxr, 10.0 * scale, rgba(1.0, 1.0, 1.0, if hover == Some(hit_box) { 0.1 } else { 0.05 }))?;
        p.text(
            &format!("{value:02}"),
            boxr,
            16.0 * scale,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_CENTER,
            WHITE,
        )?;
    }
    p.text(
        ":",
        r.colon,
        16.0 * scale,
        DWRITE_FONT_WEIGHT_SEMI_BOLD,
        DWRITE_TEXT_ALIGNMENT_CENTER,
        rgba(1.0, 1.0, 1.0, 0.7),
    )?;
    paint_button(p, r.set, scale, hover == Some(RemHit::Set), true, "Set", 13.0)
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
            "What to remind?  or type a time: in 30 min call mom",
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

    paint_picker(p, pill, scale, ui)?;

    let picker_bottom = pill.1 + (PICK_TOP + 34.0) * scale;
    let status = if ui.remind.status.is_empty() {
        "Pick a time, or press Enter to use the time in the text"
    } else {
        ui.remind.status.as_str()
    };
    p.text(
        status,
        (frame.0 + 2.0 * scale, picker_bottom + 4.0 * scale, frame.2, picker_bottom + 22.0 * scale),
        11.0 * scale,
        DWRITE_FONT_WEIGHT_NORMAL,
        DWRITE_TEXT_ALIGNMENT_LEADING,
        if ui.remind.status.is_empty() { rgba(1.0, 1.0, 1.0, 0.35) } else { acc(1.0) },
    )?;

    let fit = remind_rows(pill, scale);
    for (i, (_, label, when)) in ui.remind.list.iter().skip(ui.remind.scroll).take(fit).enumerate() {
        let row = remind_row(pill, scale, i);
        let hovered = matches!(ui.remind.hover, Some(RemHit::Row(n) | RemHit::Done(n)) if n == i);
        p.fill_round_rect(row, 10.0 * scale, rgba(1.0, 1.0, 1.0, if hovered { 0.09 } else { 0.045 }))?;
        let right = if hovered { remind_done_button(row, scale).0 } else { row.2 - 8.0 * scale };
        p.text(
            label,
            (row.0 + 10.0 * scale, row.1 + 2.0 * scale, right - 4.0 * scale, row.1 + 21.0 * scale),
            13.0 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            WHITE,
        )?;
        p.text(
            when,
            (row.0 + 10.0 * scale, row.1 + 19.0 * scale, right - 4.0 * scale, row.3 - 2.0 * scale),
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
    if ui.remind.list.len() > fit + ui.remind.scroll {
        let rest = ui.remind.list.len() - fit - ui.remind.scroll;
        let last = remind_row(pill, scale, fit - 1);
        p.text(
            &format!("+{rest}"),
            (last.2 - 70.0 * scale, last.3 + 1.0 * scale, last.2, last.3 + 15.0 * scale),
            10.5 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_TRAILING,
            rgba(1.0, 1.0, 1.0, 0.4),
        )?;
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
        let hover = ui.home.hover_act == Some(1500 + i);
        p.fill_round_rect(
            r,
            12.0 * scale,
            acc(match (strong, hover) {
                (true, false) => 0.55,
                (true, true) => 0.75,
                (false, false) => 0.18,
                (false, true) => 0.32,
            }),
        )?;
        p.text(label, r, 13.5 * scale, DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_TEXT_ALIGNMENT_CENTER, WHITE)?;
    }
    Ok(())
}
