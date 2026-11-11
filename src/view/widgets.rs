use crate::view::*;

pub(in crate::view) fn card_depth(p: &Painter, rect: Rect, radius: f32, scale: f32, ui: &UiState) -> Result<()> {
    if !ui.cfg.card_shadows {
        return Ok(());
    }
    for (spread, opacity) in [(1.65, 0.10), (0.75, 0.22)] {
        let d = spread * scale;
        let outline = (rect.0 - d, rect.1, rect.2 + d, rect.3 + 2.0 * d);
        p.stroke_round_rect(outline, radius + d, 1.4 * scale, rgba(0.0, 0.0, 0.0, opacity))?;
    }
    p.fill_round_gradient(rect, radius, rgba(1.0, 1.0, 1.0, 0.055), rgba(0.0, 0.0, 0.0, 0.035))?;
    let inset = 0.5 * scale;
    p.stroke_round_rect(
        (rect.0 + inset, rect.1 + inset, rect.2 - inset, rect.3 - inset),
        (radius - inset).max(0.0),
        scale,
        rgba(1.0, 1.0, 1.0, 0.095),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn paint_line_edit(
    p: &Painter,
    frame: Rect,
    inner: Rect,
    ed: &TextEdit,
    masked: bool,
    hint: &str,
    focused: bool,
    caret_on: bool,
    scale: f32,
) -> Result<()> {
    p.fill_round_rect(frame, 12.0 * scale, rgba(1.0, 1.0, 1.0, 0.08))?;
    if focused {
        p.stroke_round_rect(frame, 12.0 * scale, 1.2 * scale, acc(0.7))?;
    }
    let real = ed.text();
    if real.is_empty() {
        p.text(
            hint,
            inner,
            13.0 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            rgba(1.0, 1.0, 1.0, 0.4),
        )?;
    }
    let shown = if masked { "\u{2022}".repeat(real.chars().count()) } else { real.to_string() };
    if let Ok(layout) = crate::ui::gfx::make_layout(p.dwrite(), &shown, 100_000.0, 13.0 * scale) {
        let (_, _, line_h) = edit::caret_xy(&layout, 0);
        let top = inner.1 + ((inner.3 - inner.1) - line_h) / 2.0;
        let (caret_x, _, _) = edit::caret_xy(&layout, ed.utf16_at(ed.caret));
        let shift = if focused { (caret_x - (inner.2 - inner.0) + 8.0 * scale).max(0.0) } else { 0.0 };
        let left = inner.0 - shift;
        p.push_clip(inner);
        if ed.has_selection() {
            let (a, b) = ed.selection();
            let (a16, b16) = (ed.utf16_at(a), ed.utf16_at(b));
            for (rx, ry, rw, rh) in edit::selection_rects(&layout, a16, b16 - a16) {
                p.fill_rect((left + rx, top + ry, left + rx + rw, top + ry + rh), acc(0.35))?;
            }
        }
        p.draw_layout(left, top, &layout, WHITE)?;
        if focused && caret_on {
            let (cx, cy, ch) = edit::caret_xy(&layout, ed.utf16_at(ed.caret));
            p.fill_rect((left + cx, top + cy, left + cx + 1.6 * scale, top + cy + ch), WHITE)?;
        }
        p.pop_clip();
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(in crate::view) fn paint_editor(
    p: &Painter,
    frame: Rect,
    inner: Rect,
    ed: &TextEdit,
    editing: bool,
    caret_on: bool,
    hint: &str,
    font: f32,
    scale: f32,
) -> Result<()> {
    p.fill_round_rect(frame, 12.0 * scale, rgba(1.0, 1.0, 1.0, 0.05))?;
    if editing {
        p.stroke_round_rect(frame, 12.0 * scale, 1.2 * scale, acc(0.7))?;
    }
    if ed.is_empty() {
        let hint_rect = (inner.0, inner.1, inner.2, inner.1 + 24.0 * scale);
        p.text(
            hint,
            hint_rect,
            font * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            rgba(1.0, 1.0, 1.0, 0.4),
        )?;
    }
    let text = ed.text();
    let Ok(layout) = crate::ui::gfx::make_layout(p.dwrite(), &text, inner.2 - inner.0, font * scale) else {
        return Ok(());
    };
    p.push_clip(inner);
    let top = inner.1 - ed.scroll;
    if editing && ed.has_selection() {
        let (a, b) = ed.selection();
        let (a16, b16) = (ed.utf16_at(a), ed.utf16_at(b));
        for (rx, ry, rw, rh) in edit::selection_rects(&layout, a16, b16 - a16) {
            p.fill_rect((inner.0 + rx, top + ry, inner.0 + rx + rw, top + ry + rh), acc(0.35))?;
        }
    }
    p.draw_layout(inner.0, top, &layout, WHITE)?;
    if editing && caret_on {
        let (cx, cy, ch) = edit::caret_xy(&layout, ed.utf16_at(ed.caret));
        p.fill_rect((inner.0 + cx, top + cy, inner.0 + cx + 1.6 * scale, top + cy + ch), WHITE)?;
    }
    p.pop_clip();
    Ok(())
}
