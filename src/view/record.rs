use crate::view::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RecHit {
    Start,
    Folder,
}

pub(in crate::view) fn right_column(pill: Rect, scale: f32) -> (f32, f32) {
    let (x, _, w, _) = pill;
    let (side, gap) = (SIDE_PAD * scale, SET_COL_GAP * scale);
    let col_w = (w - 2.0 * side - gap) / 2.0;
    (x + side + col_w + gap, x + w - side)
}

pub(in crate::view) fn rec_start_button(pill: Rect, scale: f32) -> Rect {
    let (l, r) = right_column(pill, scale);
    let top = pill.1 + (LIST_TOP + 2.0) * scale;
    (l, top, r, top + 64.0 * scale)
}

pub(in crate::view) fn rec_folder_button(pill: Rect, scale: f32) -> Rect {
    let (l, r) = right_column(pill, scale);
    let top = pill.1 + (LIST_TOP + 2.0 + 76.0) * scale;
    (l, top, r, top + 38.0 * scale)
}

pub fn hit_record(pill: Rect, scale: f32, px: f32, py: f32) -> Option<RecHit> {
    if inside(rec_start_button(pill, scale), px, py) {
        Some(RecHit::Start)
    } else if inside(rec_folder_button(pill, scale), px, py) {
        Some(RecHit::Folder)
    } else {
        None
    }
}

pub(in crate::view) fn paint_record(p: &Painter, pill: Rect, scale: f32, ui: &UiState) -> Result<()> {
    paint_rows(p, pill, scale, ui, &RECORD_ROWS)?;

    let recording = ui.recording_secs.is_some();
    let start = rec_start_button(pill, scale);
    let (fill, title, hint) = if recording {
        (rgba(1.0, 0.3, 0.3, 0.32), "Stop recording".to_string(), "Ctrl+Alt+R  ·  or click the pill".to_string())
    } else {
        (rgba(1.0, 0.3, 0.3, 0.2), "Record an area".to_string(), "Ctrl+Alt+R".to_string())
    };
    let hover_start = ui.home.hover_act == Some(1400);
    let fill = if hover_start { rgba(1.0, 0.3, 0.3, fill.a + 0.12) } else { fill };
    p.fill_round_rect(start, 16.0 * scale, fill)?;
    let dot = (
        start.0 + 22.0 * scale,
        (start.1 + start.3) / 2.0 - 7.0 * scale,
        start.0 + 36.0 * scale,
        (start.1 + start.3) / 2.0 + 7.0 * scale,
    );
    p.fill_round_rect(dot, 7.0 * scale, rgba(1.0, 0.33, 0.33, 1.0))?;
    p.text(
        &title,
        (start.0 + 48.0 * scale, start.1 + 8.0 * scale, start.2, start.1 + 34.0 * scale),
        16.0 * scale,
        DWRITE_FONT_WEIGHT_BOLD,
        DWRITE_TEXT_ALIGNMENT_LEADING,
        WHITE,
    )?;
    p.text(
        &hint,
        (start.0 + 48.0 * scale, start.1 + 34.0 * scale, start.2, start.1 + 56.0 * scale),
        12.0 * scale,
        DWRITE_FONT_WEIGHT_NORMAL,
        DWRITE_TEXT_ALIGNMENT_LEADING,
        rgba(1.0, 1.0, 1.0, DIM),
    )?;

    let folder = rec_folder_button(pill, scale);
    p.fill_round_rect(
        folder,
        12.0 * scale,
        rgba(1.0, 1.0, 1.0, if ui.home.hover_act == Some(1401) { 0.14 } else { 0.07 }),
    )?;
    p.glyph('\u{E838}', (folder.0 + 8.0 * scale, folder.1, folder.0 + 40.0 * scale, folder.3), 15.0 * scale, acc(1.0))?;
    p.text(
        "Open the recordings folder",
        (folder.0 + 42.0 * scale, folder.1, folder.2, folder.3),
        13.0 * scale,
        DWRITE_FONT_WEIGHT_NORMAL,
        DWRITE_TEXT_ALIGNMENT_LEADING,
        WHITE,
    )?;

    let notes: [&str; 3] = if ui.cfg.record_format == RecordFormat::Gif {
        ["GIF: silent, up to 720 px wide.", "Only the part of a frame that", "changed is stored: small files."]
    } else {
        ["MP4: hardware H.264 + AAC sound.", "The GPU captures and encodes: a still", "screen costs almost nothing."]
    };
    let top = folder.3 + 14.0 * scale;
    for (i, line) in notes.iter().enumerate() {
        p.text(
            line,
            (folder.0 + 4.0 * scale, top + i as f32 * 18.0 * scale, folder.2, top + (i as f32 + 1.0) * 18.0 * scale),
            12.0 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            rgba(1.0, 1.0, 1.0, 0.5),
        )?;
    }
    Ok(())
}
