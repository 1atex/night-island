use crate::view::*;

pub fn tab_rects(pill: Rect, scale: f32, active: Tab) -> [Rect; TABS.len()] {
    let (x, y, w, _) = pill;
    let (bh, gap, icon_w) = (36.0 * scale, 1.0 * scale, 32.0 * scale);
    let y0 = y + 12.0 * scale;
    let mut left = x + 12.0 * scale;
    std::array::from_fn(|i| {
        let (tab, _, name) = TABS[i];
        if tab == Tab::Settings {
            let r = x + w - 12.0 * scale;
            return (r - icon_w, y0, r, y0 + bh);
        }
        let width = if tab == active {
            icon_w + (crate::ui::i18n::tr(name).chars().count() as f32 * 8.6 + 8.0) * scale
        } else {
            icon_w
        };
        let rect = (left, y0, left + width, y0 + bh);
        left += width + gap;
        rect
    })
}

pub fn hit_tab(pill: Rect, scale: f32, px: f32, py: f32, active: Tab) -> Option<usize> {
    tab_rects(pill, scale, active).iter().position(|&(l, t, r, b)| px >= l && px < r && py >= t && py < b)
}

pub fn pin_rect(pill: Rect, scale: f32) -> Rect {
    let (x, y, w, _) = pill;
    let r = x + w - 12.0 * scale - 32.0 * scale - 2.0 * scale;
    (r - 32.0 * scale, y + 12.0 * scale, r, y + 48.0 * scale)
}

pub fn hit_pin(pill: Rect, scale: f32, px: f32, py: f32) -> bool {
    inside(pin_rect(pill, scale), px, py)
}

pub(in crate::view) fn privacy_chip(pill: Rect, scale: f32, ui: &UiState) -> Option<(Rect, char, &'static str)> {
    let (glyph, label) = if ui.privacy.cam {
        ('\u{E722}', "Camera")
    } else if ui.privacy.mic {
        ('\u{E720}', "Microphone")
    } else {
        return None;
    };
    let pin = pin_rect(pill, scale);
    let right = pin.0 - 4.0 * scale;
    let mut width = (46.0 + crate::ui::i18n::tr(label).chars().count() as f32 * 8.6) * scale;
    let tabs_end = tab_rects(pill, scale, ui.tab)
        .iter()
        .zip(TABS)
        .filter(|(_, t)| t.0 != Tab::Settings)
        .map(|(r, _)| r.2)
        .fold(0.0, f32::max);
    if tabs_end + 8.0 * scale > right - width {
        width = 38.0 * scale;
    }
    Some(((right - width, pin.1 + 2.0 * scale, right, pin.3 - 2.0 * scale), glyph, label))
}

pub fn paint_expanded(
    p: &Painter,
    pill: Rect,
    scale: f32,
    ui: &UiState,
    clips: &[Clip],
    shelf: &[ShelfItem],
) -> Result<()> {
    if ui.remind.alert.is_some() {
        return paint_alert(p, pill, scale, ui);
    }
    if ui.cmd.open {
        return paint_command(p, pill, scale, ui);
    }

    for (i, rect) in tab_rects(pill, scale, ui.tab).iter().enumerate() {
        let (tab, glyph, name) = TABS[i];
        let selected = tab == ui.tab;
        let radius = (rect.3 - rect.1) / 2.0;
        if selected {
            p.fill_round_rect(*rect, radius, rgba(1.0, 1.0, 1.0, 0.16))?;
        } else if ui.hover_tab == Some(i) {
            p.fill_round_rect(*rect, radius, rgba(1.0, 1.0, 1.0, 0.09))?;
        }
        let alpha = if selected { 1.0 } else { 0.62 };
        let icon_box = (rect.0, rect.1, rect.0 + 32.0 * scale, rect.3);
        if selected && tab != Tab::Settings {
            p.glyph(glyph, icon_box, 17.0 * scale, rgba(1.0, 1.0, 1.0, alpha))?;
            p.text(
                name,
                (rect.0 + 30.0 * scale, rect.1, rect.2 - 4.0 * scale, rect.3),
                14.0 * scale,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                WHITE,
            )?;
        } else {
            p.glyph(glyph, *rect, 17.0 * scale, rgba(1.0, 1.0, 1.0, alpha))?;
        }
    }
    let pin = pin_rect(pill, scale);
    if ui.pinned || ui.hover_pin {
        p.fill_round_rect(pin, (pin.3 - pin.1) / 2.0, rgba(1.0, 1.0, 1.0, if ui.pinned { 0.16 } else { 0.09 }))?;
    }
    p.glyph(
        if ui.pinned { '\u{E840}' } else { '\u{E718}' },
        pin,
        16.0 * scale,
        rgba(1.0, 1.0, 1.0, if ui.pinned { 1.0 } else { 0.62 }),
    )?;
    if let Some((chip, glyph, label)) = privacy_chip(pill, scale, ui) {
        p.fill_round_rect(chip, (chip.3 - chip.1) / 2.0, rgba(1.0, 0.62, 0.2, 0.2))?;
        p.glyph(
            glyph,
            (chip.0 + 4.0 * scale, chip.1, chip.0 + 32.0 * scale, chip.3),
            14.0 * scale,
            rgba(1.0, 0.7, 0.3, 1.0),
        )?;
        if chip.2 - chip.0 > 60.0 * scale {
            p.text(
                label,
                (chip.0 + 32.0 * scale, chip.1, chip.2 - 6.0 * scale, chip.3),
                13.0 * scale,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                rgba(1.0, 0.72, 0.35, 1.0),
            )?;
        }
    }

    match ui.tab {
        Tab::Home => paint_home(p, pill, scale, ui)?,
        Tab::Weather => paint_weather(p, pill, scale, ui)?,
        Tab::Integrations => paint_integrations(p, pill, scale, ui)?,
        Tab::Agy => paint_agy(p, pill, scale, ui)?,
        Tab::Theme => paint_theme(p, pill, scale, ui)?,
        Tab::Clipboard => paint_clipboard(p, pill, scale, ui, clips)?,
        Tab::Controls => paint_controls(p, pill, scale, ui)?,
        Tab::Shelf => paint_shelf(p, pill, scale, ui, shelf)?,
        Tab::Note => paint_note(p, pill, scale, ui)?,
        Tab::Text => paint_text(p, pill, scale, ui)?,
        Tab::Settings => paint_settings(p, pill, scale, ui)?,
        Tab::Record => paint_record(p, pill, scale, ui)?,
    }
    Ok(())
}
