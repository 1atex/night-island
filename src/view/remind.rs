use crate::view::*;

#[derive(Default)]
pub struct RemindUi {
    pub list: Vec<(i64, String, String)>,
    pub alert: Option<(i64, String)>,
}

pub const ALERT_BUTTONS: [&str; 3] = ["Done", "Snooze 10 min", "Snooze 1 hour"];

pub(in crate::view) fn alert_button(pill: Rect, scale: f32, i: usize) -> Rect {
    let (x, y, w, h) = pill;
    let (bw, bh, gap) = (170.0 * scale, 40.0 * scale, 14.0 * scale);
    let total = 3.0 * bw + 2.0 * gap;
    let left = x + (w - total) / 2.0 + i as f32 * (bw + gap);
    (left, y + h - 76.0 * scale, left + bw, y + h - 76.0 * scale + bh)
}

pub fn hit_alert(pill: Rect, scale: f32, px: f32, py: f32) -> Option<usize> {
    (0..3).find(|&i| inside(alert_button(pill, scale, i), px, py))
}

pub(in crate::view) fn paint_alert(p: &Painter, pill: Rect, scale: f32, ui: &UiState) -> Result<()> {
    let Some((_, message)) = &ui.remind.alert else { return Ok(()) };
    let (x, y, w, _) = pill;
    p.glyph('\u{EA8F}', (x, y + 24.0 * scale, x + w, y + 80.0 * scale), 34.0 * scale, acc(1.0))?;
    p.text(
        "Reminder",
        (x, y + 82.0 * scale, x + w, y + 106.0 * scale),
        13.0 * scale,
        DWRITE_FONT_WEIGHT_SEMI_BOLD,
        DWRITE_TEXT_ALIGNMENT_CENTER,
        acc(1.0),
    )?;
    let area = (x + 70.0 * scale, y + 112.0 * scale, x + w - 70.0 * scale, y + 196.0 * scale);
    if let Ok(layout) = crate::ui::gfx::make_layout(p.dwrite(), message, area.2 - area.0, 22.0 * scale) {
        p.push_clip(area);
        p.draw_layout(area.0, area.1, &layout, WHITE)?;
        p.pop_clip();
    }
    for (i, label) in ALERT_BUTTONS.iter().enumerate() {
        let r = alert_button(pill, scale, i);
        let strong = i == 0;
        let hover = ui.home.hover_act == Some(1500 + i);
        p.fill_round_rect(
            r,
            12.0 * scale,
            acc(match (strong, hover) {
                (true, false) => 0.55,
                (true, true) => 0.75,
                (false, false) => 0.18,
                (false, true) => 0.32,
            }),
        )?;
        p.text(label, r, 13.5 * scale, DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_TEXT_ALIGNMENT_CENTER, WHITE)?;
    }
    Ok(())
}
