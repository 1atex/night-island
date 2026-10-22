use crate::view::*;

#[derive(Default)]
pub struct TextUi {
    pub input: TextEdit,
    pub status: String,
    pub qr: Option<(Vec<bool>, usize)>,
    pub hover: Option<TextHit>,
    pub chip_scroll: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TextHit {
    Chip(usize),
    Action(usize),
}

pub const TEXT_ACTIONS: [&str; 5] = ["Paste clipboard", "Copy", "Paste to window", "Clear", "Developer"];

pub const DEV_ACTION: usize = 4;

const SIDE_W: f32 = 236.0;
const SIDE_GAP: f32 = 14.0;

const CHIP_H: f32 = 26.0;

const CHIP_GAP: f32 = 4.0;

const ACTION_H: f32 = 26.0;

const CHIP_COLS: usize = 2;

const STATUS_H: f32 = 30.0;

pub fn text_frame(pill: Rect, scale: f32) -> (Rect, Rect) {
    let (x, y, w, h) = pill;
    let frame = (
        x + SIDE_PAD * scale,
        y + LIST_TOP * scale,
        x + w - (SIDE_PAD + SIDE_W + SIDE_GAP) * scale,
        y + h - LIST_BOTTOM_PAD * scale,
    );
    let inner = (frame.0 + 14.0 * scale, frame.1 + 8.0 * scale, frame.2 - 14.0 * scale, frame.3 - 8.0 * scale);
    (frame, inner)
}

fn side_left(pill: Rect, scale: f32) -> f32 {
    pill.0 + pill.2 - (SIDE_PAD + SIDE_W) * scale
}

fn action_block_h(scale: f32) -> f32 {
    (3.0 * ACTION_H + 2.0 * CHIP_GAP) * scale
}

fn chips_area(pill: Rect, scale: f32) -> Rect {
    let (_, y, _, h) = pill;
    let left = side_left(pill, scale);
    let top = y + (LIST_TOP + STATUS_H + 4.0) * scale;
    let bottom = y + h - LIST_BOTTOM_PAD * scale - action_block_h(scale) - 8.0 * scale;
    (left, top, left + SIDE_W * scale, bottom.max(top))
}

pub fn chip_rows(pill: Rect, scale: f32) -> usize {
    let area = chips_area(pill, scale);
    (((area.3 - area.1) / scale + CHIP_GAP) / (CHIP_H + CHIP_GAP)).floor().max(1.0) as usize
}

pub fn chip_scroll_max(pill: Rect, scale: f32, developer: bool) -> usize {
    let total = ops(developer).len().div_ceil(CHIP_COLS);
    total.saturating_sub(chip_rows(pill, scale))
}

pub fn over_text_side(pill: Rect, scale: f32, px: f32, py: f32) -> bool {
    px >= side_left(pill, scale) && px < pill.0 + pill.2 && py >= pill.1 + LIST_TOP * scale
}

fn chip_rect(pill: Rect, scale: f32, i: usize, scroll: usize) -> Option<Rect> {
    let area = chips_area(pill, scale);
    let row = (i / CHIP_COLS).checked_sub(scroll)?;
    if row >= chip_rows(pill, scale) {
        return None;
    }
    let gap = CHIP_GAP * scale;
    let cw = (area.2 - area.0 - (CHIP_COLS - 1) as f32 * gap) / CHIP_COLS as f32;
    let left = area.0 + (i % CHIP_COLS) as f32 * (cw + gap);
    let top = area.1 + row as f32 * (CHIP_H * scale + gap);
    Some((left, top, left + cw, top + CHIP_H * scale))
}

fn action_rect(pill: Rect, scale: f32, i: usize) -> Rect {
    let (_, y, _, h) = pill;
    let left = side_left(pill, scale);
    let right = left + SIDE_W * scale;
    let gap = CHIP_GAP * scale;
    let top0 = y + h - LIST_BOTTOM_PAD * scale - action_block_h(scale);
    let row_top = |row: usize| top0 + row as f32 * (ACTION_H * scale + gap);
    if i == DEV_ACTION {
        return (left, row_top(2), right, row_top(2) + ACTION_H * scale);
    }
    let half = (right - left - gap) / 2.0;
    let l = left + (i % 2) as f32 * (half + gap);
    (l, row_top(i / 2), l + half, row_top(i / 2) + ACTION_H * scale)
}

pub fn hit_text(pill: Rect, scale: f32, px: f32, py: f32, developer: bool, scroll: usize) -> Option<TextHit> {
    let chip =
        (0..ops(developer).len()).find(|&i| chip_rect(pill, scale, i, scroll).is_some_and(|r| inside(r, px, py)));
    if let Some(i) = chip {
        return Some(TextHit::Chip(i));
    }
    (0..TEXT_ACTIONS.len()).find(|&i| inside(action_rect(pill, scale, i), px, py)).map(TextHit::Action)
}

fn paint_qr(p: &Painter, pill: Rect, scale: f32, modules: &[bool], side: usize) -> Result<()> {
    let area = text_frame(pill, scale).0;
    p.fill_round_rect(area, 14.0 * scale, rgba(0.0, 0.0, 0.0, 0.55))?;
    let avail = (area.3 - area.1 - 34.0 * scale).min(area.2 - area.0);
    let quiet = 2usize;
    let cell = (avail / (side + 2 * quiet) as f32).floor().max(1.0);
    let size = cell * (side + 2 * quiet) as f32;
    let left = (area.0 + area.2) / 2.0 - size / 2.0;
    let top = area.1 + 8.0 * scale;
    p.fill_rect((left, top, left + size, top + size), WHITE)?;
    for (i, dark) in modules.iter().enumerate() {
        if *dark {
            let (cx, cy) = ((i % side + quiet) as f32, (i / side + quiet) as f32);
            p.fill_rect(
                (left + cx * cell, top + cy * cell, left + (cx + 1.0) * cell, top + (cy + 1.0) * cell),
                rgba(0.0, 0.0, 0.0, 1.0),
            )?;
        }
    }
    p.text(
        "Click to close",
        (area.0, area.3 - 28.0 * scale, area.2, area.3 - 4.0 * scale),
        12.0 * scale,
        DWRITE_FONT_WEIGHT_NORMAL,
        DWRITE_TEXT_ALIGNMENT_CENTER,
        rgba(1.0, 1.0, 1.0, 0.6),
    )
}

pub(in crate::view) fn paint_text(p: &Painter, pill: Rect, scale: f32, ui: &UiState) -> Result<()> {
    let (frame, inner) = text_frame(pill, scale);
    if let Some((modules, side)) = &ui.text.qr {
        paint_qr(p, pill, scale, modules, *side)?;
    } else {
        paint_editor(
            p,
            frame,
            inner,
            &ui.text.input,
            ui.editing,
            ui.caret_on,
            "Paste or type text, then pick a tool",
            NOTE_FONT,
            scale,
        )?;
    }

    if !ui.text.status.is_empty() {
        let (left, top) = (side_left(pill, scale), pill.1 + LIST_TOP * scale);
        p.text(
            &ui.text.status,
            (left + 2.0 * scale, top, left + SIDE_W * scale, top + STATUS_H * scale),
            11.5 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            acc(1.0),
        )?;
    }
    for (i, (_, label)) in ops(ui.cfg.dev_tools).iter().enumerate() {
        let Some(r) = chip_rect(pill, scale, i, ui.text.chip_scroll) else { continue };
        let alpha = if ui.text.hover == Some(TextHit::Chip(i)) { 0.13 } else { 0.06 };
        p.fill_round_rect(r, 9.0 * scale, rgba(1.0, 1.0, 1.0, alpha))?;
        p.text(label, r, 12.0 * scale, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_TEXT_ALIGNMENT_CENTER, WHITE)?;
    }
    let max = chip_scroll_max(pill, scale, ui.cfg.dev_tools);
    if max > 0 {
        let area = chips_area(pill, scale);
        let rows = chip_rows(pill, scale);
        let track_h = area.3 - area.1;
        let bar_h = track_h * rows as f32 / (rows + max) as f32;
        let top = area.1 + (track_h - bar_h) * ui.text.chip_scroll.min(max) as f32 / max as f32;
        p.fill_round_rect(
            (area.2 + 2.0 * scale, top, area.2 + 4.0 * scale, top + bar_h),
            scale,
            rgba(1.0, 1.0, 1.0, 0.3),
        )?;
    }
    for (i, label) in TEXT_ACTIONS.iter().enumerate() {
        let r = action_rect(pill, scale, i);
        let hovered = ui.text.hover == Some(TextHit::Action(i));
        let on = i == DEV_ACTION && ui.cfg.dev_tools;
        let alpha = match (on, hovered) {
            (true, true) => 0.75,
            (true, false) => 0.6,
            (false, true) => 0.34,
            (false, false) => 0.2,
        };
        p.fill_round_rect(r, 9.0 * scale, acc(alpha))?;
        p.text(label, r, 12.0 * scale, DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_TEXT_ALIGNMENT_CENTER, WHITE)?;
    }
    Ok(())
}
