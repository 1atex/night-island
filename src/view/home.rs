use crate::view::*;

pub struct HomeUi {
    pub vol: f32,
    pub device: String,
    pub dev_menu: Option<(Vec<crate::platform::volume::Device>, usize)>,
    pub dock: Vec<ShelfItem>,
    pub seek: Option<f32>,
    pub vol_track: bool,
    pub app_vol: f32,
    pub hover_act: Option<usize>,
}

impl Default for HomeUi {
    fn default() -> Self {
        Self {
            vol: 0.5,
            device: Default::default(),
            dev_menu: Default::default(),
            dock: Default::default(),
            seek: Default::default(),
            vol_track: Default::default(),
            app_vol: 1.0,
            hover_act: Default::default(),
        }
    }
}

pub const HOME_ACTIONS: [(char, &str); 8] = [
    ('\u{E7A8}', "Area"),
    ('\u{E740}', "Full screen"),
    ('\u{E8E9}', "Text from screen"),
    ('\u{E8B9}', "QR code"),
    ('\u{EF3C}', "Color picker"),
    ('\u{ED60}', "Clean text"),
    ('\u{E7C8}', "Screen recording"),
    ('\u{E721}', "Command"),
];

pub(in crate::view) const HOME_TOP: f32 = 58.0;

pub(in crate::view) const GRID_COL_W: f32 = 124.0;

pub(in crate::view) const GRID_ROW_H: f32 = 46.0;

pub(in crate::view) const GRID_GAP: f32 = 8.0;

pub(in crate::view) fn grid_left(pill: Rect, scale: f32) -> f32 {
    let (x, _, w, _) = pill;
    x + w - SIDE_PAD * scale + 4.0 * scale - (2.0 * GRID_COL_W + GRID_GAP) * scale
}

pub(in crate::view) fn action_tile(pill: Rect, scale: f32, i: usize) -> Rect {
    let (_, y, _, _) = pill;
    let (col, row) = ((i % 2) as f32, (i / 2) as f32);
    let left = grid_left(pill, scale) + col * (GRID_COL_W + GRID_GAP) * scale;
    let top = y + (HOME_TOP + row * (GRID_ROW_H + GRID_GAP)) * scale;
    (left, top, left + GRID_COL_W * scale, top + GRID_ROW_H * scale)
}

pub(in crate::view) fn media_card(pill: Rect, scale: f32) -> Rect {
    let (x, y, _, _) = pill;
    let left = x + (SIDE_PAD - 4.0) * scale;
    (left, y + HOME_TOP * scale, grid_left(pill, scale) - 12.0 * scale, y + (HOME_TOP + 154.0) * scale)
}

pub(in crate::view) fn volume_row(pill: Rect, scale: f32) -> Rect {
    let card = media_card(pill, scale);
    (card.0, card.3 + 8.0 * scale, card.2, card.3 + (8.0 + 46.0) * scale)
}

pub fn volume_track(pill: Rect, scale: f32) -> (f32, f32, f32) {
    let row = volume_row(pill, scale);
    (row.0 + 44.0 * scale, row.0 + 150.0 * scale, (row.1 + row.3) / 2.0)
}

pub(in crate::view) fn device_chip(pill: Rect, scale: f32) -> Rect {
    let row = volume_row(pill, scale);
    (row.0 + 192.0 * scale, row.1 + 8.0 * scale, row.2 - 8.0 * scale, row.3 - 8.0 * scale)
}

pub(in crate::view) const MENU_ROW_H: f32 = 30.0;

pub(in crate::view) const MENU_MAX_ROWS: usize = 5;

pub fn menu_visible(count: usize) -> usize {
    count.min(MENU_MAX_ROWS)
}

pub fn menu_rect(pill: Rect, scale: f32, count: usize) -> Rect {
    let chip = device_chip(pill, scale);
    let width = 300.0 * scale;
    let right = chip.2;
    let left = (right - width).max(pill.0 + 8.0 * scale);
    let bottom = chip.1 - 8.0 * scale;
    (left, bottom - (menu_visible(count) as f32 * MENU_ROW_H + 12.0) * scale, right, bottom)
}

pub(in crate::view) fn menu_row(frame: Rect, scale: f32, i: usize) -> Rect {
    let top = frame.1 + (6.0 + i as f32 * MENU_ROW_H) * scale;
    (frame.0 + 6.0 * scale, top, frame.2 - 6.0 * scale, top + MENU_ROW_H * scale)
}

pub fn hit_menu(pill: Rect, scale: f32, px: f32, py: f32, count: usize) -> Option<Option<usize>> {
    let frame = menu_rect(pill, scale, count);
    if !inside(frame, px, py) {
        return None;
    }
    Some((0..menu_visible(count)).find(|&i| inside(menu_row(frame, scale, i), px, py)))
}

pub(in crate::view) fn paint_menu(p: &Painter, pill: Rect, scale: f32, ui: &UiState) -> Result<()> {
    let Some((items, scroll)) = &ui.home.dev_menu else { return Ok(()) };
    let frame = menu_rect(pill, scale, items.len());
    p.fill_round_rect(frame, 16.0 * scale, rgba(0.07, 0.07, 0.09, 0.98))?;
    p.stroke_round_rect(frame, 16.0 * scale, 1.0 * scale, rgba(1.0, 1.0, 1.0, 0.14))?;
    for (i, d) in items.iter().skip(*scroll).take(menu_visible(items.len())).enumerate() {
        let row = menu_row(frame, scale, i);
        if ui.home.hover_act == Some(300 + i) {
            p.fill_round_rect(row, 10.0 * scale, rgba(1.0, 1.0, 1.0, 0.1))?;
        }
        if d.current {
            p.glyph('\u{E73E}', (row.0 + 2.0 * scale, row.1, row.0 + 28.0 * scale, row.3), 13.0 * scale, WHITE)?;
        }
        p.glyph(
            if d.capture { '\u{E720}' } else { '\u{E767}' },
            (row.0 + 28.0 * scale, row.1, row.0 + 52.0 * scale, row.3),
            13.0 * scale,
            rgba(1.0, 1.0, 1.0, 0.65),
        )?;
        p.text(
            &d.name,
            (row.0 + 56.0 * scale, row.1, row.2 - 8.0 * scale, row.3),
            13.0 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            WHITE,
        )?;
    }
    Ok(())
}

pub(in crate::view) const DOCK_TOP: f32 = HOME_TOP + 4.0 * (GRID_ROW_H + GRID_GAP) + 4.0;

pub(in crate::view) const DOCK_ICON: f32 = 40.0;

pub(in crate::view) const DOCK_GAP: f32 = 12.0;

pub(in crate::view) fn dock_zone(pill: Rect, scale: f32, compact: bool) -> Rect {
    let (x, y, w, _) = pill;
    (
        x + (SIDE_PAD - 4.0) * scale,
        y + DOCK_TOP * scale,
        x + w - (SIDE_PAD - 4.0) * scale,
        y + if compact { 326.0 } else { 372.0 } * scale,
    )
}

pub(in crate::view) fn strip_zone(pill: Rect, scale: f32) -> Rect {
    let (x, y, w, _) = pill;
    (x + (SIDE_PAD - 4.0) * scale, y + 332.0 * scale, x + w - (SIDE_PAD - 4.0) * scale, y + 372.0 * scale)
}

pub(in crate::view) fn dock_capacity(pill: Rect, scale: f32, compact: bool) -> usize {
    let zone = dock_zone(pill, scale, compact);
    let room = (zone.2 - zone.0) / scale - 28.0 + DOCK_GAP;
    ((room / (DOCK_ICON + DOCK_GAP)).floor() as usize).saturating_sub(1).max(1)
}

pub(in crate::view) fn dock_slot(pill: Rect, scale: f32, compact: bool, i: usize) -> Rect {
    let zone = dock_zone(pill, scale, compact);
    let left = zone.0 + (14.0 + i as f32 * (DOCK_ICON + DOCK_GAP)) * scale;
    let top = zone.1 + if compact { 4.0 } else { 12.0 } * scale;
    (left, top, left + DOCK_ICON * scale, top + DOCK_ICON * scale)
}

pub(in crate::view) fn hit_dock(
    pill: Rect,
    scale: f32,
    px: f32,
    py: f32,
    compact: bool,
    count: usize,
) -> Option<HomeHit> {
    let shown = count.min(dock_capacity(pill, scale, compact));
    if let Some(i) = (0..shown).find(|&i| inside(dock_slot(pill, scale, compact, i), px, py)) {
        return Some(HomeHit::Dock(i));
    }
    inside(dock_slot(pill, scale, compact, shown), px, py).then_some(HomeHit::DockAdd)
}

pub(in crate::view) fn dock_name(path: &str) -> String {
    let p = std::path::Path::new(path);
    p.file_stem().or_else(|| p.file_name()).map_or_else(|| path.to_string(), |n| n.to_string_lossy().into_owned())
}

pub(in crate::view) fn paint_dock(p: &Painter, pill: Rect, scale: f32, ui: &UiState) -> Result<()> {
    let compact = discord_strip(ui);
    let zone = dock_zone(pill, scale, compact);
    p.fill_round_rect(zone, 18.0 * scale, rgba(1.0, 1.0, 1.0, 0.05))?;
    let shown = ui.home.dock.len().min(dock_capacity(pill, scale, compact));
    let mut hovered: Option<&str> = None;
    for (i, item) in ui.home.dock.iter().take(shown).enumerate() {
        let slot = dock_slot(pill, scale, compact, i);
        let hover = ui.home.hover_act == Some(600 + i);
        if hover {
            let g = 5.0 * scale;
            p.fill_round_rect(
                (slot.0 - g, slot.1 - g, slot.2 + g, slot.3 + g),
                14.0 * scale,
                rgba(1.0, 1.0, 1.0, 0.13),
            )?;
            hovered = Some(&item.path);
        }
        let inner = 3.0 * scale;
        let icon_rect = (slot.0 + inner, slot.1 + inner, slot.2 - inner, slot.3 - inner);
        match crate::platform::apps::icon_for_path(&item.path) {
            Some(icon) => p.image(icon_rect, 6.0 * scale, icon.side, &icon.bgra)?,
            None => p.glyph('\u{E8A5}', slot, 22.0 * scale, rgba(1.0, 1.0, 1.0, 0.7))?,
        }
    }
    let add = dock_slot(pill, scale, compact, shown);
    let add_hover = ui.home.hover_act == Some(700);
    p.stroke_round_rect(add, 12.0 * scale, 1.2 * scale, rgba(1.0, 1.0, 1.0, if add_hover { 0.55 } else { 0.28 }))?;
    p.glyph('\u{E710}', add, 16.0 * scale, rgba(1.0, 1.0, 1.0, if add_hover { 0.9 } else { 0.55 }))?;
    if ui.home.dock.is_empty() {
        p.text(
            "Drop shortcuts, folders or files here",
            (add.2 + 14.0 * scale, add.1, zone.2 - 12.0 * scale, add.3),
            13.5 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            rgba(1.0, 1.0, 1.0, 0.5),
        )?;
    }
    if !compact {
        let name = hovered.map(dock_name).unwrap_or_default();
        if !name.is_empty() {
            p.text(
                &name,
                (zone.0 + 14.0 * scale, zone.3 - 34.0 * scale, zone.2 - 14.0 * scale, zone.3 - 10.0 * scale),
                13.0 * scale,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                WHITE,
            )?;
        } else if !ui.home.dock.is_empty() {
            p.text(
                "Click to open, right-click for more",
                (zone.0 + 14.0 * scale, zone.3 - 34.0 * scale, zone.2 - 14.0 * scale, zone.3 - 10.0 * scale),
                12.0 * scale,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                rgba(1.0, 1.0, 1.0, 0.4),
            )?;
        }
    }
    Ok(())
}

pub fn dock_path(ui: &UiState, i: usize) -> Option<&str> {
    ui.home.dock.get(i).map(|d| d.path.as_str())
}

pub(in crate::view) fn media_btn(pill: Rect, scale: f32, i: usize) -> Rect {
    let card = media_card(pill, scale);
    let (bw, gap) = ([34.0, 42.0, 34.0][i] * scale, 22.0 * scale);
    let total = 34.0 * 2.0 * scale + 42.0 * scale + 2.0 * gap;
    let mut left = card.0 + (card.2 - card.0 - total) / 2.0;
    for k in 0..i {
        left += [34.0, 42.0, 34.0][k] * scale + gap;
    }
    let cy = card.3 - 25.0 * scale;
    (left, cy - bw / 2.0, left + bw, cy + bw / 2.0)
}

#[derive(Clone, Copy, PartialEq)]
pub enum HomeHit {
    Media(usize),
    Action(usize),
    Volume,
    Device,
    Extra(usize),
    DiscordMute,
    DiscordDeaf,
    DiscordOverlay,
    Dock(usize),
    DockAdd,
    Seek(f32),
    VolumeIcon,
}

#[allow(clippy::too_many_arguments)]
pub fn hit_home(
    pill: Rect,
    scale: f32,
    px: f32,
    py: f32,
    has_media: bool,
    spotify: bool,
    strip: bool,
    dock: usize,
) -> Option<HomeHit> {
    if let Some(hit) = hit_dock(pill, scale, px, py, strip, dock) {
        return Some(hit);
    }
    if strip {
        for (i, hit) in [HomeHit::DiscordDeaf, HomeHit::DiscordMute, HomeHit::DiscordOverlay].into_iter().enumerate() {
            if inside(strip_toggle(pill, scale, i), px, py) {
                return Some(hit);
            }
        }
    }
    if spotify && let Some(i) = (0..3).find(|&i| inside(extra_btn(pill, scale, i), px, py)) {
        return Some(HomeHit::Extra(i));
    }
    if has_media && let Some(i) = (0..3).find(|&i| inside(media_btn(pill, scale, i), px, py)) {
        return Some(HomeHit::Media(i));
    }
    if has_media {
        let (l, r, y) = seek_track(pill, scale);
        if px >= l - 6.0 * scale && px <= r + 6.0 * scale && (py - y).abs() < 10.0 * scale {
            return Some(HomeHit::Seek(((px - l) / (r - l).max(1.0)).clamp(0.0, 1.0)));
        }
    }
    if let Some(i) = (0..HOME_ACTIONS.len()).find(|&i| inside(action_tile(pill, scale, i), px, py)) {
        return Some(HomeHit::Action(i));
    }
    let row = volume_row(pill, scale);
    if inside((row.0 + 2.0 * scale, row.1, row.0 + 42.0 * scale, row.3), px, py) {
        return Some(HomeHit::VolumeIcon);
    }
    let (tl, tr, cy) = volume_track(pill, scale);
    if px >= tl - 10.0 * scale && px <= tr + 10.0 * scale && (py - cy).abs() < 14.0 * scale {
        return Some(HomeHit::Volume);
    }
    inside(device_chip(pill, scale), px, py).then_some(HomeHit::Device)
}

pub fn seek_track(pill: Rect, scale: f32) -> (f32, f32, f32) {
    let card = media_card(pill, scale);
    (card.0 + 16.0 * scale, card.2 - 16.0 * scale, card.1 + 100.0 * scale)
}

pub(in crate::view) fn extra_btn(pill: Rect, scale: f32, i: usize) -> Rect {
    let card = media_card(pill, scale);
    let size = 30.0 * scale;
    match i {
        0 => {
            let prev = media_btn(pill, scale, 0);
            let cy = (prev.1 + prev.3) / 2.0;
            (prev.0 - 18.0 * scale - size, cy - size / 2.0, prev.0 - 18.0 * scale, cy + size / 2.0)
        }
        1 => {
            let next = media_btn(pill, scale, 2);
            let cy = (next.1 + next.3) / 2.0;
            (next.2 + 18.0 * scale, cy - size / 2.0, next.2 + 18.0 * scale + size, cy + size / 2.0)
        }
        _ => (card.2 - 14.0 * scale - size, card.1 + 10.0 * scale, card.2 - 14.0 * scale, card.1 + 10.0 * scale + size),
    }
}

pub fn spotify_active(ui: &UiState) -> bool {
    ui.spotify.status == crate::services::spotify::Status::Connected
        && ui.media.source == "SPOTIFY"
        && !ui.media.title.is_empty()
}

pub(in crate::view) fn mmss(secs: f32) -> String {
    let s = secs.max(0.0) as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

pub(in crate::view) fn paint_home(p: &Painter, pill: Rect, scale: f32, ui: &UiState) -> Result<()> {
    let card = media_card(pill, scale);
    p.fill_round_rect(card, 20.0 * scale, rgba(1.0, 1.0, 1.0, 0.07))?;
    let (cl, ct, cr, cb) = card;
    let pad = 16.0 * scale;
    if ui.media.title.is_empty() {
        let clock = clock();
        p.text(
            &clock.hh_mm,
            (cl + pad, ct + 20.0 * scale, cr - pad, ct + 92.0 * scale),
            52.0 * scale,
            DWRITE_FONT_WEIGHT_BOLD,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            WHITE,
        )?;
        p.text(
            &clock.date,
            (cl + pad, ct + 92.0 * scale, cr - pad, ct + 118.0 * scale),
            15.0 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            rgba(1.0, 1.0, 1.0, DIM),
        )?;
        p.text(
            "Nothing is playing",
            (cl + pad, cb - 34.0 * scale, cr - pad, cb - 10.0 * scale),
            13.0 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            rgba(1.0, 1.0, 1.0, 0.45),
        )?;
    } else {
        let side = 72.0 * scale;
        let cover = (cl + pad, ct + pad, cl + pad + side, ct + pad + side);
        match &ui.media.cover {
            Some(c) => p.image(cover, 12.0 * scale, c.size, &c.bgra)?,
            None => {
                p.fill_round_rect(cover, 12.0 * scale, rgba(1.0, 1.0, 1.0, 0.1))?;
                p.glyph('\u{E8D6}', cover, 28.0 * scale, rgba(1.0, 1.0, 1.0, 0.6))?;
            }
        }
        let tx = cover.2 + 14.0 * scale;
        let (ar, ag, ab) = ui.media.accent.unwrap_or_else(crate::ui::theme::accent_u8);
        let accent = rgba(f32::from(ar) / 255.0, f32::from(ag) / 255.0, f32::from(ab) / 255.0, 1.0);
        p.text(
            &ui.media.source,
            (tx, ct + 14.0 * scale, cr - pad, ct + 32.0 * scale),
            11.0 * scale,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            rgba(1.0, 1.0, 1.0, 0.5),
        )?;
        p.text(
            &ui.media.title,
            (tx, ct + 32.0 * scale, cr - pad, ct + 58.0 * scale),
            18.0 * scale,
            DWRITE_FONT_WEIGHT_BOLD,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            WHITE,
        )?;
        p.text(
            &ui.media.artist,
            (tx, ct + 58.0 * scale, cr - pad, ct + 80.0 * scale),
            14.0 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            rgba(1.0, 1.0, 1.0, 0.65),
        )?;
        let (tl, tr_) = (cl + pad, cr - pad);
        let ty = ct + 100.0 * scale;
        p.fill_round_rect((tl, ty - 1.5 * scale, tr_, ty + 1.5 * scale), 1.5 * scale, rgba(1.0, 1.0, 1.0, 0.18))?;
        if let Some((real_pos, len)) = crate::platform::media::progress() {
            let pos = ui.home.seek.map_or(real_pos, |f| f * len);
            let x = tl + (tr_ - tl) * (pos / len).clamp(0.0, 1.0);
            let live = ui.home.hover_act == Some(520) || ui.home.seek.is_some();
            let half = if live { 2.5 } else { 1.5 } * scale;
            p.fill_round_rect((tl, ty - half, tr_, ty + half), half, rgba(1.0, 1.0, 1.0, 0.18))?;
            p.fill_round_rect((tl, ty - half, x, ty + half), half, accent)?;
            if live {
                p.fill_round_rect(
                    (x - 5.0 * scale, ty - 5.0 * scale, x + 5.0 * scale, ty + 5.0 * scale),
                    5.0 * scale,
                    WHITE,
                )?;
            }
            p.text(
                &mmss(pos),
                (tl, ty + 2.0 * scale, tl + 60.0 * scale, ty + 18.0 * scale),
                11.0 * scale,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                rgba(1.0, 1.0, 1.0, 0.55),
            )?;
            p.text(
                &format!("-{}", mmss(len - pos)),
                (tr_ - 60.0 * scale, ty + 2.0 * scale, tr_, ty + 18.0 * scale),
                11.0 * scale,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_TEXT_ALIGNMENT_TRAILING,
                rgba(1.0, 1.0, 1.0, 0.55),
            )?;
        }
        if spotify_active(ui) {
            let accent_on = accent;
            let off = rgba(1.0, 1.0, 1.0, 0.55);
            let repeat_glyph =
                if ui.spotify.repeat == crate::services::spotify::Repeat::Track { '\u{E8ED}' } else { '\u{E8EE}' };
            let items = [
                ('\u{E8B1}', ui.spotify.shuffle),
                (repeat_glyph, ui.spotify.repeat != crate::services::spotify::Repeat::Off),
                (if ui.spotify.liked { '\u{EB51}' } else { '\u{E006}' }, ui.spotify.liked),
            ];
            for (i, (glyph, on)) in items.iter().enumerate() {
                if ui.home.hover_act == Some(510 + i) {
                    p.fill_round_rect(extra_btn(pill, scale, i), 15.0 * scale, rgba(1.0, 1.0, 1.0, 0.14))?;
                }
                p.glyph(*glyph, extra_btn(pill, scale, i), 15.0 * scale, if *on { accent_on } else { off })?;
            }
        }
        for (i, g) in
            ['\u{E892}', if ui.media.playing { '\u{E769}' } else { '\u{E768}' }, '\u{E893}'].iter().enumerate()
        {
            let r = media_btn(pill, scale, i);
            let hover = ui.home.hover_act == Some(500 + i);
            if hover && i != 1 {
                p.fill_round_rect(r, 17.0 * scale, rgba(1.0, 1.0, 1.0, 0.16))?;
            }
            if i == 1 {
                p.fill_round_rect(r, 21.0 * scale, if hover { rgba(0.85, 0.85, 0.88, 1.0) } else { WHITE })?;
                p.glyph(*g, r, 16.0 * scale, rgba(0.0, 0.0, 0.0, 1.0))?;
            } else {
                p.glyph(*g, r, 16.0 * scale, WHITE)?;
            }
        }
    }

    let row = volume_row(pill, scale);
    p.fill_round_rect(row, 16.0 * scale, rgba(1.0, 1.0, 1.0, 0.07))?;
    let level = if ui.home.vol_track { ui.home.app_vol } else { ui.home.vol };
    let tint = if ui.home.vol_track {
        ui.media
            .accent
            .map_or(WHITE, |(r, g, b)| rgba(f32::from(r) / 255.0, f32::from(g) / 255.0, f32::from(b) / 255.0, 1.0))
    } else {
        WHITE
    };
    let icon = if ui.home.vol_track {
        '\u{E8D6}'
    } else if level <= 0.001 {
        '\u{E74F}'
    } else {
        '\u{E767}'
    };
    let icon_box = (row.0 + 6.0 * scale, row.1 + 6.0 * scale, row.0 + 40.0 * scale, row.3 - 6.0 * scale);
    if ui.home.hover_act == Some(530) {
        p.fill_round_rect(icon_box, 12.0 * scale, rgba(1.0, 1.0, 1.0, 0.14))?;
    }
    p.glyph(icon, icon_box, 15.0 * scale, if ui.home.vol_track { tint } else { rgba(1.0, 1.0, 1.0, 0.8) })?;
    let (tl, tr, cy) = volume_track(pill, scale);
    let x = tl + (tr - tl) * level.clamp(0.0, 1.0);
    p.fill_round_rect((tl, cy - 2.0 * scale, tr, cy + 2.0 * scale), 2.0 * scale, rgba(1.0, 1.0, 1.0, 0.2))?;
    p.fill_round_rect((tl, cy - 2.0 * scale, x, cy + 2.0 * scale), 2.0 * scale, tint)?;
    p.fill_round_rect((x - 6.0 * scale, cy - 6.0 * scale, x + 6.0 * scale, cy + 6.0 * scale), 6.0 * scale, WHITE)?;
    p.text(
        &format!("{}", (level * 100.0).round() as u32),
        (tr + 8.0 * scale, row.1, tr + 38.0 * scale, row.3),
        13.0 * scale,
        DWRITE_FONT_WEIGHT_NORMAL,
        DWRITE_TEXT_ALIGNMENT_TRAILING,
        rgba(1.0, 1.0, 1.0, 0.75),
    )?;
    if !ui.home.device.is_empty() {
        let chip = device_chip(pill, scale);
        let hover = ui.home.hover_act == Some(100);
        p.fill_round_rect(chip, 12.0 * scale, rgba(1.0, 1.0, 1.0, if hover { 0.16 } else { 0.1 }))?;
        p.glyph(
            '\u{E7F5}',
            (chip.0 + 4.0 * scale, chip.1, chip.0 + 26.0 * scale, chip.3),
            12.0 * scale,
            rgba(1.0, 1.0, 1.0, 0.75),
        )?;
        p.text(
            &ui.home.device,
            (chip.0 + 28.0 * scale, chip.1, chip.2 - 6.0 * scale, chip.3),
            12.0 * scale,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            rgba(1.0, 1.0, 1.0, 0.9),
        )?;
    }

    for (i, (glyph, label)) in HOME_ACTIONS.iter().enumerate() {
        let r = action_tile(pill, scale, i);
        let recording = i == 6 && ui.recording_secs.is_some();
        let bg = if recording {
            rgba(1.0, 0.3, 0.3, 0.3)
        } else if ui.home.hover_act == Some(i) {
            rgba(1.0, 1.0, 1.0, 0.15)
        } else {
            rgba(1.0, 1.0, 1.0, 0.08)
        };
        p.fill_round_rect(r, 16.0 * scale, bg)?;
        p.glyph(*glyph, (r.0 + 6.0 * scale, r.1, r.0 + 38.0 * scale, r.3), 17.0 * scale, rgba(1.0, 1.0, 1.0, 0.9))?;
        p.text_wrapped(
            if recording { "Stop recording" } else { label },
            (r.0 + 40.0 * scale, r.1 + 2.0 * scale, r.2 - 6.0 * scale, r.3 - 2.0 * scale),
            12.5 * scale,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            WHITE,
        )?;
    }

    paint_dock(p, pill, scale, ui)?;
    if discord_strip(ui) {
        paint_discord_strip(p, pill, scale, ui)?;
    }
    paint_menu(p, pill, scale, ui)
}
