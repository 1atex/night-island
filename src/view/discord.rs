use crate::view::*;

pub(in crate::view) fn strip_toggle(pill: Rect, scale: f32, i: usize) -> Rect {
    let z = strip_zone(pill, scale);
    let size = 30.0 * scale;
    let right = z.2 - 8.0 * scale - i as f32 * (size + 6.0 * scale);
    (right - size, z.1 + 5.0 * scale, right, z.1 + 5.0 * scale + size)
}

pub fn discord_strip(ui: &UiState) -> bool {
    ui.discord.status == crate::services::discord::Status::Connected && !ui.discord.members.is_empty()
}

pub(in crate::view) fn paint_discord_strip(p: &Painter, pill: Rect, scale: f32, ui: &UiState) -> Result<()> {
    let z = strip_zone(pill, scale);
    p.fill_round_rect(z, 18.0 * scale, rgba(0.35, 0.4, 0.95, 0.14))?;
    card_depth(p, z, 18.0 * scale, scale, ui)?;
    p.glyph(
        crate::ui::icons::DISCORD_GLYPH,
        (z.0 + 8.0 * scale, z.1, z.0 + 32.0 * scale, z.3),
        14.0 * scale,
        rgba(0.6, 0.65, 1.0, 1.0),
    )?;
    let (left, right) = (z.0 + 36.0 * scale, z.2 - 3.0 * 36.0 * scale - 14.0 * scale);
    let (mut x, y) = (left, z.1 + 8.0 * scale);
    let chip_h = 24.0 * scale;
    let mut shown = 0;
    for m in &ui.discord.members {
        let w = (30.0 + m.name.chars().count() as f32 * 7.2 + if m.muted || m.deaf { 16.0 } else { 0.0 }) * scale;
        if x + w > right - 36.0 * scale && shown < ui.discord.members.len() - 1 || x + w > right {
            break;
        }
        shown += 1;
        let r = (x, y, x + w, y + chip_h);
        p.fill_round_rect(r, chip_h / 2.0, rgba(1.0, 1.0, 1.0, if m.speaking { 0.2 } else { 0.1 }))?;
        let dot = if m.speaking { rgba(0.35, 0.9, 0.5, 1.0) } else { rgba(1.0, 1.0, 1.0, 0.3) };
        p.fill_ellipse(x + 11.0 * scale, y + chip_h / 2.0, 4.5 * scale, 4.5 * scale, dot)?;
        let name_right = r.2 - if m.muted || m.deaf { 20.0 } else { 8.0 } * scale;
        p.text(
            &m.name,
            (x + 22.0 * scale, y, name_right, r.3),
            12.5 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            WHITE,
        )?;
        if m.muted || m.deaf {
            p.glyph(
                if m.deaf { '\u{E74F}' } else { '\u{F781}' },
                (r.2 - 22.0 * scale, y, r.2 - 4.0 * scale, r.3),
                11.0 * scale,
                rgba(1.0, 0.45, 0.45, 1.0),
            )?;
        }
        x += w + 6.0 * scale;
    }
    let hidden = ui.discord.members.len() - shown;
    if hidden > 0 {
        p.text(
            &format!("+{hidden}"),
            (x, y, x + 40.0 * scale, y + chip_h),
            12.0 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            rgba(1.0, 1.0, 1.0, 0.6),
        )?;
    }
    let toggles =
        [('\u{E7F5}', ui.discord.self_deaf), ('\u{E720}', ui.discord.self_mute), ('\u{E8A7}', ui.cfg.discord_overlay)];
    for (n, (glyph, on)) in toggles.into_iter().enumerate() {
        let r = strip_toggle(pill, scale, n);
        let hover = ui.home.hover_act == Some(540 + n);
        let live = on && n == 2;
        p.fill_round_rect(
            r,
            15.0 * scale,
            if live {
                rgba(0.3, 0.8, 0.45, 0.45)
            } else if on {
                rgba(1.0, 0.35, 0.35, 0.4)
            } else {
                rgba(1.0, 1.0, 1.0, if hover { 0.22 } else { 0.12 })
            },
        )?;
        let g = if n == 2 {
            glyph
        } else if on && n == 1 {
            '\u{F781}'
        } else if on {
            '\u{E74F}'
        } else {
            glyph
        };
        p.glyph(
            g,
            r,
            14.0 * scale,
            if n == 2 || ui.discord.voice_available { WHITE } else { rgba(1.0, 1.0, 1.0, 0.3) },
        )?;
    }
    Ok(())
}

pub fn overlay_size(scale: f32, members: usize) -> (i32, i32) {
    let rows = members.min(10) as f32;
    let extra = if members > 10 { 20.0 } else { 0.0 };
    ((260.0 * scale).round() as i32, ((44.0 + rows * 26.0 + extra + 46.0) * scale).round() as i32)
}

pub fn overlay_buttons(size: (f32, f32), scale: f32) -> [Rect; 3] {
    let (w, h) = size;
    let s = 30.0 * scale;
    let close = (w - 8.0 * scale - 26.0 * scale, 8.0 * scale, w - 8.0 * scale, 8.0 * scale + 26.0 * scale);
    let deaf = (w - 10.0 * scale - s, h - 8.0 * scale - s, w - 10.0 * scale, h - 8.0 * scale);
    let mute = (deaf.0 - 6.0 * scale - s, deaf.1, deaf.0 - 6.0 * scale, deaf.3);
    [close, mute, deaf]
}

pub fn paint_discord_overlay(
    p: &Painter,
    size: (f32, f32),
    scale: f32,
    d: &crate::services::discord::State,
) -> Result<()> {
    let (w, h) = size;
    let (pr, pg, pb) = crate::ui::theme::panel();
    let panel = (0.0, 0.0, w, h);
    p.fill_round_rect(panel, 18.0 * scale, rgba(pr, pg, pb, 0.94))?;
    p.stroke_round_rect((0.5, 0.5, w - 0.5, h - 0.5), 18.0 * scale, 1.0, rgba(1.0, 1.0, 1.0, 0.12))?;
    p.glyph(
        crate::ui::icons::DISCORD_GLYPH,
        (10.0 * scale, 8.0 * scale, 34.0 * scale, 34.0 * scale),
        14.0 * scale,
        rgba(0.6, 0.65, 1.0, 1.0),
    )?;
    let title = if d.channel.is_empty() { "Not in a voice channel" } else { d.channel.as_str() };
    p.text(
        title,
        (38.0 * scale, 8.0 * scale, w - 40.0 * scale, 34.0 * scale),
        13.5 * scale,
        DWRITE_FONT_WEIGHT_SEMI_BOLD,
        DWRITE_TEXT_ALIGNMENT_LEADING,
        WHITE,
    )?;
    let [close, mute, deaf] = overlay_buttons(size, scale);
    p.glyph('\u{E711}', close, 12.0 * scale, rgba(1.0, 1.0, 1.0, 0.6))?;
    for (i, m) in d.members.iter().take(10).enumerate() {
        let top = (44.0 + i as f32 * 26.0) * scale;
        let row = (8.0 * scale, top, w - 8.0 * scale, top + 24.0 * scale);
        if m.speaking {
            p.fill_round_rect(row, 8.0 * scale, rgba(0.35, 0.9, 0.5, 0.16))?;
        }
        let dot = if m.speaking { rgba(0.35, 0.9, 0.5, 1.0) } else { rgba(1.0, 1.0, 1.0, 0.25) };
        p.fill_ellipse(20.0 * scale, top + 12.0 * scale, 4.5 * scale, 4.5 * scale, dot)?;
        p.text(
            &m.name,
            (32.0 * scale, top, w - 36.0 * scale, top + 24.0 * scale),
            13.0 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            WHITE,
        )?;
        if m.muted || m.deaf {
            p.glyph(
                if m.deaf { '\u{E74F}' } else { '\u{F781}' },
                (w - 34.0 * scale, top, w - 12.0 * scale, top + 24.0 * scale),
                12.0 * scale,
                rgba(1.0, 0.45, 0.45, 1.0),
            )?;
        }
    }
    if d.members.len() > 10 {
        let top = (44.0 + 10.0 * 26.0) * scale;
        p.text(
            &format!("+{} {}", d.members.len() - 10, crate::ui::i18n::tr("more")),
            (12.0 * scale, top, w - 12.0 * scale, top + 20.0 * scale),
            12.0 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            rgba(1.0, 1.0, 1.0, 0.55),
        )?;
    }
    for (r, glyph, on) in [(mute, '\u{E720}', d.self_mute), (deaf, '\u{E7F5}', d.self_deaf)] {
        p.fill_round_rect(r, 15.0 * scale, if on { rgba(1.0, 0.35, 0.35, 0.4) } else { rgba(1.0, 1.0, 1.0, 0.12) })?;
        let g = if on && glyph == '\u{E720}' {
            '\u{F781}'
        } else if on {
            '\u{E74F}'
        } else {
            glyph
        };
        p.glyph(g, r, 14.0 * scale, if d.voice_available { WHITE } else { rgba(1.0, 1.0, 1.0, 0.3) })?;
    }
    Ok(())
}
