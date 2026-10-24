use std::time::Instant;

use crate::island::*;
use crate::view::LibGame;

impl App {
    pub(in crate::island) fn steam_search(&self) -> bool {
        self.ui.tab == view::Tab::Steam && !self.ui.cmd.open && self.ui.editing
    }

    pub(in crate::island) fn reload_steam(&mut self) {
        steam::refresh();
        let accounts = steam::accounts();
        let active = steam::active_account_id();
        let owner = active.or_else(|| accounts.first().map(steam::Account::account_id));
        let local = owner.map(steam::local_config).unwrap_or_default();
        let mut games: Vec<LibGame> = steam::installed()
            .into_iter()
            .map(|g| {
                let p = local.played.get(&g.id).copied().unwrap_or_default();
                LibGame { id: g.id, name: g.name, last: p.last, minutes: p.minutes, update: g.update }
            })
            .collect();
        games.sort_by(|a, b| b.last.cmp(&a.last).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
        let mut art: Vec<steam_art::ArtKey> = games.iter().map(|g| steam_art::ArtKey::Game(g.id)).collect();
        art.extend(accounts.iter().map(|a| steam_art::ArtKey::Avatar(a.id64)));
        let s = &mut self.ui.steam;
        s.all = games;
        s.accounts = accounts;
        s.active = active;
        s.status = local.status;
        s.steam_up = steam::is_running();
        self.apply_steam_filter();
        self.sync_steam_running();
        steam_art::load(self.hwnd, &art);
        self.arm_steam_poll();
    }

    pub(in crate::island) fn apply_steam_filter(&mut self) {
        let query = self.ui.steam.query.text().to_lowercase();
        let words: Vec<&str> = query.split_whitespace().collect();
        let rows = view::steam_rows(self.pill_rect(true), self.scale);
        let s = &mut self.ui.steam;
        s.games = if words.is_empty() {
            s.all.clone()
        } else {
            let mut hits: Vec<(bool, &LibGame)> = s
                .all
                .iter()
                .filter_map(|g| {
                    let name = g.name.to_lowercase();
                    words.iter().all(|w| name.contains(w)).then(|| (name.starts_with(words[0]), g))
                })
                .collect();
            hits.sort_by_key(|(starts, _)| !starts);
            hits.into_iter().map(|(_, g)| g.clone()).collect()
        };
        s.scroll = if words.is_empty() { s.scroll.min(s.games.len().saturating_sub(rows)) } else { 0 };
    }

    pub(in crate::island) fn launch_first_steam_game(&mut self) {
        if let Some(game) = self.ui.steam.games.first().cloned() {
            self.launch_steam_game(&game);
        }
    }

    fn launch_steam_game(&mut self, game: &LibGame) {
        cmd::shell_open(&steam::launch_url(game.id), None);
        self.show_notice_for('\u{E7FC}', &game.name, 2500);
        self.collapse();
    }

    fn arm_steam_poll(&self) {
        if self.ui.steam.all.iter().any(|g| g.update == steam::Update::Running) {
            self.set_timer(Timer::SteamPoll, 4000);
        }
    }

    pub(in crate::island) fn on_steam_poll(&mut self) {
        if self.state != State::Expanded || self.ui.tab != view::Tab::Steam {
            return;
        }
        steam::refresh();
        let installed = steam::installed();
        for game in self.ui.steam.all.iter_mut() {
            if let Some(now) = installed.iter().find(|g| g.id == game.id) {
                game.update = now.update;
            }
        }
        self.apply_steam_filter();
        self.arm_steam_poll();
        self.repaint_expanded();
    }

    pub(in crate::island) fn on_steam_art(&mut self) {
        if self.state == State::Expanded && self.ui.tab == view::Tab::Steam {
            self.repaint_expanded();
        }
    }

    fn sync_steam_running(&mut self) {
        self.ui.steam.running =
            self.steam_game.map(|(id, since)| (id, steam::name_of(id).unwrap_or_else(|| format!("#{id}")), since));
    }

    pub(in crate::island) fn click_steam(&mut self, x: i32, y: i32) {
        let pill = self.pill_rect(true);
        let hit = view::hit_steam(pill, self.scale, x as f32, y as f32, &self.ui.steam);
        if let Some(view::SteamHit::Search) = hit {
            self.begin_edit();
            if let Some(layout) = self.active_layout() {
                let (_, inner) = view::steam_search_frame(pill, self.scale);
                let shift = unsafe { GetKeyState(VK_SHIFT.0 as i32) } < 0;
                self.ui.steam.query.click(&layout, x as f32 - inner.0, 4.0, shift);
            }
            return self.repaint_expanded();
        }
        if self.ui.editing {
            self.end_edit(true);
        }
        match hit {
            Some(view::SteamHit::Link(i)) => {
                cmd::shell_open(view::STEAM_LINKS[i].1, None);
                self.collapse();
            }
            Some(view::SteamHit::Start) => {
                cmd::shell_open("steam://open/main", None);
                self.ui.steam.steam_up = true;
                self.repaint_expanded();
            }
            Some(view::SteamHit::Game(i)) => {
                let Some(game) = self.ui.steam.games.get(self.ui.steam.scroll + i).cloned() else { return };
                self.launch_steam_game(&game);
            }
            Some(view::SteamHit::Update(i)) => self.update_steam_game(i),
            Some(view::SteamHit::Status(k)) => self.set_steam_status(k),
            Some(view::SteamHit::Account(i)) => self.switch_steam_account(i),
            Some(view::SteamHit::Search) | None => {}
        }
    }

    fn update_steam_game(&mut self, row: usize) {
        let Some(game) = self.ui.steam.games.get(self.ui.steam.scroll + row).cloned() else { return };
        cmd::shell_open(&steam::update_url(game.id), None);
        for g in self.ui.steam.all.iter_mut().chain(self.ui.steam.games.iter_mut()).filter(|g| g.id == game.id) {
            g.update = steam::Update::Running;
        }
        let text = format!("{} {}", crate::ui::i18n::tr("Updating"), game.name);
        self.notify_for(Notice::Steam, '\u{E7FC}', &text, 3000);
        self.arm_steam_poll();
        self.repaint_expanded();
    }

    fn set_steam_status(&mut self, index: usize) {
        let Some(status) = steam::Status::ALL.get(index).copied() else { return };
        if !self.ui.steam.steam_up {
            return self.notify(Notice::Steam, '\u{E783}', "Steam is not running");
        }
        cmd::shell_open(status.url(), None);
        self.ui.steam.status = Some(status);
        self.repaint_expanded();
    }

    fn switch_steam_account(&mut self, index: usize) {
        let Some(account) = self.ui.steam.accounts.get(index).cloned() else { return };
        if self.ui.steam.active == Some(account.account_id()) {
            return self.notify(Notice::Steam, '\u{E73E}', "Steam is already signed in with this account");
        }
        if !account.remembered {
            return self.notify(
                Notice::Error,
                '\u{E783}',
                "Steam does not remember that account: sign in to it once in Steam",
            );
        }
        if steam::switching() {
            return;
        }
        if self.steam_game.is_some() && self.ui.steam.armed.as_deref() != Some(account.name.as_str()) {
            self.ui.steam.armed = Some(account.name.clone());
            self.set_timer(Timer::Disarm, 3000);
            return self.repaint_expanded();
        }
        self.ui.steam.armed = None;
        if steam::switch_account(self.hwnd, &account.name) {
            self.ui.steam.switching = Some(account.persona.clone());
            let text = format!("{} {}…", crate::ui::i18n::tr("Switching Steam to"), account.persona);
            self.show_notice_for('\u{E7FC}', &text, 60_000);
            self.repaint_expanded();
        }
    }

    pub(in crate::island) fn on_steam_switch(&mut self) {
        let Some(result) = steam::take_switch_result() else { return };
        self.ui.steam.switching = None;
        self.kill_timer(Timer::Notice);
        self.ui.notice = None;
        match result {
            Ok(persona) => {
                let text = format!("{} {persona}", crate::ui::i18n::tr("Steam is signing in as"));
                self.notify_for(Notice::Steam, '\u{E73E}', &text, 5000);
            }
            Err(why) => self.notify_for(Notice::Error, '\u{E783}', why, 5000),
        }
        self.update_eq();
        self.repaint_collapsed();
        if self.state == State::Expanded && self.ui.tab == view::Tab::Steam {
            self.reload_steam();
            self.repaint_expanded();
        }
    }

    pub(in crate::island) fn init_steam(&mut self) {
        self.steam_game = steam::running_app().map(|id| (id, Instant::now()));
        self.sync_steam_running();
    }

    pub(in crate::island) fn on_steam(&mut self) {
        let running = steam::running_app();
        if running == self.steam_game.map(|(id, _)| id) {
            return;
        }
        let steam_open = self.state == State::Expanded && self.ui.tab == view::Tab::Steam;
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
        if steam_open {
            self.reload_steam();
            self.repaint_expanded();
        } else {
            self.sync_steam_running();
        }
    }
}
