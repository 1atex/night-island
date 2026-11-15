use crate::data::settings::VpnProvider;
use crate::view::*;

fn caption(p: &Painter, text: &str, r: Rect, s: f32, strong: bool, alpha: f32) -> Result<()> {
    p.text(
        text,
        r,
        if strong { 11.0 * s } else { 9.5 * s },
        if strong { DWRITE_FONT_WEIGHT_SEMI_BOLD } else { DWRITE_FONT_WEIGHT_NORMAL },
        DWRITE_TEXT_ALIGNMENT_LEADING,
        rgba(1.0, 1.0, 1.0, alpha),
    )
}

pub fn vpn_quick_rect(r: Rect, s: f32) -> Rect {
    (r.2 - 40.0 * s, r.1 + 6.0 * s, r.2 - 8.0 * s, r.1 + 38.0 * s)
}

pub fn vpn_favorite_rects(r: Rect, s: f32, ui: &UiState) -> Vec<(Rect, Option<usize>)> {
    let (w, h) = ((r.2 - r.0) / s, (r.3 - r.1) / s);
    if !(w > 150.0 && h > 80.0 || h > 160.0) || ui.svoi.access != "allowed" {
        return vec![];
    }
    let count = ui.svoi.favorites.len();
    let cells = count.min(4);
    let more = count > 4;
    (0..cells)
        .map(|i| {
            let (col, row, columns) = if w > 150.0 { (i % 2, i / 2, 2.0) } else { (0, i, 1.0) };
            let width = (r.2 - r.0 - 20.0 * s - (columns - 1.0) * 4.0 * s) / columns;
            let x = r.0 + 10.0 * s + col as f32 * (width + 4.0 * s);
            let y = r.1 + (if w > 150.0 { 45.0 } else { 65.0 } + row as f32 * 23.0) * s;
            (
                (x, y, x + width, y + 20.0 * s),
                if more && i == cells - 1 { None } else { Some((ui.vpn_favorite_offset + i) % count.max(1)) },
            )
        })
        .collect()
}

pub fn hit_vpn_widget(r: Rect, s: f32, ui: &UiState, x: f32, y: f32) -> Option<HomeHit> {
    if inside(vpn_quick_rect(r, s), x, y) {
        return Some(HomeHit::WidgetCommand(WidgetCommand::Vpn));
    }
    if !ui.vpn_busy {
        for (rect, index) in vpn_favorite_rects(r, s, ui) {
            if inside(rect, x, y) {
                return Some(index.map_or(HomeHit::VpnMore, HomeHit::VpnFavorite));
            }
        }
    }
    if r.3 - r.1 < 60.0 * s && r.2 - r.0 < 150.0 * s { Some(HomeHit::WidgetCommand(WidgetCommand::Vpn)) } else { None }
}

pub fn paint_vpn_widget(p: &Painter, r: Rect, s: f32, ui: &UiState) -> Result<()> {
    let off = ui.cfg.vpn_provider == VpnProvider::Off;
    let state = &ui.svoi;
    let (w, h) = ((r.2 - r.0) / s, (r.3 - r.1) / s);
    let quick = vpn_quick_rect(r, s);
    let x = r.0 + 10.0 * s;
    let end = quick.0 - 4.0 * s;
    let compact = w < 150.0 && h < 60.0;
    let title = if off {
        "VPN".to_owned()
    } else if !compact {
        if let Some(active) = &state.active { active.name.clone() } else { "SVOI".to_owned() }
    } else {
        "SVOI".to_owned()
    };
    caption(p, &title, (x, r.1 + 3.0 * s, end, r.1 + 23.0 * s), s, true, 1.0)?;
    let status = if off {
        "Off"
    } else if ui.vpn_busy {
        "Connecting…"
    } else {
        state.status()
    };
    let mode = match state.mode.as_str() {
        "tun" => "TUN",
        "system" => "Proxy",
        "local" => "Local proxy",
        _ => "",
    };
    let detail = if compact {
        if off {
            crate::ui::i18n::tr("Off").into_owned()
        } else if ui.vpn_busy {
            crate::ui::i18n::tr("Connecting…").into_owned()
        } else {
            crate::ui::i18n::tr(match state.access.as_str() {
                "closed" => "Closed",
                "denied" => "No access",
                _ => {
                    if state.verified {
                        "Connected"
                    } else if state.connected() == Some(true) {
                        "Check connection"
                    } else {
                        "Off"
                    }
                }
            })
            .into_owned()
        }
    } else if state.access == "allowed" {
        let mut text = format!("{}{}{}", mode, if mode.is_empty() { "" } else { " · " }, crate::ui::i18n::tr(status));
        if let Some(latency) = state.latency.filter(|_| state.verified) {
            text.push_str(&format!(" · {latency} ms"));
        }
        text
    } else {
        crate::ui::i18n::tr(status).into_owned()
    };
    caption(p, &detail, (x, r.1 + 23.0 * s, end, r.1 + 43.0 * s), s, false, 0.65)?;
    p.fill_round_rect(quick, 8.0 * s, acc(if ui.vpn_busy { 0.1 } else { 0.22 }))?;
    p.glyph(
        if ui.vpn_busy {
            '\u{E712}'
        } else if state.access != "allowed" {
            '\u{E8A7}'
        } else {
            '\u{E7E8}'
        },
        quick,
        18.0 * s,
        acc(0.95),
    )?;
    let favorites = vpn_favorite_rects(r, s, ui);
    if (w > 150.0 && h > 80.0 || h > 160.0) && favorites.is_empty() && !off {
        caption(
            p,
            if state.access == "allowed" { "Star servers in SVOI" } else { "Open SVOI to enable the integration" },
            (x, r.1 + 49.0 * s, r.2 - 8.0 * s, r.1 + 83.0 * s),
            s,
            false,
            0.55,
        )?;
    }
    for (rect, index) in favorites {
        let selected = index.is_some_and(|i| state.active.as_ref().is_some_and(|a| a.id == state.favorites[i].id));
        p.fill_round_rect(rect, 6.0 * s, if selected { acc(0.24) } else { rgba(1.0, 1.0, 1.0, 0.065) })?;
        let text = index.map_or_else(
            || format!("{} · {}", crate::ui::i18n::tr("More"), state.favorites.len()),
            |i| state.favorites[i].name.clone(),
        );
        caption(
            p,
            &text,
            (rect.0 + 6.0 * s, rect.1, rect.2 - 4.0 * s, rect.3),
            s,
            false,
            if ui.vpn_busy { 0.4 } else { 0.9 },
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_widget_sizes_have_safe_controls_and_favorites_page() {
        let mut ui = UiState::new();
        ui.cfg.vpn_provider = VpnProvider::Svoi;
        ui.svoi=serde_json::from_value(serde_json::json!({"access":"allowed","state":"stopped","favorites":(0..8).map(|i|serde_json::json!({"id":i.to_string(),"name":"Server"})).collect::<Vec<_>>()})).unwrap();
        for scale in [0.8, 1.0, 1.5, 2.0] {
            for (width, height, has_favorites) in [
                (124.0, 46.0, false),
                (258.0, 46.0, false),
                (124.0, 100.0, false),
                (258.0, 100.0, true),
                (124.0, 208.0, true),
            ] {
                let r = (0.0, 0.0, width * scale, height * scale);
                let quick = vpn_quick_rect(r, scale);
                assert!(quick.0 >= r.0 && quick.2 <= r.2 && quick.3 <= r.3);
                assert!(matches!(
                    hit_vpn_widget(r, scale, &ui, (quick.0 + quick.2) / 2.0, (quick.1 + quick.3) / 2.0),
                    Some(HomeHit::WidgetCommand(WidgetCommand::Vpn))
                ));
                let favorites = vpn_favorite_rects(r, scale, &ui);
                assert_eq!(!favorites.is_empty(), has_favorites);
                for (rect, index) in favorites {
                    assert!(rect.0 >= r.0 && rect.2 <= r.2 && rect.3 <= r.3);
                    assert!(rect.3 <= quick.1 || rect.1 >= quick.3);
                    let hit = hit_vpn_widget(r, scale, &ui, (rect.0 + rect.2) / 2.0, (rect.1 + rect.3) / 2.0);
                    assert!(match (index, hit) {
                        (Some(i), Some(HomeHit::VpnFavorite(j))) => i == j,
                        (None, Some(HomeHit::VpnMore)) => true,
                        _ => false,
                    });
                }
            }
        }
        ui.svoi.access = "denied".into();
        assert!(vpn_favorite_rects((0.0, 0.0, 258.0, 100.0), 1.0, &ui).is_empty());
    }
}
