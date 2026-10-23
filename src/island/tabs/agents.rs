use std::time::{SystemTime, UNIX_EPOCH};

use crate::island::*;
use crate::services::agents::State;

impl App {
    pub(in crate::island) fn on_agent(&mut self) {
        let now = agents::sessions();
        let clock = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
        let tr = |s: &str| crate::ui::i18n::tr(s).into_owned();
        for s in &now {
            let before = self.ui.agents.iter().find(|b| b.agent == s.agent && b.id == s.id).map(|b| b.state);
            if before == Some(s.state) {
                continue;
            }
            let who = format!("{} · {}", s.agent.name(), s.project);
            match s.state {
                State::Waiting => {
                    self.notify_for(Notice::Agents, '\u{E7BA}', &format!("{who}: {}", tr("waiting for you")), 7000);
                }
                State::Done if before != Some(State::Done) && clock - s.at < 20 => {
                    self.notify_for(Notice::Agents, '\u{E73E}', &format!("{who}: {}", tr("finished")), 5000);
                }
                _ => {}
            }
        }
        self.ui.agents = now;
        self.update_eq();
        self.repaint_collapsed();
        if self.settings_win.is_some() {
            self.render_settings_win();
        }
    }
}
