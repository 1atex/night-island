use super::*;

fn rect(pill: Rect, s: f32, r: Rect) -> Rect {
    (pill.0 + r.0 * s, pill.1 + r.1 * s, pill.0 + r.2 * s, pill.1 + r.3 * s)
}

pub(super) fn controls(pill: Rect, s: f32, ui: &YandexUi) -> Vec<(Rect, YandexHit)> {
    use YandexHit::*;
    let w = pill.2 / s;
    let r = |a, b, c, d| rect(pill, s, (a, b, c, d));
    if let Some((track, _)) = &ui.track_menu {
        let mut out = vec![
            (r(w - 62.0, 150.0, w - 30.0, 180.0), CloseMenu),
            (r(40.0, 214.0, w - 40.0, 248.0), MenuPlay),
            (r(40.0, 254.0, w - 40.0, 288.0), MenuTrackStation),
        ];
        if !track.artists.is_empty() {
            out.push((r(40.0, 294.0, w - 40.0, 328.0), MenuArtistStation));
        }
        return out;
    }
    if let Some(index) = ui.open_group {
        let mut out = vec![(r(w - 62.0, 150.0, w - 30.0, 180.0), CloseMenu)];
        if let Some(group) = ui.state.groups.get(index) {
            if group.items.len() > 9 {
                if ui.group_scroll > 0 {
                    out.push((r(w - 154.0, 150.0, w - 120.0, 180.0), GroupPage(-1)));
                }
                if ui.group_scroll + 9 < group.items.len() {
                    out.push((r(w - 112.0, 150.0, w - 78.0, 180.0), GroupPage(1)));
                }
            }
            for option in 0..=group.items.len().saturating_sub(ui.group_scroll).min(9) {
                let x = 40.0 + (option % 2) as f32 * ((w - 88.0) / 2.0);
                let y = 190.0 + (option / 2) as f32 * 31.0;
                out.push((
                    r(x, y, x + (w - 104.0) / 2.0, y + 27.0),
                    GroupPick(index, option.checked_sub(1).map(|i| i + ui.group_scroll)),
                ));
            }
        }
        return out;
    }
    let mut out = vec![
        (r(w - 270.0, 62.0, w - 116.0, 96.0), StartWave),
        (r(w - 106.0, 62.0, w - 24.0, 96.0), Disconnect),
        (r(24.0, 106.0, 190.0, 140.0), Overview),
    ];
    if !ui.search_hidden {
        out.push((r(198.0, 106.0, 350.0, 140.0), Search));
    }
    if !ui.ratings_hidden {
        out.push((r(358.0, 106.0, w - 24.0, 140.0), Library));
    }
    if ui.search_open {
        out.push((wave_search_field(pill, s).0, SearchInput));
        for (i, _) in ui.search_rows().iter().enumerate() {
            let y = 192.0 + i as f32 * 48.0;
            out.push((r(24.0, y, w - 72.0, y + 42.0), PlaySearch(i)));
            out.push((r(w - 66.0, y, w - 24.0, y + 42.0), TrackMenu(i, false)));
        }
        if ui.search_page > 0 {
            out.push((r(24.0, 344.0, 60.0, 374.0), SearchPage(-1)));
        }
        if ui.search_result().is_some_and(|v| (ui.search_page + 1) * SEARCH_ROWS < v.tracks.len()) {
            out.push((r(w - 60.0, 344.0, w - 24.0, 374.0), SearchPage(1)));
        }
    } else if ui.show_library {
        if let Some(page) = &ui.state.library {
            for (i, _) in page.tracks.iter().enumerate().skip(ui.library_scroll).take(4) {
                let y = 148.0 + (i - ui.library_scroll) as f32 * 48.0;
                out.push((r(24.0, y, w - 82.0, y + 42.0), PlayLiked(i)));
                out.push((r(w - 76.0, y, w - 36.0, y + 42.0), TrackMenu(i, true)));
            }
            out.push((r(w - 30.0, 150.0, w - 18.0, 338.0), LibraryJump));
            if page.page > 0 || ui.library_scroll > 0 {
                out.push((r(24.0, 344.0, 60.0, 374.0), LibraryPage(-1)));
            }
            if page.page * LIBRARY_PAGE + ui.library_scroll + 4 < page.total {
                out.push((r(w - 60.0, 344.0, w - 24.0, 374.0), LibraryPage(1)));
            }
        }
    } else {
        if ui.state.current_track.is_some() {
            out.push((r(24.0, 174.0, w / 2.0 - 4.0, 224.0), CurrentStation(false)));
            if ui.state.current_track.as_ref().is_some_and(|v| !v.artists.is_empty()) {
                out.push((r(w / 2.0 + 4.0, 174.0, w - 24.0, 224.0), CurrentStation(true)));
            }
        }
        for (index, _) in ui.state.groups.iter().take(4).enumerate() {
            let x = if index % 2 == 0 { 24.0 } else { w / 2.0 + 4.0 };
            let right = if index % 2 == 0 { w / 2.0 - 4.0 } else { w - 24.0 };
            let y = 232.0 + (index / 2) as f32 * 54.0;
            out.push((r(x, y, right, y + 46.0), GroupMenu(index)));
        }
        out.push((r(24.0, 344.0, w - 190.0, 374.0), Autoplay));
        out.push((r(w - 174.0, 344.0, w - 24.0, 374.0), Volume));
    }
    if !ui.state.error.is_empty() {
        out.retain(|(r, _)| r.1 < pill.1 + 344.0 * s);
        out.push((r(w - 116.0, 344.0, w - 24.0, 374.0), Retry));
    }
    out
}

fn label(p: &Painter, text: &str, r: Rect, s: f32, size: f32, strong: bool, alpha: f32) -> Result<()> {
    p.text(
        text,
        r,
        size * s,
        if strong { DWRITE_FONT_WEIGHT_SEMI_BOLD } else { DWRITE_FONT_WEIGHT_NORMAL },
        DWRITE_TEXT_ALIGNMENT_LEADING,
        rgba(1.0, 1.0, 1.0, alpha),
    )
}

fn cover(p: &Painter, r: Rect, s: f32, ui: &YandexUi, track: &crate::services::yandex::Track) -> Result<()> {
    if let Some(image) = ui.state.thumbnails.get(&track.id) {
        p.image(r, 8.0 * s, image.size, &image.bgra)
    } else {
        p.fill_round_rect(r, 8.0 * s, wave_colour(ui.colours().accent, 0.18))?;
        p.glyph('\u{E8D6}', r, 17.0 * s, acc(0.9))
    }
}

pub(super) fn paint(p: &Painter, pill: Rect, s: f32, ui: &YandexUi, editing: bool, caret: bool) -> Result<()> {
    use YandexHit::*;
    let w = pill.2 / s;
    let r = |a, b, c, d| rect(pill, s, (a, b, c, d));
    let colours = ui.colours();
    p.fill_round_gradient(
        r(12.0, 54.0, w - 12.0, 378.0),
        22.0 * s,
        wave_colour(colours.background, 1.0),
        rgba(0.02, 0.02, 0.03, 1.0),
    )?;
    label(p, "My Wave", r(24.0, 62.0, w - 282.0, 96.0), s, 18.0, true, 1.0)?;
    if ui.track_menu.is_some() || ui.open_group.is_some() {
        p.fill_round_rect(r(24.0, 146.0, w - 24.0, 346.0), 18.0 * s, wave_colour(colours.card, 1.0))?;
        if let Some((track, _)) = &ui.track_menu {
            cover(p, r(40.0, 158.0, 82.0, 200.0), s, ui, track)?;
            label(p, &track.title, r(94.0, 154.0, w - 72.0, 179.0), s, 14.0, true, 1.0)?;
            label(p, &track.artist, r(94.0, 179.0, w - 72.0, 200.0), s, 11.5, false, 0.7)?;
        } else if let Some(group) = ui.open_group.and_then(|i| ui.state.groups.get(i)) {
            label(p, &group.name, r(40.0, 151.0, w - 72.0, 181.0), s, 16.0, true, 1.0)?;
        }
    } else {
        if ui.search_open {
            let field = ui.search_field(pill, s);
            paint_line_edit(
                p,
                field.0,
                field.1,
                &ui.search_input,
                false,
                "Search tracks or artists",
                editing,
                caret,
                s,
            )?;
            if ui.search_result().is_none() {
                label(
                    p,
                    if ui.search_query().is_empty() {
                        "Find a track to play or start its wave"
                    } else {
                        "Searching…"
                    },
                    r(24.0, 204.0, w - 24.0, 242.0),
                    s,
                    13.0,
                    false,
                    0.65,
                )?;
            }
        } else if ui.show_library {
            if let Some(page) = &ui.state.library {
                let first = page.page * LIBRARY_PAGE + ui.library_scroll;
                label(
                    p,
                    &format!("{}–{} / {}", (first + 1).min(page.total), (first + 4).min(page.total), page.total),
                    r(76.0, 346.0, w - 76.0, 374.0),
                    s,
                    12.0,
                    false,
                    0.6,
                )?;
                if page.total > 0 {
                    let track = r(w - 27.0, 150.0, w - 21.0, 338.0);
                    p.fill_round_rect(track, 3.0 * s, rgba(1.0, 1.0, 1.0, 0.08))?;
                    let height = ((track.3 - track.1) * 4.0 / page.total.max(4) as f32).max(24.0 * s);
                    let y = track.1
                        + (track.3 - track.1 - height)
                            * (first as f32 / page.total.saturating_sub(4).max(1) as f32).clamp(0.0, 1.0);
                    p.fill_round_rect((track.0, y, track.2, y + height), 3.0 * s, acc(0.8))?;
                }
                if page.tracks.is_empty() {
                    label(p, "No liked tracks yet", r(24.0, 170.0, w - 24.0, 210.0), s, 14.0, false, 0.7)?;
                }
            } else {
                label(p, "Loading liked tracks…", r(24.0, 170.0, w - 24.0, 210.0), s, 14.0, false, 0.7)?;
            }
        } else {
            label(
                p,
                if ui.state.station_name.is_empty() { "Your personal wave" } else { &ui.state.station_name },
                r(24.0, 145.0, w - 24.0, 170.0),
                s,
                12.0,
                false,
                0.7,
            )?;
            if ui.state.current_track.is_none() {
                label(p, "Start your wave or find a track", r(24.0, 180.0, w - 24.0, 220.0), s, 14.0, false, 0.7)?;
            }
        }
    }
    for (button, hit) in controls(pill, s, ui) {
        if matches!(hit, SearchInput | LibraryJump) {
            continue;
        }
        let selected = match hit {
            Autoplay => ui.state.config.autoplay,
            Overview => !ui.search_open && !ui.show_library,
            Search => ui.search_open,
            Library => ui.show_library,
            GroupPick(index, option) => ui.state.groups.get(index).is_some_and(|g| {
                ui.state.config.selection.get(&g.key) == option.and_then(|i| g.items.get(i).map(|v| &v.1))
            }),
            PlaySearch(index) => {
                ui.search_selected == Some(index)
                    || ui.search_rows().get(index).is_some_and(|t| ui.state.current_id.as_ref() == Some(&t.id))
            }
            PlayLiked(index) => ui
                .state
                .library
                .as_ref()
                .and_then(|v| v.tracks.get(index))
                .is_some_and(|t| ui.state.current_id.as_ref() == Some(&t.id)),
            _ => false,
        };
        let hover = ui.hover == Some(hit);
        p.fill_round_rect(
            button,
            12.0 * s,
            if hit == StartWave || selected {
                wave_colour(colours.accent, if hover { 0.38 } else { 0.25 })
            } else {
                rgba(1.0, 1.0, 1.0, if hover { 0.13 } else { 0.055 })
            },
        )?;
        match hit {
            PlayLiked(index) | PlaySearch(index) => {
                let track = if matches!(hit, PlayLiked(_)) {
                    ui.state.library.as_ref().and_then(|v| v.tracks.get(index))
                } else {
                    ui.search_rows().get(index)
                };
                if let Some(track) = track {
                    let image = (button.0 + 4.0 * s, button.1 + 3.0 * s, button.0 + 40.0 * s, button.1 + 39.0 * s);
                    cover(p, image, s, ui, track)?;
                    label(
                        p,
                        &track.title,
                        (button.0 + 50.0 * s, button.1 + 2.0 * s, button.2 - 8.0 * s, button.1 + 23.0 * s),
                        s,
                        13.0,
                        true,
                        1.0,
                    )?;
                    label(
                        p,
                        &track.artist,
                        (button.0 + 50.0 * s, button.1 + 22.0 * s, button.2 - 8.0 * s, button.3 - 2.0 * s),
                        s,
                        11.0,
                        false,
                        0.6,
                    )?;
                }
            }
            TrackMenu(..) => p.glyph('\u{E712}', button, 16.0 * s, WHITE)?,
            CloseMenu => p.glyph('\u{E711}', button, 12.0 * s, WHITE)?,
            LibraryPage(dir) | SearchPage(dir) | GroupPage(dir) => {
                p.glyph(if dir < 0 { '\u{E76B}' } else { '\u{E76C}' }, button, 12.0 * s, WHITE)?
            }
            GroupMenu(index) => {
                if let Some(group) = ui.state.groups.get(index) {
                    let value = ui
                        .state
                        .config
                        .selection
                        .get(&group.key)
                        .and_then(|seed| group.items.iter().find(|v| &v.1 == seed))
                        .map_or("Any", |v| v.0.as_str());
                    label(
                        p,
                        &group.name,
                        (button.0 + 12.0 * s, button.1 + 3.0 * s, button.2 - 24.0 * s, button.1 + 21.0 * s),
                        s,
                        10.5,
                        false,
                        0.55,
                    )?;
                    label(
                        p,
                        value,
                        (button.0 + 12.0 * s, button.1 + 21.0 * s, button.2 - 24.0 * s, button.3 - 3.0 * s),
                        s,
                        13.0,
                        true,
                        0.95,
                    )?;
                    p.glyph(
                        '\u{E70D}',
                        (button.2 - 28.0 * s, button.1, button.2 - 4.0 * s, button.3),
                        10.0 * s,
                        WHITE,
                    )?;
                }
            }
            CurrentStation(artist) => {
                label(
                    p,
                    if artist { "Wave from artist" } else { "Wave from track" },
                    (button.0 + 12.0 * s, button.1 + 3.0 * s, button.2 - 8.0 * s, button.1 + 25.0 * s),
                    s,
                    12.0,
                    true,
                    1.0,
                )?;
                if let Some(track) = &ui.state.current_track {
                    label(
                        p,
                        if artist { &track.artist } else { &track.title },
                        (button.0 + 12.0 * s, button.1 + 25.0 * s, button.2 - 8.0 * s, button.3 - 2.0 * s),
                        s,
                        11.0,
                        false,
                        0.65,
                    )?;
                }
            }
            Autoplay => {
                let check = (button.0 + 10.0 * s, button.1 + 7.0 * s, button.0 + 26.0 * s, button.1 + 23.0 * s);
                p.stroke_round_rect(check, 4.0 * s, s, rgba(1.0, 1.0, 1.0, 0.5))?;
                if ui.state.config.autoplay {
                    p.glyph('\u{E73E}', check, 12.0 * s, WHITE)?;
                }
                label(
                    p,
                    "Start with Island",
                    (button.0 + 34.0 * s, button.1, button.2 - 8.0 * s, button.3),
                    s,
                    12.0,
                    false,
                    1.0,
                )?;
            }
            Volume => {
                let value = ui.volume_preview.unwrap_or(ui.state.config.volume);
                p.glyph('\u{E767}', (button.0, button.1, button.0 + 24.0 * s, button.3), 13.0 * s, WHITE)?;
                let (left, right, y) = (button.0 + 30.0 * s, button.2 - 32.0 * s, (button.1 + button.3) / 2.0);
                p.fill_round_rect((left, y - 2.0 * s, right, y + 2.0 * s), 2.0 * s, rgba(1.0, 1.0, 1.0, 0.2))?;
                let thumb = left + (right - left) * f32::from(value) / 100.0;
                p.fill_ellipse(thumb, y, 4.0 * s, 4.0 * s, WHITE)?;
                label(p, &value.to_string(), (right + 7.0 * s, button.1, button.2, button.3), s, 10.0, false, 0.7)?;
            }
            _ => {
                let text = match hit {
                    Retry => "Try again",
                    StartWave => {
                        if ui.state.busy {
                            "Loading…"
                        } else {
                            "Start My Wave"
                        }
                    }
                    Disconnect => "Sign out",
                    Overview => "Wave settings",
                    Search => "Search",
                    Library => "Liked tracks",
                    CurrentStation(false) | MenuTrackStation => "Wave from track",
                    CurrentStation(true) | MenuArtistStation => "Wave from artist",
                    MenuPlay => "Play track",
                    Autoplay => {
                        if ui.state.config.autoplay {
                            "✓ Start with Island"
                        } else {
                            "Start with Island"
                        }
                    }
                    GroupPick(group, option) => option
                        .and_then(|i| ui.state.groups.get(group)?.items.get(i).map(|v| v.0.as_str()))
                        .unwrap_or("Any"),
                    _ => "",
                };
                label(p, text, (button.0 + 12.0 * s, button.1, button.2 - 8.0 * s, button.3), s, 12.0, true, 1.0)?;
            }
        }
    }
    if !ui.state.error.is_empty() {
        label(p, &ui.state.error, r(24.0, 344.0, w - 124.0, 374.0), s, 11.0, false, 0.8)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn filter_menu_pages_retain_exact_option_indices_and_fit_the_island() {
        let mut ui = YandexUi::default();
        ui.state.groups.push(crate::services::yandex::Group {
            key: "mood".into(),
            name: "Mood".into(),
            items: (0..23).map(|i| (format!("Mood {i}"), format!("mood:{i}"))).collect(),
        });
        ui.open_group = Some(0);
        for start in [0, 9, 14] {
            ui.group_scroll = start;
            for scale in [0.75, 1.0, 1.5, 2.0] {
                let buttons = controls((0.0, 0.0, 640.0 * scale, 380.0 * scale), scale, &ui);
                let options: Vec<_> = buttons
                    .iter()
                    .filter_map(|(_, h)| if let YandexHit::GroupPick(0, Some(index)) = h { Some(*index) } else { None })
                    .collect();
                assert_eq!(options, (start..(start + 9).min(23)).collect::<Vec<_>>());
                assert!(
                    buttons
                        .iter()
                        .all(|(r, _)| r.0 >= 0.0 && r.1 >= 0.0 && r.2 <= 640.0 * scale && r.3 <= 380.0 * scale)
                );
            }
        }
    }
}
