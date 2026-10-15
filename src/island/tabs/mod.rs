use crate::island::*;

mod agy;
mod clipboard;
mod command;
mod controls;
mod home;
mod integrations;
mod note;
mod record;
mod remind;
mod settings;
mod shelf;
mod text;
mod weather;

pub(in crate::island) use self::clipboard::ClipboardState;

impl App {
    pub(in crate::island) fn select_tab(&mut self, tab: view::Tab) {
        if tab != self.ui.tab || !matches!(tab, view::Tab::Note | view::Tab::Text | view::Tab::Remind) {
            self.end_edit(true);
        }
        self.ui.notes.armed_delete = false;
        if tab == view::Tab::Remind && tab != self.ui.tab {
            self.reset_picker();
            self.ui.remind.status.clear();
        }
        self.ui.tab = tab;
        self.ui.home.dev_menu = None;
        self.ui.text.qr = None;
        self.refresh_reminder_labels();
        if tab == view::Tab::Text
            && self.ui.text.input.is_empty()
            && let Some(text) = clips::sys::read_text(self.hwnd)
        {
            self.ui.text.input.set_text(&text);
        }
        if tab == view::Tab::Controls {
            self.ui.dark = system::dark_theme();
            system::radios_async(self.hwnd, None);
        }
        if tab == view::Tab::Home {
            self.refresh_home();
            spotify::refresh();
        }
        if tab == view::Tab::Integrations {
            spotify::reload_status();
        }
        if tab == view::Tab::Agy {
            self.reload_agy();
        }
        self.repaint_expanded();
    }

    pub(in crate::island) fn open_tab(&mut self, tab: view::Tab) {
        if self.ui.tab != tab {
            self.select_tab(tab);
        }
        self.peek();
    }
}
