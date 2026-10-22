use crate::island::*;
use crate::services::ai::ChatTurn;

const MAX_TURNS: usize = 200;

impl App {
    pub(in crate::island) fn refresh_chat(&mut self) {
        self.ui.chat.keyed = ai::has_key(self.settings.ai.provider);
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
        self.ui.chat.turns.clear();
        self.ui.chat.error = None;
        self.ui.chat.pending = false;
        self.ui.chat.rebuild();
    }

    pub(in crate::island) fn send_chat(&mut self) {
        if self.ui.chat.pending {
            return;
        }
        let text = self.ui.chat.input.text().trim().to_string();
        if text.is_empty() {
            return;
        }
        self.refresh_chat();
        if !self.ui.chat.keyed {
            return self.repaint_expanded();
        }
        self.ui.chat.error = None;
        self.ui.chat.turns.push(ChatTurn { assistant: false, text: text.clone() });
        if ai::ask_chat(self.hwnd, self.ui.chat.turns.clone()) {
            self.ui.chat.input.set_text("");
            self.ui.chat.pending = true;
        } else {
            self.ui.chat.turns.pop();
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
                self.ui.chat.turns.push(ChatTurn { assistant: true, text: text.clone() });
                let overflow = self.ui.chat.turns.len().saturating_sub(MAX_TURNS);
                self.ui.chat.turns.drain(..overflow);
                if self.state != State::Expanded || self.ui.tab != view::Tab::Chat {
                    let short: String =
                        text.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(70).collect();
                    self.notify_for(Notice::Ai, '\u{E8F2}', &format!("AI: {short}"), 7000);
                }
            }
            Err(why) => {
                if let Some(last) = self.ui.chat.turns.pop_if(|t| !t.assistant)
                    && self.ui.chat.input.is_empty()
                {
                    self.ui.chat.input.set_text(&last.text);
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
