use crate::island::*;
use crate::services::browser;

impl App {
    pub(in crate::island) fn on_browser(&mut self) {
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
        self.ui.browser = snapshot;
        self.ui.browser_scroll = self.ui.browser_scroll.min(self.ui.browser.items.len().saturating_sub(1));
        self.ui.browser_hover = None;
        self.update_eq();
        self.repaint_collapsed();
        if self.ui.tab == view::Tab::Downloads {
            self.repaint_expanded();
        }
        self.render_settings_win();
    }

    pub(in crate::island) fn click_browser(&mut self, x: i32, y: i32) {
        if let Some((index, action)) = view::hit_browser(self.pill_rect(true), self.scale, x as f32, y as f32, &self.ui)
        {
            let id = self.ui.browser.items[index].id.clone();
            if !browser::command(&id, action) {
                self.ui.browser.error = true;
                self.repaint_expanded();
            }
        }
    }
}
