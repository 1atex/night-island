use crate::view::*;

#[derive(Default)]
pub struct CityUi {
    pub input: TextEdit,
    pub matches: Vec<crate::services::weather::City>,
}

pub fn weather_icon(p: &Painter, cx: f32, cy: f32, r: f32, sky: Sky, night: bool) -> Result<()> {
    macro_rules! icon {
        ($name:literal, $b:expr) => {
            (&include_bytes!(concat!("../../assets/weather/src/", $name, ".svg"))[..], $b)
        };
    }
    let (data, bounds): (&[u8], [f32; 4]) = match sky {
        Sky::Clear if night => icon!("clear-night", [15.6, 15.2, 48.8, 48.8]),
        Sky::Clear => icon!("clear-day", [8.0, 8.0, 56.0, 56.0]),
        Sky::Partly if night => icon!("partly-cloudy-night", [9.4, 13.2, 53.8, 45.8]),
        Sky::Partly => icon!("partly-cloudy-day", [6.4, 11.5, 53.8, 45.8]),
        Sky::Cloudy => icon!("overcast", [10.3, 18.2, 59.5, 45.8]),
        Sky::Fog => icon!("fog", [10.3, 18.2, 53.8, 59.5]),
        Sky::Rain => icon!("rain", [10.3, 18.2, 53.8, 49.0]),
        Sky::Sleet => icon!("sleet", [10.3, 18.2, 53.8, 48.0]),
        Sky::Snow => icon!("snow", [10.3, 18.2, 53.8, 48.0]),
        Sky::Thunder => icon!("thunderstorms-rain", [10.3, 18.2, 53.8, 59.0]),
    };
    p.svg(data, bounds, (cx - 1.25 * r, cy - r, cx + 1.25 * r, cy + r))
}

pub(in crate::view) const WEEKDAYS_EN: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

pub(in crate::view) const WEEKDAYS_RU: [&str; 7] = ["Пн", "Вт", "Ср", "Чт", "Пт", "Сб", "Вс"];

pub(in crate::view) fn weekday_name(i: u32) -> &'static str {
    let table = if crate::ui::i18n::is_ru() { &WEEKDAYS_RU } else { &WEEKDAYS_EN };
    table[(i % 7) as usize]
}

pub fn city_field(pill: Rect, scale: f32) -> (Rect, Rect) {
    let (x, y, _, _) = pill;
    let left = x + (SIDE_PAD - 4.0) * scale;
    let card_right = x + (SIDE_PAD - 4.0) * scale + ((pill.2 - 2.0 * (SIDE_PAD - 4.0) * scale) * 0.5 - 6.0 * scale);
    let top = y + (HOME_TOP + 118.0) * scale;
    let frame = (left + 10.0 * scale, top, card_right - 8.0 * scale, top + 28.0 * scale);
    (frame, (frame.0 + 10.0 * scale, frame.1, frame.2 - 8.0 * scale, frame.3))
}

pub(in crate::view) fn city_frame(pill: Rect, scale: f32, has_weather: bool) -> (Rect, Rect) {
    if has_weather {
        return city_field(pill, scale);
    }
    let (x, y, w, _) = pill;
    let mid = x + w / 2.0;
    let frame = (mid - 160.0 * scale, y + 300.0 * scale, mid + 160.0 * scale, y + 330.0 * scale);
    (frame, (frame.0 + 10.0 * scale, frame.1, frame.2 - 8.0 * scale, frame.3))
}

pub fn hit_city(pill: Rect, scale: f32, px: f32, py: f32, has_weather: bool) -> bool {
    inside(city_frame(pill, scale, has_weather).0, px, py)
}

pub(in crate::view) fn city_menu_rect(pill: Rect, scale: f32, count: usize, has_weather: bool) -> Rect {
    let (frame, _) = city_frame(pill, scale, has_weather);
    let h = (count as f32 * 30.0 + 12.0) * scale;
    let top = if has_weather { frame.3 + 6.0 * scale } else { frame.1 - h - 6.0 * scale };
    (frame.0, top, frame.0 + 300.0 * scale, top + h)
}

pub fn hit_city_match(pill: Rect, scale: f32, px: f32, py: f32, count: usize, has_weather: bool) -> Option<usize> {
    let menu = city_menu_rect(pill, scale, count, has_weather);
    (0..count).find(|&i| {
        let top = menu.1 + (6.0 + i as f32 * 30.0) * scale;
        inside((menu.0 + 6.0 * scale, top, menu.2 - 6.0 * scale, top + 30.0 * scale), px, py)
    })
}

pub(in crate::view) fn paint_city_matches(p: &Painter, pill: Rect, scale: f32, ui: &UiState) -> Result<()> {
    if !(ui.editing && ui.tab == Tab::Weather) || ui.city.matches.is_empty() {
        return Ok(());
    }
    let has_weather = ui.weather.is_some();
    let menu = city_menu_rect(pill, scale, ui.city.matches.len(), has_weather);
    p.fill_round_rect(menu, 14.0 * scale, rgba(0.07, 0.07, 0.09, 0.98))?;
    p.stroke_round_rect(menu, 14.0 * scale, 1.0 * scale, rgba(1.0, 1.0, 1.0, 0.14))?;
    for (i, city) in ui.city.matches.iter().enumerate() {
        let top = menu.1 + (6.0 + i as f32 * 30.0) * scale;
        let row = (menu.0 + 6.0 * scale, top, menu.2 - 6.0 * scale, top + 30.0 * scale);
        if ui.home.hover_act == Some(910 + i) {
            p.fill_round_rect(row, 9.0 * scale, rgba(1.0, 1.0, 1.0, 0.1))?;
        }
        p.glyph(
            '\u{E707}',
            (row.0 + 2.0 * scale, row.1, row.0 + 26.0 * scale, row.3),
            12.0 * scale,
            rgba(1.0, 1.0, 1.0, 0.6),
        )?;
        p.text(
            &city.label,
            (row.0 + 28.0 * scale, row.1, row.2 - 6.0 * scale, row.3),
            12.5 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            WHITE,
        )?;
    }
    Ok(())
}

pub(in crate::view) fn paint_city(
    p: &Painter,
    pill: Rect,
    scale: f32,
    ui: &UiState,
    place: &str,
    missing: bool,
) -> Result<()> {
    let (frame, inner) = city_field(pill, scale);
    if ui.editing && ui.tab == Tab::Weather {
        return paint_line_edit(
            p,
            frame,
            inner,
            &ui.city.input,
            false,
            "City (empty: by IP address)",
            true,
            ui.caret_on,
            scale,
        );
    }
    let hover = ui.home.hover_act == Some(900);
    if hover {
        p.fill_round_rect(frame, 10.0 * scale, rgba(1.0, 1.0, 1.0, 0.1))?;
    }
    let text = if place.is_empty() { "Choose a city" } else { place };
    p.text(
        text,
        (frame.0 + 6.0 * scale, frame.1, frame.2 - 28.0 * scale, frame.3),
        12.5 * scale,
        DWRITE_FONT_WEIGHT_NORMAL,
        DWRITE_TEXT_ALIGNMENT_LEADING,
        rgba(1.0, 1.0, 1.0, if hover { 0.9 } else { DIM }),
    )?;
    p.glyph(
        '\u{E70F}',
        (frame.2 - 26.0 * scale, frame.1, frame.2 - 4.0 * scale, frame.3),
        12.0 * scale,
        rgba(1.0, 1.0, 1.0, if hover { 0.9 } else { 0.5 }),
    )?;
    if missing {
        p.text(
            "City not found",
            (frame.0 + 6.0 * scale, frame.1 - 14.0 * scale, frame.2, frame.1 + 2.0 * scale),
            10.5 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            rgba(1.0, 0.5, 0.45, 1.0),
        )?;
    }
    Ok(())
}

pub(in crate::view) fn paint_weather(p: &Painter, pill: Rect, scale: f32, ui: &UiState) -> Result<()> {
    let (x, y, w, _) = pill;
    let dim = rgba(1.0, 1.0, 1.0, DIM);
    let Some(wx) = &ui.weather else {
        let msg = if ui.cfg.weather { "Locating…" } else { "Weather is off" };
        p.text(
            msg,
            (x, y + 60.0 * scale, x + w, y + 300.0 * scale),
            15.0 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_CENTER,
            rgba(1.0, 1.0, 1.0, 0.5),
        )?;
        if ui.cfg.weather {
            let (wide, inner) = city_frame(pill, scale, false);
            paint_line_edit(
                p,
                wide,
                inner,
                &ui.city.input,
                false,
                "City (empty: by IP address)",
                ui.editing && ui.tab == Tab::Weather,
                ui.caret_on,
                scale,
            )?;
        }
        return paint_city_matches(p, pill, scale, ui);
    };
    let pad = (SIDE_PAD - 4.0) * scale;
    let (left, right) = (x + pad, x + w - pad);
    let top = y + HOME_TOP * scale;
    let split = left + (right - left) * 0.5;

    let now = (left, top, split - 6.0 * scale, top + 150.0 * scale);
    p.fill_round_rect(now, 20.0 * scale, rgba(1.0, 1.0, 1.0, 0.07))?;
    weather_icon(p, now.0 + 56.0 * scale, now.1 + 62.0 * scale, 40.0 * scale, wx.sky, wx.night)?;
    p.text(
        &format!("{}°", wx.temp.round() as i32),
        (now.0 + 108.0 * scale, now.1 + 14.0 * scale, now.2 - 8.0 * scale, now.1 + 88.0 * scale),
        52.0 * scale,
        DWRITE_FONT_WEIGHT_BOLD,
        DWRITE_TEXT_ALIGNMENT_LEADING,
        WHITE,
    )?;
    p.text(
        wx.sky.name(),
        (now.0 + 16.0 * scale, now.1 + 100.0 * scale, now.2 - 10.0 * scale, now.1 + 124.0 * scale),
        15.0 * scale,
        DWRITE_FONT_WEIGHT_SEMI_BOLD,
        DWRITE_TEXT_ALIGNMENT_LEADING,
        WHITE,
    )?;
    paint_city(p, pill, scale, ui, &wx.place, wx.city_missing)?;

    let today = wx.daily.first();
    let tiles = [
        ("Wind", format!("{:.1} {}", wx.wind, if crate::ui::i18n::is_ru() { "м/с" } else { "m/s" })),
        ("Humidity", format!("{}%", wx.humidity.round() as i32)),
        ("Max", today.map_or("–".into(), |d| format!("{}°", d.max.round() as i32))),
        ("Min", today.map_or("–".into(), |d| format!("{}°", d.min.round() as i32))),
    ];
    let (tw, th) = ((right - (split + 6.0 * scale) - 8.0 * scale) / 2.0, 71.0 * scale);
    for (i, (label, value)) in tiles.iter().enumerate() {
        let (col, row) = ((i % 2) as f32, (i / 2) as f32);
        let l = split + 6.0 * scale + col * (tw + 8.0 * scale);
        let t = top + row * (th + 8.0 * scale);
        p.fill_round_rect((l, t, l + tw, t + th), 16.0 * scale, rgba(1.0, 1.0, 1.0, 0.07))?;
        p.text(
            label,
            (l + 14.0 * scale, t + 10.0 * scale, l + tw - 6.0 * scale, t + 30.0 * scale),
            12.0 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            dim,
        )?;
        p.text(
            value,
            (l + 14.0 * scale, t + 30.0 * scale, l + tw - 6.0 * scale, t + 62.0 * scale),
            20.0 * scale,
            DWRITE_FONT_WEIGHT_BOLD,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            WHITE,
        )?;
    }

    let strip = (left, top + 160.0 * scale, right, top + 160.0 * scale + 86.0 * scale);
    p.fill_round_rect(strip, 18.0 * scale, rgba(1.0, 1.0, 1.0, 0.07))?;
    let n = wx.hourly.len().max(1) as f32;
    let cell = (strip.2 - strip.0) / n;
    for (i, h) in wx.hourly.iter().enumerate() {
        let cx = strip.0 + (i as f32 + 0.5) * cell;
        p.text(
            &format!("{:02}:00", h.hour),
            (cx - cell / 2.0, strip.1 + 6.0 * scale, cx + cell / 2.0, strip.1 + 26.0 * scale),
            11.5 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_CENTER,
            dim,
        )?;
        weather_icon(p, cx, strip.1 + 44.0 * scale, 13.0 * scale, h.sky, h.night)?;
        p.text(
            &format!("{}°", h.temp.round() as i32),
            (cx - cell / 2.0, strip.1 + 60.0 * scale, cx + cell / 2.0, strip.3 - 4.0 * scale),
            13.5 * scale,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_CENTER,
            WHITE,
        )?;
    }

    let row_top = strip.3 + 8.0 * scale;
    let days = &wx.daily;
    let dn = days.len().max(1) as f32;
    let dw = ((right - left) - (dn - 1.0) * 8.0 * scale) / dn;
    for (i, d) in days.iter().enumerate() {
        let l = left + i as f32 * (dw + 8.0 * scale);
        let r = (l, row_top, l + dw, row_top + 60.0 * scale);
        p.fill_round_rect(r, 16.0 * scale, rgba(1.0, 1.0, 1.0, 0.07))?;
        let name = if i == 0 { crate::ui::i18n::tr("Today").into_owned() } else { weekday_name(d.weekday).to_string() };
        p.text(
            &name,
            (l + 12.0 * scale, r.1 + 6.0 * scale, r.2 - 40.0 * scale, r.1 + 30.0 * scale),
            13.5 * scale,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            WHITE,
        )?;
        p.text(
            &format!("{}° / {}°", d.max.round() as i32, d.min.round() as i32),
            (l + 12.0 * scale, r.1 + 30.0 * scale, r.2 - 36.0 * scale, r.3 - 4.0 * scale),
            12.0 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            dim,
        )?;
        weather_icon(p, r.2 - 24.0 * scale, (r.1 + r.3) / 2.0, 12.0 * scale, d.sky, false)?;
    }
    paint_city_matches(p, pill, scale, ui)
}
