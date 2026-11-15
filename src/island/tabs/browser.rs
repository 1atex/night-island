use crate::island::*;
use crate::services::browser;

impl App {
    pub(in crate::island) fn on_browser(&mut self) {
        self.receive_pocket();
        let snapshot = browser::snapshot();
        for item in &snapshot.items {
            if item.state == browser::State::Complete
                && self.ui.browser.items.iter().any(|old| old.id == item.id && old.state != browser::State::Complete)
            {
                self.ui.browser_done.push((item.name.clone(), std::time::Instant::now()));
                self.notify_for(Notice::Download, '\u{E73E}', &format!("Downloaded {}", item.name), 7000);
            }
        }
        self.ui.browser_done.retain(|(_, at)| at.elapsed().as_secs() < 10);
        self.ui.browser_done.truncate(32);
        let reconnect_search = snapshot.connected
            && snapshot.features.tabs
            && (!self.ui.browser.connected || !self.ui.browser.features.tabs)
            && self.ui.cmd.open
            && self.ui.cmd.input.text().starts_with('@');
        self.ui.browser = snapshot;
        if reconnect_search {
            browser::search_tabs(self.ui.cmd.input.text().trim_start_matches('@').trim());
            self.ui.browser = browser::snapshot();
        }
        self.ui.browser_scroll = self.ui.browser_scroll.min(self.ui.browser.items.len().saturating_sub(1));
        if !self.ui.browser_hover.as_ref().is_some_and(|(id, action)| {
            self.ui.browser.connected
                && !self.ui.browser.pending.contains(id)
                && self.ui.browser.items.iter().any(|item| &item.id == id && item.actions.contains(action))
        }) {
            self.ui.browser_hover = None;
        }
        self.update_eq();
        self.repaint_collapsed();
        self.update_browser_search();
        if matches!(self.ui.tab, view::Tab::Downloads | view::Tab::Browser | view::Tab::Pocket) {
            self.repaint_expanded();
        }
        self.render_settings_win();
        #[cfg(feature = "demo")]
        demo::browser_snapshot(self);
    }

    pub(in crate::island) fn click_browser(&mut self, x: i32, y: i32) {
        if let Some((index, action)) = view::hit_browser(self.pill_rect(true), self.scale, x as f32, y as f32, &self.ui)
        {
            let id = self.ui.browser.items[index].id.clone();
            if !browser::command(&id, action) {
                self.ui.browser.error = true;
                self.repaint_expanded();
            } else {
                self.on_browser();
            }
        }
    }
}
