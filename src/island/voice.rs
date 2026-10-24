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

    fn start_voice_command(&mut self, command: &str) {
        if command.is_empty() {
            return self.show_notice_for('\u{E720}', "Say a command after the name", 2200);
        }
        if ai::ask_command(self.hwnd, command) {
            self.show_notice_for('\u{E99A}', "Thinking…", 60_000);
        } else {
            self.notify_for(Notice::Error, '\u{E783}', "Another request is running, try again in a moment", 3500);
        }
    }

    fn open_by_voice(&mut self, name: &str) {
        let game = steam::find(name, 1).into_iter().next();
        let app = apps::find(name, 1).into_iter().next();
        let said = name.trim().to_lowercase();
        let game_exact = game.as_ref().is_some_and(|g| g.name.to_lowercase() == said);
        if let Some(g) = game.filter(|_| game_exact || app.is_none()) {
            cmd::shell_open(&steam::launch_url(g.id), None);
            return self.show_notice_for('\u{E7FC}', &g.name, 2500);
        }
        match app {
            Some(app) => {
                cmd::shell_open(&app.launch, None);
                self.show_notice_for('\u{E71D}', &app.name, 2500);
            }
            None => self.notify_for(Notice::Error, '\u{E783}', &format!("Nothing called {name} found"), 3500),
        }
    }

    pub(in crate::island) fn on_voice_command(&mut self) {
        let Some(answer) = ai::take_command_result() else { return };
        self.clear_notice();
        let reply = match answer {
            Ok(reply) => reply,
            Err(why) => {
                let short: String = why.chars().take(70).collect();
                return self.notify_for(Notice::Error, '\u{E783}', &short, 4500);
            }
        };
        match voicecmd::parse(&reply) {
            voicecmd::Command::Media(what) => {
                media::control(match what {
                    voicecmd::Media::PlayPause => media::Control::PlayPause,
                    voicecmd::Media::Next => media::Control::Next,
                    voicecmd::Media::Previous => media::Control::Previous,
                });
                self.show_notice_for('\u{E768}', "Done", 1500);
            }
            voicecmd::Command::Control(what) => {
                self.run_ctl(match what {
                    voicecmd::Toggle::Mic => view::Ctl::Mic,
                    voicecmd::Toggle::WiFi => view::Ctl::WiFi,
                    voicecmd::Toggle::Bluetooth => view::Ctl::Bluetooth,
                    voicecmd::Toggle::Theme => view::Ctl::Theme,
                    voicecmd::Toggle::Lock => view::Ctl::Lock,
                });
                self.show_notice_for('\u{E73E}', "Done", 1500);
            }
            voicecmd::Command::Remind(phrase) => match remind::parse(&phrase, chrono::Local::now()) {
                Some(parsed) => {
                    let text = if parsed.text.is_empty() { "Reminder".to_string() } else { parsed.text };
                    self.store_reminder(parsed.due, &text);
                    self.show_notice_for(
                        '\u{E916}',
                        &format!("{} · {}", remind::describe(parsed.due, chrono::Local::now()), text),
                        3500,
                    );
                }
                None => self.notify_for(Notice::Error, '\u{E783}', "No time in that command", 3500),
            },
            voicecmd::Command::Timer(secs) => {
                let due = chrono::Local::now() + chrono::Duration::seconds(i64::from(secs));
                self.store_reminder(due, &crate::ui::i18n::tr("Timer"));
                let length = view::duration_text(i64::from(secs));
                self.show_notice_for('\u{E916}', &format!("{} · {length}", crate::ui::i18n::tr("Timer")), 3500);
            }
            voicecmd::Command::Open(name) => self.open_by_voice(&name),
            voicecmd::Command::Power(what) => {
                self.pending_power = Some((what, std::time::Instant::now()));
                let (glyph, text) = match what {
                    voicecmd::Power::Shutdown => ('\u{E7E8}', "Shut down? Say the name and \"yes\""),
                    voicecmd::Power::Restart => ('\u{E777}', "Restart? Say the name and \"yes\""),
                    voicecmd::Power::Sleep => ('\u{E708}', "Sleep? Say the name and \"yes\""),
                };
                self.notify_for(Notice::Integrations, glyph, text, (voicecmd::CONFIRM_SECS * 1000) as u32);
            }
            voicecmd::Command::Confirm => {
                let fresh =
                    self.pending_power.take().filter(|(_, at)| at.elapsed().as_secs() <= voicecmd::CONFIRM_SECS);
                match fresh {
                    Some((voicecmd::Power::Shutdown, _)) => self.run_ctl(view::Ctl::PowerOff),
                    Some((voicecmd::Power::Restart, _)) => self.run_ctl(view::Ctl::Restart),
                    Some((voicecmd::Power::Sleep, _)) => self.run_ctl(view::Ctl::Sleep),
                    None => self.notify_for(Notice::Error, '\u{E783}', "There is nothing to confirm", 3000),
                }
            }
            voicecmd::Command::Cancel => {
                self.pending_power = None;
                self.show_notice_for('\u{E711}', "Cancelled", 1500);
            }
            voicecmd::Command::Answer(text) => {
                let short: String = text.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(120).collect();
                self.notify_for(Notice::Ai, '\u{E8F2}', &short, 8000);
            }
            voicecmd::Command::Unknown => {
                self.notify_for(Notice::Error, '\u{E783}', "That command is not something I can do by voice", 3500);
            }
        }
    }

    pub(in crate::island) fn on_transcribed(&mut self) {
        let Some(answer) = ai::take_transcript() else { return };
        self.clear_notice();
        match answer {
            Ok(text) => {
                let wakes = voicecmd::wake_words(&self.settings.ai.voice_wake);
                if let Some(command) = voicecmd::strip_wake(&text, &wakes) {
                    return self.start_voice_command(&command);
                }
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
