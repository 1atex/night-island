use crate::island::*;
use crate::services::ai::ChatTurn;
use crate::ui::mini_chat::MiniChatWindow;

pub(in crate::island) struct CompanionChat {
    window: MiniChatWindow,
    turns: Vec<ChatTurn>,
    pub(in crate::island) pending: bool,
    error: Option<String>,
}

impl CompanionChat {
    pub(in crate::island) fn repaint(&mut self) {
        self.window.repaint();
    }

    pub(in crate::island) fn set_capture_hidden(&self, hidden: bool) {
        self.window.set_capture_hidden(hidden);
    }

    fn refresh(&mut self, provider: &str) {
        self.window.update(&self.turns, self.error.as_deref(), self.pending, provider);
    }
}

impl App {
    pub(in crate::island) fn open_companion_chat(&mut self) {
        let Some(desktop) = &self.pet.desktop else { return };
        let anchor = desktop.center();
        self.end_edit(true);
        if self.pet.chat.is_none() {
            match MiniChatWindow::new(self.hwnd, self.dpi_scale, &self.gpu) {
                Ok(window) => {
                    self.pet.chat = Some(CompanionChat { window, turns: Vec::new(), pending: false, error: None })
                }
                Err(error) => {
                    crate::log_warn!("could not open companion chat: {error}");
                    return;
                }
            }
        }
        if let Some(chat) = &mut self.pet.chat {
            chat.window.set_capture_hidden(self.settings.capture_hidden);
            chat.refresh(&format!(
                "{} · {}",
                self.settings.ai.provider.name(),
                self.settings.ai.model(self.settings.ai.provider)
            ));
            chat.window.show(anchor, self.settings.character);
        }
    }

    pub(in crate::island) fn on_mini_chat(&mut self, action: usize, source: isize) {
        let Some(chat) = &mut self.pet.chat else { return };
        if chat.window.hwnd().0 as isize != source {
            return;
        }
        if action == crate::ui::mini_chat::SETUP {
            return self.open_ai_settings();
        }
        if action != crate::ui::mini_chat::SEND || chat.pending {
            return;
        }
        let text = chat.window.input().trim().to_string();
        if text.is_empty() {
            return;
        }
        if !ai::has_key(self.settings.ai.provider) {
            chat.error = Some(crate::ui::i18n::tr("Connect your AI provider in AI settings.").into_owned());
            chat.refresh(&format!(
                "{} · {}",
                self.settings.ai.provider.name(),
                self.settings.ai.model(self.settings.ai.provider)
            ));
            return;
        }
        chat.turns.push(ChatTurn::text(false, text));
        if ai::ask_chat(self.hwnd, chat.turns.clone()) {
            chat.pending = true;
            chat.error = None;
            chat.window.clear_input();
        } else {
            chat.turns.pop();
            chat.error = Some(crate::ui::i18n::tr("Another request is running, try again in a moment").into_owned());
        }
        chat.refresh(&format!(
            "{} · {}",
            self.settings.ai.provider.name(),
            self.settings.ai.model(self.settings.ai.provider)
        ));
    }

    pub(in crate::island) fn finish_companion_chat(&mut self, answer: std::result::Result<String, String>) {
        let Some(chat) = &mut self.pet.chat else { return };
        chat.pending = false;
        match answer {
            Ok(text) => {
                chat.turns.push(ChatTurn::text(true, text));
                chat.error = None;
            }
            Err(error) => {
                if let Some(turn) = chat.turns.pop_if(|turn| !turn.assistant)
                    && chat.window.input().is_empty()
                {
                    chat.window.set_input(&turn.text);
                }
                chat.error = Some(error);
            }
        }
        let overflow = chat.turns.len().saturating_sub(100);
        chat.turns.drain(..overflow);
        chat.refresh(&format!(
            "{} · {}",
            self.settings.ai.provider.name(),
            self.settings.ai.model(self.settings.ai.provider)
        ));
    }
}
