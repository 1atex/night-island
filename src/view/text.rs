use crate::view::*;

#[derive(Default)]
pub struct TextUi {
    pub input: TextEdit,
    pub status: String,
    pub qr: Option<(Vec<bool>, usize)>,
    pub hover: Option<TextHit>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TextHit {
    Chip(usize),
    Action(usize),
}

pub const TEXT_ACTIONS: [&str; 4] = ["Paste clipboard", "Copy", "Paste to window", "Clear"];

pub(in crate::view) const TEXT_FIELD_H: f32 = 70.0;

pub(in crate::view) const CHIP_TOP: f32 = 158.0;

pub(in crate::view) const CHIP_H: f32 = 30.0;

pub(in crate::view) const CHIP_GAP: f32 = 6.0;

pub(in crate::view) const CHIP_COLS: usize = 5;

pub(in crate::view) const ACTION_TOP: f32 = 268.0;

pub fn text_frame(pill: Rect, scale: f32) -> (Rect, Rect) {
    let (x, y, w, _) = pill;
    let frame =
        (x + SIDE_PAD * scale, y + LIST_TOP * scale, x + w - SIDE_PAD * scale, y + (LIST_TOP + TEXT_FIELD_H) * scale);
    let inner = (frame.0 + 14.0 * scale, frame.1 + 8.0 * scale, frame.2 - 14.0 * scale, frame.3 - 8.0 * scale);
    (frame, inner)
}

pub(in crate::view) fn chip_rect(pill: Rect, scale: f32, i: usize) -> Rect {
    let (x, y, w, _) = pill;
    let (side, gap) = (SIDE_PAD * scale, CHIP_GAP * scale);
    let cw = (w - 2.0 * side - (CHIP_COLS - 1) as f32 * gap) / CHIP_COLS as f32;
    let (col, row) = ((i % CHIP_COLS) as f32, (i / CHIP_COLS) as f32);
    let left = x + side + col * (cw + gap);
    let top = y + (CHIP_TOP + row * (CHIP_H + CHIP_GAP)) * scale;
    (left, top, left + cw, top + CHIP_H * scale)
}

pub(in crate::view) fn action_rect(pill: Rect, scale: f32, i: usize) -> Rect {
    let (x, y, w, _) = pill;
    let n = TEXT_ACTIONS.len();
    let (side, gap) = (SIDE_PAD * scale, 8.0 * scale);
    let bw = (w - 2.0 * side - (n - 1) as f32 * gap) / n as f32;
    let left = x + side + i as f32 * (bw + gap);
    let top = y + ACTION_TOP * scale;
    (left, top, left + bw, top + CHIP_H * scale)
}

pub fn hit_text(pill: Rect, scale: f32, px: f32, py: f32) -> Option<TextHit> {
    if let Some(i) = (0..OPS.len()).find(|&i| inside(chip_rect(pill, scale, i), px, py)) {
        return Some(TextHit::Chip(i));
    }
    (0..TEXT_ACTIONS.len()).find(|&i| inside(action_rect(pill, scale, i), px, py)).map(TextHit::Action)
}

pub(in crate::view) fn paint_qr(p: &Painter, pill: Rect, scale: f32, modules: &[bool], side: usize) -> Result<()> {
    let (x, y, w, h) = pill;
    let area = (
        x + SIDE_PAD * scale,
        y + (LIST_TOP + TEXT_FIELD_H + 12.0) * scale,
        x + w - SIDE_PAD * scale,
        y + h - 12.0 * scale,
    );
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

    if !ui.text.status.is_empty() {
        p.text(
            &ui.text.status,
            (frame.0 + 4.0 * scale, frame.3 + 2.0 * scale, frame.2, frame.3 + 18.0 * scale),
            11.5 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            acc(1.0),
        )?;
    }
    if let Some((modules, side)) = &ui.text.qr {
        return paint_qr(p, pill, scale, modules, *side);
    }
    for (i, (_, label)) in OPS.iter().enumerate() {
        let r = chip_rect(pill, scale, i);
        let alpha = if ui.text.hover == Some(TextHit::Chip(i)) { 0.13 } else { 0.06 };
        p.fill_round_rect(r, 9.0 * scale, rgba(1.0, 1.0, 1.0, alpha))?;
        p.text(label, r, 12.5 * scale, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_TEXT_ALIGNMENT_CENTER, WHITE)?;
    }
    for (i, label) in TEXT_ACTIONS.iter().enumerate() {
        let r = action_rect(pill, scale, i);
        let hovered = ui.text.hover == Some(TextHit::Action(i));
        let alpha = if hovered { 0.34 } else { 0.2 };
        p.fill_round_rect(r, 9.0 * scale, acc(alpha))?;
        p.text(label, r, 12.5 * scale, DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_TEXT_ALIGNMENT_CENTER, WHITE)?;
    }
    Ok(())
}
