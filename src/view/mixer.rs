use crate::platform::audio_mixer::{AppVolume, Snapshot};
use crate::view::*;

#[derive(Default)]
pub struct MixerUi {
    pub state: Snapshot,
    pub scroll: usize,
    pub dragging: Option<String>,
    pub hover: Option<MixerHit>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MixerHit {
    Volume(usize),
    Mute(usize),
    Refresh,
}

pub fn mixer_list(pill: Rect, scale: f32) -> Rect {
    (
        pill.0 + SIDE_PAD * scale,
        pill.1 + 106.0 * scale,
        pill.0 + pill.2 - SIDE_PAD * scale,
        pill.1 + pill.3 - 20.0 * scale,
    )
}

pub fn mixer_visible_rows(pill: Rect, scale: f32) -> usize {
    let list = mixer_list(pill, scale);
    ((list.3 - list.1) / (58.0 * scale)).floor().max(1.0) as usize
}

pub fn mixer_row(pill: Rect, scale: f32, row: usize) -> Rect {
    let list = mixer_list(pill, scale);
    let top = list.1 + row as f32 * 58.0 * scale;
    (list.0, top, list.2, top + 52.0 * scale)
}

pub fn mixer_track(pill: Rect, scale: f32, row: usize) -> Rect {
    let row = mixer_row(pill, scale, row);
    (row.0 + 54.0 * scale, row.1 + 28.0 * scale, row.2 - 48.0 * scale, row.1 + 52.0 * scale)
}

fn mute_button(row: Rect, scale: f32) -> Rect {
    (row.2 - 38.0 * scale, row.1 + 22.0 * scale, row.2 - 8.0 * scale, row.1 + 52.0 * scale)
}

fn refresh_button(pill: Rect, scale: f32) -> Rect {
    (
        pill.0 + pill.2 - (SIDE_PAD + 32.0) * scale,
        pill.1 + 64.0 * scale,
        pill.0 + pill.2 - SIDE_PAD * scale,
        pill.1 + 96.0 * scale,
    )
}

pub fn hit_mixer(pill: Rect, scale: f32, ui: &MixerUi, x: f32, y: f32) -> Option<MixerHit> {
    if inside(refresh_button(pill, scale), x, y) {
        return Some(MixerHit::Refresh);
    }
    for row in 0..mixer_visible_rows(pill, scale).min(ui.state.apps.len().saturating_sub(ui.scroll)) {
        let rect = mixer_row(pill, scale, row);
        if inside(mute_button(rect, scale), x, y) {
            return Some(MixerHit::Mute(row + ui.scroll));
        }
        if inside(mixer_track(pill, scale, row), x, y) {
            return Some(MixerHit::Volume(row + ui.scroll));
        }
    }
    None
}

pub fn mixer_level_at(pill: Rect, scale: f32, x: f32) -> f32 {
    let track = mixer_track(pill, scale, 0);
    ((x - track.0) / (track.2 - track.0).max(1.0)).clamp(0.0, 1.0)
}

fn paint_app(p: &Painter, pill: Rect, scale: f32, row: usize, app: &AppVolume, ui: &MixerUi) -> Result<()> {
    let r = mixer_row(pill, scale, row);
    let hovered = matches!(ui.hover, Some(MixerHit::Volume(i) | MixerHit::Mute(i)) if i == row + ui.scroll);
    p.fill_round_rect(r, 14.0 * scale, rgba(1.0, 1.0, 1.0, if hovered { 0.1 } else { 0.055 }))?;
    let icon = (r.0 + 10.0 * scale, r.1 + 9.0 * scale, r.0 + 42.0 * scale, r.1 + 41.0 * scale);
    if let Some(image) = &app.icon {
        p.image(icon, 7.0 * scale, image.side, &image.bgra)?;
    } else {
        p.fill_round_rect(icon, 9.0 * scale, acc(0.2))?;
        p.glyph('\u{E8D6}', icon, 17.0 * scale, acc(1.0))?;
    }
    p.text(
        &app.name,
        (r.0 + 54.0 * scale, r.1 + 4.0 * scale, r.2 - 64.0 * scale, r.1 + 26.0 * scale),
        13.0 * scale,
        DWRITE_FONT_WEIGHT_SEMI_BOLD,
        DWRITE_TEXT_ALIGNMENT_LEADING,
        WHITE,
    )?;
    let value = format!("{}%", (app.level * 100.0).round() as i32);
    p.text(
        &value,
        (r.2 - 62.0 * scale, r.1 + 4.0 * scale, r.2 - 12.0 * scale, r.1 + 26.0 * scale),
        12.0 * scale,
        DWRITE_FONT_WEIGHT_NORMAL,
        DWRITE_TEXT_ALIGNMENT_TRAILING,
        rgba(1.0, 1.0, 1.0, 0.65),
    )?;
    let track = mixer_track(pill, scale, row);
    let cy = (track.1 + track.3) / 2.0;
    let end = track.0 + (track.2 - track.0) * app.level.clamp(0.0, 1.0);
    let color = if app.muted { rgba(1.0, 1.0, 1.0, 0.3) } else { acc(0.95) };
    p.fill_round_rect((track.0, cy - 2.0 * scale, track.2, cy + 2.0 * scale), 2.0 * scale, rgba(1.0, 1.0, 1.0, 0.12))?;
    p.fill_round_rect((track.0, cy - 2.0 * scale, end, cy + 2.0 * scale), 2.0 * scale, color)?;
    p.fill_ellipse(end, cy, 5.0 * scale, 5.0 * scale, WHITE)?;
    let mute = mute_button(r, scale);
    p.glyph(if app.muted || app.level <= 0.001 { '\u{E74F}' } else { '\u{E767}' }, mute, 15.0 * scale, color)?;
    if app.active {
        p.fill_ellipse(
            icon.2 - 1.0 * scale,
            icon.3 - 1.0 * scale,
            3.0 * scale,
            3.0 * scale,
            rgba(0.4, 0.9, 0.65, 1.0),
        )?;
    }
    Ok(())
}

pub fn paint_mixer(p: &Painter, pill: Rect, scale: f32, ui: &MixerUi) -> Result<()> {
    let left = pill.0 + SIDE_PAD * scale;
    p.text(
        "App mixer",
        (left, pill.1 + 62.0 * scale, pill.0 + pill.2 - SIDE_PAD * scale - 40.0 * scale, pill.1 + 86.0 * scale),
        17.0 * scale,
        DWRITE_FONT_WEIGHT_SEMI_BOLD,
        DWRITE_TEXT_ALIGNMENT_LEADING,
        WHITE,
    )?;
    p.text(
        "Separate volume for each app",
        (left, pill.1 + 84.0 * scale, pill.0 + pill.2 - SIDE_PAD * scale - 40.0 * scale, pill.1 + 102.0 * scale),
        11.5 * scale,
        DWRITE_FONT_WEIGHT_NORMAL,
        DWRITE_TEXT_ALIGNMENT_LEADING,
        rgba(1.0, 1.0, 1.0, 0.5),
    )?;
    p.glyph('\u{E72C}', refresh_button(pill, scale), 16.0 * scale, rgba(1.0, 1.0, 1.0, 0.7))?;
    let list = mixer_list(pill, scale);
    if ui.state.apps.is_empty() {
        let label = if !ui.state.ready {
            "Looking for audio apps…"
        } else if ui.state.error.is_some() {
            "Audio mixer is unavailable"
        } else {
            "Play music, a video or a game to see its volume here"
        };
        p.text_wrapped(
            label,
            (list.0 + 30.0 * scale, list.1 + 36.0 * scale, list.2 - 30.0 * scale, list.3 - 36.0 * scale),
            14.0 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            rgba(1.0, 1.0, 1.0, 0.55),
        )?;
        return Ok(());
    }
    for (row, app) in ui.state.apps.iter().skip(ui.scroll).take(mixer_visible_rows(pill, scale)).enumerate() {
        paint_app(p, pill, scale, row, app, ui)?;
    }
    let rows = mixer_visible_rows(pill, scale);
    if ui.state.apps.len() > rows {
        let height = list.3 - list.1;
        let thumb = height * rows as f32 / ui.state.apps.len() as f32;
        let top = list.1 + (height - thumb) * ui.scroll as f32 / (ui.state.apps.len() - rows) as f32;
        p.fill_round_rect(
            (list.2 + 5.0 * scale, top, list.2 + 8.0 * scale, top + thumb),
            1.5 * scale,
            rgba(1.0, 1.0, 1.0, 0.25),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixer_drag_clamps_and_scrolled_rows_target_the_visible_application() {
        for scale in [0.75, 1.0, 1.5, 2.0] {
            let pill = (18.0, 23.0, 640.0 * scale, 384.0 * scale);
            let mut ui = MixerUi { scroll: 3, ..Default::default() };
            ui.state.apps = (0..12)
                .map(|i| AppVolume {
                    key: i.to_string(),
                    name: i.to_string(),
                    level: 0.5,
                    muted: false,
                    active: false,
                    icon: None,
                })
                .collect();
            let track = mixer_track(pill, scale, 0);
            assert_eq!(mixer_level_at(pill, scale, track.0 - 100.0), 0.0);
            assert_eq!(mixer_level_at(pill, scale, track.2 + 100.0), 1.0);
            assert!((mixer_level_at(pill, scale, (track.0 + track.2) / 2.0) - 0.5).abs() < 0.001);
            assert!(hit_mixer(pill, scale, &ui, track.0, (track.1 + track.3) / 2.0) == Some(MixerHit::Volume(3)));
            let mute = mute_button(mixer_row(pill, scale, 1), scale);
            assert!(
                hit_mixer(pill, scale, &ui, (mute.0 + mute.2) / 2.0, (mute.1 + mute.3) / 2.0)
                    == Some(MixerHit::Mute(4))
            );
            assert!(hit_mixer(pill, scale, &ui, pill.0, pill.1).is_none());
        }
    }
}
