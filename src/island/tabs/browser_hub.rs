use crate::data::pocket::PocketItem;
use crate::island::*;
use crate::services::browser;

impl App {
    pub(in crate::island) fn reload_pocket(&mut self) {
        self.ui.hub.pocket = self.store.as_ref().map(Store::pocket_items).unwrap_or_default();
        self.ui.hub.pocket_scroll = self.ui.hub.pocket_scroll.min(self.ui.hub.pocket.len().saturating_sub(1));
    }
    pub(in crate::island) fn receive_pocket(&mut self) {
        for (request, item) in browser::take_pocket() {
            let ok = self.save_pocket(&item);
            browser::pocket_result(&request, ok);
        }
    }
    pub(in crate::island) fn save_pocket(&mut self, item: &PocketItem) -> bool {
        let ok = self.store.as_ref().is_some_and(|store| store.pocket_add(item));
        self.ui.hub.feedback =
            crate::ui::i18n::tr(if ok { "Saved to Pocket" } else { "Pocket is full or unavailable" }).into_owned();
        self.reload_pocket();
        self.select_tab(view::Tab::Pocket);
        self.expand();
        self.react(Reaction::File, 1800);
        self.repaint_expanded();
        #[cfg(feature = "demo")]
        demo::browser_snapshot(self);
        ok
    }
    pub(in crate::island) fn open_browser_search(&mut self) {
        self.restore_pill();
        self.kill_timer(Timer::Collapse);
        self.ui.cmd.open = true;
        self.ui.cmd.input.set_text("@");
        self.refresh_cmd();
        self.expand();
        self.begin_edit();
        self.repaint_expanded();
        #[cfg(feature = "demo")]
        demo::browser_snapshot(self);
    }
    pub(in crate::island) fn update_browser_search(&mut self) {
        if !self.ui.cmd.open || !self.ui.cmd.input.text().starts_with('@') {
            return;
        }
        let state = &self.ui.browser;
        self.ui.cmd.items = state
            .tabs
            .iter()
            .take(cmd::MAX_ITEMS)
            .map(|tab| cmd::Item {
                icon: tab.icon(),
                glyph: '\u{E774}',
                title: tab.title.clone(),
                hint: format!("{}{} · {}", if tab.sleeping { "◌ " } else { "" }, tab.path, tab.host),
                action: cmd::Action::BrowserTab(tab.id.clone()),
            })
            .collect();
        if self.ui.cmd.items.is_empty() {
            let message = if !state.connected {
                "Waiting for Night Browser"
            } else if !state.features.tabs {
                "Enable tab search in browser integration settings"
            } else if state.searching {
                "Looking for tabs…"
            } else {
                "No matching tabs"
            };
            self.ui.cmd.items.push(cmd::Item {
                icon: None,
                glyph: '\u{E721}',
                title: crate::ui::i18n::tr(message).into_owned(),
                hint: String::new(),
                action: cmd::Action::None,
            });
        }
        self.ui.cmd.sel = self.ui.cmd.sel.min(self.ui.cmd.items.len().saturating_sub(1));
        self.repaint_expanded();
    }
    pub(in crate::island) fn click_hub(&mut self, x: i32, y: i32) {
        let Some(hit) = view::hit_hub(self.pill_rect(true), self.scale, x as f32, y as f32, &self.ui) else { return };
        match hit {
            view::HubHit::Search => self.open_browser_search(),
            view::HubHit::Request(index, action) => {
                let id = self.ui.browser.requests[index].id.clone();
                browser::hub_action("request-action", &id, ["edit", "forward", "abort"][action]);
                self.on_browser();
            }
            view::HubHit::Activity(index, action) => {
                let id = self.ui.browser.activity[index].id.clone();
                browser::hub_action("tab-action", &id, ["focus", "mute"][action]);
                self.on_browser();
            }
            view::HubHit::Pocket(index, action) => {
                let item = self.ui.hub.pocket[index].clone();
                match action {
                    0 => {
                        let url = if item.source.is_empty() { &item.url } else { &item.source };
                        if !browser::hub_action("open-link", url, "") && crate::data::pocket::web_url(url) {
                            cmd::shell_open(url, None);
                        }
                    }
                    1 => {
                        clips::sys::write_text(self.hwnd, if item.kind == "text" { &item.text } else { &item.url });
                        self.ui.hub.feedback = crate::ui::i18n::tr("Copied").into_owned();
                    }
                    2 => {
                        if let Some(store) = &self.store
                            && let Some(id) = store.note_add(None, false, &item.title)
                        {
                            store.note_set_text(id, &item.content());
                            self.reload_notes();
                            self.select_tab(view::Tab::Note);
                            self.open_note(id);
                            self.begin_edit();
                        }
                    }
                    3 if self.store.as_ref().is_some_and(|store| store.pocket_remove(&item.id)) => {
                        self.ui.hub.undo = Some(item);
                        self.reload_pocket();
                    }
                    _ => {}
                }
                self.repaint_expanded();
            }
            view::HubHit::Undo => {
                if let Some(item) = self.ui.hub.undo.clone()
                    && self.save_pocket(&item)
                {
                    self.ui.hub.undo = None;
                }
            }
        }
        #[cfg(feature = "demo")]
        demo::browser_snapshot(self);
    }
}
