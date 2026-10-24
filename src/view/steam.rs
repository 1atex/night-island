use std::time::Instant;

use crate::platform::steam::{Account, Status, Update};
use crate::platform::steam_art::{self, ArtKey};
use crate::view::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LibGame {
    pub id: u32,
    pub name: String,
    pub last: i64,
    pub minutes: u32,
    pub update: Update,
}

#[derive(Default)]
pub struct SteamUi {
    pub games: Vec<LibGame>,
    pub all: Vec<LibGame>,
    pub accounts: Vec<Account>,
    pub active: Option<u32>,
    pub status: Option<Status>,
    pub running: Option<(u32, String, Instant)>,
    pub steam_up: bool,
    pub scroll: usize,
    pub armed: Option<String>,
    pub switching: Option<String>,
    pub query: TextEdit,
}

impl SteamUi {
    pub fn updates(&self) -> usize {
        self.all.iter().filter(|g| g.update != Update::None).count()
    }
}

pub const STEAM_LINKS: [(&str, &str); 4] = [
    ("Library", "steam://open/games"),
    ("Store", "steam://store"),
    ("Downloads", "steam://open/downloads"),
    ("Friends", "steam://open/friends"),
];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SteamHit {
    Link(usize),
    Account(usize),
    Game(usize),
    Update(usize),
    Status(usize),
    Search,
    Start,
}

const ROW_H: f32 = 32.0;
const ROW_GAP: f32 = 3.0;
const ACCOUNTS_TOP: f32 = 94.0;
const ACCOUNT_H: f32 = 28.0;
const TOOLS_TOP: f32 = 128.0;
const TOOLS_H: f32 = 28.0;
const LIST_TOP: f32 = 164.0;
const STATUS_W: f32 = 82.0;
const STATUS_GAP: f32 = 3.0;
const UPDATE_W: f32 = 80.0;
const DETAIL_W: f32 = 150.0;
const MAX_ACCOUNTS: usize = 5;
pub const STEAM_SEARCH_FONT: f32 = 13.0;

fn side(pill: Rect, scale: f32) -> (f32, f32) {
    (pill.0 + (SIDE_PAD - 4.0) * scale, pill.0 + pill.2 - (SIDE_PAD - 4.0) * scale)
}

pub fn steam_rows(pill: Rect, scale: f32) -> usize {
    let avail = pill.3 / scale - LIST_TOP - 10.0;
    (((avail + ROW_GAP) / (ROW_H + ROW_GAP)).floor().max(1.0)) as usize
}

fn link_rect(pill: Rect, scale: f32, i: usize) -> Rect {
    let (_, r) = side(pill, scale);
    let n = STEAM_LINKS.len();
    let width = 74.0 * scale;
    let gap = 6.0 * scale;
    let left = r - (n - i) as f32 * width - (n - 1 - i) as f32 * gap;
    (left, pill.1 + 58.0 * scale, left + width, pill.1 + 84.0 * scale)
}

fn account_rect(pill: Rect, scale: f32, i: usize, count: usize) -> Rect {
    let (l, r) = side(pill, scale);
    let n = count.clamp(1, MAX_ACCOUNTS) as f32;
    let gap = 6.0 * scale;
    let width = ((r - l - (n - 1.0) * gap) / n).min(160.0 * scale);
    let left = l + i as f32 * (width + gap);
    (left, pill.1 + ACCOUNTS_TOP * scale, left + width, pill.1 + (ACCOUNTS_TOP + ACCOUNT_H) * scale)
}

fn status_rect(pill: Rect, scale: f32, k: usize) -> Rect {
    let (_, r) = side(pill, scale);
    let n = Status::ALL.len();
    let (width, gap) = (STATUS_W * scale, STATUS_GAP * scale);
    let left = r - (n - k) as f32 * width - (n - 1 - k) as f32 * gap;
    (left, pill.1 + TOOLS_TOP * scale, left + width, pill.1 + (TOOLS_TOP + TOOLS_H) * scale)
}

fn status_block(pill: Rect, scale: f32) -> Rect {
    let (first, last) = (status_rect(pill, scale, 0), status_rect(pill, scale, Status::ALL.len() - 1));
    (first.0, first.1, last.2, last.3)
}

pub fn steam_search_frame(pill: Rect, scale: f32) -> (Rect, Rect) {
    let (l, _) = side(pill, scale);
    let frame = (
        l,
        pill.1 + TOOLS_TOP * scale,
        status_block(pill, scale).0 - 8.0 * scale,
        pill.1 + (TOOLS_TOP + TOOLS_H) * scale,
    );
    (frame, (frame.0 + 32.0 * scale, frame.1, frame.2 - 10.0 * scale, frame.3))
}

fn game_rect(pill: Rect, scale: f32, i: usize) -> Rect {
    let (l, r) = side(pill, scale);
    let top = pill.1 + (LIST_TOP + i as f32 * (ROW_H + ROW_GAP)) * scale;
    (l, top, r, top + ROW_H * scale)
}

fn update_rect(row: Rect, scale: f32) -> Rect {
    (row.2 - (UPDATE_W + 6.0) * scale, row.1 + 5.0 * scale, row.2 - 6.0 * scale, row.3 - 5.0 * scale)
}

pub fn hit_steam(pill: Rect, scale: f32, px: f32, py: f32, ui: &SteamUi) -> Option<SteamHit> {
    if !crate::platform::steam::client_installed() {
        return None;
    }
    if inside(status_block(pill, scale), px, py) {
        if !ui.steam_up {
            return Some(SteamHit::Start);
        }
        return (0..Status::ALL.len()).find(|&k| inside(status_rect(pill, scale, k), px, py)).map(SteamHit::Status);
    }
    if inside(steam_search_frame(pill, scale).0, px, py) {
        return Some(SteamHit::Search);
    }
    if let Some(i) = (0..STEAM_LINKS.len()).find(|&i| inside(link_rect(pill, scale, i), px, py)) {
        return Some(SteamHit::Link(i));
    }
    let count = ui.accounts.len().min(MAX_ACCOUNTS);
    if let Some(i) = (0..count).find(|&i| inside(account_rect(pill, scale, i, count), px, py)) {
        return Some(SteamHit::Account(i));
    }
    let shown = ui.games.len().saturating_sub(ui.scroll).min(steam_rows(pill, scale));
    let row = (0..shown).find(|&i| inside(game_rect(pill, scale, i), px, py))?;
    let wants_update = ui.games.get(ui.scroll + row).is_some_and(|g| g.update == Update::Needed);
    if wants_update && inside(update_rect(game_rect(pill, scale, row), scale), px, py) {
        return Some(SteamHit::Update(row));
    }
    Some(SteamHit::Game(row))
}

fn badge_color(id: u32, alpha: f32) -> windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F {
    const COLORS: [(f32, f32, f32); 6] = [
        (0.36, 0.61, 0.94),
        (0.42, 0.80, 0.47),
        (0.91, 0.64, 0.24),
        (0.65, 0.55, 0.98),
        (0.89, 0.40, 0.31),
        (0.31, 0.82, 0.77),
    ];
    let (r, g, b) = COLORS[(id as usize / 10) % COLORS.len()];
    rgba(r, g, b, alpha)
}

fn initial(name: &str) -> String {
    name.chars().find(|c| c.is_alphanumeric()).map(|c| c.to_uppercase().collect()).unwrap_or_else(|| "?".into())
}

fn played_text(minutes: u32) -> String {
    let (h, min) = (crate::ui::i18n::tr("h"), crate::ui::i18n::tr("min"));
    match minutes {
        0 => String::new(),
        1..=59 => format!("{minutes} {min}"),
        60..=599 => {
            let tenths = (minutes + 3) / 6;
            if tenths.is_multiple_of(10) {
                format!("{} {h}", tenths / 10)
            } else {
                format!("{}.{} {h}", tenths / 10, tenths % 10)
            }
        }
        m => format!("{} {h}", (m + 30) / 60),
    }
}

fn status_color(status: Status, alpha: f32) -> windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F {
    match status {
        Status::Online => rgba(0.35, 0.85, 0.5, alpha),
        Status::Away => rgba(1.0, 0.75, 0.3, alpha),
        Status::Invisible => rgba(0.62, 0.64, 0.74, alpha),
        Status::Offline => rgba(0.42, 0.43, 0.5, alpha),
    }
}

fn paint_badge(p: &Painter, center: (f32, f32), radius: f32, key: ArtKey, name: &str, color_seed: u32) -> Result<()> {
    let rect = (center.0 - radius, center.1 - radius, center.0 + radius, center.1 + radius);
    if let Some(art) = steam_art::get(key) {
        return p.image(rect, radius, art.side, &art.bgra);
    }
    p.fill_ellipse(center.0, center.1, radius, radius, badge_color(color_seed, 0.9))?;
    p.text(
        &initial(name),
        rect,
        radius * 1.1,
        DWRITE_FONT_WEIGHT_BOLD,
        DWRITE_TEXT_ALIGNMENT_CENTER,
        rgba(0.05, 0.05, 0.08, 1.0),
    )
}

fn paint_search(p: &Painter, pill: Rect, scale: f32, ui: &UiState) -> Result<()> {
    let (frame, inner) = steam_search_frame(pill, scale);
    p.fill_round_rect(frame, 12.0 * scale, rgba(1.0, 1.0, 1.0, 0.07))?;
    if ui.editing {
        p.stroke_round_rect(frame, 12.0 * scale, 1.2 * scale, acc(0.6))?;
    }
    p.glyph(
        '\u{E721}',
        (frame.0 + 6.0 * scale, frame.1, frame.0 + 30.0 * scale, frame.3),
        13.0 * scale,
        rgba(1.0, 1.0, 1.0, 0.6),
    )?;
    let query = &ui.steam.query;
    let text = query.text();
    if text.is_empty() {
        p.text(
            "Search games",
            inner,
            STEAM_SEARCH_FONT * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            rgba(1.0, 1.0, 1.0, 0.4),
        )?;
    }
    if let Ok(layout) = crate::ui::gfx::make_layout(p.dwrite(), &text, 100_000.0, STEAM_SEARCH_FONT * scale) {
        let (_, _, line_h) = edit::caret_xy(&layout, 0);
        let top = inner.1 + ((inner.3 - inner.1) - line_h) / 2.0;
        p.push_clip(inner);
        if query.has_selection() {
            let (a, b) = query.selection();
            let (a16, b16) = (query.utf16_at(a), query.utf16_at(b));
            for (rx, ry, rw, rh) in edit::selection_rects(&layout, a16, b16 - a16) {
                p.fill_rect((inner.0 + rx, top + ry, inner.0 + rx + rw, top + ry + rh), acc(0.35))?;
            }
        }
        p.draw_layout(inner.0, top, &layout, WHITE)?;
        if ui.editing && ui.caret_on {
            let (cx, cy, ch) = edit::caret_xy(&layout, query.utf16_at(query.caret));
            p.fill_rect((inner.0 + cx, top + cy, inner.0 + cx + 1.6 * scale, top + cy + ch), WHITE)?;
        }
        p.pop_clip();
    }
    Ok(())
}

pub(in crate::view) fn paint_steam(p: &Painter, pill: Rect, scale: f32, ui: &UiState) -> Result<()> {
    let (l, r) = side(pill, scale);
    let (x, y, w, _) = pill;
    let s = &ui.steam;
    p.text(
        "Steam",
        (l + 2.0 * scale, y + 56.0 * scale, l + 200.0 * scale, y + 74.0 * scale),
        14.0 * scale,
        DWRITE_FONT_WEIGHT_BOLD,
        DWRITE_TEXT_ALIGNMENT_LEADING,
        WHITE,
    )?;
    if !crate::platform::steam::client_installed() {
        return p.text(
            "Steam is not installed on this computer",
            (x, y + 160.0 * scale, x + w, y + 260.0 * scale),
            15.0 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_CENTER,
            rgba(1.0, 1.0, 1.0, 0.55),
        );
    }
    let mut subtitle = match (&s.running, s.steam_up) {
        (Some((_, name, since)), _) => {
            format!("{} {name} · {}", crate::ui::i18n::tr("Playing"), duration_text(since.elapsed().as_secs() as i64))
        }
        (None, true) => crate::ui::i18n::tr("Steam is running").into_owned(),
        (None, false) => crate::ui::i18n::tr("Steam is not running").into_owned(),
    };
    let playing = s.running.is_some();
    let updates = s.updates();
    if !playing && updates > 0 {
        subtitle = format!("{subtitle} · {}: {updates}", crate::ui::i18n::tr("Updates"));
    }
    p.text(
        &subtitle,
        (l + 2.0 * scale, y + 72.0 * scale, r - 330.0 * scale, y + 90.0 * scale),
        11.5 * scale,
        DWRITE_FONT_WEIGHT_NORMAL,
        DWRITE_TEXT_ALIGNMENT_LEADING,
        if playing {
            rgba(0.45, 0.9, 0.6, 1.0)
        } else if updates > 0 {
            rgba(1.0, 0.82, 0.45, 0.95)
        } else {
            rgba(1.0, 1.0, 1.0, 0.5)
        },
    )?;
    for (i, (label, _)) in STEAM_LINKS.iter().enumerate() {
        let chip = link_rect(pill, scale, i);
        let hovered = ui.home.hover_act == Some(1400 + i);
        p.fill_round_rect(chip, 13.0 * scale, rgba(1.0, 1.0, 1.0, if hovered { 0.18 } else { 0.08 }))?;
        p.text(label, chip, 12.0 * scale, DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_TEXT_ALIGNMENT_CENTER, WHITE)?;
    }

    let count = s.accounts.len().min(MAX_ACCOUNTS);
    for (i, account) in s.accounts.iter().take(MAX_ACCOUNTS).enumerate() {
        let chip = account_rect(pill, scale, i, count);
        let current = s.active == Some(account.account_id());
        let armed = s.armed.as_deref() == Some(account.name.as_str());
        let busy = s.switching.as_deref() == Some(account.persona.as_str());
        let hovered = ui.home.hover_act == Some(1410 + i);
        let fill = if armed {
            rgba(1.0, 0.75, 0.3, if hovered { 0.4 } else { 0.3 })
        } else if current {
            acc(if hovered { 0.5 } else { 0.38 })
        } else {
            rgba(1.0, 1.0, 1.0, if hovered { 0.16 } else { 0.07 })
        };
        p.fill_round_rect(chip, 14.0 * scale, fill)?;
        let radius = 10.0 * scale;
        let dot = (chip.0 + 5.0 * scale + radius, (chip.1 + chip.3) / 2.0);
        paint_badge(p, dot, radius, ArtKey::Avatar(account.id64), &account.persona, account.account_id())?;
        if !account.remembered {
            p.fill_ellipse(dot.0, dot.1, radius, radius, rgba(0.0, 0.0, 0.0, 0.5))?;
        }
        if current
            && s.steam_up
            && let Some(status) = s.status
        {
            let at = (dot.0 + radius * 0.75, dot.1 + radius * 0.75);
            p.fill_ellipse(at.0, at.1, 5.0 * scale, 5.0 * scale, rgba(0.1, 0.1, 0.14, 1.0))?;
            p.fill_ellipse(at.0, at.1, 3.5 * scale, 3.5 * scale, status_color(status, 1.0))?;
        }
        let label = if armed {
            crate::ui::i18n::tr("Click again").into_owned()
        } else if busy {
            "…".to_string()
        } else {
            account.persona.clone()
        };
        p.text(
            &label,
            (chip.0 + 32.0 * scale, chip.1, chip.2 - 8.0 * scale, chip.3),
            12.5 * scale,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            rgba(1.0, 1.0, 1.0, if account.remembered { 1.0 } else { 0.5 }),
        )?;
    }

    paint_search(p, pill, scale, ui)?;
    if s.steam_up {
        for (k, status) in Status::ALL.iter().enumerate() {
            let chip = status_rect(pill, scale, k);
            let on = s.status == Some(*status);
            let hovered = ui.home.hover_act == Some(1420 + k);
            let fill = if on {
                acc(if hovered { 0.5 } else { 0.38 })
            } else {
                rgba(1.0, 1.0, 1.0, if hovered { 0.16 } else { 0.07 })
            };
            p.fill_round_rect(chip, 13.0 * scale, fill)?;
            p.fill_ellipse(
                chip.0 + 12.0 * scale,
                (chip.1 + chip.3) / 2.0,
                4.0 * scale,
                4.0 * scale,
                status_color(*status, 1.0),
            )?;
            p.text(
                status.label(),
                (chip.0 + 22.0 * scale, chip.1, chip.2 - 4.0 * scale, chip.3),
                11.0 * scale,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                rgba(1.0, 1.0, 1.0, if on { 1.0 } else { 0.8 }),
            )?;
        }
    } else {
        let button = status_block(pill, scale);
        p.fill_round_rect(button, 13.0 * scale, acc(if ui.home.hover_act == Some(1405) { 0.5 } else { 0.35 }))?;
        p.text("Start Steam", button, 12.5 * scale, DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_TEXT_ALIGNMENT_CENTER, WHITE)?;
    }

    if s.games.is_empty() {
        let text = if s.query.is_empty() { "No installed games found" } else { "Nothing found" };
        return p.text(
            text,
            (x, y + 230.0 * scale, x + w, y + 300.0 * scale),
            14.0 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_CENTER,
            rgba(1.0, 1.0, 1.0, 0.45),
        );
    }
    for (i, game) in s.games.iter().skip(s.scroll).take(steam_rows(pill, scale)).enumerate() {
        let row = game_rect(pill, scale, i);
        let hovered = ui.home.hover_act == Some(1430 + i);
        let running = s.running.as_ref().is_some_and(|(id, _, _)| *id == game.id);
        let fill = if running {
            rgba(0.3, 0.85, 0.5, if hovered { 0.28 } else { 0.2 })
        } else {
            rgba(1.0, 1.0, 1.0, if hovered { 0.12 } else { 0.05 })
        };
        p.fill_round_rect(row, 11.0 * scale, fill)?;
        let radius = 11.0 * scale;
        let center = (row.0 + 8.0 * scale + radius, (row.1 + row.3) / 2.0);
        paint_badge(p, center, radius, ArtKey::Game(game.id), &game.name, game.id)?;

        let mut edge = row.2 - 12.0 * scale;
        match game.update {
            Update::Needed => {
                let button = update_rect(row, scale);
                let hot = ui.home.hover_act == Some(1450 + i);
                p.fill_round_rect(button, 10.0 * scale, rgba(1.0, 0.72, 0.25, if hot { 0.5 } else { 0.3 }))?;
                p.text(
                    "Update",
                    button,
                    11.5 * scale,
                    DWRITE_FONT_WEIGHT_SEMI_BOLD,
                    DWRITE_TEXT_ALIGNMENT_CENTER,
                    rgba(1.0, 0.9, 0.65, 1.0),
                )?;
                edge = button.0 - 10.0 * scale;
            }
            Update::Running => {
                let label = (edge - UPDATE_W * scale, row.1, edge, row.3);
                p.text(
                    "Updating…",
                    label,
                    11.5 * scale,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_TEXT_ALIGNMENT_TRAILING,
                    rgba(1.0, 0.82, 0.45, 0.95),
                )?;
                edge = label.0 - 8.0 * scale;
            }
            Update::None => {}
        }
        let detail = if running {
            crate::ui::i18n::tr("playing now").into_owned()
        } else if game.last > 0 {
            format!("{} · {}", time_ago(game.last), played_text(game.minutes))
        } else {
            played_text(game.minutes)
        };
        let detail_rect = (edge - DETAIL_W * scale, row.1, edge, row.3);
        p.text(
            &detail,
            detail_rect,
            12.0 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_TRAILING,
            if running { rgba(0.5, 0.95, 0.65, 1.0) } else { rgba(1.0, 1.0, 1.0, 0.55) },
        )?;
        p.text(
            &game.name,
            (row.0 + 40.0 * scale, row.1, detail_rect.0 - 8.0 * scale, row.3),
            13.5 * scale,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            WHITE,
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn badges_use_the_first_letter_and_a_steady_colour() {
        assert_eq!(initial("hades"), "H");
        assert_eq!(initial("  7 Days"), "7");
        assert_eq!(initial("!!!"), "?");
        assert_eq!(badge_color(570, 1.0).r, badge_color(570, 0.5).r);
    }

    #[test]
    fn play_time_reads_in_minutes_and_hours_never_in_thousands() {
        crate::ui::i18n::set_ru(false);
        assert_eq!(played_text(0), "");
        assert_eq!(played_text(45), "45 min");
        assert_eq!(played_text(60), "1 h");
        assert_eq!(played_text(135), "2.3 h");
        assert_eq!(played_text(599), "10 h");
        assert_eq!(played_text(1_000), "17 h");
        assert_eq!(played_text(115_872), "1931 h");
        assert_eq!(played_text(282_000), "4700 h");
    }

    #[test]
    fn the_rows_fit_and_the_chips_do_not_overlap() {
        let pill = (0.0, 0.0, 640.0, 384.0);
        assert!(steam_rows(pill, 1.0) >= 6);
        let last = game_rect(pill, 1.0, steam_rows(pill, 1.0) - 1);
        assert!(last.3 <= pill.3);
        for count in 1..=MAX_ACCOUNTS {
            for i in 0..count - 1 {
                assert!(account_rect(pill, 1.0, i, count).2 < account_rect(pill, 1.0, i + 1, count).0);
            }
            assert!(account_rect(pill, 1.0, count - 1, count).2 <= side(pill, 1.0).1 + 0.01);
        }
        for i in 0..STEAM_LINKS.len() - 1 {
            assert!(link_rect(pill, 1.0, i).2 < link_rect(pill, 1.0, i + 1).0);
        }
        for k in 0..Status::ALL.len() - 1 {
            assert!(status_rect(pill, 1.0, k).2 < status_rect(pill, 1.0, k + 1).0);
        }
        let (frame, _) = steam_search_frame(pill, 1.0);
        assert!(frame.2 - frame.0 > 150.0 && frame.2 < status_block(pill, 1.0).0);
    }

    #[test]
    fn the_update_button_sits_inside_its_row() {
        let row = game_rect((0.0, 0.0, 640.0, 384.0), 1.0, 0);
        let button = update_rect(row, 1.0);
        assert!(button.0 > row.0 + 300.0 && button.2 <= row.2 && button.1 > row.1 && button.3 < row.3);
    }
}
