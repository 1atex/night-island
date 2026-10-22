use std::time::Instant;

use crate::island::*;

impl App {
    pub(in crate::island) fn init_steam(&mut self) {
        self.steam_game = steam::running_app().map(|id| (id, Instant::now()));
    }

    pub(in crate::island) fn on_steam(&mut self) {
        let running = steam::running_app();
        if running == self.steam_game.map(|(id, _)| id) {
            return;
        }
        if let Some((id, since)) = self.steam_game.take() {
            let name = steam::name_of(id).unwrap_or_else(|| format!("#{id}"));
            let played = view::duration_text(since.elapsed().as_secs() as i64);
            let text = format!("{} {name} · {played}", crate::ui::i18n::tr("Played"));
            self.notify_for(Notice::Steam, '\u{E7FC}', &text, 5000);
        }
        if let Some(id) = running {
            self.steam_game = Some((id, Instant::now()));
            let name = steam::name_of(id).unwrap_or_else(|| format!("#{id}"));
            let text = format!("{} {name}", crate::ui::i18n::tr("Playing"));
            self.notify_for(Notice::Steam, '\u{E7FC}', &text, 4000);
        }
    }
}
