use crate::island::*;

impl App {
    pub(in crate::island) fn on_discord(&mut self) {
        if demo::active("NI_DEBUG_OVERLAY") {
            return;
        }
        let now = discord::state();
        if now == self.ui.discord {
            return;
        }
        let new_note = now.note_seq != self.ui.discord.note_seq;
        let (title, body) = now.note.clone();
        self.ui.discord = now;
        if new_note {
            let text = if body.is_empty() { title } else { format!("{title}: {body}") };
            self.notify_for(Notice::Integrations, '\u{E8BD}', &text, 5000);
        }
        self.sync_overlay();
        self.render_settings_win();
        if self.ui.tab == view::Tab::Home {
            self.repaint_expanded();
        }
    }

    pub(in crate::island) fn sync_overlay(&mut self) {
        let d = &self.ui.discord;
        let want = self.settings.discord_overlay && d.status == discord::Status::Connected && !d.members.is_empty();
        if !want {
            self.overlay = None;
            return;
        }
        let size = view::overlay_size(self.scale, d.members.len());
        if self.overlay.is_none() {
            use windows::Win32::UI::WindowsAndMessaging::{
                GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
            };
            let (vx, vy, vw, vh) = unsafe {
                (
                    GetSystemMetrics(SM_XVIRTUALSCREEN),
                    GetSystemMetrics(SM_YVIRTUALSCREEN),
                    GetSystemMetrics(SM_CXVIRTUALSCREEN),
                    GetSystemMetrics(SM_CYVIRTUALSCREEN),
                )
            };
            let (mut x, mut y) = self.settings.overlay_pos.map_or((vx + vw - size.0 - 24, vy + 90), |p| (p[0], p[1]));
            x = x.clamp(vx, (vx + vw - 60).max(vx));
            y = y.clamp(vy, (vy + vh - 60).max(vy));
            self.overlay = overlay::create(self.hwnd, (x, y), size, self.scale);
            if demo::active("NI_DEBUG_OVERLAY") {
                let _ = std::fs::write(
                    std::env::temp_dir().join("ni_overlay_at.txt"),
                    format!("{x},{y} {size:?} created={}", self.overlay.is_some()),
                );
            }
        }
        if let Some(o) = &self.overlay {
            o.set_scale(self.scale);
            let (w, h) = (size.0 as f32, size.1 as f32);
            let scale = self.scale;
            let discord = &self.ui.discord;
            let r = o.render(&self.gpu, size, |p| {
                let _ = view::paint_discord_overlay(p, (w, h), scale, discord);
            });
            if let (Err(e), true) = (&r, demo::active("NI_DEBUG_OVERLAY")) {
                let _ = std::fs::write(std::env::temp_dir().join("ni_overlay_err.txt"), format!("{e:?}"));
            }
        }
    }

    pub(in crate::island) fn toggle_overlay(&mut self) {
        self.settings.discord_overlay = !self.settings.discord_overlay;
        self.ui.cfg = self.settings.clone();
        self.settings.save();
        self.sync_overlay();
        self.repaint_expanded();
    }

    pub(in crate::island) fn on_overlay(&mut self, code: usize) {
        match code {
            1 => self.toggle_overlay(),
            2 => discord::set_mute(!self.ui.discord.self_mute),
            3 => discord::set_deaf(!self.ui.discord.self_deaf),
            10 => {
                if let Some(o) = &self.overlay {
                    let (x, y) = o.position();
                    self.settings.overlay_pos = Some([x, y]);
                    self.settings.save();
                }
            }
            _ => {}
        }
    }

    pub(in crate::island) fn on_update(&mut self) {
        let now = update::state();
        if let update::Status::Available(found) = &now
            && self.ui.update != now
        {
            let text = format!("{} {}", crate::ui::i18n::tr("New version available"), found.version);
            self.notify_for(Notice::Update, '\u{E895}', &text, 6000);
        }
        self.ui.update = now;
        self.render_settings_win();
    }

    pub(in crate::island) fn on_spotify(&mut self) {
        let now = spotify::state();
        if now == self.ui.spotify {
            return;
        }
        self.ui.spotify = now;
        self.render_settings_win();
        if self.ui.tab == view::Tab::Home {
            self.repaint_expanded();
        }
    }

    pub(in crate::island) fn save_credentials(&mut self) {
        if discord::set_credentials(&self.ui.int.id.text(), &self.ui.int.secret.text()) {
            self.ui.int.id.set_text("");
            self.ui.int.secret.set_text("");
            self.end_edit(false);
        }
    }
}
