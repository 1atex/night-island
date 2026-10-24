use std::time::Instant;

use crate::data::groups::{Group, GroupTotal};
use crate::view::*;

type Color = windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;

#[derive(Default)]
pub struct ActivityUi {
    pub week: bool,
    pub groups: Vec<Group>,
    pub totals: Vec<GroupTotal>,
    pub days: Vec<Vec<(u8, i64)>>,
    pub open: Option<i64>,
    pub picking: Option<String>,
    pub scroll: usize,
    pub ai_used: u32,
    pub clear_armed: bool,
    pub drawn_at: Option<Instant>,
    pub opened_at: Option<Instant>,
    pub picked_at: Option<Instant>,
    pub glow: Vec<(usize, f32)>,
}

impl ActivityUi {
    pub fn open_total(&self) -> Option<&GroupTotal> {
        let id = self.open?;
        self.totals.iter().find(|t| t.id == id)
    }

    pub fn restart_draw(&mut self) {
        self.drawn_at = Some(Instant::now());
        self.opened_at = Some(Instant::now());
    }

    pub fn step_glow(&mut self, hovered: Option<usize>) -> bool {
        const STEP: f32 = 0.16;
        if let Some(id) = hovered
            && !self.glow.iter().any(|(g, _)| *g == id)
        {
            self.glow.push((id, 0.0));
        }
        let mut moving = false;
        for (id, level) in &mut self.glow {
            let target = if Some(*id) == hovered { 1.0 } else { 0.0 };
            if (*level - target).abs() > f32::EPSILON {
                *level = if *level < target { (*level + STEP).min(target) } else { (*level - STEP).max(target) };
                moving = true;
            }
        }
        self.glow.retain(|(id, level)| *level > 0.0 || Some(*id) == hovered);
        moving
    }

    pub fn animating(&self) -> bool {
        const LONGEST_MS: u128 = 900;
        [self.drawn_at, self.opened_at, self.picked_at]
            .into_iter()
            .flatten()
            .any(|t| t.elapsed().as_millis() < LONGEST_MS)
    }

    fn glow_of(&self, id: usize) -> f32 {
        self.glow.iter().find(|(g, _)| *g == id).map_or(0.0, |(_, l)| ease(*l))
    }
}

fn ease(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

fn progress(since: Option<Instant>, delay_ms: f32, ms: f32) -> f32 {
    since.map_or(1.0, |t| ease((t.elapsed().as_secs_f32() * 1000.0 - delay_ms) / ms))
}

fn faded(c: Color, a: f32) -> Color {
    Color { a: c.a * a.clamp(0.0, 1.0), ..c }
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
    Pick(usize),
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
const PICK_COLS: usize = 3;
const PICK_H: f32 = 36.0;

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

fn color(index: u8, alpha: f32) -> Color {
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

fn pick_chip(pill: Rect, scale: f32, i: usize) -> Rect {
    let (l, r) = side(pill, scale);
    let gap = 8.0 * scale;
    let width = (r - l - gap * (PICK_COLS - 1) as f32) / PICK_COLS as f32;
    let (col, row) = ((i % PICK_COLS) as f32, (i / PICK_COLS) as f32);
    let left = l + col * (width + gap);
    let top = pill.1 + (TOP + ROW_H + 12.0) * scale + row * (PICK_H * scale + gap);
    (left, top, left + width, top + PICK_H * scale)
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
        if ui.picking.is_some() {
            return (0..ui.groups.len()).find(|&i| inside(pick_chip(pill, scale, i), px, py)).map(ActivityHit::Pick);
        }
        let shown = group.apps.len().saturating_sub(ui.scroll).min(rows.saturating_sub(1));
        return (0..shown).find(|&i| inside(list_row(pill, scale, i + 1, true), px, py)).map(ActivityHit::App);
    }
    let shown = ui.totals.len().min(rows);
    (0..shown).find(|&i| inside(list_row(pill, scale, i, false), px, py)).map(ActivityHit::Group)
}

fn paint_ring(p: &Painter, centre: (f32, f32), outer: f32, inner: f32, slices: &[(f32, u8)], reach: f32) -> Result<()> {
    use std::f32::consts::{FRAC_PI_2, TAU};
    let point = |angle: f32, radius: f32| (centre.0 + radius * angle.cos(), centre.1 + radius * angle.sin());
    let mut start = -FRAC_PI_2;
    let mut left = reach.clamp(0.0, 1.0) * TAU;
    for (share, index) in slices {
        let whole = share.clamp(0.0, 1.0) * TAU;
        let span = whole.min(left);
        if span <= 0.0 {
            break;
        }
        let steps = ((span / (TAU / 90.0)).ceil() as usize).max(1);
        for s in 0..steps {
            let a0 = start + span * s as f32 / steps as f32;
            let a1 = start + span * (s + 1) as f32 / steps as f32 + if s + 1 < steps { 0.01 } else { 0.0 };
            let c = color(*index, 1.0);
            p.fill_triangle(point(a0, outer), point(a1, outer), point(a0, inner), c)?;
            p.fill_triangle(point(a1, outer), point(a1, inner), point(a0, inner), c)?;
        }
        start += whole;
        left -= span;
    }
    Ok(())
}

fn paint_week(p: &Painter, pill: Rect, scale: f32, ui: &ActivityUi) -> Result<()> {
    let (l, _) = side(pill, scale);
    let (_, y, _, _) = pill;
    let days = &ui.days;
    let most = days.iter().map(|d| d.iter().map(|(_, s)| *s).sum::<i64>()).max().unwrap_or(0).max(1) as f32;
    let base = y + (TOP + 138.0 + BAR_H) * scale;
    let today = crate::platform::activity::today();
    for (i, day) in days.iter().enumerate() {
        let grow = progress(ui.drawn_at, 120.0 + i as f32 * 45.0, 420.0);
        let left = l + 4.0 * scale + i as f32 * (BAR_W + BAR_GAP) * scale;
        p.fill_round_rect(
            (left, base - BAR_H * scale, left + BAR_W * scale, base),
            5.0 * scale,
            rgba(1.0, 1.0, 1.0, 0.05),
        )?;
        let mut bottom = base;
        for (index, secs) in day {
            let h = (*secs as f32 / most * BAR_H * scale * grow).max(if grow > 0.0 { 1.5 * scale } else { 0.0 });
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

fn lively(row: Rect, scale: f32, glow: f32, appear: f32) -> (Rect, f32) {
    let grow = 2.0 * scale * glow;
    let slide = (1.0 - appear) * 10.0 * scale;
    ((row.0 - grow, row.1 - grow + slide, row.2 + grow, row.3 + grow + slide), appear)
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
        return paint_group(p, pill, scale, &ui.activity, group, rows);
    }

    let centre = (l + (LEFT_W / 2.0) * scale, y + (TOP + RING_OUT + 4.0) * scale);
    let slices: Vec<(f32, u8)> =
        ui.activity.totals.iter().map(|t| (t.secs as f32 / total.max(1) as f32, t.color)).collect();
    let sweep = progress(ui.activity.drawn_at, 0.0, 650.0);
    paint_ring(p, centre, RING_OUT * scale, RING_IN * scale, &slices, sweep)?;
    p.text(
        &super::privacy::duration_text(total),
        (centre.0 - 36.0 * scale, centre.1 - 11.0 * scale, centre.0 + 36.0 * scale, centre.1 + 11.0 * scale),
        12.5 * scale,
        DWRITE_FONT_WEIGHT_SEMI_BOLD,
        DWRITE_TEXT_ALIGNMENT_CENTER,
        faded(WHITE, sweep),
    )?;
    if ui.activity.week {
        paint_week(p, pill, scale, &ui.activity)?;
    }
    for (i, group) in ui.activity.totals.iter().take(rows).enumerate() {
        let appear = progress(ui.activity.opened_at, 60.0 + i as f32 * 55.0, 260.0);
        let glow = ui.activity.glow_of(1320 + i);
        let (row, a) = lively(list_row(pill, scale, i, false), scale, glow, appear);
        p.fill_round_rect(row, 10.0 * scale, rgba(1.0, 1.0, 1.0, (0.05 + 0.08 * glow) * a))?;
        let share = group.secs as f32 / total.max(1) as f32;
        let fill = (row.2 - row.0) * share * progress(ui.activity.opened_at, 120.0 + i as f32 * 55.0, 500.0);
        p.fill_round_rect(
            (row.0, row.1, row.0 + fill, row.3),
            10.0 * scale,
            color(group.color, (0.22 + 0.12 * glow) * a),
        )?;
        let dot = 5.0 * scale * (1.0 + 0.3 * glow);
        p.fill_ellipse(row.0 + 15.0 * scale, (row.1 + row.3) / 2.0, dot, dot, color(group.color, a))?;
        p.text(
            &group.name,
            (row.0 + 30.0 * scale, row.1, row.2 - 150.0 * scale, row.3),
            13.5 * scale,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            faded(WHITE, a),
        )?;
        p.text(
            &format!("{}%  {}", (share * 100.0).round() as i64, super::privacy::duration_text(group.secs)),
            (row.2 - 150.0 * scale, row.1, row.2 - 12.0 * scale, row.3),
            12.5 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_TRAILING,
            rgba(1.0, 1.0, 1.0, (0.75 + 0.25 * glow) * a),
        )?;
    }
    Ok(())
}

fn paint_group(p: &Painter, pill: Rect, scale: f32, ui: &ActivityUi, group: &GroupTotal, rows: usize) -> Result<()> {
    let head = back_row(pill, scale);
    let glow = ui.glow_of(1319);
    p.fill_round_rect(head, 10.0 * scale, color(group.color, 0.22 + 0.16 * glow))?;
    p.glyph(
        '\u{E72B}',
        (head.0 + 4.0 * scale - 2.0 * scale * glow, head.1, head.0 + 34.0 * scale, head.3),
        14.0 * scale,
        WHITE,
    )?;
    let title = match &ui.picking {
        Some(app) => format!("{app}  \u{2192}  {}", crate::ui::i18n::tr("choose a group")),
        None => group.name.clone(),
    };
    p.text(
        &title,
        (head.0 + 40.0 * scale, head.1, head.2 - 150.0 * scale, head.3),
        13.5 * scale,
        DWRITE_FONT_WEIGHT_BOLD,
        DWRITE_TEXT_ALIGNMENT_LEADING,
        WHITE,
    )?;
    if ui.picking.is_none() {
        p.text(
            &super::privacy::duration_text(group.secs),
            (head.2 - 150.0 * scale, head.1, head.2 - 12.0 * scale, head.3),
            12.5 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_TRAILING,
            rgba(1.0, 1.0, 1.0, 0.75),
        )?;
    } else {
        return paint_picker(p, pill, scale, ui, group.id);
    }
    let longest = group.apps.first().map_or(1, |(_, s)| (*s).max(1)) as f32;
    for (i, (app, secs)) in group.apps.iter().skip(ui.scroll).take(rows.saturating_sub(1)).enumerate() {
        let appear = progress(ui.opened_at, 40.0 + i as f32 * 35.0, 230.0);
        let glow = ui.glow_of(1340 + i);
        let (row, a) = lively(list_row(pill, scale, i + 1, true), scale, glow, appear);
        p.fill_round_rect(row, 10.0 * scale, rgba(1.0, 1.0, 1.0, (0.05 + 0.08 * glow) * a))?;
        let fill = (row.2 - row.0) * (*secs as f32 / longest) * progress(ui.opened_at, 100.0 + i as f32 * 35.0, 450.0);
        p.fill_round_rect(
            (row.0, row.1, row.0 + fill, row.3),
            10.0 * scale,
            color(group.color, (0.2 + 0.1 * glow) * a),
        )?;
        p.text(
            app,
            (row.0 + 14.0 * scale, row.1, row.0 + 260.0 * scale, row.3),
            13.5 * scale,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            faded(WHITE, a),
        )?;
        p.text(
            &super::privacy::duration_text(*secs),
            (row.2 - 300.0 * scale, row.1, row.2 - 170.0 * scale, row.3),
            12.5 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_TRAILING,
            rgba(1.0, 1.0, 1.0, 0.75 * a),
        )?;
        if glow > 0.0 {
            let tag = (row.2 - 160.0 * scale, row.1 + 5.0 * scale, row.2 - 8.0 * scale, row.3 - 5.0 * scale);
            p.fill_round_rect(tag, 9.0 * scale, rgba(1.0, 1.0, 1.0, 0.16 * glow))?;
            p.text(
                "Move to a group",
                tag,
                11.5 * scale,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_CENTER,
                rgba(1.0, 1.0, 1.0, glow),
            )?;
        }
    }
    Ok(())
}

fn paint_picker(p: &Painter, pill: Rect, scale: f32, ui: &ActivityUi, current: i64) -> Result<()> {
    for (i, group) in ui.groups.iter().enumerate() {
        let appear = progress(ui.picked_at, i as f32 * 28.0, 240.0);
        let glow = ui.glow_of(1360 + i);
        let (chip, a) = lively(pick_chip(pill, scale, i), scale, glow, appear);
        let here = group.id == current;
        p.fill_round_rect(chip, 12.0 * scale, color(group.color, (if here { 0.34 } else { 0.14 } + 0.2 * glow) * a))?;
        if here {
            p.stroke_round_rect(chip, 12.0 * scale, 1.5 * scale, color(group.color, 0.9 * a))?;
        }
        let dot = 5.5 * scale * (1.0 + 0.25 * glow);
        p.fill_ellipse(chip.0 + 16.0 * scale, (chip.1 + chip.3) / 2.0, dot, dot, color(group.color, a))?;
        p.text(
            &group.name,
            (chip.0 + 32.0 * scale, chip.1, chip.2 - 8.0 * scale, chip.3),
            13.0 * scale,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            faded(WHITE, a),
        )?;
    }
    Ok(())
}
