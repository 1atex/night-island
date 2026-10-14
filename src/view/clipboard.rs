use crate::view::*;

#[derive(Default)]
pub struct ClipUi {
    pub query: TextEdit,
    pub scroll: usize,
    pub hover: Option<ClipHit>,
}

pub(in crate::view) const LIST_TOP: f32 = 66.0;

pub(in crate::view) const CLIP_TOP: f32 = 92.0;

pub const CLIP_FONT: f32 = 14.0;

pub(in crate::view) const LIST_BOTTOM_PAD: f32 = 16.0;

pub(in crate::view) const ROW_H: f32 = 36.0;

pub(in crate::view) const ROW_GAP: f32 = 4.0;

pub(in crate::view) const SIDE_PAD: f32 = 20.0;

pub(in crate::view) const BTN: f32 = 28.0;

pub fn clip_rows(pill: Rect, scale: f32) -> usize {
    let avail = pill.3 / scale - CLIP_TOP - LIST_BOTTOM_PAD;
    ((avail + ROW_GAP) / (ROW_H + ROW_GAP)).floor().max(1.0) as usize
}

pub(in crate::view) fn row_rect(pill: Rect, scale: f32, i: usize) -> Rect {
    let (x, y, w, _) = pill;
    let top = y + (CLIP_TOP + i as f32 * (ROW_H + ROW_GAP)) * scale;
    (x + SIDE_PAD * scale, top, x + w - SIDE_PAD * scale, top + ROW_H * scale)
}

pub(in crate::view) fn button_rect(row: Rect, scale: f32, delete: bool) -> Rect {
    let (_, t, r, b) = row;
    let size = BTN * scale;
    let right = if delete { r - 4.0 * scale } else { r - 4.0 * scale - size };
    let top = t + (b - t - size) / 2.0;
    (right - size, top, right, top + size)
}

pub fn hit_clip(pill: Rect, scale: f32, px: f32, py: f32, visible: usize) -> Option<ClipHit> {
    for i in 0..visible.min(clip_rows(pill, scale)) {
        let row = row_rect(pill, scale, i);
        if inside(button_rect(row, scale, true), px, py) {
            return Some(ClipHit::Delete(i));
        }
        if inside(button_rect(row, scale, false), px, py) {
            return Some(ClipHit::Pin(i));
        }
        if inside(row, px, py) {
            return Some(ClipHit::Row(i));
        }
    }
    None
}

pub(in crate::view) fn preview(text: &str) -> String {
    text.trim().chars().take(240).map(|c| if c.is_control() { ' ' } else { c }).collect()
}

pub fn clip_search_frame(pill: Rect, scale: f32) -> (Rect, Rect) {
    let (x, y, w, _) = pill;
    let frame = (x + SIDE_PAD * scale, y + 54.0 * scale, x + w - SIDE_PAD * scale, y + 84.0 * scale);
    (frame, (frame.0 + 34.0 * scale, frame.1, frame.2 - 12.0 * scale, frame.3))
}

pub(in crate::view) fn paint_clip_search(p: &Painter, pill: Rect, scale: f32, ui: &UiState) -> Result<()> {
    let (frame, inner) = clip_search_frame(pill, scale);
    p.fill_round_rect(frame, 12.0 * scale, rgba(1.0, 1.0, 1.0, 0.07))?;
    if ui.editing {
        p.stroke_round_rect(frame, 12.0 * scale, 1.2 * scale, acc(0.6))?;
    }
    p.glyph(
        '\u{E721}',
        (frame.0 + 6.0 * scale, frame.1, frame.0 + 32.0 * scale, frame.3),
        14.0 * scale,
        rgba(1.0, 1.0, 1.0, 0.6),
    )?;
    let text = ui.clip.query.text();
    if text.is_empty() {
        p.text(
            "Search the clipboard",
            inner,
            CLIP_FONT * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            rgba(1.0, 1.0, 1.0, 0.4),
        )?;
    }
    if let Ok(layout) = crate::ui::gfx::make_layout(p.dwrite(), &text, 100_000.0, CLIP_FONT * scale) {
        let (_, _, line_h) = edit::caret_xy(&layout, 0);
        let top = inner.1 + ((inner.3 - inner.1) - line_h) / 2.0;
        p.push_clip(inner);
        if ui.clip.query.has_selection() {
            let (a, b) = ui.clip.query.selection();
            let (a16, b16) = (ui.clip.query.utf16_at(a), ui.clip.query.utf16_at(b));
            for (rx, ry, rw, rh) in edit::selection_rects(&layout, a16, b16 - a16) {
                p.fill_rect((inner.0 + rx, top + ry, inner.0 + rx + rw, top + ry + rh), acc(0.35))?;
            }
        }
        p.draw_layout(inner.0, top, &layout, WHITE)?;
        if ui.editing && ui.caret_on {
            let (cx, cy, ch) = edit::caret_xy(&layout, ui.clip.query.utf16_at(ui.clip.query.caret));
            p.fill_rect((inner.0 + cx, top + cy, inner.0 + cx + 1.6 * scale, top + cy + ch), WHITE)?;
        }
        p.pop_clip();
    }
    Ok(())
}

pub(in crate::view) fn paint_clipboard(
    p: &Painter,
    pill: Rect,
    scale: f32,
    ui: &UiState,
    clips: &[Clip],
) -> Result<()> {
    paint_clip_search(p, pill, scale, ui)?;
    if clips.is_empty() {
        let (x, y, w, h) = pill;
        return p.text(
            if ui.clip.query.is_empty() { "Copy something and it will show up here" } else { "Nothing found" },
            (x, y + CLIP_TOP * scale, x + w, y + h - LIST_BOTTOM_PAD * scale),
            15.0 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_CENTER,
            rgba(1.0, 1.0, 1.0, DIM),
        );
    }
    let hover_row = match ui.clip.hover {
        Some(ClipHit::Row(i) | ClipHit::Pin(i) | ClipHit::Delete(i)) => Some(i),
        None => None,
    };
    for (i, clip) in clips.iter().skip(ui.clip.scroll).take(clip_rows(pill, scale)).enumerate() {
        let row = row_rect(pill, scale, i);
        let hovered = hover_row == Some(i);
        if hovered {
            p.fill_round_rect(row, 10.0 * scale, rgba(1.0, 1.0, 1.0, 0.09))?;
        } else {
            p.fill_round_rect(row, 10.0 * scale, rgba(1.0, 1.0, 1.0, 0.04))?;
        }
        let show_buttons = hovered || clip.pinned;
        let text_right = if hovered { button_rect(row, scale, false).0 } else { row.2 - 12.0 * scale };
        p.text(
            &preview(&clip.text),
            (row.0 + 14.0 * scale, row.1, text_right - 6.0 * scale, row.3),
            14.0 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            WHITE,
        )?;
        if show_buttons {
            let pin = button_rect(row, scale, false);
            if ui.clip.hover == Some(ClipHit::Pin(i)) {
                p.fill_round_rect(pin, 8.0 * scale, rgba(1.0, 1.0, 1.0, 0.12))?;
            }
            let pin_color = if clip.pinned { acc(1.0) } else { rgba(1.0, 1.0, 1.0, 0.6) };
            p.glyph(if clip.pinned { '\u{E842}' } else { '\u{E718}' }, pin, 14.0 * scale, pin_color)?;
        }
        if hovered {
            let del = button_rect(row, scale, true);
            if ui.clip.hover == Some(ClipHit::Delete(i)) {
                p.fill_round_rect(del, 8.0 * scale, rgba(1.0, 0.35, 0.35, 0.22))?;
            }
            p.glyph('\u{E74D}', del, 14.0 * scale, rgba(1.0, 1.0, 1.0, 0.7))?;
        }
    }
    Ok(())
}
