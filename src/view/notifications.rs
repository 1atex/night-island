use crate::platform::notifications::{Record, Snapshot, Status};
use crate::view::*;

#[derive(Default)]
pub struct NotificationsUi {
    pub state: Snapshot,
    pub scroll: usize,
    pub hover: Option<NotificationsHit>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NotificationsHit {
    Connect,
    WindowsSettings,
    Example,
}

pub const NOTIFICATION_SIZE: (f32, f32) = (420.0, 158.0);

pub fn notification_buttons(pill: Rect, scale: f32) -> [Rect; 3] {
    let left = pill.0 + SIDE_PAD * scale;
    let right = pill.0 + pill.2 - SIDE_PAD * scale;
    let gap = 10.0 * scale;
    let width = (right - left - 2.0 * gap) / 3.0;
    std::array::from_fn(|i| {
        let x = left + i as f32 * (width + gap);
        (x, pill.1 + 145.0 * scale, x + width, pill.1 + 179.0 * scale)
    })
}

pub fn notifications_rows(pill: Rect, scale: f32) -> usize {
    ((pill.3 - 218.0 * scale) / (62.0 * scale)).floor().max(1.0) as usize
}

pub fn hit_notifications(pill: Rect, scale: f32, x: f32, y: f32) -> Option<NotificationsHit> {
    notification_buttons(pill, scale)
        .iter()
        .position(|rect| inside(*rect, x, y))
        .map(|i| [NotificationsHit::Connect, NotificationsHit::WindowsSettings, NotificationsHit::Example][i])
}

pub fn notification_close(rect: Rect, scale: f32) -> Rect {
    (rect.0 + rect.2 - 42.0 * scale, rect.1 + 10.0 * scale, rect.0 + rect.2 - 12.0 * scale, rect.1 + 40.0 * scale)
}

pub fn paint_notification_card(
    p: &Painter,
    rect: Rect,
    scale: f32,
    record: &Record,
    pending: usize,
    hover_close: bool,
) -> Result<()> {
    let (x, y, w, h) = rect;
    let icon = (x + 18.0 * scale, y + 16.0 * scale, x + 50.0 * scale, y + 48.0 * scale);
    if let Some(icon_data) = &record.icon {
        p.image(icon, 9.0 * scale, icon_data.side, &icon_data.bgra)?;
    } else {
        p.fill_round_rect(icon, 9.0 * scale, acc(0.18))?;
        p.glyph('\u{E7F4}', icon, 17.0 * scale, acc(1.0))?;
    }
    p.text(
        &record.app,
        (x + 62.0 * scale, y + 14.0 * scale, x + w - 104.0 * scale, y + 38.0 * scale),
        12.0 * scale,
        DWRITE_FONT_WEIGHT_SEMI_BOLD,
        DWRITE_TEXT_ALIGNMENT_LEADING,
        rgba(1.0, 1.0, 1.0, 0.62),
    )?;
    if pending > 0 {
        p.text(
            &format!("+{pending}"),
            (x + w - 98.0 * scale, y + 16.0 * scale, x + w - 48.0 * scale, y + 38.0 * scale),
            11.5 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_TRAILING,
            rgba(1.0, 1.0, 1.0, 0.5),
        )?;
    }
    let close = notification_close(rect, scale);
    if hover_close {
        p.fill_round_rect(close, 15.0 * scale, rgba(1.0, 1.0, 1.0, 0.1))?;
    }
    p.glyph('\u{E711}', close, 10.0 * scale, rgba(1.0, 1.0, 1.0, 0.55))?;
    p.text(
        &record.title,
        (x + 18.0 * scale, y + 54.0 * scale, x + w - 18.0 * scale, y + 80.0 * scale),
        15.0 * scale,
        DWRITE_FONT_WEIGHT_SEMI_BOLD,
        DWRITE_TEXT_ALIGNMENT_LEADING,
        WHITE,
    )?;
    let body = (x + 18.0 * scale, y + 84.0 * scale, x + w - 18.0 * scale, y + h - 16.0 * scale);
    p.text_wrapped_trimmed(&record.body, body, 13.0 * scale, DWRITE_FONT_WEIGHT_NORMAL, rgba(1.0, 1.0, 1.0, 0.72))
}

pub fn notification_status(status: Status) -> &'static str {
    match status {
        Status::Disabled => "Show Windows notifications from the island",
        Status::NeedsAccess => "Allow notification access to connect the island",
        Status::Denied => "Notification access is disabled in Windows privacy settings",
        Status::NeedsQuietMode => "Turn on Do not disturb in Windows to hide the standard banners",
        Status::Ready => "Connected. New notifications will appear from the island",
        Status::Unavailable => "Windows notifications are unavailable. Try connecting again",
    }
}

pub fn paint_notifications(p: &Painter, pill: Rect, scale: f32, ui: &NotificationsUi, enabled: bool) -> Result<()> {
    let (x, y, w, _) = pill;
    let left = x + SIDE_PAD * scale;
    let right = x + w - SIDE_PAD * scale;
    p.text(
        "Windows notifications",
        (left, y + 62.0 * scale, right, y + 86.0 * scale),
        17.0 * scale,
        DWRITE_FONT_WEIGHT_SEMI_BOLD,
        DWRITE_TEXT_ALIGNMENT_LEADING,
        WHITE,
    )?;
    p.text_wrapped(
        notification_status(ui.state.status),
        (left, y + 91.0 * scale, right, y + 137.0 * scale),
        12.5 * scale,
        DWRITE_FONT_WEIGHT_NORMAL,
        rgba(1.0, 1.0, 1.0, 0.6),
    )?;
    for (i, rect) in notification_buttons(pill, scale).into_iter().enumerate() {
        let selected = ui.hover
            == Some([NotificationsHit::Connect, NotificationsHit::WindowsSettings, NotificationsHit::Example][i]);
        p.fill_round_rect(
            rect,
            11.0 * scale,
            if i == 0 && enabled { acc(0.24) } else { rgba(1.0, 1.0, 1.0, if selected { 0.13 } else { 0.07 }) },
        )?;
        let label = match i {
            0 => {
                if matches!(ui.state.status, Status::NeedsAccess | Status::Denied) {
                    "Allow access"
                } else if ui.state.status == Status::Unavailable {
                    "Try again"
                } else if enabled {
                    "Disable"
                } else {
                    "Connect"
                }
            }
            1 => "Windows settings",
            _ => "Show example",
        };
        p.text(label, rect, 12.0 * scale, DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_TEXT_ALIGNMENT_CENTER, WHITE)?;
    }
    p.text(
        "Notification history",
        (left, y + 188.0 * scale, right, y + 210.0 * scale),
        12.0 * scale,
        DWRITE_FONT_WEIGHT_SEMI_BOLD,
        DWRITE_TEXT_ALIGNMENT_LEADING,
        rgba(1.0, 1.0, 1.0, 0.45),
    )?;
    if ui.state.records.is_empty() {
        return p.text(
            "New notifications will be listed here",
            (left, y + 228.0 * scale, right, y + 298.0 * scale),
            13.0 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_CENTER,
            rgba(1.0, 1.0, 1.0, 0.45),
        );
    }
    for (row, record) in ui.state.records.iter().skip(ui.scroll).take(notifications_rows(pill, scale)).enumerate() {
        let top = y + (216.0 + row as f32 * 62.0) * scale;
        p.fill_round_rect((left, top, right, top + 56.0 * scale), 12.0 * scale, rgba(1.0, 1.0, 1.0, 0.045))?;
        p.text(
            &format!("{} · {}", record.app, record.title),
            (left + 12.0 * scale, top + 4.0 * scale, right - 12.0 * scale, top + 27.0 * scale),
            12.5 * scale,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            WHITE,
        )?;
        p.text(
            &record.body.replace('\n', " "),
            (left + 12.0 * scale, top + 27.0 * scale, right - 12.0 * scale, top + 49.0 * scale),
            12.0 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            rgba(1.0, 1.0, 1.0, 0.55),
        )?;
    }
    Ok(())
}
