use crate::view::*;

pub fn tab_in_bar(cfg: &Settings, tab: Tab, name: &str) -> bool {
    match tab {
        Tab::Home | Tab::Settings => true,
        _ => !cfg.tab_hidden(name),
    }
}

pub fn tab_rects(pill: Rect, scale: f32, ui: &UiState) -> [Rect; TABS.len()] {
    let active = ui.tab;
    let (x, y, w, _) = pill;
    let (bh, gap, icon_w) = (36.0 * scale, 1.0 * scale, 32.0 * scale);
    let y0 = y + 12.0 * scale;
    let mut left = x + 12.0 * scale;
    let pin_left = x + w - 78.0 * scale;
    let chip = if ui.privacy.cam || ui.privacy.mic { 42.0 * scale } else { 0.0 };
    let room = pin_left - 6.0 * scale - chip - left;
    let shown = TABS.iter().filter(|(t, _, n)| *t != Tab::Settings && tab_in_bar(&ui.cfg, *t, n)).count() as f32;
    let label = |name: &str| (crate::ui::i18n::tr(name).chars().count() as f32 * 8.6 + 8.0) * scale;
    let active_extra = TABS
        .iter()
        .find(|(t, _, n)| *t == active && *t != Tab::Settings && tab_in_bar(&ui.cfg, *t, n))
        .map_or(0.0, |(_, _, n)| label(n));
    let gaps = (shown - 1.0).max(0.0) * gap;
    let bare_w = if shown * icon_w + gaps + active_extra > room {
        icon_w * ((room - active_extra - gaps) / (shown * icon_w)).clamp(0.6, 1.0)
    } else {
        icon_w
    };
    std::array::from_fn(|i| {
        let (tab, _, name) = TABS[i];
        if !tab_in_bar(&ui.cfg, tab, name) {
            return (0.0, 0.0, 0.0, 0.0);
        }
        if tab == Tab::Settings {
            let r = x + w - 12.0 * scale;
            return (r - icon_w, y0, r, y0 + bh);
        }
        let width = if tab == active { icon_w + label(name) } else { bare_w };
        let rect = (left, y0, left + width, y0 + bh);
        left += width + gap;
        rect
    })
}

pub fn hit_tab(pill: Rect, scale: f32, px: f32, py: f32, ui: &UiState) -> Option<usize> {
    tab_rects(pill, scale, ui).iter().position(|&(l, t, r, b)| px >= l && px < r && py >= t && py < b)
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
    let tabs_end = tab_rects(pill, scale, ui)
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

pub fn hit_mic_chip(pill: Rect, scale: f32, ui: &UiState, px: f32, py: f32) -> bool {
    privacy_chip(pill, scale, ui).is_some_and(|(rect, _, label)| label == "Microphone" && inside(rect, px, py))
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

    for (i, rect) in tab_rects(pill, scale, ui).iter().enumerate() {
        let (tab, glyph, name) = TABS[i];
        if rect.2 <= rect.0 {
            continue;
        }
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
        let muted = label == "Microphone" && ui.mic_muted == Some(true);
        let (fill, icon, text) = if muted {
            (rgba(1.0, 0.3, 0.3, 0.2), rgba(1.0, 0.45, 0.45, 1.0), rgba(1.0, 0.5, 0.5, 1.0))
        } else {
            (rgba(1.0, 0.62, 0.2, 0.2), rgba(1.0, 0.7, 0.3, 1.0), rgba(1.0, 0.72, 0.35, 1.0))
        };
        p.fill_round_rect(chip, (chip.3 - chip.1) / 2.0, fill)?;
        p.glyph(
            if muted { '\u{F781}' } else { glyph },
            (chip.0 + 4.0 * scale, chip.1, chip.0 + 32.0 * scale, chip.3),
            14.0 * scale,
            icon,
        )?;
        if chip.2 - chip.0 > 60.0 * scale {
            p.text(
                label,
                (chip.0 + 32.0 * scale, chip.1, chip.2 - 6.0 * scale, chip.3),
                13.0 * scale,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                text,
            )?;
        }
    }

    match ui.tab {
        Tab::Home => paint_home(p, pill, scale, ui)?,
        Tab::Weather => paint_weather(p, pill, scale, ui)?,
        Tab::Agy => paint_agy(p, pill, scale, ui)?,
        Tab::Chat => paint_chat(p, pill, scale, ui)?,
        Tab::Privacy => paint_privacy(p, pill, scale, ui)?,
        Tab::Activity => paint_activity(p, pill, scale, ui)?,
        Tab::System => paint_system(p, pill, scale, ui)?,
        Tab::Theme => paint_theme(p, pill, scale, ui)?,
        Tab::Clipboard => paint_clipboard(p, pill, scale, ui, clips)?,
        Tab::Controls => paint_controls(p, pill, scale, ui)?,
        Tab::Shelf => paint_shelf(p, pill, scale, ui, shelf)?,
        Tab::Note => paint_note(p, pill, scale, ui)?,
        Tab::Remind => paint_reminders(p, pill, scale, ui)?,
        Tab::Text => paint_text(p, pill, scale, ui)?,
        Tab::Settings => {}
        Tab::Record => paint_record(p, pill, scale, ui)?,
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_hideable_tab_exists_and_the_fixed_ones_stay() {
        for name in crate::data::settings::HIDEABLE_TABS {
            assert!(TABS.iter().any(|t| t.2 == name), "{name} is not a tab");
        }
        let all_hidden = Settings {
            hidden_tabs: crate::data::settings::HIDEABLE_TABS.map(String::from).to_vec(),
            ..Settings::default()
        };
        assert!(tab_in_bar(&all_hidden, Tab::Home, "Home"));
        assert!(tab_in_bar(&all_hidden, Tab::Settings, "Settings"));
        assert!(!tab_in_bar(&all_hidden, Tab::Note, "Note"));
        assert!(tab_in_bar(&Settings::default(), Tab::Note, "Note"));
    }
}
