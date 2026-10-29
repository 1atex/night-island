use std::time::Duration;

use crate::island::*;

impl App {
    fn metrics_wanted(&self) -> bool {
        let pill = view::pill_metrics_on(&self.settings);
        let tab = self.state == State::Expanded
            && self.ui.tab == view::Tab::Controls
            && !self.settings.metrics_tab.is_empty();
        let home = self.state == State::Expanded
            && self.ui.tab == view::Tab::Home
            && (self.ui.home.edit
                || self.settings.home_actions.iter().any(|&id| view::tile_needs_metrics(usize::from(id))));
        pill || tab || home
    }

    pub(in crate::island) fn sync_metrics(&mut self) {
        let every = (self.settings.metrics_secs, self.metrics_on);
        let wanted = self.metrics_wanted();
        match (wanted, every.1) {
            (true, Some(running)) if running == every.0 => {}
            (true, _) => {
                metrics::start(self.hwnd, Duration::from_secs(u64::from(every.0)));
                self.metrics_on = Some(every.0);
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
