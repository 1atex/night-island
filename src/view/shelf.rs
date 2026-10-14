use crate::view::*;

pub(in crate::view) fn file_glyph(path: &std::path::Path) -> char {
    if path.is_dir() {
        return '\u{E8B7}';
    }
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "svg" | "ico" => '\u{E91B}',
        "mp4" | "mkv" | "avi" | "mov" | "webm" => '\u{E714}',
        "mp3" | "wav" | "flac" | "ogg" | "m4a" => '\u{E8D6}',
        _ => '\u{E8A5}',
    }
}

pub(in crate::view) fn paint_shelf(
    p: &Painter,
    pill: Rect,
    scale: f32,
    ui: &UiState,
    shelf: &[ShelfItem],
) -> Result<()> {
    if shelf.is_empty() {
        let (x, y, w, h) = pill;
        return p.text(
            "Drop files here, drag them out when you need them",
            (x, y + LIST_TOP * scale, x + w, y + h - LIST_BOTTOM_PAD * scale),
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
    for (i, item) in shelf.iter().skip(ui.shelf_scroll).take(clip_rows(pill, scale)).enumerate() {
        let row = row_rect(pill, scale, i);
        let hovered = hover_row == Some(i);
        p.fill_round_rect(row, 10.0 * scale, rgba(1.0, 1.0, 1.0, if hovered { 0.09 } else { 0.04 }))?;

        let path = std::path::Path::new(&item.path);
        let name = path.file_name().map_or_else(|| item.path.clone(), |n| n.to_string_lossy().into_owned());
        let icon = (row.0 + 6.0 * scale, row.1, row.0 + 6.0 * scale + 30.0 * scale, row.3);
        p.glyph(file_glyph(path), icon, 17.0 * scale, acc(1.0))?;
        let right = if hovered { button_rect(row, scale, true).0 } else { row.2 - 12.0 * scale };
        p.text(
            &name,
            (row.0 + 42.0 * scale, row.1, right - 6.0 * scale, row.3),
            14.0 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            WHITE,
        )?;
        if hovered {
            let del = button_rect(row, scale, true);
            if ui.clip.hover == Some(ClipHit::Delete(i)) {
                p.fill_round_rect(del, 8.0 * scale, rgba(1.0, 0.35, 0.35, 0.22))?;
            }
            p.glyph('\u{E711}', del, 13.0 * scale, rgba(1.0, 1.0, 1.0, 0.7))?;
        }
    }
    Ok(())
}
