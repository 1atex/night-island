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

pub fn note_frame(pill: Rect, scale: f32) -> (Rect, Rect) {
    let (x, y, w, h) = pill;
    let tree = note_tree(pill, scale);
    let frame =
        (tree.2 + 10.0 * scale, y + LIST_TOP * scale, x + w - SIDE_PAD * scale, y + h - LIST_BOTTOM_PAD * scale);
    let inner = (frame.0 + 14.0 * scale, frame.1 + 10.0 * scale, frame.2 - 14.0 * scale, frame.3 - 10.0 * scale);
    (frame, inner)
}

pub const NOTE_FONT: f32 = 15.0;

fn tool_button(tree: Rect, scale: f32, i: usize) -> Rect {
    let size = TOOL_BTN * scale;
    let gap = 4.0 * scale;
    let left = if i == 3 { tree.2 - size - 4.0 * scale } else { tree.0 + 4.0 * scale + i as f32 * (size + gap) };
    (left, tree.1 + 3.0 * scale, left + size, tree.1 + 3.0 * scale + size)
}

pub(in crate::view) fn tree_row(tree: Rect, scale: f32, slot: usize) -> Rect {
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
    let (frame, inner) = note_frame(pill, scale);
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
    paint_note_tree(p, pill, scale, ui)
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
        for slot in 0..tree_rows_fit(pill, scale) {
            let index = n.scroll + slot;
            let Some(&(depth, item)) = rows.get(index) else { break };
            let r = tree_row(tree, scale, slot);
            let is_current = Some(item.id) == n.current;
            let is_sel = Some(item.id) == n.sel;
            if is_current {
                p.fill_round_rect(r, 9.0 * scale, acc(0.22))?;
            } else if is_sel || n.hover == Some(NoteHit::Row(index)) {
                p.fill_round_rect(r, 9.0 * scale, rgba(1.0, 1.0, 1.0, if is_sel { 0.1 } else { 0.06 }))?;
            }
            let indent = depth as f32 * TREE_INDENT * scale;
            let glyph = if !item.folder {
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
