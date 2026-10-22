use crate::view::*;

#[derive(Default)]
pub struct ActivityUi {
    pub week: bool,
    pub rows: Vec<(String, i64)>,
    pub clear_armed: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ActivityHit {
    Today,
    Week,
    Clear,
    TurnOn,
}

const ROW_H: f32 = 34.0;
const ROW_GAP: f32 = 4.0;
const TOP: f32 = 124.0;

pub fn activity_rows(pill: Rect, scale: f32) -> usize {
    let avail = pill.3 / scale - TOP - 14.0;
    (((avail + ROW_GAP) / (ROW_H + ROW_GAP)).floor().max(1.0)) as usize
}

fn side(pill: Rect, scale: f32) -> (f32, f32) {
    (pill.0 + (SIDE_PAD - 4.0) * scale, pill.0 + pill.2 - (SIDE_PAD - 4.0) * scale)
}

fn period_chip(pill: Rect, scale: f32, week: bool) -> Rect {
    let (l, _) = side(pill, scale);
    let left = l + if week { 92.0 } else { 0.0 } * scale;
    (left, pill.1 + 92.0 * scale, left + 86.0 * scale, pill.1 + 116.0 * scale)
}

fn turn_on_button(pill: Rect, scale: f32) -> Rect {
    let (x, y, w, _) = pill;
    (x + w / 2.0 - 100.0 * scale, y + 206.0 * scale, x + w / 2.0 + 100.0 * scale, y + 244.0 * scale)
}

pub fn hit_activity(pill: Rect, scale: f32, px: f32, py: f32, tracking: bool) -> Option<ActivityHit> {
    if !tracking {
        return inside(turn_on_button(pill, scale), px, py).then_some(ActivityHit::TurnOn);
    }
    if inside(usage_clear_button(pill, scale), px, py) {
        return Some(ActivityHit::Clear);
    }
    if inside(period_chip(pill, scale, false), px, py) {
        return Some(ActivityHit::Today);
    }
    inside(period_chip(pill, scale, true), px, py).then_some(ActivityHit::Week)
}

pub(in crate::view) fn paint_activity(p: &Painter, pill: Rect, scale: f32, ui: &UiState) -> Result<()> {
    let (l, r) = side(pill, scale);
    let (x, y, w, _) = pill;
    p.text(
        "Activity",
        (l + 2.0 * scale, y + 56.0 * scale, r - 100.0 * scale, y + 74.0 * scale),
        14.0 * scale,
        DWRITE_FONT_WEIGHT_BOLD,
        DWRITE_TEXT_ALIGNMENT_LEADING,
        WHITE,
    )?;
    p.text(
        "Time in the program in front",
        (l + 2.0 * scale, y + 72.0 * scale, r - 100.0 * scale, y + 88.0 * scale),
        11.0 * scale,
        DWRITE_FONT_WEIGHT_NORMAL,
        DWRITE_TEXT_ALIGNMENT_LEADING,
        rgba(1.0, 1.0, 1.0, 0.5),
    )?;
    if !ui.cfg.activity_track {
        p.text(
            "Activity tracking is off. It only notes which program is in front and for how long, on this computer.",
            (x + 40.0 * scale, y + 120.0 * scale, x + w - 40.0 * scale, y + 196.0 * scale),
            14.0 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_CENTER,
            rgba(1.0, 1.0, 1.0, 0.55),
        )?;
        let button = turn_on_button(pill, scale);
        p.fill_round_rect(button, 14.0 * scale, acc(if ui.home.hover_act == Some(1310) { 0.5 } else { 0.35 }))?;
        return p.text(
            "Turn tracking on",
            button,
            13.0 * scale,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_CENTER,
            WHITE,
        );
    }
    super::privacy::paint_clear_button(p, pill, scale, ui.activity.clear_armed, ui.home.hover_act == Some(1301))?;
    for week in [false, true] {
        let chip = period_chip(pill, scale, week);
        let on = ui.activity.week == week;
        p.fill_round_rect(chip, 12.0 * scale, if on { acc(0.4) } else { rgba(1.0, 1.0, 1.0, 0.07) })?;
        p.text(
            if week { "7 days" } else { "Today" },
            chip,
            12.5 * scale,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_CENTER,
            rgba(1.0, 1.0, 1.0, if on { 1.0 } else { 0.7 }),
        )?;
    }
    let total: i64 = ui.activity.rows.iter().map(|(_, s)| *s).sum();
    p.text(
        &format!("{} {}", crate::ui::i18n::tr("Total"), super::privacy::duration_text(total)),
        (r - 260.0 * scale, y + 92.0 * scale, r, y + 116.0 * scale),
        12.5 * scale,
        DWRITE_FONT_WEIGHT_NORMAL,
        DWRITE_TEXT_ALIGNMENT_TRAILING,
        rgba(1.0, 1.0, 1.0, 0.6),
    )?;
    if ui.activity.rows.is_empty() {
        return p.text(
            "Nothing recorded yet",
            (x, y + 160.0 * scale, x + w, y + 300.0 * scale),
            14.0 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_CENTER,
            rgba(1.0, 1.0, 1.0, 0.45),
        );
    }
    let longest = ui.activity.rows.first().map_or(1, |(_, s)| (*s).max(1)) as f32;
    for (i, (app, secs)) in ui.activity.rows.iter().take(activity_rows(pill, scale)).enumerate() {
        let top = y + (TOP + i as f32 * (ROW_H + ROW_GAP)) * scale;
        let row = (l, top, r, top + ROW_H * scale);
        p.fill_round_rect(row, 10.0 * scale, rgba(1.0, 1.0, 1.0, 0.05))?;
        let bar = (row.0, row.1, row.0 + (row.2 - row.0) * (*secs as f32 / longest), row.3);
        p.fill_round_rect(bar, 10.0 * scale, acc(0.2))?;
        p.text(
            app,
            (row.0 + 14.0 * scale, row.1, row.0 + 300.0 * scale, row.3),
            13.5 * scale,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            WHITE,
        )?;
        p.text(
            &super::privacy::duration_text(*secs),
            (row.2 - 140.0 * scale, row.1, row.2 - 12.0 * scale, row.3),
            12.5 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_TRAILING,
            rgba(1.0, 1.0, 1.0, 0.75),
        )?;
    }
    Ok(())
}
