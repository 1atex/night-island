use crate::data::groups::{Group, GroupTotal};
use crate::view::*;

#[derive(Default)]
pub struct ActivityUi {
    pub week: bool,
    pub groups: Vec<Group>,
    pub totals: Vec<GroupTotal>,
    pub days: Vec<Vec<(u8, i64)>>,
    pub open: Option<i64>,
    pub scroll: usize,
    pub ai_used: u32,
    pub clear_armed: bool,
}

impl ActivityUi {
    pub fn open_total(&self) -> Option<&GroupTotal> {
        let id = self.open?;
        self.totals.iter().find(|t| t.id == id)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ActivityHit {
    Today,
    Week,
    Clear,
    TurnOn,
    Group(usize),
    Back,
    App(usize),
}

const ROW_H: f32 = 34.0;
const ROW_GAP: f32 = 4.0;
const TOP: f32 = 124.0;
const LEFT_W: f32 = 204.0;
const RING_OUT: f32 = 60.0;
const RING_IN: f32 = 38.0;
const BAR_W: f32 = 20.0;
const BAR_GAP: f32 = 9.0;
const BAR_H: f32 = 74.0;

const PALETTE: [(f32, f32, f32); crate::data::groups::PALETTE] = [
    (0.31, 0.82, 0.77),
    (0.89, 0.40, 0.31),
    (0.91, 0.64, 0.24),
    (0.65, 0.55, 0.98),
    (0.36, 0.61, 0.94),
    (0.42, 0.80, 0.47),
    (0.54, 0.58, 0.65),
    (0.96, 0.45, 0.71),
    (0.77, 0.85, 0.29),
    (0.22, 0.74, 0.97),
];

fn color(index: u8, alpha: f32) -> windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F {
    let (r, g, b) = PALETTE[usize::from(index).min(PALETTE.len() - 1)];
    rgba(r, g, b, alpha)
}

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

fn list_row(pill: Rect, scale: f32, i: usize, wide: bool) -> Rect {
    let (l, r) = side(pill, scale);
    let left = if wide { l } else { l + (LEFT_W + 14.0) * scale };
    let top = pill.1 + (TOP + i as f32 * (ROW_H + ROW_GAP)) * scale;
    (left, top, r, top + ROW_H * scale)
}

fn back_row(pill: Rect, scale: f32) -> Rect {
    list_row(pill, scale, 0, true)
}

pub fn hit_activity(pill: Rect, scale: f32, px: f32, py: f32, tracking: bool, ui: &ActivityUi) -> Option<ActivityHit> {
    if !tracking {
        return inside(turn_on_button(pill, scale), px, py).then_some(ActivityHit::TurnOn);
    }
    if inside(usage_clear_button(pill, scale), px, py) {
        return Some(ActivityHit::Clear);
    }
    if inside(period_chip(pill, scale, false), px, py) {
        return Some(ActivityHit::Today);
    }
    if inside(period_chip(pill, scale, true), px, py) {
        return Some(ActivityHit::Week);
    }
    let rows = activity_rows(pill, scale);
    if let Some(group) = ui.open_total() {
        if inside(back_row(pill, scale), px, py) {
            return Some(ActivityHit::Back);
        }
        let shown = group.apps.len().saturating_sub(ui.scroll).min(rows.saturating_sub(1));
        return (0..shown).find(|&i| inside(list_row(pill, scale, i + 1, true), px, py)).map(ActivityHit::App);
    }
    let shown = ui.totals.len().min(rows);
    (0..shown).find(|&i| inside(list_row(pill, scale, i, false), px, py)).map(ActivityHit::Group)
}

fn paint_ring(p: &Painter, centre: (f32, f32), outer: f32, inner: f32, slices: &[(f32, u8)]) -> Result<()> {
    use std::f32::consts::{FRAC_PI_2, TAU};
    let point = |angle: f32, radius: f32| (centre.0 + radius * angle.cos(), centre.1 + radius * angle.sin());
    let mut start = -FRAC_PI_2;
    for (share, index) in slices {
        let span = share.clamp(0.0, 1.0) * TAU;
        let steps = ((span / (TAU / 90.0)).ceil() as usize).max(1);
        for s in 0..steps {
            let a0 = start + span * s as f32 / steps as f32;
            let a1 = start + span * (s + 1) as f32 / steps as f32 + if s + 1 < steps { 0.01 } else { 0.0 };
            let c = color(*index, 1.0);
            p.fill_triangle(point(a0, outer), point(a1, outer), point(a0, inner), c)?;
            p.fill_triangle(point(a1, outer), point(a1, inner), point(a0, inner), c)?;
        }
        start += span;
    }
    Ok(())
}

fn paint_week(p: &Painter, pill: Rect, scale: f32, days: &[Vec<(u8, i64)>]) -> Result<()> {
    let (l, _) = side(pill, scale);
    let (_, y, _, _) = pill;
    let most = days.iter().map(|d| d.iter().map(|(_, s)| *s).sum::<i64>()).max().unwrap_or(0).max(1) as f32;
    let base = y + (TOP + 138.0 + BAR_H) * scale;
    let today = crate::platform::activity::today();
    for (i, day) in days.iter().enumerate() {
        let left = l + 4.0 * scale + i as f32 * (BAR_W + BAR_GAP) * scale;
        p.fill_round_rect(
            (left, base - BAR_H * scale, left + BAR_W * scale, base),
            5.0 * scale,
            rgba(1.0, 1.0, 1.0, 0.05),
        )?;
        let mut bottom = base;
        for (index, secs) in day {
            let h = (*secs as f32 / most * BAR_H * scale).max(1.5 * scale);
            p.fill_rect((left, bottom - h, left + BAR_W * scale, bottom), color(*index, 0.95))?;
            bottom -= h;
        }
        let label = weekday_initial(today - (6 - i as i64));
        p.text(
            &label,
            (left - 4.0 * scale, base + 2.0 * scale, left + (BAR_W + 4.0) * scale, base + 16.0 * scale),
            10.5 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_CENTER,
            rgba(1.0, 1.0, 1.0, if i == 6 { 0.9 } else { 0.5 }),
        )?;
    }
    Ok(())
}

fn weekday_initial(day: i64) -> String {
    use chrono::{Datelike, NaiveDate, Weekday};
    let Some(date) =
        NaiveDate::from_ymd_opt(1970, 1, 1).and_then(|d| d.checked_add_signed(chrono::Duration::days(day)))
    else {
        return String::new();
    };
    let ru = crate::ui::i18n::is_ru();
    match (date.weekday(), ru) {
        (Weekday::Mon, false) => "M",
        (Weekday::Tue, false) => "T",
        (Weekday::Wed, false) => "W",
        (Weekday::Thu, false) => "T",
        (Weekday::Fri, false) => "F",
        (Weekday::Sat, false) => "S",
        (Weekday::Sun, false) => "S",
        (Weekday::Mon, true) => "П",
        (Weekday::Tue, true) => "В",
        (Weekday::Wed, true) => "С",
        (Weekday::Thu, true) => "Ч",
        (Weekday::Fri, true) => "П",
        (Weekday::Sat, true) => "С",
        (Weekday::Sun, true) => "В",
    }
    .to_string()
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
    let total: i64 = ui.activity.totals.iter().map(|t| t.secs).sum();
    p.text(
        &format!("{} {}", crate::ui::i18n::tr("Total"), super::privacy::duration_text(total)),
        (r - 260.0 * scale, y + 92.0 * scale, r, y + 116.0 * scale),
        12.5 * scale,
        DWRITE_FONT_WEIGHT_NORMAL,
        DWRITE_TEXT_ALIGNMENT_TRAILING,
        rgba(1.0, 1.0, 1.0, 0.6),
    )?;
    if ui.activity.totals.is_empty() {
        return p.text(
            "Nothing recorded yet",
            (x, y + 160.0 * scale, x + w, y + 300.0 * scale),
            14.0 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_CENTER,
            rgba(1.0, 1.0, 1.0, 0.45),
        );
    }
    let rows = activity_rows(pill, scale);
    if let Some(group) = ui.activity.open_total() {
        return paint_group(p, pill, scale, ui, group, rows);
    }

    let centre = (l + (LEFT_W / 2.0) * scale, y + (TOP + RING_OUT + 4.0) * scale);
    let slices: Vec<(f32, u8)> =
        ui.activity.totals.iter().map(|t| (t.secs as f32 / total.max(1) as f32, t.color)).collect();
    paint_ring(p, centre, RING_OUT * scale, RING_IN * scale, &slices)?;
    p.text(
        &super::privacy::duration_text(total),
        (centre.0 - 36.0 * scale, centre.1 - 11.0 * scale, centre.0 + 36.0 * scale, centre.1 + 11.0 * scale),
        12.5 * scale,
        DWRITE_FONT_WEIGHT_SEMI_BOLD,
        DWRITE_TEXT_ALIGNMENT_CENTER,
        WHITE,
    )?;
    if ui.activity.week {
        paint_week(p, pill, scale, &ui.activity.days)?;
    }
    for (i, group) in ui.activity.totals.iter().take(rows).enumerate() {
        let row = list_row(pill, scale, i, false);
        let hovered = ui.home.hover_act == Some(1320 + i);
        p.fill_round_rect(row, 10.0 * scale, rgba(1.0, 1.0, 1.0, if hovered { 0.1 } else { 0.05 }))?;
        let share = group.secs as f32 / total.max(1) as f32;
        p.fill_round_rect(
            (row.0, row.1, row.0 + (row.2 - row.0) * share, row.3),
            10.0 * scale,
            color(group.color, 0.22),
        )?;
        p.fill_ellipse(row.0 + 15.0 * scale, (row.1 + row.3) / 2.0, 5.0 * scale, 5.0 * scale, color(group.color, 1.0))?;
        p.text(
            &group.name,
            (row.0 + 30.0 * scale, row.1, row.2 - 150.0 * scale, row.3),
            13.5 * scale,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            WHITE,
        )?;
        p.text(
            &format!("{}%  {}", (share * 100.0).round() as i64, super::privacy::duration_text(group.secs)),
            (row.2 - 150.0 * scale, row.1, row.2 - 12.0 * scale, row.3),
            12.5 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_TRAILING,
            rgba(1.0, 1.0, 1.0, 0.75),
        )?;
    }
    Ok(())
}

fn paint_group(p: &Painter, pill: Rect, scale: f32, ui: &UiState, group: &GroupTotal, rows: usize) -> Result<()> {
    let head = back_row(pill, scale);
    let hovered = ui.home.hover_act == Some(1319);
    p.fill_round_rect(head, 10.0 * scale, color(group.color, if hovered { 0.35 } else { 0.22 }))?;
    p.glyph('\u{E72B}', (head.0 + 4.0 * scale, head.1, head.0 + 34.0 * scale, head.3), 14.0 * scale, WHITE)?;
    p.text(
        &group.name,
        (head.0 + 40.0 * scale, head.1, head.2 - 150.0 * scale, head.3),
        13.5 * scale,
        DWRITE_FONT_WEIGHT_BOLD,
        DWRITE_TEXT_ALIGNMENT_LEADING,
        WHITE,
    )?;
    p.text(
        &super::privacy::duration_text(group.secs),
        (head.2 - 150.0 * scale, head.1, head.2 - 12.0 * scale, head.3),
        12.5 * scale,
        DWRITE_FONT_WEIGHT_NORMAL,
        DWRITE_TEXT_ALIGNMENT_TRAILING,
        rgba(1.0, 1.0, 1.0, 0.75),
    )?;
    let longest = group.apps.first().map_or(1, |(_, s)| (*s).max(1)) as f32;
    for (i, (app, secs)) in group.apps.iter().skip(ui.activity.scroll).take(rows.saturating_sub(1)).enumerate() {
        let row = list_row(pill, scale, i + 1, true);
        let hovered = ui.home.hover_act == Some(1340 + i);
        p.fill_round_rect(row, 10.0 * scale, rgba(1.0, 1.0, 1.0, if hovered { 0.1 } else { 0.05 }))?;
        p.fill_round_rect(
            (row.0, row.1, row.0 + (row.2 - row.0) * (*secs as f32 / longest), row.3),
            10.0 * scale,
            color(group.color, 0.2),
        )?;
        p.text(
            app,
            (row.0 + 14.0 * scale, row.1, row.0 + 260.0 * scale, row.3),
            13.5 * scale,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            WHITE,
        )?;
        p.text(
            &super::privacy::duration_text(*secs),
            (row.2 - 300.0 * scale, row.1, row.2 - 170.0 * scale, row.3),
            12.5 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_TRAILING,
            rgba(1.0, 1.0, 1.0, 0.75),
        )?;
        if let Some(next) = next_group(&ui.activity.groups, group.id) {
            let tag = (row.2 - 160.0 * scale, row.1 + 5.0 * scale, row.2 - 8.0 * scale, row.3 - 5.0 * scale);
            p.fill_round_rect(tag, 9.0 * scale, rgba(1.0, 1.0, 1.0, if hovered { 0.2 } else { 0.1 }))?;
            p.text(
                &format!("\u{2192} {}", crate::ui::i18n::tr(&next.name)),
                tag,
                11.5 * scale,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_CENTER,
                color(next.color, 1.0),
            )?;
        }
    }
    Ok(())
}

pub fn next_group(groups: &[Group], id: i64) -> Option<&Group> {
    let at = groups.iter().position(|g| g.id == id)?;
    groups.get((at + 1) % groups.len())
}
