use crate::data::pocket::PocketItem;
use crate::view::*;

#[derive(Default)]
pub struct BrowserHubUi {
    pub scroll: usize,
    pub pocket_scroll: usize,
    pub pocket: Vec<PocketItem>,
    pub undo: Option<PocketItem>,
    pub feedback: String,
    pub drop_hot: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HubHit {
    Search,
    Request(usize, usize),
    Activity(usize, usize),
    Pocket(usize, usize),
    Undo,
}

impl HubHit {
    pub fn hover(self) -> usize {
        match self {
            Self::Search => 2400,
            Self::Undo => 2401,
            Self::Request(index, action) => 2500 + index * 4 + action,
            Self::Activity(index, action) => 2600 + index * 4 + action,
            Self::Pocket(index, action) => 2700 + index * 4 + action,
        }
    }
}

pub fn hub_rows(pill: Rect, scale: f32) -> usize {
    (((pill.3 - 125.0 * scale) / (88.0 * scale)).floor() as usize).max(1)
}
pub fn hub_row(pill: Rect, scale: f32, index: usize) -> Rect {
    let top = pill.1 + (116.0 + index as f32 * 88.0) * scale;
    (pill.0 + 16.0 * scale, top, pill.0 + pill.2 - 16.0 * scale, top + 80.0 * scale)
}
pub fn hub_button(rect: Rect, scale: f32, index: usize) -> Rect {
    let step = ((rect.2 - rect.0 - 16.0 * scale) / 4.0).min(78.0 * scale);
    let left = rect.0 + 8.0 * scale + index as f32 * step;
    (left, rect.3 - 30.0 * scale, left + step - 4.0 * scale, rect.3 - 4.0 * scale)
}
pub fn hub_search(pill: Rect, scale: f32) -> Rect {
    (pill.0 + 16.0 * scale, pill.1 + 58.0 * scale, pill.0 + pill.2 - 16.0 * scale, pill.1 + 96.0 * scale)
}
pub fn hub_undo(pill: Rect, scale: f32) -> Rect {
    (pill.0 + pill.2 - 90.0 * scale, pill.1 + 58.0 * scale, pill.0 + pill.2 - 16.0 * scale, pill.1 + 90.0 * scale)
}

pub fn hit_hub(pill: Rect, scale: f32, x: f32, y: f32, ui: &UiState) -> Option<HubHit> {
    if ui.tab == Tab::Pocket {
        if ui.hub.undo.is_some() && inside(hub_undo(pill, scale), x, y) {
            return Some(HubHit::Undo);
        }
        for (offset, item) in
            ui.hub.pocket.iter().enumerate().skip(ui.hub.pocket_scroll).take(hub_rows(pill, scale)).enumerate()
        {
            let (index, entry) = item;
            for action in 0..4 {
                if action == 0 && entry.url.is_empty() && entry.source.is_empty() {
                    continue;
                }
                if inside(hub_button(hub_row(pill, scale, offset), scale, action), x, y) {
                    return Some(HubHit::Pocket(index, action));
                }
            }
        }
    } else {
        if ui.browser.connected && ui.browser.features.tabs && inside(hub_search(pill, scale), x, y) {
            return Some(HubHit::Search);
        }
        let count = ui.browser.requests.len() + ui.browser.activity.len();
        for (offset, index) in (ui.hub.scroll..count).take(hub_rows(pill, scale)).enumerate() {
            let request = index < ui.browser.requests.len();
            let item_id = if request {
                &ui.browser.requests[index].id
            } else {
                &ui.browser.activity[index - ui.browser.requests.len()].id
            };
            if !ui.browser.connected || ui.browser.pending.contains(item_id) {
                continue;
            }
            for action in 0..if request { 3 } else { 2 } {
                if !request && action == 1 {
                    let tab = &ui.browser.activity[index - ui.browser.requests.len()];
                    if !tab.activity.iter().any(|s| s == "audio" || s == "muted") {
                        continue;
                    }
                }
                if inside(hub_button(hub_row(pill, scale, offset), scale, action), x, y) {
                    return Some(if request {
                        HubHit::Request(index, action)
                    } else {
                        HubHit::Activity(index - ui.browser.requests.len(), action)
                    });
                }
            }
        }
    }
    None
}

fn label(p: &Painter, text: &str, rect: Rect, scale: f32, size: f32, alpha: f32) -> Result<()> {
    p.text(
        text,
        rect,
        size * scale,
        DWRITE_FONT_WEIGHT_NORMAL,
        DWRITE_TEXT_ALIGNMENT_LEADING,
        rgba(1.0, 1.0, 1.0, alpha),
    )
}
fn action(p: &Painter, text: &str, rect: Rect, scale: f32, enabled: bool, hovered: bool) -> Result<()> {
    p.fill_round_rect(rect, 13.0 * scale, rgba(1.0, 1.0, 1.0, if enabled && hovered { 0.18 } else { 0.08 }))?;
    p.text(
        text,
        rect,
        11.0 * scale,
        DWRITE_FONT_WEIGHT_NORMAL,
        DWRITE_TEXT_ALIGNMENT_CENTER,
        rgba(1.0, 1.0, 1.0, if enabled { 1.0 } else { 0.35 }),
    )
}

pub(in crate::view) fn paint_hub(p: &Painter, pill: Rect, scale: f32, ui: &UiState) -> Result<()> {
    let header = hub_search(pill, scale);
    if ui.tab == Tab::Pocket {
        label(
            p,
            if ui.hub.drop_hot { "Drop to save in Pocket" } else { "Pocket" },
            (header.0, header.1, header.2 - 85.0 * scale, header.3),
            scale,
            16.0,
            1.0,
        )?;
        if ui.hub.undo.is_some() {
            action(p, "Undo", hub_undo(pill, scale), scale, true, ui.home.hover_act == Some(HubHit::Undo.hover()))?;
        }
    } else {
        p.fill_round_rect(
            header,
            19.0 * scale,
            rgba(1.0, 1.0, 1.0, if ui.home.hover_act == Some(HubHit::Search.hover()) { 0.14 } else { 0.07 }),
        )?;
        p.glyph(
            '\u{E721}',
            (header.0 + 8.0 * scale, header.1, header.0 + 38.0 * scale, header.3),
            16.0 * scale,
            acc(0.8),
        )?;
        label(
            p,
            "Find a browser tab",
            (header.0 + 42.0 * scale, header.1, header.2 - 10.0 * scale, header.3),
            scale,
            13.0,
            if ui.browser.features.tabs { 1.0 } else { 0.4 },
        )?;
    }
    let empty = if ui.tab == Tab::Pocket {
        ui.hub.pocket.is_empty()
    } else {
        ui.browser.activity.is_empty() && ui.browser.requests.is_empty()
    };
    if empty {
        let message = if ui.tab == Tab::Pocket {
            "Drag a link or text here, or save from the browser menu"
        } else if !ui.browser.connected {
            "Waiting for Night Browser"
        } else if !ui.browser.features.tabs && !ui.browser.features.activity && !ui.browser.features.requests {
            "Enable tabs and activity in browser integration settings"
        } else {
            "Playing tabs and paused requests appear here"
        };
        label(p, message, (header.0, pill.1 + 130.0 * scale, header.2, pill.1 + 195.0 * scale), scale, 13.0, 0.55)?;
    }
    let caption = if ui.browser.error { "The action is no longer available" } else { &ui.hub.feedback };
    if !caption.is_empty() {
        label(p, caption, (header.0, header.3 + 2.0 * scale, header.2, header.3 + 18.0 * scale), scale, 10.0, 0.6)?;
    }
    if ui.tab == Tab::Pocket {
        for (offset, item) in ui.hub.pocket.iter().skip(ui.hub.pocket_scroll).take(hub_rows(pill, scale)).enumerate() {
            let row = hub_row(pill, scale, offset);
            p.fill_round_rect(row, 12.0 * scale, rgba(1.0, 1.0, 1.0, 0.055))?;
            label(
                p,
                &item.title,
                (row.0 + 12.0 * scale, row.1 + 4.0 * scale, row.2 - 12.0 * scale, row.1 + 27.0 * scale),
                scale,
                13.0,
                1.0,
            )?;
            let preview = if item.kind == "text" { item.text.lines().next().unwrap_or("") } else { &item.url };
            label(
                p,
                preview,
                (row.0 + 12.0 * scale, row.1 + 27.0 * scale, row.2 - 12.0 * scale, row.1 + 45.0 * scale),
                scale,
                11.0,
                0.55,
            )?;
            for (i, text) in ["Open source", "Copy", "Make note", "Remove"].iter().enumerate() {
                action(
                    p,
                    text,
                    hub_button(row, scale, i),
                    scale,
                    i != 0 || !item.url.is_empty() || !item.source.is_empty(),
                    ui.home.hover_act == Some(HubHit::Pocket(ui.hub.pocket_scroll + offset, i).hover()),
                )?;
            }
        }
    } else {
        let count = ui.browser.requests.len() + ui.browser.activity.len();
        for (offset, index) in (ui.hub.scroll..count).take(hub_rows(pill, scale)).enumerate() {
            let row = hub_row(pill, scale, offset);
            p.fill_round_rect(row, 12.0 * scale, rgba(1.0, 1.0, 1.0, 0.055))?;
            let (title, detail, id, buttons) = if index < ui.browser.requests.len() {
                let item = &ui.browser.requests[index];
                (
                    format!("{} · {}", item.method, item.host),
                    crate::ui::i18n::tr("Request waiting for a decision").into_owned(),
                    &item.id,
                    vec!["Edit request", "Forward", "Reject"],
                )
            } else {
                let item = &ui.browser.activity[index - ui.browser.requests.len()];
                let mut buttons = vec!["Open tab"];
                if item.activity.iter().any(|s| s == "audio" || s == "muted") {
                    buttons.push(if item.activity.iter().any(|s| s == "muted") { "Unmute tab" } else { "Mute tab" });
                }
                (item.title.clone(), item.activity_label(), &item.id, buttons)
            };
            label(
                p,
                &title,
                (row.0 + 12.0 * scale, row.1 + 4.0 * scale, row.2 - 12.0 * scale, row.1 + 27.0 * scale),
                scale,
                13.0,
                1.0,
            )?;
            label(
                p,
                &detail,
                (row.0 + 12.0 * scale, row.1 + 27.0 * scale, row.2 - 12.0 * scale, row.1 + 45.0 * scale),
                scale,
                11.0,
                0.55,
            )?;
            for (i, text) in buttons.iter().enumerate() {
                action(
                    p,
                    text,
                    hub_button(row, scale, i),
                    scale,
                    ui.browser.connected && !ui.browser.pending.contains(id),
                    ui.home.hover_act
                        == Some(
                            if index < ui.browser.requests.len() {
                                HubHit::Request(index, i)
                            } else {
                                HubHit::Activity(index - ui.browser.requests.len(), i)
                            }
                            .hover(),
                        ),
                )?;
            }
        }
    }
    Ok(())
}

pub fn browser_attention(ui: &UiState) -> Option<String> {
    ui.browser.requests.first().map(|request| format!("{} · {}", request.method, request.host))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hub_actions_fit_small_panels_and_pending_actions_are_disabled() {
        let mut ui = UiState::new();
        ui.tab = Tab::Browser;
        ui.browser.connected = true;
        ui.browser.requests.push(crate::services::browser::RequestItem {
            id: "request-1".into(),
            method: "POST".into(),
            host: "example.com".into(),
        });
        for scale in [0.75, 1.0, 1.25, 1.5, 2.0] {
            let pill = (12.0, 18.0, 320.0 * scale, 340.0 * scale);
            let row = hub_row(pill, scale, 0);
            for i in 0..4 {
                let r = hub_button(row, scale, i);
                assert!(r.0 >= row.0 && r.2 <= row.2);
            }
            let r = hub_button(row, scale, 1);
            let (x, y) = ((r.0 + r.2) / 2.0, (r.1 + r.3) / 2.0);
            assert_eq!(hit_hub(pill, scale, x, y, &ui), Some(HubHit::Request(0, 1)));
            ui.browser.pending.push("request-1".into());
            assert_eq!(hit_hub(pill, scale, x, y, &ui), None);
            ui.browser.pending.clear();
        }
    }
}
