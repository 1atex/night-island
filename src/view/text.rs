use crate::data::settings::TextLayout;
use crate::tools::texttools;
use crate::view::*;

#[derive(Default)]
pub struct TextUi {
    pub input: TextEdit,
    pub status: String,
    pub qr: Option<(Vec<bool>, usize)>,
    pub hover: Option<TextHit>,
    pub chip_scroll: usize,
    pub cat: usize,
    pub palette_open: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TextHit {
    Chip(usize),
    Action(usize),
    Cat(usize),
    Palette,
    PaletteClose,
}

pub const TEXT_ACTIONS: [&str; 5] = ["Paste clipboard", "Copy", "Paste to window", "Clear", "Developer"];

pub const DEV_ACTION: usize = 4;

const SIDE_W: f32 = 236.0;
const SIDE_GAP: f32 = 14.0;

const CHIP_H: f32 = 26.0;

const CHIP_GAP: f32 = 4.0;

const ACTION_H: f32 = 26.0;

const CHIP_COLS: usize = 2;

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
    let top = y + LIST_TOP * scale;
    let bottom = y + h - LIST_BOTTOM_PAD * scale - action_block_h(scale) - 8.0 * scale;
    (left, top, left + SIDE_W * scale, bottom.max(top))
}

const LIST_ROW_H: f32 = 40.0;
const CATALOGUE_ROW_H: f32 = 46.0;

pub fn text_groups(developer: bool) -> Vec<usize> {
    let all = ops(developer);
    (0..texttools::CATEGORIES.len()).filter(|&c| all.iter().any(|(op, _)| texttools::category(*op) == c)).collect()
}

struct Cell {
    hit: TextHit,
    rect: Rect,
    two_lines: bool,
}

fn scroll_rows(pill: Rect, scale: f32, cfg: &Settings, text: &TextUi) -> (usize, usize) {
    let n = ops(cfg.dev_tools).len();
    let area = chips_area(pill, scale);
    let gap = CHIP_GAP * scale;
    let fit = |h: f32, row: f32| (((h / scale) + CHIP_GAP) / (row + CHIP_GAP)).floor().max(1.0) as usize;
    match cfg.text_layout {
        TextLayout::Categories => (0, 0),
        TextLayout::List => (n, fit(area.3 - area.1, LIST_ROW_H)),
        TextLayout::Palette if text.palette_open => {
            let frame = text_frame(pill, scale).0;
            (n.div_ceil(2), fit(frame.3 - frame.1 - (CHIP_H * scale + gap), CATALOGUE_ROW_H))
        }
        TextLayout::Palette => (n.div_ceil(CHIP_COLS), fit(area.3 - area.1 - (CHIP_H * scale + gap), CHIP_H)),
    }
}

pub fn chip_scroll_max(pill: Rect, scale: f32, cfg: &Settings, text: &TextUi) -> usize {
    let (total, visible) = scroll_rows(pill, scale, cfg, text);
    total.saturating_sub(visible)
}

pub fn over_text_side(pill: Rect, scale: f32, px: f32, py: f32, text: &TextUi) -> bool {
    let frame = text_frame(pill, scale).0;
    (text.palette_open && inside(frame, px, py))
        || (px >= side_left(pill, scale) && px < pill.0 + pill.2 && py >= pill.1 + LIST_TOP * scale)
}

fn cells(pill: Rect, scale: f32, cfg: &Settings, text: &TextUi) -> Vec<Cell> {
    let all = ops(cfg.dev_tools);
    let area = chips_area(pill, scale);
    let gap = CHIP_GAP * scale;
    let col_w = |cols: usize, width: f32| (width - (cols - 1) as f32 * gap) / cols as f32;
    let grid = |i: usize, cols: usize, top: f32, row_h: f32, left: f32, width: f32| {
        let cw = col_w(cols, width);
        let l = left + (i % cols) as f32 * (cw + gap);
        let t = top + (i / cols) as f32 * (row_h + gap);
        (l, t, l + cw, t + row_h)
    };
    let mut out = Vec::new();
    match cfg.text_layout {
        TextLayout::Categories => {
            let groups = text_groups(cfg.dev_tools);
            for k in 0..groups.len() {
                out.push(Cell {
                    hit: TextHit::Cat(k),
                    rect: grid(k, 3, area.1, CHIP_H * scale, area.0, area.2 - area.0),
                    two_lines: false,
                });
            }
            let open = groups.get(text.cat.min(groups.len().saturating_sub(1))).copied();
            let top = area.1 + groups.len().div_ceil(3) as f32 * (CHIP_H * scale + gap) + 8.0 * scale;
            let mut n = 0;
            for (i, (op, _)) in all.iter().enumerate() {
                if Some(texttools::category(*op)) == open {
                    out.push(Cell {
                        hit: TextHit::Chip(i),
                        rect: grid(n, 2, top, CHIP_H * scale, area.0, area.2 - area.0),
                        two_lines: false,
                    });
                    n += 1;
                }
            }
        }
        TextLayout::List => {
            let (_, visible) = scroll_rows(pill, scale, cfg, text);
            for i in text.chip_scroll..(text.chip_scroll + visible).min(all.len()) {
                let t = area.1 + (i - text.chip_scroll) as f32 * (LIST_ROW_H * scale + gap);
                out.push(Cell {
                    hit: TextHit::Chip(i),
                    rect: (area.0, t, area.2, t + LIST_ROW_H * scale),
                    two_lines: true,
                });
            }
        }
        TextLayout::Palette if text.palette_open => {
            let frame = text_frame(pill, scale).0;
            let bar = (
                frame.0 + 8.0 * scale,
                frame.1 + 8.0 * scale,
                frame.2 - 8.0 * scale,
                frame.1 + 8.0 * scale + CHIP_H * scale,
            );
            out.push(Cell { hit: TextHit::PaletteClose, rect: bar, two_lines: false });
            let top = bar.3 + gap;
            let (_, visible) = scroll_rows(pill, scale, cfg, text);
            let first = text.chip_scroll * 2;
            for i in first..(first + visible * 2).min(all.len()) {
                let rect = grid(i - first, 2, top, CATALOGUE_ROW_H * scale, bar.0, bar.2 - bar.0);
                out.push(Cell { hit: TextHit::Chip(i), rect, two_lines: true });
            }
        }
        TextLayout::Palette => {
            out.push(Cell {
                hit: TextHit::Palette,
                rect: (area.0, area.1, area.2, area.1 + CHIP_H * scale),
                two_lines: false,
            });
            let top = area.1 + CHIP_H * scale + gap;
            let (_, visible) = scroll_rows(pill, scale, cfg, text);
            let first = text.chip_scroll * CHIP_COLS;
            for i in first..(first + visible * CHIP_COLS).min(all.len()) {
                let rect = grid(i - first, CHIP_COLS, top, CHIP_H * scale, area.0, area.2 - area.0);
                out.push(Cell { hit: TextHit::Chip(i), rect, two_lines: false });
            }
        }
    }
    out
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

pub fn hit_text(pill: Rect, scale: f32, px: f32, py: f32, cfg: &Settings, text: &TextUi) -> Option<TextHit> {
    if text.palette_open && cfg.text_layout == TextLayout::Palette {
        return cells(pill, scale, cfg, text).into_iter().find(|c| inside(c.rect, px, py)).map(|c| c.hit);
    }
    if let Some(cell) = cells(pill, scale, cfg, text).into_iter().find(|c| inside(c.rect, px, py)) {
        return Some(cell.hit);
    }
    (0..TEXT_ACTIONS.len()).find(|&i| inside(action_rect(pill, scale, i), px, py)).map(TextHit::Action)
}

pub fn catalogue_open(cfg: &Settings, text: &TextUi) -> bool {
    text.palette_open && cfg.text_layout == TextLayout::Palette
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

    let all = ops(ui.cfg.dev_tools);
    if !ui.text.status.is_empty() && !catalogue_open(&ui.cfg, &ui.text) {
        p.text(
            &ui.text.status,
            (frame.0 + 14.0 * scale, frame.3 - 26.0 * scale, frame.2 - 14.0 * scale, frame.3 - 6.0 * scale),
            11.5 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            acc(1.0),
        )?;
    }

    let groups = text_groups(ui.cfg.dev_tools);
    let open_group = ui.text.cat.min(groups.len().saturating_sub(1));
    let catalogue = catalogue_open(&ui.cfg, &ui.text);
    if catalogue {
        p.fill_round_rect(frame, 14.0 * scale, rgba(0.0, 0.0, 0.0, 0.82))?;
        p.stroke_round_rect(frame, 14.0 * scale, scale, rgba(1.0, 1.0, 1.0, 0.12))?;
    }
    for cell in cells(pill, scale, &ui.cfg, &ui.text) {
        let hovered = ui.text.hover == Some(cell.hit);
        let r = cell.rect;
        match cell.hit {
            TextHit::Chip(i) => {
                let Some((op, label)) = all.get(i) else { continue };
                let alpha = if hovered { 0.13 } else { 0.06 };
                p.fill_round_rect(r, 9.0 * scale, rgba(1.0, 1.0, 1.0, alpha))?;
                if cell.two_lines {
                    let pad = 10.0 * scale;
                    p.text(
                        label,
                        (r.0 + pad, r.1 + 3.0 * scale, r.2 - pad, (r.1 + r.3) / 2.0 + 1.0 * scale),
                        12.5 * scale,
                        DWRITE_FONT_WEIGHT_SEMI_BOLD,
                        DWRITE_TEXT_ALIGNMENT_LEADING,
                        WHITE,
                    )?;
                    p.text(
                        texttools::desc(*op),
                        (r.0 + pad, (r.1 + r.3) / 2.0 - 1.0 * scale, r.2 - pad, r.3 - 2.0 * scale),
                        10.5 * scale,
                        DWRITE_FONT_WEIGHT_NORMAL,
                        DWRITE_TEXT_ALIGNMENT_LEADING,
                        rgba(1.0, 1.0, 1.0, 0.55),
                    )?;
                } else {
                    p.text(label, r, 12.0 * scale, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_TEXT_ALIGNMENT_CENTER, WHITE)?;
                }
            }
            TextHit::Cat(k) => {
                let on = k == open_group;
                let alpha = match (on, hovered) {
                    (true, _) => 0.5,
                    (false, true) => 0.24,
                    (false, false) => 0.12,
                };
                p.fill_round_rect(r, 9.0 * scale, acc(alpha))?;
                let name = groups.get(k).map_or("", |c| texttools::CATEGORIES[*c]);
                p.text(name, r, 11.5 * scale, DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_TEXT_ALIGNMENT_CENTER, WHITE)?;
            }
            TextHit::Palette | TextHit::PaletteClose => {
                let open = cell.hit == TextHit::PaletteClose;
                p.fill_round_rect(r, 9.0 * scale, acc(if hovered { 0.5 } else { 0.32 }))?;
                p.text(
                    if open { "Close the catalogue" } else { "All tools…" },
                    r,
                    12.0 * scale,
                    DWRITE_FONT_WEIGHT_SEMI_BOLD,
                    DWRITE_TEXT_ALIGNMENT_CENTER,
                    WHITE,
                )?;
            }
            TextHit::Action(_) => {}
        }
    }
    let max = chip_scroll_max(pill, scale, &ui.cfg, &ui.text);
    if max > 0 {
        let area = if catalogue { frame } else { chips_area(pill, scale) };
        let (_, rows) = scroll_rows(pill, scale, &ui.cfg, &ui.text);
        let track_h = area.3 - area.1;
        let bar_h = track_h * rows as f32 / (rows + max) as f32;
        let top = area.1 + (track_h - bar_h) * ui.text.chip_scroll.min(max) as f32 / max as f32;
        let x = if catalogue { area.2 - 5.0 * scale } else { area.2 + 2.0 * scale };
        p.fill_round_rect((x, top, x + 2.0 * scale, top + bar_h), scale, rgba(1.0, 1.0, 1.0, 0.3))?;
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
