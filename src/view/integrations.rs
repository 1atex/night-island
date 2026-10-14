use crate::view::*;

#[derive(Default)]
pub struct IntegrationsUi {
    pub id: TextEdit,
    pub secret: TextEdit,
    pub focus: u8,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum IntHit {
    Spotify,
    Discord,
    DiscordMute,
    DiscordDeaf,
    DiscordOverlay,
}

pub(in crate::view) fn int_card(pill: Rect, scale: f32, i: usize) -> Rect {
    let (x, y, w, _) = pill;
    let pad = (SIDE_PAD - 4.0) * scale;
    let gap = 12.0 * scale;
    let cw = (w - 2.0 * pad - gap) / 2.0;
    let left = x + pad + i as f32 * (cw + gap);
    (left, y + HOME_TOP * scale, left + cw, y + (HOME_TOP + 270.0) * scale)
}

pub(in crate::view) fn int_button(card: Rect, scale: f32) -> Rect {
    (card.0 + 16.0 * scale, card.3 - 56.0 * scale, card.2 - 16.0 * scale, card.3 - 16.0 * scale)
}

pub(in crate::view) fn discord_toggle(pill: Rect, scale: f32, i: usize) -> Rect {
    let card = int_card(pill, scale, 0);
    let size = 32.0 * scale;
    let right = card.2 - 12.0 * scale - i as f32 * (size + 6.0 * scale);
    (right - size, card.1 + 14.0 * scale, right, card.1 + 14.0 * scale + size)
}

pub fn int_fields(pill: Rect, scale: f32) -> [(Rect, Rect); 2] {
    let card = int_card(pill, scale, 0);
    std::array::from_fn(|i| {
        let top = card.1 + (78.0 + i as f32 * 42.0) * scale;
        let frame = (card.0 + 16.0 * scale, top, card.2 - 16.0 * scale, top + 34.0 * scale);
        (frame, (frame.0 + 12.0 * scale, frame.1, frame.2 - 10.0 * scale, frame.3))
    })
}

pub fn int_fields_visible(ui: &UiState) -> bool {
    ui.discord.status == crate::services::discord::Status::NoConfig
}

pub fn discord_fields_ok(ui: &UiState) -> bool {
    crate::services::discord::credentials_look_valid(&ui.int.id.text(), &ui.int.secret.text())
}

pub fn hit_int_field(pill: Rect, scale: f32, px: f32, py: f32) -> Option<u8> {
    int_fields(pill, scale).iter().position(|(f, _)| inside(*f, px, py)).map(|i| i as u8)
}

pub fn hit_int(pill: Rect, scale: f32, px: f32, py: f32, discord_connected: bool) -> Option<IntHit> {
    if discord_connected {
        for (i, hit) in [IntHit::DiscordDeaf, IntHit::DiscordMute, IntHit::DiscordOverlay].into_iter().enumerate() {
            if inside(discord_toggle(pill, scale, i), px, py) {
                return Some(hit);
            }
        }
    }
    for (i, hit) in [IntHit::Discord, IntHit::Spotify].into_iter().enumerate() {
        if inside(int_button(int_card(pill, scale, i), scale), px, py) {
            return Some(hit);
        }
    }
    None
}

pub fn discord_button(st: &crate::services::discord::Status, fields_ok: bool) -> (&'static str, bool) {
    use crate::services::discord::Status;
    match st {
        Status::NoConfig => ("Save", fields_ok),
        Status::Idle | Status::Error(_) => ("Connect", true),
        Status::Authorizing => ("Approve in Discord…", false),
        Status::NotRunning | Status::Connected => ("Disconnect", true),
    }
}

pub fn spotify_button(st: &crate::services::spotify::Status) -> (&'static str, bool) {
    use crate::services::spotify::Status;
    match st {
        Status::NoClientId => ("Open settings file", true),
        Status::Disconnected | Status::Error(_) => ("Connect", true),
        Status::Connecting => ("Waiting for the browser…", false),
        Status::Connected => ("Disconnect", true),
    }
}

pub(in crate::view) fn paint_integrations(p: &Painter, pill: Rect, scale: f32, ui: &UiState) -> Result<()> {
    use crate::services::spotify::Status;
    let dim = rgba(1.0, 1.0, 1.0, DIM);
    type Card<'a> = (&'a str, char, (f32, f32, f32), &'a str);
    let cards: [Card; 2] = [
        ("Discord", '\u{E8F2}', (0.35, 0.4, 0.95), "Who is in your voice channel and who writes to you."),
        ("Spotify", '\u{E8D6}', (0.12, 0.84, 0.38), "Shuffle, repeat and likes for the playing track."),
    ];
    for (i, (name, glyph, (r, g, b), about)) in cards.iter().enumerate() {
        let card = int_card(pill, scale, i);
        p.fill_round_rect(card, 22.0 * scale, rgba(1.0, 1.0, 1.0, 0.07))?;
        let d = 46.0 * scale;
        let circle =
            (card.0 + 16.0 * scale, card.1 + 16.0 * scale, card.0 + 16.0 * scale + d, card.1 + 16.0 * scale + d);
        p.fill_round_rect(circle, d / 2.0, rgba(*r, *g, *b, 1.0))?;
        p.glyph(*glyph, circle, 20.0 * scale, WHITE)?;
        p.text(
            name,
            (circle.2 + 12.0 * scale, circle.1, card.2 - 12.0 * scale, circle.1 + 26.0 * scale),
            17.0 * scale,
            DWRITE_FONT_WEIGHT_BOLD,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            WHITE,
        )?;
        let status = if i == 0 {
            use crate::services::discord::Status as D;
            match &ui.discord.status {
                D::NoConfig => "Needs a Client ID".to_string(),
                D::Idle => "Not connected".to_string(),
                D::NotRunning => "Not running".to_string(),
                D::Authorizing => "Approve in Discord…".to_string(),
                D::Connected if ui.discord.channel.is_empty() => "Not in a voice channel".to_string(),
                D::Connected => ui.discord.channel.clone(),
                D::Error(why) => why.clone(),
            }
        } else {
            match &ui.spotify.status {
                Status::NoClientId => "Needs a Client ID".to_string(),
                Status::Disconnected => "Not connected".to_string(),
                Status::Connecting => "Waiting for the browser…".to_string(),
                Status::Connected => "Connected".to_string(),
                Status::Error(why) => why.clone(),
            }
        };
        let live = if i == 0 {
            ui.discord.status == crate::services::discord::Status::Connected
        } else {
            ui.spotify.status == Status::Connected
        };
        let status_color = if live { rgba(0.4, 0.9, 0.55, 1.0) } else { dim };
        p.text(
            &status,
            (circle.2 + 12.0 * scale, circle.1 + 26.0 * scale, card.2 - 12.0 * scale, circle.3),
            13.0 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            status_color,
        )?;
        if !(i == 0 && (ui.discord.status == crate::services::discord::Status::Connected || int_fields_visible(ui))) {
            p.text_wrapped(
                about,
                (card.0 + 18.0 * scale, circle.3 + 10.0 * scale, card.2 - 14.0 * scale, circle.3 + 52.0 * scale),
                12.5 * scale,
                DWRITE_FONT_WEIGHT_NORMAL,
                rgba(1.0, 1.0, 1.0, 0.75),
            )?;
        }
        if i == 0 && int_fields_visible(ui) {
            let fields = int_fields(pill, scale);
            paint_line_edit(
                p,
                fields[0].0,
                fields[0].1,
                &ui.int.id,
                false,
                "Client ID",
                ui.editing && ui.int.focus == 0,
                ui.caret_on,
                scale,
            )?;
            paint_line_edit(
                p,
                fields[1].0,
                fields[1].1,
                &ui.int.secret,
                true,
                "Client Secret",
                ui.editing && ui.int.focus == 1,
                ui.caret_on,
                scale,
            )?;
            let lines = [
                "discord.com/developers: New Application,",
                "OAuth2: Redirect http://127.0.0.1,",
                "App Testers: add yourself.",
            ];
            for (n, line) in lines.iter().enumerate() {
                let top = fields[1].0.3 + 8.0 * scale + n as f32 * 16.0 * scale;
                p.text(
                    line,
                    (card.0 + 18.0 * scale, top, card.2 - 10.0 * scale, top + 16.0 * scale),
                    10.5 * scale,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_TEXT_ALIGNMENT_LEADING,
                    rgba(1.0, 1.0, 1.0, 0.45),
                )?;
            }
        } else if i == 0
            && matches!(
                ui.discord.status,
                crate::services::discord::Status::Idle | crate::services::discord::Status::Error(_)
            )
        {
            let lines = [
                "Discord must be running; you will see an",
                "Authorize prompt inside it. Your account has to",
                "be an App Tester of the application.",
            ];
            for (n, line) in lines.iter().enumerate() {
                let top = circle.3 + 64.0 * scale + n as f32 * 17.0 * scale;
                p.text(
                    line,
                    (card.0 + 18.0 * scale, top, card.2 - 10.0 * scale, top + 17.0 * scale),
                    11.0 * scale,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_TEXT_ALIGNMENT_LEADING,
                    rgba(1.0, 1.0, 1.0, 0.45),
                )?;
            }
        }
        if i == 0 && ui.discord.status == crate::services::discord::Status::Connected {
            for (n, m) in ui.discord.members.iter().take(5).enumerate() {
                let top = circle.3 + 14.0 * scale + n as f32 * 24.0 * scale;
                let dot = if m.speaking { rgba(0.35, 0.9, 0.5, 1.0) } else { rgba(1.0, 1.0, 1.0, 0.25) };
                p.fill_ellipse(card.0 + 24.0 * scale, top + 10.0 * scale, 4.5 * scale, 4.5 * scale, dot)?;
                p.text(
                    &m.name,
                    (card.0 + 38.0 * scale, top, card.2 - 44.0 * scale, top + 20.0 * scale),
                    13.0 * scale,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_TEXT_ALIGNMENT_LEADING,
                    WHITE,
                )?;
                if m.deaf || m.muted {
                    p.glyph(
                        if m.deaf { '\u{E74F}' } else { '\u{F781}' },
                        (card.2 - 40.0 * scale, top, card.2 - 14.0 * scale, top + 20.0 * scale),
                        12.0 * scale,
                        rgba(1.0, 0.45, 0.45, 1.0),
                    )?;
                }
            }
            if ui.discord.members.len() > 5 {
                let top = circle.3 + 14.0 * scale + 5.0 * 24.0 * scale;
                p.text(
                    &format!("+{}", ui.discord.members.len() - 5),
                    (card.0 + 38.0 * scale, top, card.2 - 14.0 * scale, top + 20.0 * scale),
                    12.0 * scale,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_TEXT_ALIGNMENT_LEADING,
                    dim,
                )?;
            }
            let toggles = [
                (0, '\u{E7F5}', ui.discord.self_deaf, 403),
                (1, '\u{E720}', ui.discord.self_mute, 402),
                (2, '\u{E8A7}', ui.cfg.discord_overlay, 404),
            ];
            for (slot, glyph, on, hover_id) in toggles {
                let r = discord_toggle(pill, scale, slot);
                let tint = if on && slot == 2 {
                    rgba(0.3, 0.8, 0.45, 0.4)
                } else if on {
                    rgba(1.0, 0.35, 0.35, 0.35)
                } else {
                    rgba(1.0, 1.0, 1.0, if ui.home.hover_act == Some(hover_id) { 0.2 } else { 0.1 })
                };
                p.fill_round_rect(r, 16.0 * scale, tint)?;
                let g = if slot == 2 {
                    glyph
                } else if on && slot == 1 {
                    '\u{F781}'
                } else if on {
                    '\u{E74F}'
                } else {
                    glyph
                };
                p.glyph(g, r, 14.0 * scale, WHITE)?;
            }
        }
        if i == 1 && matches!(ui.spotify.status, Status::NoClientId | Status::Disconnected | Status::Error(_)) {
            let lines = [
                "Create an app at developer.spotify.com,",
                "add the Redirect URI http://127.0.0.1:8898/callback",
                "and put its Client ID into the settings file.",
            ];
            for (n, line) in lines.iter().enumerate() {
                let top = circle.3 + 64.0 * scale + n as f32 * 17.0 * scale;
                p.text(
                    line,
                    (card.0 + 18.0 * scale, top, card.2 - 10.0 * scale, top + 17.0 * scale),
                    11.0 * scale,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_TEXT_ALIGNMENT_LEADING,
                    rgba(1.0, 1.0, 1.0, 0.45),
                )?;
            }
        }
        let btn = int_button(card, scale);
        let (label, enabled) = if i == 0 {
            discord_button(&ui.discord.status, discord_fields_ok(ui))
        } else {
            spotify_button(&ui.spotify.status)
        };
        let hover = ui.home.hover_act == Some(400 + i) && enabled;
        p.fill_round_rect(
            btn,
            14.0 * scale,
            rgba(
                1.0,
                1.0,
                1.0,
                if hover {
                    0.2
                } else if enabled {
                    0.12
                } else {
                    0.05
                },
            ),
        )?;
        p.text(
            label,
            btn,
            14.0 * scale,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_CENTER,
            rgba(1.0, 1.0, 1.0, if enabled { 1.0 } else { 0.4 }),
        )?;
    }
    Ok(())
}
