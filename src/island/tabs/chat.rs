use std::path::PathBuf;

use crate::data::chats;
use crate::island::*;
use crate::services::ai::ChatTurn;
use crate::services::attach;

const MAX_TURNS: usize = 200;

impl App {
    pub(in crate::island) fn refresh_chat(&mut self) {
        self.ui.chat.keyed = ai::has_key(self.settings.ai.provider);
        self.reload_chats();
        if !self.ui.chat.loaded {
            self.ui.chat.loaded = true;
            if let Some(first) = self.ui.chat.list.first().map(|c| c.id) {
                self.open_saved_chat(first);
            }
        }
    }

    fn reload_chats(&mut self) {
        self.ui.chat.list = self.store.as_ref().map(Store::chat_list).unwrap_or_default();
        let most = self.ui.chat.list.len().saturating_sub(view::chat_rows());
        self.ui.chat.list_scroll = self.ui.chat.list_scroll.min(most);
    }

    pub(in crate::island) fn open_ai_settings(&mut self) {
        self.open_settings_window();
        if let Some(group) = view::GROUPS.iter().position(|g| g.name == "AI") {
            self.ui.sw.group = group;
            self.ui.sw.scroll = 0.0;
            self.render_settings_win();
        }
    }

    pub(in crate::island) fn click_chat(&mut self, x: i32, y: i32) {
        let pill = self.pill_rect(true);
        match view::hit_chat(pill, self.scale, x as f32, y as f32, &self.ui) {
            Some(view::ChatHit::Setup) => return self.open_ai_settings(),
            Some(view::ChatHit::New) => self.new_chat(),
            Some(view::ChatHit::Chats) => {
                self.ui.chat.list_open = !self.ui.chat.list_open;
                if self.ui.chat.list_open {
                    self.end_edit(false);
                    self.reload_chats();
                }
            }
            Some(view::ChatHit::Attach) => self.want_chat_dialog = true,
            Some(view::ChatHit::Row(i)) => {
                if let Some(id) = self.ui.chat.list.get(self.ui.chat.list_scroll + i).map(|c| c.id) {
                    self.open_saved_chat(id);
                }
            }
            Some(view::ChatHit::RowDelete(i)) => {
                if let Some(id) = self.ui.chat.list.get(self.ui.chat.list_scroll + i).map(|c| c.id) {
                    self.delete_saved_chat(id);
                }
            }
            Some(view::ChatHit::ChipRemove(i)) => {
                if i < self.ui.chat.files.len() {
                    self.ui.chat.files.remove(i);
                }
            }
            Some(view::ChatHit::Send) => self.send_chat(),
            Some(view::ChatHit::Input) => {
                self.begin_edit();
                if let Some(layout) = self.active_layout() {
                    let (_, inner) = view::agy_input(pill, self.scale, self.ui.chat.input_h.get());
                    let shift = unsafe { GetKeyState(VK_SHIFT.0 as i32) } < 0;
                    let scroll = self.ui.chat.input.scroll;
                    self.ui.chat.input.click(&layout, x as f32 - inner.0, y as f32 - inner.1 + scroll, shift);
                }
            }
            Some(view::ChatHit::History) | None => {
                if self.ui.editing {
                    self.end_edit(false);
                }
            }
        }
        self.repaint_expanded();
    }

    fn new_chat(&mut self) {
        let chat = &mut self.ui.chat;
        chat.turns.clear();
        chat.error = None;
        chat.pending = false;
        chat.chat_id = None;
        chat.title.clear();
        chat.saved = 0;
        chat.files.clear();
        chat.list_open = false;
        chat.rebuild();
    }

    fn open_saved_chat(&mut self, id: i64) {
        let Some(store) = &self.store else { return };
        let turns = store.chat_turns(id);
        let chat = &mut self.ui.chat;
        chat.title = chat.list.iter().find(|c| c.id == id).map(|c| c.title.clone()).unwrap_or_default();
        chat.saved = turns.len();
        chat.turns = turns;
        chat.chat_id = Some(id);
        chat.error = None;
        chat.pending = false;
        chat.files.clear();
        chat.list_open = false;
        chat.rebuild();
    }

    fn delete_saved_chat(&mut self, id: i64) {
        if let Some(store) = &self.store {
            store.chat_delete(id);
        }
        if self.ui.chat.chat_id == Some(id) {
            self.new_chat();
            self.ui.chat.list_open = true;
        }
        self.reload_chats();
    }

    fn save_chat(&mut self) {
        let Some(store) = &self.store else { return };
        let chat = &mut self.ui.chat;
        if chat.saved >= chat.turns.len() {
            return;
        }
        let id = match chat.chat_id {
            Some(id) => id,
            None => {
                let title = chat
                    .turns
                    .iter()
                    .find(|t| !t.assistant)
                    .map_or_else(|| "Chat".to_string(), |t| chats::title_of(&t.text, &t.attachments));
                let Some(id) = store.chat_create(&title) else { return };
                chat.chat_id = Some(id);
                chat.title = title;
                id
            }
        };
        for turn in &chat.turns[chat.saved..] {
            store.chat_add(id, turn);
        }
        chat.saved = chat.turns.len();
        self.reload_chats();
    }

    pub(in crate::island) fn attach_files(&mut self, paths: Vec<PathBuf>) {
        for path in paths {
            if self.ui.chat.files.len() >= attach::MAX_FILES {
                self.notify(Notice::Error, '\u{E783}', "Up to 4 files go with one message");
                break;
            }
            match attach::load(&path) {
                Ok(file) => self.ui.chat.files.push(file),
                Err(why) => self.notify(Notice::Error, '\u{E783}', why),
            }
        }
        self.repaint_expanded();
    }

    pub(in crate::island) fn finish_chat_dialog(&mut self, picked: Option<String>) {
        self.popup_open = false;
        self.pointer.tracking = false;
        if let Some(path) = picked {
            self.attach_files(vec![PathBuf::from(path)]);
        }
        self.repaint_expanded();
        if !self.cursor_inside() {
            self.set_timer(Timer::Collapse, self.settings.close_delay_ms.max(1));
        }
    }

    pub(in crate::island) fn send_chat(&mut self) {
        if self.ui.chat.pending {
            return;
        }
        let text = self.ui.chat.input.text().trim().to_string();
        if text.is_empty() && self.ui.chat.files.is_empty() {
            return;
        }
        self.ui.chat.keyed = ai::has_key(self.settings.ai.provider);
        if !self.ui.chat.keyed {
            return self.repaint_expanded();
        }
        self.ui.chat.error = None;
        let attachments = std::mem::take(&mut self.ui.chat.files);
        self.ui.chat.turns.push(ChatTurn { assistant: false, text, attachments });
        if ai::ask_chat(self.hwnd, self.ui.chat.turns.clone()) {
            self.ui.chat.input.set_text("");
            self.ui.chat.pending = true;
        } else {
            if let Some(turn) = self.ui.chat.turns.pop() {
                self.ui.chat.files = turn.attachments;
            }
            self.ui.chat.error = Some("Another request is running, try again in a moment".into());
        }
        self.ui.chat.rebuild();
        self.repaint_expanded();
    }

    pub(in crate::island) fn on_chat(&mut self) {
        let Some(answer) = ai::take_chat_result() else { return };
        if !self.ui.chat.pending {
            return;
        }
        self.ui.chat.pending = false;
        match answer {
            Ok(text) => {
                self.ui.chat.turns.push(ChatTurn::text(true, text.clone()));
                self.save_chat();
                let chat = &mut self.ui.chat;
                let overflow = chat.turns.len().saturating_sub(MAX_TURNS);
                chat.turns.drain(..overflow);
                chat.saved = chat.saved.saturating_sub(overflow);
                if self.state != State::Expanded || self.ui.tab != view::Tab::Chat {
                    let short: String =
                        text.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(70).collect();
                    self.notify_for(Notice::Ai, '\u{E8F2}', &format!("AI: {short}"), 7000);
                }
            }
            Err(why) => {
                if let Some(last) = self.ui.chat.turns.pop_if(|t| !t.assistant) {
                    if self.ui.chat.input.is_empty() {
                        self.ui.chat.input.set_text(&last.text);
                    }
                    if self.ui.chat.files.is_empty() {
                        self.ui.chat.files = last.attachments;
                    }
                }
                self.ui.chat.error = Some(why);
            }
        }
        self.ui.chat.rebuild();
        if self.state == State::Expanded && self.ui.tab == view::Tab::Chat {
            self.repaint_expanded();
        }
    }
}
