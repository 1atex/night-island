use crate::view::*;

#[derive(Default)]
pub struct CommandUi {
    pub open: bool,
    pub input: TextEdit,
    pub items: Vec<cmd::Item>,
    pub sel: usize,
    pub ai_pending: bool,
    pub files_shown: bool,
}

pub const CMD_FONT: f32 = 18.0;

pub(in crate::view) const CMD_LIST_TOP: f32 = 76.0;

pub fn cmd_field(pill: Rect, scale: f32) -> (Rect, Rect) {
    let (x, y, w, _) = pill;
    let frame = (x + SIDE_PAD * scale, y + 16.0 * scale, x + w - SIDE_PAD * scale, y + 60.0 * scale);
    let inner = (frame.0 + 46.0 * scale, frame.1, frame.2 - 14.0 * scale, frame.3);
    (frame, inner)
}

pub(in crate::view) fn cmd_row_rect(pill: Rect, scale: f32, i: usize) -> Rect {
    let (x, y, w, _) = pill;
    let top = y + (CMD_LIST_TOP + i as f32 * (ROW_H + ROW_GAP)) * scale;
    (x + SIDE_PAD * scale, top, x + w - SIDE_PAD * scale, top + ROW_H * scale)
}

pub fn cmd_row_buttons(row: Rect, scale: f32, has_file: bool) -> (Option<Rect>, Rect) {
    let side = 30.0 * scale;
    let top = (row.1 + row.3 - side) / 2.0;
    let admin = (row.2 - 8.0 * scale - side, top, row.2 - 8.0 * scale, top + side);
    let folder = has_file.then_some((admin.0 - 6.0 * scale - side, top, admin.0 - 6.0 * scale, top + side));
    (folder, admin)
}

pub fn hit_cmd_button(pill: Rect, scale: f32, px: f32, py: f32, ui: &UiState) -> Option<usize> {
    let item = ui.cmd.items.get(ui.cmd.sel)?;
    let cmd::Action::Launch { path, .. } = &item.action else { return None };
    let row = cmd_row_rect(pill, scale, ui.cmd.sel);
    let (folder, admin) = cmd_row_buttons(row, scale, !path.as_os_str().is_empty());
    if folder.is_some_and(|r| inside(r, px, py)) {
        return Some(0);
    }
    inside(admin, px, py).then_some(1)
}

pub fn hit_cmd(pill: Rect, scale: f32, px: f32, py: f32, count: usize) -> Option<usize> {
    (0..count.min(cmd::MAX_ITEMS)).find(|&i| inside(cmd_row_rect(pill, scale, i), px, py))
}

pub(in crate::view) fn paint_command(p: &Painter, pill: Rect, scale: f32, ui: &UiState) -> Result<()> {
    let (frame, inner) = cmd_field(pill, scale);
    p.fill_round_rect(frame, 14.0 * scale, rgba(1.0, 1.0, 1.0, 0.07))?;
    p.stroke_round_rect(frame, 14.0 * scale, 1.2 * scale, acc(0.6))?;
    p.glyph(
        '\u{E721}',
        (frame.0 + 8.0 * scale, frame.1, frame.0 + 42.0 * scale, frame.3),
        17.0 * scale,
        rgba(1.0, 1.0, 1.0, 0.7),
    )?;

    let text = ui.cmd.input.text();
    if text.is_empty() {
        p.text(
            "Search apps, calculate, run commands",
            inner,
            CMD_FONT * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            rgba(1.0, 1.0, 1.0, 0.4),
        )?;
    }
    if let Ok(layout) = crate::ui::gfx::make_layout(p.dwrite(), &text, 100_000.0, CMD_FONT * scale) {
        let (_, _, line_h) = edit::caret_xy(&layout, 0);
        let top = inner.1 + ((inner.3 - inner.1) - line_h) / 2.0;
        p.push_clip(inner);
        if ui.cmd.input.has_selection() {
            let (a, b) = ui.cmd.input.selection();
            let (a16, b16) = (ui.cmd.input.utf16_at(a), ui.cmd.input.utf16_at(b));
            for (rx, ry, rw, rh) in edit::selection_rects(&layout, a16, b16 - a16) {
                p.fill_rect((inner.0 + rx, top + ry, inner.0 + rx + rw, top + ry + rh), acc(0.35))?;
            }
        }
        p.draw_layout(inner.0, top, &layout, WHITE)?;
        if ui.editing && ui.caret_on {
            let (cx, cy, ch) = edit::caret_xy(&layout, ui.cmd.input.utf16_at(ui.cmd.input.caret));
            p.fill_rect((inner.0 + cx, top + cy, inner.0 + cx + 1.6 * scale, top + cy + ch), WHITE)?;
        }
        p.pop_clip();
    }

    if ui.cmd.items.is_empty() {
        let (x, y, w, _) = pill;
        return p.text(
            "Apps  ·  2+2  ·  10 km to mi  ·  lock  ·  wifi  ·  /files  ·  ?ask AI",
            (x, y + CMD_LIST_TOP * scale, x + w, y + (CMD_LIST_TOP + 40.0) * scale),
            13.0 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_CENTER,
            rgba(1.0, 1.0, 1.0, 0.45),
        );
    }
    for (i, item) in ui.cmd.items.iter().take(cmd::MAX_ITEMS).enumerate() {
        let row = cmd_row_rect(pill, scale, i);
        let selected = ui.cmd.sel == i;
        let armed = ui.armed == Some(i);
        let bg = if armed {
            rgba(1.0, 0.35, 0.35, 0.26)
        } else if selected {
            acc(0.22)
        } else {
            rgba(1.0, 1.0, 1.0, 0.04)
        };
        p.fill_round_rect(row, 10.0 * scale, bg)?;
        match &item.icon {
            Some(icon) => {
                let side = 26.0 * scale;
                let top = (row.1 + row.3 - side) / 2.0;
                p.image(
                    (row.0 + 9.0 * scale, top, row.0 + 9.0 * scale + side, top + side),
                    5.0 * scale,
                    icon.side,
                    &icon.bgra,
                )?;
            }
            None => {
                p.glyph(item.glyph, (row.0 + 6.0 * scale, row.1, row.0 + 36.0 * scale, row.3), 17.0 * scale, acc(1.0))?
            }
        }
        let is_app = matches!(item.action, cmd::Action::Launch { .. });
        if is_app && selected {
            let (folder, admin) = cmd_row_buttons(
                row,
                scale,
                matches!(&item.action, cmd::Action::Launch { path, .. } if !path.as_os_str().is_empty()),
            );
            if let Some(r) = folder {
                p.fill_round_rect(
                    r,
                    9.0 * scale,
                    rgba(1.0, 1.0, 1.0, if ui.home.hover_act == Some(200) { 0.2 } else { 0.1 }),
                )?;
                p.glyph('\u{E838}', r, 14.0 * scale, WHITE)?;
            }
            p.fill_round_rect(
                admin,
                9.0 * scale,
                rgba(1.0, 1.0, 1.0, if ui.home.hover_act == Some(201) { 0.2 } else { 0.1 }),
            )?;
            p.glyph('\u{EA18}', admin, 14.0 * scale, WHITE)?;
        }
        p.text(
            &item.title,
            (row.0 + 44.0 * scale, row.1, row.2 - if is_app && selected { 96.0 } else { 220.0 } * scale, row.3),
            14.5 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            WHITE,
        )?;
        let hint = if armed { "Press Enter again" } else { item.hint.as_str() };
        if is_app && selected {
            continue;
        }
        p.text(
            hint,
            (row.2 - 214.0 * scale, row.1, row.2 - 12.0 * scale, row.3),
            11.5 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_TRAILING,
            rgba(1.0, 1.0, 1.0, 0.45),
        )?;
    }
    Ok(())
}
