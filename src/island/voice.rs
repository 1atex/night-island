use crate::island::*;

impl App {
    fn clear_notice(&mut self) {
        self.kill_timer(Timer::Notice);
        self.ui.notice = None;
        self.notice_link = None;
        self.update_eq();
        self.repaint_collapsed();
    }

    pub(in crate::island) fn toggle_dictation(&mut self) {
        if dictation::active() {
            dictation::stop();
            self.show_notice_for('\u{E9D9}', "Recognizing…", 60_000);
            return;
        }
        if !ai::can_transcribe(&self.settings.ai) {
            return self.notify_for(
                Notice::Error,
                '\u{E783}',
                "Dictation needs Gemini, an OpenAI-compatible service or a separate speech service",
                4000,
            );
        }
        if !ai::voice_ready(&self.settings.ai) {
            return self.notify_for(Notice::Error, '\u{E783}', "Save a key for the speech service first", 3500);
        }
        if dictation::start(self.hwnd) {
            self.show_notice_for(
                '\u{E720}',
                "Listening… press the keys again",
                (dictation::MAX_SECS as u32 + 5) * 1000,
            );
        }
    }

    pub(in crate::island) fn on_voice(&mut self) {
        match dictation::take() {
            Some(Ok(wav)) => {
                if ai::transcribe(self.hwnd, wav) {
                    self.show_notice_for('\u{E9D9}', "Recognizing…", 60_000);
                } else {
                    self.clear_notice();
                    self.notify_for(
                        Notice::Error,
                        '\u{E783}',
                        "Another request is running, try again in a moment",
                        3500,
                    );
                }
            }
            Some(Err(why)) => {
                self.clear_notice();
                self.notify_for(Notice::Error, '\u{E783}', why, 3500);
            }
            None => self.clear_notice(),
        }
    }

    pub(in crate::island) fn on_transcribed(&mut self) {
        let Some(answer) = ai::take_transcript() else { return };
        self.clear_notice();
        match answer {
            Ok(text) => {
                if self.settings.voice_paste {
                    self.clipboard.own_clip_seq = clips::sys::write_and_paste(self.hwnd, &text);
                } else {
                    clips::sys::write_text(self.hwnd, &text);
                }
                let short: String = text.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(46).collect();
                self.show_notice_for('\u{E73E}', &short, 1800);
            }
            Err(why) => {
                let short: String = why.chars().take(70).collect();
                self.notify_for(Notice::Error, '\u{E783}', &short, 4500);
            }
        }
    }
}
