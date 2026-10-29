use std::collections::HashSet;

use crate::data::notes::{NoteItem, visible_rows};
use crate::view::*;

#[derive(Default)]
pub struct NotesUi {
    pub items: Vec<NoteItem>,
    pub expanded: HashSet<i64>,
    pub current: Option<i64>,
    pub sel: Option<i64>,
    pub scroll: usize,
    pub hover: Option<NoteHit>,
    pub renaming: Option<i64>,
    pub name_input: TextEdit,
    pub armed_delete: bool,
    pub drag_from: Option<i64>,
    pub drag_xy: (i32, i32),
    pub reorder: Option<Reorder>,
    pub into: Option<i64>,
    pub remind: Option<(i64, i64)>,
    pub picker: Option<(i32, u32)>,
}

impl NotesUi {
    pub fn target_folder(&self) -> Option<i64> {
        let sel = self.items.iter().find(|i| Some(i.id) == self.sel)?;
        if sel.folder { Some(sel.id) } else { sel.parent }
    }

    pub fn item(&self, id: i64) -> Option<&NoteItem> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn reveal(&mut self, id: i64) {
        let mut at = self.item(id).and_then(|i| i.parent);
        let mut hops = 0;
        while let Some(p) = at {
            if hops > crate::data::notes::MAX_DEPTH + 1 {
                break;
            }
            hops += 1;
            self.expanded.insert(p);
            at = self.item(p).and_then(|i| i.parent);
        }
    }

    pub fn rows(&self) -> Vec<(usize, &NoteItem)> {
        visible_rows(&self.items, &self.expanded)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NoteHit {
    Row(usize),
    AddNote,
    AddFolder,
    Rename,
    Delete,
    TypeToggle,
    Remind(usize),
    RemindOpen,
    PickMonth(i32),
    PickDay(u32),
    PickHour(i32),
    PickMinute(i32),
    PickDone,
    PickBackground,
}

const TREE_W: f32 = 196.0;
const TREE_ROW_H: f32 = 30.0;
const TREE_TOOLBAR_H: f32 = 36.0;
const TREE_INDENT: f32 = 14.0;
const TOOL_BTN: f32 = 30.0;

pub fn note_tree(pill: Rect, scale: f32) -> Rect {
    let (x, y, w, h) = pill;
    let _ = w;
    (x + SIDE_PAD * scale, y + LIST_TOP * scale, x + (SIDE_PAD + TREE_W) * scale, y + h - LIST_BOTTOM_PAD * scale)
}

const PANEL_H: f32 = 92.0;
const TYPE_BTN: f32 = 28.0;

pub fn note_frame_for(pill: Rect, scale: f32, reminder: bool) -> (Rect, Rect) {
    let (x, y, w, h) = pill;
    let tree = note_tree(pill, scale);
    let bottom = y + h - LIST_BOTTOM_PAD * scale - if reminder { (PANEL_H + 8.0) * scale } else { 0.0 };
    let frame = (tree.2 + 10.0 * scale, y + LIST_TOP * scale, x + w - SIDE_PAD * scale, bottom);
    let inner = (
        frame.0 + 14.0 * scale,
        frame.1 + 10.0 * scale,
        frame.2 - (14.0 + TYPE_BTN + 6.0) * scale,
        frame.3 - 10.0 * scale,
    );
    (frame, inner)
}

pub fn type_button(pill: Rect, scale: f32, reminder: bool) -> Rect {
    let frame = note_frame_for(pill, scale, reminder).0;
    (
        frame.2 - (TYPE_BTN + 6.0) * scale,
        frame.1 + 6.0 * scale,
        frame.2 - 6.0 * scale,
        frame.1 + (6.0 + TYPE_BTN) * scale,
    )
}

fn remind_panel(pill: Rect, scale: f32) -> Rect {
    let frame = note_frame_for(pill, scale, true).0;
    (frame.0, frame.3 + 8.0 * scale, frame.2, frame.3 + (8.0 + PANEL_H) * scale)
}

const REMIND_BUTTONS: [&str; 8] = ["-1 d", "+1 d", "-1 h", "+1 h", "-10 m", "+10 m", "In 1 h", "Tomorrow 9:00"];

fn remind_button(pill: Rect, scale: f32, i: usize) -> Rect {
    let panel = remind_panel(pill, scale);
    let (gap, h) = (6.0 * scale, 26.0 * scale);
    let w = (panel.2 - panel.0 - 20.0 * scale - 3.0 * gap) / 4.0;
    let (col, row) = ((i % 4) as f32, (i / 4) as f32);
    let left = panel.0 + 10.0 * scale + col * (w + gap);
    let top = panel.1 + 32.0 * scale + row * (h + 4.0 * scale);
    (left, top, left + w, top + h)
}

const PICK_W: f32 = 250.0;
const PICK_H: f32 = 270.0;
const PICK_CELL_H: f32 = 24.0;

pub fn picker_rect(pill: Rect, scale: f32) -> Rect {
    let frame = note_frame_for(pill, scale, true).0;
    let panel = remind_panel(pill, scale);
    (frame.2 - PICK_W * scale, panel.3 - PICK_H * scale, frame.2, panel.3)
}

fn picker_arrow(r: Rect, scale: f32, next: bool) -> Rect {
    let left = if next { r.2 - 36.0 * scale } else { r.0 + 6.0 * scale };
    (left, r.1 + 4.0 * scale, left + 30.0 * scale, r.1 + 30.0 * scale)
}

pub fn month_shape(year: i32, month: u32) -> (u32, u32) {
    use chrono::{Datelike, NaiveDate};
    let first = NaiveDate::from_ymd_opt(year, month, 1);
    let next =
        if month == 12 { NaiveDate::from_ymd_opt(year + 1, 1, 1) } else { NaiveDate::from_ymd_opt(year, month + 1, 1) };
    let days = match (first, next) {
        (Some(a), Some(b)) => (b - a).num_days() as u32,
        _ => 30,
    };
    (days, first.map_or(0, |d| d.weekday().num_days_from_monday()))
}

fn picker_cell(r: Rect, scale: f32, year: i32, month: u32, day: u32) -> Rect {
    let (_, lead) = month_shape(year, month);
    let slot = lead + day - 1;
    let (col, row) = ((slot % 7) as f32, (slot / 7) as f32);
    let cw = (PICK_W - 16.0) * scale / 7.0;
    let left = r.0 + 8.0 * scale + col * cw;
    let top = r.1 + 54.0 * scale + row * PICK_CELL_H * scale;
    (left, top, left + cw, top + PICK_CELL_H * scale)
}

fn picker_time(r: Rect, scale: f32, i: usize) -> Rect {
    let widths = [28.0, 36.0, 28.0, 12.0, 28.0, 36.0, 28.0];
    let total: f32 = widths.iter().sum();
    let mut left = r.0 + (PICK_W - total) / 2.0 * scale;
    for w in &widths[..i.min(6)] {
        left += w * scale;
    }
    let top = r.1 + (54.0 + 6.0 * PICK_CELL_H + 6.0) * scale;
    (left, top, left + widths[i.min(6)] * scale, top + 30.0 * scale)
}

fn picker_done(r: Rect, scale: f32) -> Rect {
    let top = r.1 + 238.0 * scale;
    (r.0 + 8.0 * scale, top, r.2 - 8.0 * scale, top + 26.0 * scale)
}

fn hit_picker(pill: Rect, scale: f32, px: f32, py: f32, view: (i32, u32)) -> Option<NoteHit> {
    let r = picker_rect(pill, scale);
    if !inside(r, px, py) {
        return None;
    }
    if inside(picker_arrow(r, scale, false), px, py) {
        return Some(NoteHit::PickMonth(-1));
    }
    if inside(picker_arrow(r, scale, true), px, py) {
        return Some(NoteHit::PickMonth(1));
    }
    let (days, _) = month_shape(view.0, view.1);
    if let Some(d) = (1..=days).find(|&d| inside(picker_cell(r, scale, view.0, view.1, d), px, py)) {
        return Some(NoteHit::PickDay(d));
    }
    for (i, hit) in [
        (0, NoteHit::PickHour(-1)),
        (2, NoteHit::PickHour(1)),
        (4, NoteHit::PickMinute(-1)),
        (6, NoteHit::PickMinute(1)),
    ] {
        if inside(picker_time(r, scale, i), px, py) {
            return Some(hit);
        }
    }
    if inside(picker_done(r, scale), px, py) {
        return Some(NoteHit::PickDone);
    }
    Some(NoteHit::PickBackground)
}

pub fn remind_line(pill: Rect, scale: f32) -> Rect {
    let panel = remind_panel(pill, scale);
    (panel.0 + 4.0 * scale, panel.1 + 2.0 * scale, panel.2 - 4.0 * scale, panel.1 + 32.0 * scale)
}

pub fn hit_note_panel(pill: Rect, scale: f32, px: f32, py: f32, ui: &UiState) -> Option<NoteHit> {
    let reminder = ui.notes.remind.is_some();
    if let (true, Some(view)) = (reminder, ui.notes.picker) {
        return hit_picker(pill, scale, px, py, view);
    }
    if ui.notes.current.is_some() && inside(type_button(pill, scale, reminder), px, py) {
        return Some(NoteHit::TypeToggle);
    }
    if reminder {
        if inside(remind_line(pill, scale), px, py) {
            return Some(NoteHit::RemindOpen);
        }
        return (0..8).find(|&i| inside(remind_button(pill, scale, i), px, py)).map(NoteHit::Remind);
    }
    None
}

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const WEEKDAYS: [&str; 7] = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];

fn paint_picker(p: &Painter, pill: Rect, scale: f32, ui: &UiState, view: (i32, u32), due: i64) -> Result<()> {
    use chrono::{Datelike, TimeZone, Timelike};
    let r = picker_rect(pill, scale);
    let (pr, pg, pb) = crate::ui::theme::panel();
    let lift = |c: f32| c + (1.0 - c) * 0.07;
    for i in 0..5 {
        let g = i as f32 * 1.5 * scale;
        p.fill_round_rect(
            (r.0 - g, r.1 - g + 3.0 * scale, r.2 + g, r.3 + g + 3.0 * scale),
            16.0 * scale + g,
            rgba(0.0, 0.0, 0.0, 0.06 * (1.0 - i as f32 / 5.0)),
        )?;
    }
    p.fill_round_rect(r, 16.0 * scale, rgba(lift(pr), lift(pg), lift(pb), 1.0))?;
    p.stroke_round_rect(r, 16.0 * scale, 1.0, rgba(1.0, 1.0, 1.0, 0.14))?;
    let now = chrono::Local::now();
    let when = chrono::Local.timestamp_opt(due, 0).single().unwrap_or(now);
    let hot = |h: NoteHit| ui.notes.hover == Some(h);
    let button = |rect: Rect, glyph: char, on: bool| -> Result<()> {
        if on {
            p.fill_round_rect(rect, 8.0 * scale, rgba(1.0, 1.0, 1.0, 0.14))?;
        }
        p.glyph(glyph, rect, 13.0 * scale, rgba(1.0, 1.0, 1.0, if on { 1.0 } else { 0.75 }))
    };
    button(picker_arrow(r, scale, false), '\u{E76B}', hot(NoteHit::PickMonth(-1)))?;
    button(picker_arrow(r, scale, true), '\u{E76C}', hot(NoteHit::PickMonth(1)))?;
    p.text(
        &format!("{} {}", crate::ui::i18n::tr(MONTHS[(view.1 as usize + 11) % 12]), view.0),
        (r.0 + 40.0 * scale, r.1 + 4.0 * scale, r.2 - 40.0 * scale, r.1 + 30.0 * scale),
        14.0 * scale,
        DWRITE_FONT_WEIGHT_SEMI_BOLD,
        DWRITE_TEXT_ALIGNMENT_CENTER,
        WHITE,
    )?;
    let cw = (PICK_W - 16.0) * scale / 7.0;
    for (i, name) in WEEKDAYS.iter().enumerate() {
        let left = r.0 + 8.0 * scale + i as f32 * cw;
        p.text(
            name,
            (left, r.1 + 36.0 * scale, left + cw, r.1 + 52.0 * scale),
            11.0 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_CENTER,
            rgba(1.0, 1.0, 1.0, 0.45),
        )?;
    }
    let (days, _) = month_shape(view.0, view.1);
    let today = now.date_naive();
    for d in 1..=days {
        let cell = picker_cell(r, scale, view.0, view.1, d);
        let date = chrono::NaiveDate::from_ymd_opt(view.0, view.1, d);
        let past = date.is_some_and(|x| x < today);
        let chosen = date == Some(when.date_naive());
        let inner = (cell.0 + 1.5 * scale, cell.1 + 1.0 * scale, cell.2 - 1.5 * scale, cell.3 - 1.0 * scale);
        if chosen {
            p.fill_round_rect(inner, 8.0 * scale, acc(0.85))?;
        } else if hot(NoteHit::PickDay(d)) && !past {
            p.fill_round_rect(inner, 8.0 * scale, rgba(1.0, 1.0, 1.0, 0.14))?;
        } else if date == Some(today) {
            p.stroke_round_rect(inner, 8.0 * scale, 1.0, acc(0.8))?;
        }
        let alpha = if past { 0.25 } else { 1.0 };
        p.text(
            &d.to_string(),
            cell,
            12.5 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_CENTER,
            rgba(1.0, 1.0, 1.0, alpha),
        )?;
    }
    for (i, glyph, hit) in [
        (0, '\u{E76B}', NoteHit::PickHour(-1)),
        (2, '\u{E76C}', NoteHit::PickHour(1)),
        (4, '\u{E76B}', NoteHit::PickMinute(-1)),
        (6, '\u{E76C}', NoteHit::PickMinute(1)),
    ] {
        button(picker_time(r, scale, i), glyph, hot(hit))?;
    }
    for (i, text) in [(1, format!("{:02}", when.hour())), (3, ":".to_string()), (5, format!("{:02}", when.minute()))] {
        p.text(
            &text,
            picker_time(r, scale, i),
            17.0 * scale,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_CENTER,
            WHITE,
        )?;
    }
    let done = picker_done(r, scale);
    p.fill_round_rect(done, 10.0 * scale, acc(if hot(NoteHit::PickDone) { 0.8 } else { 0.55 }))?;
    p.text("Done", done, 13.0 * scale, DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_TEXT_ALIGNMENT_CENTER, WHITE)?;
    let _ = when.year();
    Ok(())
}

pub const NOTE_FONT: f32 = 15.0;

pub(in crate::view) fn tool_button(tree: Rect, scale: f32, i: usize) -> Rect {
    let size = TOOL_BTN * scale;
    let gap = 4.0 * scale;
    let left = if i == 3 { tree.2 - size - 4.0 * scale } else { tree.0 + 4.0 * scale + i as f32 * (size + gap) };
    (left, tree.1 + 3.0 * scale, left + size, tree.1 + 3.0 * scale + size)
}

pub fn tree_row(tree: Rect, scale: f32, slot: usize) -> Rect {
    let top = tree.1 + TREE_TOOLBAR_H * scale + slot as f32 * TREE_ROW_H * scale;
    (tree.0 + 4.0 * scale, top, tree.2 - 4.0 * scale, top + (TREE_ROW_H - 2.0) * scale)
}

fn tree_text_area(tree: Rect, scale: f32, slot: usize, depth: usize) -> Rect {
    let r = tree_row(tree, scale, slot);
    let icon_right = r.0 + 28.0 * scale + depth as f32 * TREE_INDENT * scale;
    (icon_right + 2.0 * scale, r.1, r.2 - 6.0 * scale, r.3)
}

pub fn rename_frame(pill: Rect, scale: f32, ui: &UiState) -> Option<(Rect, Rect)> {
    let id = ui.notes.renaming?;
    let rows = ui.notes.rows();
    let index = rows.iter().position(|(_, i)| i.id == id)?;
    let slot = index.checked_sub(ui.notes.scroll)?;
    if slot >= tree_rows_fit(pill, scale) {
        return None;
    }
    let area = tree_text_area(note_tree(pill, scale), scale, slot, rows[index].0);
    Some((area, area))
}

pub fn tree_rows_fit(pill: Rect, scale: f32) -> usize {
    let tree = note_tree(pill, scale);
    (((tree.3 - tree.1) / scale - TREE_TOOLBAR_H) / TREE_ROW_H).floor().max(1.0) as usize
}

pub fn hit_note_tree(pill: Rect, scale: f32, px: f32, py: f32, ui: &UiState) -> Option<NoteHit> {
    let tree = note_tree(pill, scale);
    if !inside(tree, px, py) {
        return None;
    }
    for (i, hit) in [NoteHit::AddNote, NoteHit::AddFolder, NoteHit::Rename, NoteHit::Delete].into_iter().enumerate() {
        if inside(tool_button(tree, scale, i), px, py) {
            return Some(hit);
        }
    }
    let count = ui.notes.rows().len();
    (0..tree_rows_fit(pill, scale))
        .map(|slot| (slot, ui.notes.scroll + slot))
        .find(|&(slot, index)| index < count && inside(tree_row(tree, scale, slot), px, py))
        .map(|(_, index)| NoteHit::Row(index))
}

pub(in crate::view) fn paint_note(p: &Painter, pill: Rect, scale: f32, ui: &UiState) -> Result<()> {
    let reminder = ui.notes.remind;
    let (frame, inner) = note_frame_for(pill, scale, reminder.is_some());
    paint_editor(
        p,
        frame,
        inner,
        &ui.note,
        ui.editing && ui.notes.renaming.is_none(),
        ui.caret_on,
        "Write something, it saves itself",
        NOTE_FONT,
        scale,
    )?;
    if ui.notes.current.is_some() {
        let r = type_button(pill, scale, reminder.is_some());
        let hot = ui.notes.hover == Some(NoteHit::TypeToggle);
        p.fill_round_rect(
            r,
            9.0 * scale,
            if reminder.is_some() {
                acc(if hot { 0.7 } else { 0.5 })
            } else {
                rgba(1.0, 1.0, 1.0, if hot { 0.16 } else { 0.08 })
            },
        )?;
        p.glyph(if reminder.is_some() { '\u{E916}' } else { '\u{E70B}' }, r, 15.0 * scale, WHITE)?;
    }
    if let Some((_, due)) = reminder {
        use chrono::TimeZone;
        let panel = remind_panel(pill, scale);
        p.fill_round_rect(panel, 14.0 * scale, rgba(1.0, 1.0, 1.0, 0.05))?;
        if ui.notes.hover == Some(NoteHit::RemindOpen) {
            p.fill_round_rect(remind_line(pill, scale), 10.0 * scale, rgba(1.0, 1.0, 1.0, 0.08))?;
        }
        let now = chrono::Local::now();
        let when = chrono::Local.timestamp_opt(due, 0).single().unwrap_or(now);
        p.glyph(
            '\u{E916}',
            (panel.0 + 8.0 * scale, panel.1 + 4.0 * scale, panel.0 + 34.0 * scale, panel.1 + 30.0 * scale),
            15.0 * scale,
            acc(1.0),
        )?;
        p.text(
            &format!("{} · {}", when.format("%d.%m %H:%M"), crate::tools::remind::describe(when, now)),
            (panel.0 + 36.0 * scale, panel.1 + 4.0 * scale, panel.2 - 10.0 * scale, panel.1 + 30.0 * scale),
            13.0 * scale,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            WHITE,
        )?;
        for (i, label) in REMIND_BUTTONS.iter().enumerate() {
            let r = remind_button(pill, scale, i);
            let hot = ui.notes.hover == Some(NoteHit::Remind(i));
            p.fill_round_rect(r, 8.0 * scale, rgba(1.0, 1.0, 1.0, if hot { 0.18 } else { 0.09 }))?;
            p.text(label, r, 11.5 * scale, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_TEXT_ALIGNMENT_CENTER, WHITE)?;
        }
    }
    paint_note_tree(p, pill, scale, ui)?;
    if let (Some((_, due)), Some(view)) = (reminder, ui.notes.picker) {
        paint_picker(p, pill, scale, ui, view, due)?;
    }
    Ok(())
}

fn paint_note_tree(p: &Painter, pill: Rect, scale: f32, ui: &UiState) -> Result<()> {
    let n = &ui.notes;
    let tree = note_tree(pill, scale);
    p.fill_round_rect(tree, 14.0 * scale, rgba(1.0, 1.0, 1.0, 0.04))?;

    let selected_is_some = n.sel.is_some();
    let tools = [
        (NoteHit::AddNote, '\u{E710}', true),
        (NoteHit::AddFolder, '\u{E8F4}', true),
        (NoteHit::Rename, '\u{E70F}', selected_is_some),
        (NoteHit::Delete, '\u{E74D}', selected_is_some),
    ];
    for (i, (hit, glyph, enabled)) in tools.into_iter().enumerate() {
        let r = tool_button(tree, scale, i);
        let armed = hit == NoteHit::Delete && n.armed_delete;
        if armed {
            p.fill_round_rect(r, 9.0 * scale, rgba(1.0, 0.3, 0.3, 0.3))?;
        } else if n.hover == Some(hit) && enabled {
            p.fill_round_rect(r, 9.0 * scale, rgba(1.0, 1.0, 1.0, 0.1))?;
        }
        let color =
            if armed { rgba(1.0, 0.5, 0.5, 1.0) } else { rgba(1.0, 1.0, 1.0, if enabled { 0.8 } else { 0.25 }) };
        p.glyph(glyph, r, 14.0 * scale, color)?;
    }

    let rows = n.rows();
    p.push_clip((tree.0, tree.1 + TREE_TOOLBAR_H * scale - 2.0 * scale, tree.2, tree.3 - 4.0 * scale));
    let painted = (|| -> Result<()> {
        let shown = tree_rows_fit(pill, scale).min(rows.len().saturating_sub(n.scroll));
        let carried = n.reorder.as_ref().filter(|r| r.pos.len() == shown);
        let order: Vec<usize> =
            (0..shown).filter(|s| Some(*s) != carried.map(|r| r.from)).chain(carried.map(|r| r.from)).collect();
        for slot in order {
            let index = n.scroll + slot;
            let Some(&(depth, item)) = rows.get(index) else { continue };
            let r = tree_row(tree, scale, slot);
            let base = p.push_offset(0.0, 0.0);
            if let Some(c) = carried {
                if slot == c.from {
                    let at = c.dragged_rect((r.2 - r.0, r.3 - r.1));
                    p.push_offset(at.0 - r.0, at.1 - r.1);
                    p.fill_round_rect(r, 9.0 * scale, rgba(1.0, 1.0, 1.0, 0.16))?;
                } else {
                    p.push_offset(0.0, c.pos[slot].1 - r.1);
                }
            }
            let target = n.into == Some(item.id) && n.reorder.is_some();
            if target {
                p.fill_round_rect(r, 9.0 * scale, acc(0.4))?;
                p.stroke_round_rect(r, 9.0 * scale, 1.2 * scale, acc(0.9))?;
            }
            let is_current = Some(item.id) == n.current;
            let is_sel = Some(item.id) == n.sel;
            if is_current {
                p.fill_round_rect(r, 9.0 * scale, acc(0.22))?;
            } else if is_sel || n.hover == Some(NoteHit::Row(index)) {
                p.fill_round_rect(r, 9.0 * scale, rgba(1.0, 1.0, 1.0, if is_sel { 0.1 } else { 0.06 }))?;
            }
            let indent = depth as f32 * TREE_INDENT * scale;
            let glyph = if !item.folder && item.remind {
                '\u{E916}'
            } else if !item.folder {
                '\u{E8A5}'
            } else if n.expanded.contains(&item.id) {
                '\u{E838}'
            } else {
                '\u{E8B7}'
            };
            let icon = (r.0 + 4.0 * scale + indent, r.1, r.0 + 28.0 * scale + indent, r.3);
            p.glyph(glyph, icon, 14.0 * scale, rgba(1.0, 1.0, 1.0, if item.folder { 0.75 } else { 0.6 }))?;
            let text_area = tree_text_area(tree, scale, slot, depth);
            if n.renaming == Some(item.id) {
                paint_rename(p, text_area, scale, ui)?;
            } else {
                p.text(
                    &item.name,
                    text_area,
                    13.0 * scale,
                    if is_current { DWRITE_FONT_WEIGHT_SEMI_BOLD } else { DWRITE_FONT_WEIGHT_NORMAL },
                    DWRITE_TEXT_ALIGNMENT_LEADING,
                    rgba(1.0, 1.0, 1.0, if is_current { 1.0 } else { 0.85 }),
                )?;
            }
            p.set_transform(&base);
        }
        Ok(())
    })();
    p.pop_clip();
    painted?;

    let fit = tree_rows_fit(pill, scale);
    if rows.len() > fit {
        let track = (tree.1 + TREE_TOOLBAR_H * scale, tree.3 - 6.0 * scale);
        let h = (track.1 - track.0) * fit as f32 / rows.len() as f32;
        let y = track.0 + (track.1 - track.0 - h) * n.scroll as f32 / (rows.len() - fit) as f32;
        p.fill_round_rect((tree.2 - 4.0 * scale, y, tree.2 - 2.0 * scale, y + h), scale, rgba(1.0, 1.0, 1.0, 0.25))?;
    }
    Ok(())
}

fn paint_rename(p: &Painter, area: Rect, scale: f32, ui: &UiState) -> Result<()> {
    let input = &ui.notes.name_input;
    p.fill_round_rect(
        (area.0 - 3.0 * scale, area.1 + 2.0 * scale, area.2 + 3.0 * scale, area.3 - 2.0 * scale),
        7.0 * scale,
        rgba(1.0, 1.0, 1.0, 0.1),
    )?;
    p.stroke_round_rect(
        (area.0 - 3.0 * scale, area.1 + 2.0 * scale, area.2 + 3.0 * scale, area.3 - 2.0 * scale),
        7.0 * scale,
        1.2 * scale,
        acc(0.7),
    )?;
    if let Ok(layout) = crate::ui::gfx::make_layout(p.dwrite(), &input.text(), 100_000.0, 13.0 * scale) {
        let (_, _, line_h) = edit::caret_xy(&layout, 0);
        let top = area.1 + ((area.3 - area.1) - line_h) / 2.0;
        p.push_clip(area);
        p.draw_layout(area.0, top, &layout, WHITE)?;
        if ui.caret_on {
            let (cx, cy, ch) = edit::caret_xy(&layout, input.utf16_at(input.caret));
            p.fill_rect((area.0 + cx, top + cy, area.0 + cx + 1.6 * scale, top + cy + ch), WHITE)?;
        }
        p.pop_clip();
    }
    Ok(())
}
