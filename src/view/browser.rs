use crate::services::browser::{Action, Download, State};
use crate::view::*;

pub fn browser_active(ui: &UiState) -> Option<&Download> {
    ui.browser.items.iter().rev().find(|item| matches!(item.state, State::Active | State::Paused))
}

pub fn browser_rows(pill: Rect, scale: f32) -> usize {
    (((pill.3 - 64.0 * scale) / (76.0 * scale)).floor() as usize).max(1)
}

fn row(pill: Rect, scale: f32, index: usize) -> Rect {
    let y = pill.1 + (60.0 + index as f32 * 76.0) * scale;
    (pill.0 + 16.0 * scale, y, pill.0 + pill.2 - 16.0 * scale, y + 68.0 * scale)
}

fn button(rect: Rect, scale: f32, index: usize, action: Action) -> Rect {
    let width = if action == Action::Reveal { 114.0 } else { 88.0 } * scale;
    let right = rect.2 - 8.0 * scale - index as f32 * 94.0 * scale;
    (right - width, rect.3 - 30.0 * scale, right, rect.3 - 4.0 * scale)
}

pub fn hit_browser(pill: Rect, scale: f32, x: f32, y: f32, ui: &UiState) -> Option<(usize, Action)> {
    for (offset, item) in
        ui.browser.items.iter().rev().skip(ui.browser_scroll).take(browser_rows(pill, scale)).enumerate()
    {
        let index = ui.browser.items.len() - 1 - ui.browser_scroll - offset;
        for (i, action) in item.actions.iter().enumerate() {
            if inside(button(row(pill, scale, offset), scale, i, *action), x, y) {
                return Some((index, *action));
            }
        }
    }
    None
}

pub(in crate::view) fn paint_browser(p: &Painter, pill: Rect, scale: f32, ui: &UiState) -> Result<()> {
    let (x, y, w, _) = pill;
    p.text(
        "Downloads",
        (x + 18.0 * scale, y + 10.0 * scale, x + w - 18.0 * scale, y + 35.0 * scale),
        20.0 * scale,
        DWRITE_FONT_WEIGHT_SEMI_BOLD,
        DWRITE_TEXT_ALIGNMENT_LEADING,
        WHITE,
    )?;
    let caption = if ui.browser.error {
        "The action is no longer available"
    } else if !ui.browser.connected {
        "Waiting for Night Browser"
    } else if ui.browser.items.is_empty() {
        "New downloads will appear here"
    } else {
        "Night Browser"
    };
    p.text(
        caption,
        (x + 18.0 * scale, y + 36.0 * scale, x + w - 18.0 * scale, y + 55.0 * scale),
        12.0 * scale,
        DWRITE_FONT_WEIGHT_NORMAL,
        DWRITE_TEXT_ALIGNMENT_LEADING,
        rgba(1.0, 1.0, 1.0, 0.55),
    )?;
    for (offset, item) in
        ui.browser.items.iter().rev().skip(ui.browser_scroll).take(browser_rows(pill, scale)).enumerate()
    {
        let r = row(pill, scale, offset);
        p.fill_round_rect(r, 12.0 * scale, rgba(1.0, 1.0, 1.0, 0.055))?;
        p.text(
            &item.name,
            (r.0 + 12.0 * scale, r.1 + 4.0 * scale, r.2 - 12.0 * scale, r.1 + 29.0 * scale),
            13.0 * scale,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            WHITE,
        )?;
        if let Some(progress) = item.progress() {
            let bar = (r.0 + 12.0 * scale, r.1 + 30.0 * scale, r.2 - 12.0 * scale, r.1 + 32.0 * scale);
            p.fill_round_rect(bar, scale, rgba(1.0, 1.0, 1.0, 0.12))?;
            p.fill_round_rect((bar.0, bar.1, bar.0 + (bar.2 - bar.0) * progress, bar.3), scale, WHITE)?;
        }
        let label = match item.state {
            State::Active => item
                .progress()
                .map(|f| format!("{}%", (f * 100.0) as u32))
                .unwrap_or_else(|| format!("{} MB", item.current / 1048576)),
            State::Paused => "Paused".into(),
            State::Complete => "Complete".into(),
            State::Failed => "Failed".into(),
            State::Cancelled => "Cancelled".into(),
        };
        p.text(
            &label,
            (r.0 + 12.0 * scale, r.3 - 30.0 * scale, r.0 + 110.0 * scale, r.3 - 4.0 * scale),
            11.0 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            rgba(1.0, 1.0, 1.0, 0.6),
        )?;
        for (i, action) in item.actions.iter().enumerate() {
            let rect = button(r, scale, i, *action);
            let hovered = ui.browser_hover.as_ref().is_some_and(|(id, selected)| id == &item.id && selected == action);
            p.fill_round_rect(rect, 13.0 * scale, rgba(1.0, 1.0, 1.0, if hovered { 0.18 } else { 0.08 }))?;
            p.text(action.label(), rect, 11.0 * scale, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_TEXT_ALIGNMENT_CENTER, WHITE)?;
        }
    }
    Ok(())
}
