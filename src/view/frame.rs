use crate::view::*;

pub fn tab_in_bar(cfg: &Settings, tab: Tab, name: &str) -> bool {
    match tab {
        Tab::Home | Tab::Settings => true,
        Tab::Agy | Tab::Chat => false,
        Tab::Steam => crate::platform::steam::client_installed() && !cfg.tab_hidden(name),
        _ => !cfg.tab_hidden(name),
    }
}

pub fn tab_order(cfg: &Settings) -> Vec<usize> {
    let movable: Vec<usize> = (0..TABS.len()).filter(|&i| !matches!(TABS[i].0, Tab::Home | Tab::Settings)).collect();
    let mut order: Vec<usize> =
        cfg.tab_order.iter().filter_map(|n| movable.iter().copied().find(|&i| TABS[i].2 == n)).collect();
    order.dedup();
    for i in movable {
        if !order.contains(&i) {
            order.push(i);
        }
    }
    let mut all = vec![TABS.iter().position(|t| t.0 == Tab::Home).unwrap_or(0)];
    all.extend(order);
    all.push(TABS.iter().position(|t| t.0 == Tab::Settings).unwrap_or(TABS.len() - 1));
    all
}

pub fn movable_tabs(ui: &UiState) -> Vec<usize> {
    tab_order(&ui.cfg)
        .into_iter()
        .filter(|&i| !matches!(TABS[i].0, Tab::Home | Tab::Settings) && tab_in_bar(&ui.cfg, TABS[i].0, TABS[i].2))
        .collect()
}

pub fn tab_rects(pill: Rect, scale: f32, ui: &UiState) -> [Rect; TABS.len()] {
    let active = bar_tab(ui.tab);
    let (x, y, w, _) = pill;
    let (bh, gap, icon_w) = (36.0 * scale, 1.0 * scale, 32.0 * scale);
    let y0 = y + 12.0 * scale;
    let mut left = x + 12.0 * scale;
    let vpn = if vpn_button(ui) { LOCK_STEP } else { 0.0 };
    let pin_left = x + w - 78.0 * scale - (3.0 * LOCK_STEP + vpn) * scale;
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
    let mut rects = [(0.0, 0.0, 0.0, 0.0); TABS.len()];
    for i in tab_order(&ui.cfg) {
        let (tab, _, name) = TABS[i];
        if !tab_in_bar(&ui.cfg, tab, name) {
            continue;
        }
        if tab == Tab::Settings {
            let r = x + w - 12.0 * scale;
            rects[i] = (r - icon_w, y0, r, y0 + bh);
            continue;
        }
        let width = if tab == active { icon_w + label(name) } else { bare_w };
        rects[i] = (left, y0, left + width, y0 + bh);
        left += width + gap;
    }
    rects
}

pub fn tab_drag_target_for(rects: &[Rect], r: &Reorder, items: &[usize], j: usize) -> (f32, f32) {
    let mut order: Vec<usize> = (0..items.len()).collect();
    let moved = order.remove(r.from);
    order.insert(r.to.min(order.len()), moved);
    let first = items.iter().map(|&i| rects[i].0).fold(f32::MAX, f32::min);
    let gap = 1.0;
    let mut left = first;
    for &k in &order {
        let w = rects[items[k]].2 - rects[items[k]].0;
        if k == j {
            return (left, rects[items[j]].1);
        }
        left += w + gap;
    }
    (rects[items[j]].0, rects[items[j]].1)
}

pub fn subtab_strip(pill: Rect, scale: f32, ui: &UiState) -> Option<(Rect, Vec<(Tab, Rect)>)> {
    let group = bar_tab(ui.tab);
    let (_, subs) = SUBTABS.iter().find(|g| g.0 == group)?;
    let chip_w = |name: &str| (34.0 + crate::ui::i18n::tr(name).chars().count() as f32 * 7.6 + 12.0) * scale;
    let (h, gap, pad) = (30.0 * scale, 4.0 * scale, 3.0 * scale);
    let total: f32 = subs.iter().map(|s| chip_w(s.2)).sum::<f32>() + gap * (subs.len() - 1) as f32 + 2.0 * pad;
    let left = pill.0 + (pill.2 - total) / 2.0;
    let top = pill.1 + 50.0 * scale;
    let mut x = left + pad;
    let chips = subs
        .iter()
        .map(|s| {
            let r = (x, top + pad / 2.0, x + chip_w(s.2), top + h - pad / 2.0);
            x += chip_w(s.2) + gap;
            (s.0, r)
        })
        .collect();
    Some(((left, top, left + total, top + h), chips))
}

pub fn hit_subtab(pill: Rect, scale: f32, px: f32, py: f32, ui: &UiState) -> Option<Tab> {
    subtab_strip(pill, scale, ui)?.1.into_iter().find(|(_, r)| inside(*r, px, py)).map(|(t, _)| t)
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

pub fn tab_drag_rect(pill: Rect, scale: f32) -> Rect {
    let lock = lock_rect(pill, scale);
    (lock.0 - 34.0 * scale, lock.1, lock.0 - 2.0 * scale, lock.3)
}

pub fn tab_settings_rect(pill: Rect, scale: f32) -> Rect {
    let sw = tab_drag_rect(pill, scale);
    (sw.0 - 34.0 * scale, sw.1, sw.0 - 2.0 * scale, sw.3)
}

pub fn vpn_btn_rect(pill: Rect, scale: f32, ui: &UiState) -> Option<Rect> {
    vpn_button(ui).then(|| {
        let sw = tab_settings_rect(pill, scale);
        (sw.0 - 34.0 * scale, sw.1, sw.0 - 2.0 * scale, sw.3)
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
    TabDrag,
    Mic,
    Cam,
    TabSettings,
}

pub fn privacy_dots(pill: Rect, scale: f32, ui: &UiState) -> (Option<Rect>, Option<Rect>) {
    let sw = tab_settings_rect(pill, scale);
    let size = 11.0 * scale;
    let mid = (sw.1 + sw.3) / 2.0;
    let mut right = vpn_btn_rect(pill, scale, ui).map_or(sw.0, |r| r.0) - 6.0 * scale;
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
    if inside(tab_drag_rect(pill, scale), px, py) {
        return Some(HeadHit::TabDrag);
    }
    if tab_settings_group(ui.tab).is_some() && inside(tab_settings_rect(pill, scale), px, py) {
        return Some(HeadHit::TabSettings);
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

pub fn paint_expanded(p: &Painter, pill: Rect, scale: f32, ui: &UiState, clips: &[Clip]) -> Result<()> {
    if ui.remind.alert.is_some() {
        return paint_alert(p, pill, scale, ui);
    }
    if ui.cmd.open {
        return paint_command(p, pill, scale, ui);
    }

    let mut rects = tab_rects(pill, scale, ui);
    let carried = ui.tab_drag.as_ref();
    if let Some(d) = carried {
        for (j, &i) in d.items.iter().enumerate() {
            if j != d.reorder.from && j < d.reorder.pos.len() {
                let (w, h) = (rects[i].2 - rects[i].0, rects[i].3 - rects[i].1);
                rects[i] = (d.reorder.pos[j].0, d.reorder.pos[j].1, d.reorder.pos[j].0 + w, d.reorder.pos[j].1 + h);
            }
        }
    }
    let carried_index = carried.and_then(|d| d.items.get(d.reorder.from).copied());
    let mut order: Vec<usize> = (0..TABS.len()).filter(|&i| Some(i) != carried_index).collect();
    order.extend(carried_index);
    for i in order {
        let mut rect = &rects[i];
        let lifted;
        if Some(i) == carried_index
            && let Some(d) = carried
        {
            let r0 = rects[i];
            lifted = d.reorder.dragged_rect((r0.2 - r0.0, r0.3 - r0.1));
            rect = &lifted;
            p.fill_round_rect(
                (lifted.0 - 2.0 * scale, lifted.1 - 2.0 * scale, lifted.2 + 2.0 * scale, lifted.3 + 2.0 * scale),
                (lifted.3 - lifted.1) / 2.0 + 2.0 * scale,
                rgba(1.0, 1.0, 1.0, 0.1),
            )?;
        }
        let (tab, glyph, name) = TABS[i];
        if rect.2 <= rect.0 {
            continue;
        }
        let selected = tab == bar_tab(ui.tab);
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
    let sw = tab_drag_rect(pill, scale);
    let on = ui.cfg.tab_drag;
    if on || ui.hover_head == Some(HeadHit::TabDrag) {
        p.fill_round_rect(sw, (sw.3 - sw.1) / 2.0, rgba(1.0, 1.0, 1.0, if on { 0.16 } else { 0.09 }))?;
    }
    p.glyph('\u{E8AB}', sw, 16.0 * scale, rgba(1.0, 1.0, 1.0, if on { 1.0 } else { 0.62 }))?;
    if tab_settings_group(ui.tab).is_some() {
        let gear = tab_settings_rect(pill, scale);
        if ui.hover_head == Some(HeadHit::TabSettings) {
            p.fill_round_rect(gear, (gear.3 - gear.1) / 2.0, rgba(1.0, 1.0, 1.0, 0.09))?;
        }
        let alpha = if ui.hover_head == Some(HeadHit::TabSettings) { 1.0 } else { 0.62 };
        p.glyph('\u{E115}', gear, 16.0 * scale, rgba(1.0, 1.0, 1.0, alpha))?;
    }
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
        let (glyph, color) = super::pill::mic_look(ui);
        if ui.hover_head == Some(HeadHit::Mic) {
            let grow = 3.0 * scale;
            let halo = (r.0 - grow, r.1 - grow, r.2 + grow, r.3 + grow);
            p.fill_round_rect(halo, (halo.2 - halo.0) / 2.0, rgba(color.r, color.g, color.b, 0.25))?;
        }
        let (cx, cy) = ((r.0 + r.2) / 2.0, (r.1 + r.3) / 2.0);
        let half = 8.0 * scale;
        p.glyph(glyph, (cx - half, cy - half, cx + half, cy + half), 14.0 * scale, color)?;
    }
    if let Some(r) = cam {
        dot(r, rgba(0.3, 0.85, 0.45, 1.0), ui.hover_head == Some(HeadHit::Cam))?;
    }

    if let Some((strip, chips)) = subtab_strip(pill, scale, ui) {
        p.fill_round_rect(strip, (strip.3 - strip.1) / 2.0, rgba(1.0, 1.0, 1.0, 0.06))?;
        let group = bar_tab(ui.tab);
        let subs = SUBTABS.iter().find(|g| g.0 == group).map_or(&[][..], |g| &g.1[..]);
        for (tab, r) in chips {
            let on = tab == ui.tab;
            let hot = ui.hover_sub == Some(tab);
            if on || hot {
                p.fill_round_rect(r, (r.3 - r.1) / 2.0, rgba(1.0, 1.0, 1.0, if on { 0.18 } else { 0.09 }))?;
            }
            let Some(&(_, glyph, name)) = subs.iter().find(|s| s.0 == tab) else { continue };
            let alpha = if on { 1.0 } else { 0.65 };
            p.glyph(
                glyph,
                (r.0 + 4.0 * scale, r.1, r.0 + 30.0 * scale, r.3),
                15.0 * scale,
                rgba(1.0, 1.0, 1.0, alpha),
            )?;
            p.text(
                name,
                (r.0 + 30.0 * scale, r.1, r.2 - 4.0 * scale, r.3),
                12.5 * scale,
                if on { DWRITE_FONT_WEIGHT_SEMI_BOLD } else { DWRITE_FONT_WEIGHT_NORMAL },
                DWRITE_TEXT_ALIGNMENT_LEADING,
                rgba(1.0, 1.0, 1.0, alpha),
            )?;
        }
    }
    if let Some(change) = ui.tab_change.filter(|c| c.from != ui.tab) {
        let t = (change.start.elapsed().as_secs_f32() * 1000.0 / change.ms.max(1) as f32).clamp(0.0, 1.0);
        let progress = crate::ui::anim::ease(ui.cfg.anim_curve, t);
        let (old, new) = crate::ui::anim::tab_looks(ui.cfg.tab_anim, progress, change.dir);
        let bar_bottom = tab_rects(pill, scale, ui).iter().map(|r| r.3).fold(pill.1, f32::max) + 4.0 * scale;
        let top = if bar_tab(ui.tab) != ui.tab { body_rect(pill, scale, ui.tab).1 } else { bar_bottom };
        let area = (pill.0, top, pill.0 + pill.2, pill.1 + pill.3);
        let (w, h) = (area.2 - area.0, area.3 - area.1);
        p.push_clip(area);
        let mut result = Ok(());
        for (tab, look) in [(change.from, old), (ui.tab, new)] {
            if look.alpha <= 0.003 {
                continue;
            }
            let bitmap = p.offscreen((w, h), (area.0, area.1), |q| paint_tab(q, tab, pill, scale, ui, clips));
            match bitmap {
                Ok(bitmap) => {
                    let (cx, cy) = (area.0 + w / 2.0 + look.dx * w, area.1 + h / 2.0 + look.dy * h);
                    let (hw, hh) = (w * look.scale / 2.0, h * look.scale / 2.0);
                    p.draw_bitmap(&bitmap, (cx - hw, cy - hh, cx + hw, cy + hh), look.alpha.clamp(0.0, 1.0));
                }
                Err(e) => result = result.and(Err(e)),
            }
        }
        p.pop_clip();
        if result.is_err() {
            result = paint_tab(p, ui.tab, pill, scale, ui, clips);
        }
        return result;
    }
    paint_tab(p, ui.tab, pill, scale, ui, clips)
}

fn paint_tab(p: &Painter, tab: Tab, pill: Rect, scale: f32, ui: &UiState, clips: &[Clip]) -> Result<()> {
    let pill = body_rect(pill, scale, tab);
    match tab {
        Tab::Home => paint_home(p, pill, scale, ui)?,
        Tab::Weather => paint_weather(p, pill, scale, ui)?,
        Tab::Ai => {}
        Tab::Agy => paint_agy(p, pill, scale, ui)?,
        Tab::Chat => paint_chat(p, pill, scale, ui)?,
        Tab::Privacy => paint_privacy(p, pill, scale, ui)?,
        Tab::Activity => paint_activity(p, pill, scale, ui)?,
        Tab::Steam => paint_steam(p, pill, scale, ui)?,
        Tab::System => paint_system(p, pill, scale, ui)?,
        Tab::Clipboard => paint_clipboard(p, pill, scale, ui, clips)?,
        Tab::Controls => paint_controls(p, pill, scale, ui)?,
        Tab::Note => paint_note(p, pill, scale, ui)?,
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
