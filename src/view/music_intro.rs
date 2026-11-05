use crate::data::settings::Edge;
use crate::view::*;

pub struct MusicEntranceLayout {
    pub cover: Rect,
    pub title: Rect,
    pub final_cover: Option<Rect>,
    pub final_title: Option<Rect>,
    pub rotation: f32,
}

pub fn music_entrance_layout(pill: Rect, scale: f32, ui: &UiState) -> MusicEntranceLayout {
    let (x, y, w, h) = pill;
    let text = (ui.media.title.chars().count() as f32 * 7.6).clamp(64.0, 240.0) * scale;
    let cover = 28.0 * scale;
    let gap = 12.0 * scale;
    if ui.cfg.vertical() {
        let top = y + (h - cover - gap - text) / 2.0;
        let cx = x + w / 2.0;
        let slots = vertical_slots(ui, scale, h);
        let side = (w - 14.0 * scale).min(30.0 * scale);
        return MusicEntranceLayout {
            cover: (cx - cover / 2.0, top, cx + cover / 2.0, top + cover),
            title: (cx - 17.0 * scale, top + cover + gap, cx + 17.0 * scale, top + cover + gap + text),
            final_cover: slots
                .cover
                .filter(|_| ui.media.cover.is_some())
                .map(|s| (cx - side / 2.0, y + s.0, cx + side / 2.0, y + s.0 + side)),
            final_title: slots.title.map(|s| (x, y + s.0, x + w, y + s.0 + s.1)),
            rotation: if ui.cfg.dock_edge == Edge::Left { -90.0 } else { 90.0 },
        };
    }
    let left = x + (w - cover - gap - text) / 2.0;
    let slots = music_slots(ui, scale, w);
    let side = (26.0 * scale).min(h - 10.0 * scale);
    let top = y + (h - side) / 2.0;
    MusicEntranceLayout {
        cover: (left, y + (h - cover) / 2.0, left + cover, y + (h + cover) / 2.0),
        title: (left + cover + gap, y, left + cover + gap + text, y + h),
        final_cover: slots
            .cover
            .filter(|_| ui.media.cover.is_some())
            .map(|s| (x + s.0, top, x + s.0 + s.1.min(side * cover_aspect(ui)), top + side)),
        final_title: slots.title.map(|s| (x + s.0, y, x + s.0 + s.1, y + h)),
        rotation: 0.0,
    }
}

pub fn paint_music_entrance_cover(p: &Painter, scale: f32, ui: &UiState) -> Result<()> {
    let size = 28.0 * scale;
    let rect = (0.0, 0.0, size, size);
    if let Some(cover) = &ui.media.cover {
        p.cover(rect, 7.0 * scale, cover, crate::data::settings::CoverMode::Crop)
    } else {
        p.fill_round_rect(rect, 7.0 * scale, acc(0.16))?;
        p.glyph('\u{E8D6}', rect, 17.0 * scale, acc(1.0))
    }
}

pub fn paint_music_entrance_title(p: &Painter, scale: f32, ui: &UiState) -> Result<()> {
    p.text(
        &ui.media.title,
        (0.0, 0.0, 240.0 * scale, 34.0 * scale),
        15.0 * scale,
        DWRITE_FONT_WEIGHT_SEMI_BOLD,
        DWRITE_TEXT_ALIGNMENT_LEADING,
        WHITE,
    )
}

#[cfg(test)]
pub fn render_music_entrance_preview(gpu: &crate::ui::gfx::Gpu, ui: &UiState, path: &std::path::Path) -> Result<()> {
    let surface = crate::ui::popup::PopupSurface::new((560, 230))?;
    surface.paint(gpu, |p| {
        p.fill_rect((0.0, 0.0, 560.0, 230.0), rgba(0.035, 0.04, 0.055, 1.0))?;
        let normal = collapsed_width(ui, 200.0);
        let wide = normal.max(340.0);
        for (row, progress) in [0.0, 0.5, 1.0].into_iter().enumerate() {
            let y = 30.0 + row as f32 * 72.0;
            let mix = |a: f32, b: f32| a + (b - a) * progress;
            let width = mix(wide, normal);
            let pill = ((560.0 - width) / 2.0, y, width, 34.0);
            p.fill_round_rect((pill.0, y, pill.0 + width, y + 34.0), 17.0, rgba(0.075, 0.08, 0.095, 1.0))?;
            p.text(
                ["First playback", "Settling", "Ready"][row],
                (24.0, y - 23.0, 536.0, y - 4.0),
                10.0,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_TEXT_ALIGNMENT_CENTER,
                rgba(1.0, 1.0, 1.0, 0.45),
            )?;
            if row == 2 {
                paint_collapsed(p, pill, 1.0, ui)?;
                let old = p.push_offset(pill.0 + 6.0, y + 2.0);
                crate::ui::characters::draw(
                    p,
                    crate::ui::characters::Character::Spark,
                    40.0,
                    &crate::ui::characters::Pose {
                        motion: Default::default(),
                        t: 0.0,
                        reaction: crate::ui::characters::Reaction::Idle,
                        look: (0.0, 0.0),
                        blink: false,
                        accent: (0.5, 0.6, 0.9),
                        beat: 0.0,
                    },
                )?;
                p.set_transform(&old);
                continue;
            }
            let start = music_entrance_layout(((560.0 - wide) / 2.0, y, wide, 34.0), 1.0, ui);
            let end = music_entrance_layout(((560.0 - normal) / 2.0, y, normal, 34.0), 1.0, ui);
            let destination = end.final_cover.unwrap_or(start.cover);
            let scale = mix(1.0, (destination.3 - destination.1) / 28.0);
            let old = p.push_offset(0.0, 0.0);
            p.set_transform(&windows_numerics::Matrix3x2 {
                M11: scale,
                M22: scale,
                M31: mix(start.cover.0, destination.0),
                M32: mix(start.cover.1, destination.1),
                ..Default::default()
            });
            paint_music_entrance_cover(p, 1.0, ui)?;
            let destination = end.final_title.unwrap_or(start.title);
            let scale = mix(1.0, 12.5 / 15.0);
            p.set_transform(&windows_numerics::Matrix3x2 {
                M11: scale,
                M22: scale,
                M31: mix(start.title.0, destination.0),
                M32: y + (34.0 - 34.0 * scale) / 2.0,
                ..Default::default()
            });
            paint_music_entrance_title(p, 1.0, ui)?;
            p.set_transform(&old);
        }
        Ok(())
    })?;
    let rgba: Vec<u8> = surface.pixels().chunks_exact(4).flat_map(|p| [p[2], p[1], p[0], p[3]]).collect();
    let mut encoder = png::Encoder::new(std::fs::File::create(path).unwrap(), 560, 230);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header().unwrap().write_image_data(&rgba).unwrap();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn music_presentations_fit_horizontal_and_vertical_islands_at_every_scale() {
        for scale in [0.75, 1.0, 1.25, 2.0] {
            for edge in Edge::ALL {
                let mut ui = UiState::new();
                ui.cfg.placement = crate::data::settings::Placement::Docked;
                ui.cfg.dock_edge = edge;
                ui.media.playing = true;
                ui.media.title = "A very long first song title which must fit the capsule".into();
                let pill = if ui.cfg.vertical() {
                    (12.0, 12.0, VERT_W * scale, 600.0 * scale)
                } else {
                    (12.0, 12.0, 600.0 * scale, 34.0 * scale)
                };
                let layout = music_entrance_layout(pill, scale, &ui);
                for rect in [Some(layout.cover), Some(layout.title), layout.final_cover, layout.final_title]
                    .into_iter()
                    .flatten()
                {
                    assert!(rect.0 >= pill.0 - 0.01 && rect.1 >= pill.1 - 0.01);
                    assert!(rect.2 <= pill.0 + pill.2 + 0.01 && rect.3 <= pill.1 + pill.3 + 0.01);
                }
            }
        }
    }
}
