use crate::island::*;
use crate::platform::{sounds, speech};
use crate::services::assistant::{Cap, Level, memory};

impl App {
    fn cue(&self, cue: sounds::Cue) {
        sounds::play(cue, self.settings.assistant.cue_volume());
    }

    fn fail_by_voice(&mut self, text: &str) {
        self.cue(sounds::Cue::Error);
        self.notify_for(Notice::Error, '\u{E783}', text, 4000);
    }

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
            self.cue(sounds::Cue::Done);
            self.show_notice_for('\u{F801}', "Recognizing…", 60_000);
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
            self.cue(sounds::Cue::Heard);
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
                    self.show_notice_for('\u{F801}', "Recognizing…", 60_000);
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
        let perms = &self.settings.assistant.perms;
        let notes = if perms.command_level(Cap::Memory, false) == Level::Off {
            String::new()
        } else {
            memory::prompt_block(&memory::load(), 1500)
        };
        let system = voicecmd::system_prompt(&perms.commands_offered(), &notes);
        if ai::ask_command(self.hwnd, command, system) {
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
        self.handle_command(voicecmd::parse(&reply), false);
    }

    fn handle_command(&mut self, command: voicecmd::Command, confirmed: bool) {
        if let Some(cap) = voicecmd::cap_of(&command) {
            match self.settings.assistant.perms.command_level(cap, false) {
                Level::Off => {
                    let text = format!("{}: off in the Assistant settings", crate::ui::i18n::tr(cap.title()));
                    return self.fail_by_voice(&text);
                }
                Level::Ask if !confirmed && !matches!(command, voicecmd::Command::Power(_)) => {
                    let text = format!("{}? Say the name and \"yes\"", crate::ui::i18n::tr(cap.title()));
                    self.pending_ask = Some((command, std::time::Instant::now()));
                    return self.notify_for(
                        Notice::Integrations,
                        '\u{E897}',
                        &text,
                        (voicecmd::CONFIRM_SECS * 1000) as u32,
                    );
                }
                _ => {}
            }
        }
        match command {
            voicecmd::Command::Remember(text) => {
                let mut notes = memory::load();
                if memory::add(&mut notes, &text) && memory::save(&notes).is_ok() {
                    self.show_notice_for('\u{E73E}', "Remembered", 1800);
                } else {
                    self.show_notice_for('\u{E711}', "Already known", 1800);
                }
            }
            voicecmd::Command::Forget(what) => {
                let mut notes = memory::load();
                let dropped = memory::forget(&mut notes, &what);
                if dropped > 0 && memory::save(&notes).is_ok() {
                    self.show_notice_for('\u{E74D}', &format!("Forgot {dropped}"), 1800);
                } else {
                    self.show_notice_for('\u{E711}', "Nothing like that in memory", 2200);
                }
            }
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
                let asked = self.pending_ask.take().filter(|(_, at)| at.elapsed().as_secs() <= voicecmd::CONFIRM_SECS);
                if let Some((command, _)) = asked {
                    return self.handle_command(command, true);
                }
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
                self.pending_ask = None;
                self.show_notice_for('\u{E711}', "Cancelled", 1500);
            }
            voicecmd::Command::Answer(text) => {
                let short: String = text.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(120).collect();
                self.notify_for(Notice::Ai, '\u{E8F2}', &short, 8000);
                speech::speak(&text, self.settings.assistant.voice_volume());
            }
            voicecmd::Command::Unknown => self.fail_by_voice("That command is not something I can do by voice"),
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
