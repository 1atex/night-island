use crate::services::yandex::Snapshot;
use crate::view::*;

#[derive(Default)]
pub struct YandexUi {
    pub state: Snapshot,
    pub hover: Option<YandexHit>,
    pub copied: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum YandexHit {
    Connect,
    Browser,
    Copy,
    Cancel,
    Toggle,
    Previous,
    Next,
    Stop,
    Disconnect,
    Retry,
    Autoplay,
    Filter(usize, i32),
}

pub fn yandex_controls(pill: Rect, s: f32, ui: &YandexUi) -> Vec<(Rect, YandexHit)> {
    let (x, y, w, _) = pill;
    let r = |a: f32, b: f32, c: f32, d: f32| (x + a * s, y + b * s, x + c * s, y + d * s);
    let width = w / s;
    if ui.state.account.is_empty() {
        if ui.state.code.is_empty() {
            vec![(
                r(32.0, 240.0, width - 32.0, 282.0),
                if ui.state.has_token { YandexHit::Retry } else { YandexHit::Connect },
            )]
        } else {
            vec![
                (r(32.0, 238.0, width * 0.5 - 5.0, 280.0), YandexHit::Browser),
                (r(width * 0.5 + 5.0, 238.0, width - 32.0, 280.0), YandexHit::Copy),
                (r(width - 106.0, 62.0, width - 24.0, 90.0), YandexHit::Cancel),
            ]
        }
    } else {
        let mut controls = vec![
            (r(24.0, 248.0, 65.0, 290.0), YandexHit::Previous),
            (r(73.0, 248.0, 171.0, 290.0), YandexHit::Toggle),
            (r(179.0, 248.0, 220.0, 290.0), YandexHit::Next),
            (r(24.0, 310.0, 320.0, 344.0), YandexHit::Autoplay),
            (r(width - 106.0, 62.0, width - 24.0, 90.0), YandexHit::Disconnect),
        ];
        if ui.state.active {
            controls.push((r(width - 132.0, 310.0, width - 24.0, 344.0), YandexHit::Stop));
        }
        if !ui.state.error.is_empty() {
            controls.push((r(width - 116.0, 350.0, width - 24.0, 376.0), YandexHit::Retry));
        }
        for (i, _) in ui.state.groups.iter().take(4).enumerate() {
            let top = 107.0 + i as f32 * 47.0;
            controls.push((r(244.0, top, width - 57.0, top + 40.0), YandexHit::Filter(i, -1)));
            controls.push((r(width - 55.0, top, width - 24.0, top + 40.0), YandexHit::Filter(i, 1)));
        }
        controls
    }
}

pub fn hit_yandex(pill: Rect, scale: f32, ui: &YandexUi, x: f32, y: f32) -> Option<YandexHit> {
    if ui.state.busy {
        return None;
    }
    yandex_controls(pill, scale, ui).into_iter().find(|(r, _)| inside(*r, x, y)).map(|(_, h)| h)
}

pub fn paint_yandex(p: &Painter, pill: Rect, s: f32, ui: &YandexUi) -> Result<()> {
    let (x, y, w, _) = pill;
    let width = w / s;
    let r = |a: f32, b: f32, c: f32, d: f32| (x + a * s, y + b * s, x + c * s, y + d * s);
    let text = |value: &str, rect: Rect, size: f32, alpha: f32| {
        p.text(
            value,
            rect,
            size * s,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            rgba(1.0, 1.0, 1.0, alpha),
        )
    };
    p.text(
        "My Wave",
        r(24.0, 62.0, width - 130.0, 90.0),
        20.0 * s,
        DWRITE_FONT_WEIGHT_SEMI_BOLD,
        DWRITE_TEXT_ALIGNMENT_LEADING,
        WHITE,
    )?;
    if ui.state.account.is_empty() {
        p.fill_round_rect(r(24.0, 104.0, width - 24.0, 224.0), 18.0 * s, acc(0.12))?;
        if ui.state.code.is_empty() {
            p.glyph('\u{E189}', r(40.0, 124.0, 98.0, 184.0), 34.0 * s, acc(1.0))?;
            text("Yandex Music inside your island", r(114.0, 121.0, width - 44.0, 151.0), 16.0, 1.0)?;
            p.text_wrapped_trimmed(
                &crate::ui::i18n::tr("Sign in in your browser. Your password stays with Yandex."),
                r(114.0, 157.0, width - 44.0, 207.0),
                13.0 * s,
                DWRITE_FONT_WEIGHT_NORMAL,
                rgba(1.0, 1.0, 1.0, 0.7),
            )?;
        } else {
            text("Enter this code at ya.ru/device", r(44.0, 117.0, width - 44.0, 143.0), 13.0, 0.75)?;
            p.text(
                &ui.state.code,
                r(44.0, 147.0, width - 44.0, 192.0),
                30.0 * s,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_CENTER,
                WHITE,
            )?;
            text("Waiting for confirmation in your browser…", r(44.0, 192.0, width - 44.0, 218.0), 12.0, 0.7)?;
        }
        p.text_wrapped_trimmed(
            &crate::ui::i18n::tr("Requires Yandex Music access. The connection uses an unofficial API."),
            r(32.0, 294.0, width - 32.0, 334.0),
            12.0 * s,
            DWRITE_FONT_WEIGHT_NORMAL,
            rgba(1.0, 1.0, 1.0, 0.6),
        )?;
    } else {
        p.fill_round_rect(r(24.0, 104.0, 220.0, 236.0), 18.0 * s, acc(0.14))?;
        let cover = r(36.0, 116.0, 92.0, 172.0);
        if let Some(image) = &ui.state.media.cover {
            p.image(cover, 12.0 * s, image.size, &image.bgra)?;
        } else {
            p.fill_round_rect(cover, 12.0 * s, acc(0.22))?;
            p.glyph('\u{E189}', cover, 27.0 * s, acc(1.0))?;
        }
        text("Yandex Music", r(103.0, 119.0, 208.0, 142.0), 11.5, 0.65)?;
        text(&ui.state.account, r(103.0, 145.0, 208.0, 169.0), 12.0, 0.9)?;
        text(
            if ui.state.media.title.is_empty() { "Made for you" } else { &ui.state.media.title },
            r(36.0, 181.0, 208.0, 207.0),
            15.0,
            1.0,
        )?;
        text(
            if ui.state.media.artist.is_empty() { "Music for this moment" } else { &ui.state.media.artist },
            r(36.0, 208.0, 208.0, 230.0),
            11.5,
            0.65,
        )?;
        if ui.state.groups.is_empty() {
            p.text_wrapped_trimmed(
                &crate::ui::i18n::tr("Wave settings are unavailable. Press Try again to reload."),
                r(244.0, 124.0, width - 24.0, 220.0),
                13.0 * s,
                DWRITE_FONT_WEIGHT_NORMAL,
                rgba(1.0, 1.0, 1.0, 0.7),
            )?;
        }
    }
    for (rect, hit) in yandex_controls(pill, s, ui) {
        let hover = !ui.state.busy && ui.hover == Some(hit);
        let primary = matches!(hit, YandexHit::Connect | YandexHit::Toggle | YandexHit::Browser);
        p.fill_round_rect(
            rect,
            11.0 * s,
            if primary {
                acc(if hover { 0.4 } else { 0.28 })
            } else {
                rgba(1.0, 1.0, 1.0, if hover { 0.12 } else { 0.06 })
            },
        )?;
        let label = match hit {
            YandexHit::Connect => {
                if ui.state.busy {
                    "Connecting…"
                } else {
                    "Connect Yandex Music"
                }
            }
            YandexHit::Browser => "Open Yandex",
            YandexHit::Copy => {
                if ui.copied {
                    "Copied"
                } else {
                    "Copy code"
                }
            }
            YandexHit::Cancel => "Cancel",
            YandexHit::Toggle => {
                if ui.state.busy {
                    "Loading music…"
                } else if ui.state.media.playing {
                    "Pause"
                } else if ui.state.active {
                    "Resume"
                } else {
                    "Start wave"
                }
            }
            YandexHit::Disconnect => "Sign out",
            YandexHit::Stop => "Stop wave",
            YandexHit::Retry => {
                if ui.state.busy {
                    "Connecting…"
                } else {
                    "Try again"
                }
            }
            YandexHit::Autoplay => {
                p.glyph(
                    if ui.state.config.autoplay { '\u{E73E}' } else { '\u{E739}' },
                    (rect.0 + 8.0 * s, rect.1, rect.0 + 36.0 * s, rect.3),
                    16.0 * s,
                    if ui.state.config.autoplay { acc(1.0) } else { WHITE },
                )?;
                text(
                    "Start wave when Island starts",
                    (rect.0 + 42.0 * s, rect.1, rect.2 - 8.0 * s, rect.3),
                    11.5,
                    0.8,
                )?;
                continue;
            }
            YandexHit::Previous | YandexHit::Next => {
                p.glyph(if hit == YandexHit::Previous { '\u{E892}' } else { '\u{E893}' }, rect, 18.0 * s, WHITE)?;
                continue;
            }
            YandexHit::Filter(i, dir) => {
                if dir == 1 {
                    p.glyph('\u{E76C}', rect, 10.0 * s, WHITE)?;
                } else {
                    let group = &ui.state.groups[i];
                    let chosen = ui
                        .state
                        .config
                        .selection
                        .get(&group.key)
                        .and_then(|seed| group.items.iter().find(|i| &i.1 == seed))
                        .map_or("Any", |i| i.0.as_str());
                    p.glyph(
                        '\u{E76B}',
                        (rect.0, rect.1, rect.0 + 28.0 * s, rect.3),
                        10.0 * s,
                        rgba(1.0, 1.0, 1.0, 0.5),
                    )?;
                    text(
                        &group.name,
                        (rect.0 + 32.0 * s, rect.1 + 2.0 * s, rect.2 - 8.0 * s, rect.1 + 17.0 * s),
                        10.5,
                        0.6,
                    )?;
                    text(
                        chosen,
                        (rect.0 + 32.0 * s, rect.1 + 16.0 * s, rect.2 - 8.0 * s, rect.3 - 2.0 * s),
                        12.5,
                        1.0,
                    )?;
                }
                continue;
            }
        };
        p.text(label, rect, 12.0 * s, DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_TEXT_ALIGNMENT_CENTER, WHITE)?;
    }
    if !ui.state.error.is_empty() {
        p.text_wrapped_trimmed(
            &crate::ui::i18n::tr(&ui.state.error),
            r(24.0, 348.0, width - 124.0, 380.0),
            11.0 * s,
            DWRITE_FONT_WEIGHT_NORMAL,
            rgba(1.0, 0.65, 0.55, 1.0),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn busy_wave_blocks_duplicate_actions_and_code_has_copy_action() {
        let mut ui = YandexUi::default();
        let pill = (0.0, 0.0, 640.0, 384.0);
        ui.state.busy = true;
        assert!(hit_yandex(pill, 1.0, &ui, 100.0, 260.0).is_none());
        ui.state.busy = false;
        ui.state.code = "123456".into();
        assert!(yandex_controls(pill, 1.0, &ui).iter().any(|(_, h)| *h == YandexHit::Copy));
    }
}
