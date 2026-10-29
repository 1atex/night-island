use std::time::Duration;

use crate::island::*;

impl App {
    fn metrics_tab_on(&self) -> bool {
        !self.settings.metrics_tab.is_empty() && view::tab_in_bar(&self.settings, view::Tab::Controls, "Controls")
    }

    fn metrics_wanted(&self) -> bool {
        let pill = view::pill_metrics_on(&self.settings);
        let home = self.state == State::Expanded
            && self.ui.tab == view::Tab::Home
            && (self.ui.home.edit
                || self.settings.home_actions.iter().any(|&id| view::tile_needs_metrics(usize::from(id))));
        pill || self.metrics_tab_on() || home
    }

    fn metrics_every(&self) -> u32 {
        let every = self.settings.metrics_secs.max(1);
        let live = view::pill_metrics_on(&self.settings)
            || (self.state == State::Expanded && matches!(self.ui.tab, view::Tab::Controls | view::Tab::Home));
        if live { every } else { (every * 3).max(6) }
    }

    pub(in crate::island) fn sync_metrics(&mut self) {
        let every = self.metrics_every();
        match (self.metrics_wanted(), self.metrics_on) {
            (true, Some(running)) if running == every => {}
            (true, _) => {
                metrics::start(self.hwnd, Duration::from_secs(u64::from(every)));
                self.metrics_on = Some(every);
            }
            (false, Some(_)) => {
                metrics::stop();
                self.metrics_on = None;
                self.ui.metrics.clear();
                self.repaint_collapsed();
            }
            (false, None) => {}
        }
    }

    pub(in crate::island) fn on_metrics(&mut self) {
        if self.metrics_on.is_none() {
            return;
        }
        let Some(sample) = metrics::latest() else { return };
        self.ui.metrics.push(sample, &self.settings.metrics_pill);
        if view::pill_metrics_on(&self.settings) {
            self.repaint_collapsed();
        }
        if self.state == State::Expanded && matches!(self.ui.tab, view::Tab::Controls | view::Tab::Home) {
            self.repaint_expanded();
        }
    }
}
