use crate::view::*;

pub fn tab_in_bar(cfg: &Settings, tab: Tab, name: &str) -> bool {
    match tab {
        Tab::Home | Tab::Settings => true,
        Tab::Steam => crate::platform::steam::client_installed() && !cfg.tab_hidden(name),
        _ => !cfg.tab_hidden(name),
    }
}

pub fn tab_rects(pill: Rect, scale: f32, ui: &UiState) -> [Rect; TABS.len()] {
    let active = ui.tab;
    let (x, y, w, _) = pill;
    let (bh, gap, icon_w) = (36.0 * scale, 1.0 * scale, 32.0 * scale);
    let y0 = y + 12.0 * scale;
    let mut left = x + 12.0 * scale;
    let vpn = if vpn_button(ui) { LOCK_STEP } else { 0.0 };
    let pin_left = x + w - 78.0 * scale - (LOCK_STEP + vpn) * scale;
    let dots = [mic_on(ui), ui.privacy.cam].iter().filter(|on| **on).count() as f32 * DOT_STEP * scale;
    let room = pin_left - 6.0 * scale - dots - left;
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

const LOCK_STEP: f32 = 34.0;
const DOT_STEP: f32 = 20.0;

pub fn lock_rect(pill: Rect, scale: f32) -> Rect {
    let pin = pin_rect(pill, scale);
    (pin.0 - 34.0 * scale, pin.1, pin.0 - 2.0 * scale, pin.3)
}

pub fn vpn_btn_rect(pill: Rect, scale: f32, ui: &UiState) -> Option<Rect> {
    vpn_button(ui).then(|| {
        let lock = lock_rect(pill, scale);
        (lock.0 - 34.0 * scale, lock.1, lock.0 - 2.0 * scale, lock.3)
    })
}

pub fn hit_vpn_btn(pill: Rect, scale: f32, ui: &UiState, px: f32, py: f32) -> bool {
    vpn_btn_rect(pill, scale, ui).is_some_and(|r| inside(r, px, py))
}

pub fn mic_on(ui: &UiState) -> bool {
    ui.privacy.mic || ui.mic_muted == Some(true)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum HeadHit {
    Pin,
    Lock,
    Mic,
    Cam,
}

pub fn privacy_dots(pill: Rect, scale: f32, ui: &UiState) -> (Option<Rect>, Option<Rect>) {
    let lock = lock_rect(pill, scale);
    let size = 11.0 * scale;
    let mid = (lock.1 + lock.3) / 2.0;
    let mut right = vpn_btn_rect(pill, scale, ui).map_or(lock.0, |r| r.0) - 6.0 * scale;
    let mut next = |on: bool| {
        on.then(|| {
            let r = (right - size, mid - size / 2.0, right, mid + size / 2.0);
            right -= DOT_STEP * scale;
            r
        })
    };
    let mic = next(mic_on(ui));
    let cam = next(ui.privacy.cam);
    (mic, cam)
}

pub fn hit_head(pill: Rect, scale: f32, ui: &UiState, px: f32, py: f32) -> Option<HeadHit> {
    if inside(pin_rect(pill, scale), px, py) {
        return Some(HeadHit::Pin);
    }
    if inside(lock_rect(pill, scale), px, py) {
        return Some(HeadHit::Lock);
    }
    let pad = 5.0 * scale;
    let near =
        |r: Rect| px >= r.0 - pad && px < r.2 + pad && py >= pin_rect(pill, scale).1 && py < pin_rect(pill, scale).3;
    let (mic, cam) = privacy_dots(pill, scale, ui);
    if mic.is_some_and(near) {
        Some(HeadHit::Mic)
    } else if cam.is_some_and(near) {
        Some(HeadHit::Cam)
    } else {
        None
    }
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
    if ui.pinned || ui.hover_head == Some(HeadHit::Pin) {
        p.fill_round_rect(pin, (pin.3 - pin.1) / 2.0, rgba(1.0, 1.0, 1.0, if ui.pinned { 0.16 } else { 0.09 }))?;
    }
    p.glyph(
        if ui.pinned { '' } else { '' },
        pin,
        16.0 * scale,
        rgba(1.0, 1.0, 1.0, if ui.pinned { 1.0 } else { 0.62 }),
    )?;
    let lock = lock_rect(pill, scale);
    let locked = ui.cfg.lock_drag;
    if locked || ui.hover_head == Some(HeadHit::Lock) {
        p.fill_round_rect(lock, (lock.3 - lock.1) / 2.0, rgba(1.0, 1.0, 1.0, if locked { 0.16 } else { 0.09 }))?;
    }
    p.glyph(if locked { '' } else { '' }, lock, 16.0 * scale, rgba(1.0, 1.0, 1.0, if locked { 1.0 } else { 0.62 }))?;
    if let Some(r) = vpn_btn_rect(pill, scale, ui) {
        p.glyph(VPN_GLYPH, r, 16.0 * scale, vpn_color(ui))?;
    }
    let (mic, cam) = privacy_dots(pill, scale, ui);
    let dot = |r: Rect, color: windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F, hover: bool| -> Result<()> {
        if hover {
            let grow = 3.0 * scale;
            let halo = (r.0 - grow, r.1 - grow, r.2 + grow, r.3 + grow);
            p.fill_round_rect(halo, (halo.2 - halo.0) / 2.0, rgba(color.r, color.g, color.b, 0.25))?;
        }
        p.fill_round_rect(r, (r.2 - r.0) / 2.0, color)
    };
    if let Some(r) = mic {
        let color = if ui.mic_muted == Some(true) { rgba(1.0, 0.3, 0.3, 1.0) } else { rgba(1.0, 0.62, 0.2, 1.0) };
        dot(r, color, ui.hover_head == Some(HeadHit::Mic))?;
    }
    if let Some(r) = cam {
        dot(r, rgba(0.3, 0.85, 0.45, 1.0), ui.hover_head == Some(HeadHit::Cam))?;
    }

    match ui.tab {
        Tab::Home => paint_home(p, pill, scale, ui)?,
        Tab::Weather => paint_weather(p, pill, scale, ui)?,
        Tab::Agy => paint_agy(p, pill, scale, ui)?,
        Tab::Chat => paint_chat(p, pill, scale, ui)?,
        Tab::Privacy => paint_privacy(p, pill, scale, ui)?,
        Tab::Activity => paint_activity(p, pill, scale, ui)?,
        Tab::Steam => paint_steam(p, pill, scale, ui)?,
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
