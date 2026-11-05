use crate::island::*;
use crate::ui::pet_speech::VoicePhase;

impl App {
    fn voice_phase(&mut self, phase: VoicePhase) {
        self.pet.voice_phase = phase;
        self.pet.char_key = None;
        self.set_timer(Timer::Char, 1);
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
            self.voice_phase(VoicePhase::Transcribing);
            self.show_notice_for('\u{F801}', "Recognizing…", 60_000);
            return;
        }
        if self.pet.voice_phase.pending() || ai::busy() {
            self.notify_for(Notice::Error, '\u{E783}', "Another request is running, try again in a moment", 2500);
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
            self.voice_phase(VoicePhase::Recording);
            self.show_notice_for(
                '\u{E720}',
                "Listening… press the keys again",
                (dictation::MAX_SECS as u32 + 5) * 1000,
            );
        }
    }

    pub(in crate::island) fn on_voice(&mut self) {
        self.finish_recording(dictation::take());
    }

    fn finish_recording(&mut self, outcome: Option<std::result::Result<Vec<u8>, &'static str>>) {
        self.voice_phase(VoicePhase::Idle);
        match outcome {
            Some(Ok(wav)) => {
                if ai::transcribe(self.hwnd, wav) {
                    self.voice_phase(VoicePhase::Transcribing);
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
        if ai::ask_command(self.hwnd, command) {
            self.voice_phase(VoicePhase::Command);
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
        self.finish_voice_command(answer);
    }

    fn finish_voice_command(&mut self, answer: ai::Answer) {
        self.voice_phase(VoicePhase::Idle);
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
        self.finish_transcription(answer);
    }

    fn finish_transcription(&mut self, answer: ai::Answer) {
        self.voice_phase(VoicePhase::Idle);
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

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::System::WinRT::{
        CreateDispatcherQueueController, DQTAT_COM_NONE, DQTYPE_THREAD_CURRENT, DispatcherQueueOptions,
        RO_INIT_SINGLETHREADED, RoInitialize, RoUninitialize,
    };

    #[test]
    #[ignore = "Uses an isolated native island with synthetic voice results; no microphone, clipboard or provider access"]
    fn native_voice_failures_and_answers_release_the_pet_and_allow_retry() -> Result<()> {
        unsafe {
            RoInitialize(RO_INIT_SINGLETHREADED)?;
        }
        struct Runtime;
        impl Drop for Runtime {
            fn drop(&mut self) {
                unsafe { RoUninitialize() };
            }
        }
        let _runtime = Runtime;
        let _queue = unsafe {
            CreateDispatcherQueueController(DispatcherQueueOptions {
                dwSize: size_of::<DispatcherQueueOptions>() as u32,
                threadType: DQTYPE_THREAD_CURRENT,
                apartmentType: DQTAT_COM_NONE,
            })?
        };
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                w!("STATIC"),
                w!("Voice lifecycle fixture"),
                WS_POPUP,
                -12000,
                -12000,
                1,
                1,
                None,
                None,
                Some(GetModuleHandleW(None)?.into()),
                None,
            )?
        };
        struct Window(HWND);
        impl Drop for Window {
            fn drop(&mut self) {
                unsafe {
                    let _ = DestroyWindow(self.0);
                }
            }
        }
        let _window = Window(hwnd);
        let settings = Settings { music_eq: false, edge_spikes: false, ..Default::default() };
        let mut app = App::new_visuals(hwnd, 1.0, settings)?;
        app.voice_phase(VoicePhase::Recording);
        assert!(app.pet.voice_phase.cue().engaged());
        app.finish_recording(Some(Err("Nothing was heard (is the microphone muted?)")));
        assert_eq!(app.pet.voice_phase, VoicePhase::Idle);
        app.voice_phase(VoicePhase::Transcribing);
        app.toggle_dictation();
        assert_eq!(app.pet.voice_phase, VoicePhase::Transcribing);
        assert!(!dictation::active());
        app.finish_transcription(Err("No connection to the service".into()));
        assert_eq!(app.pet.voice_phase, VoicePhase::Idle);
        for answer in [
            Err("Request timed out".into()),
            Ok("not JSON".into()),
            Ok(r#"{"action":"answer","text":"Привет!"}"#.into()),
            Ok(r#"{"action":"cancel"}"#.into()),
        ] {
            app.voice_phase(VoicePhase::Command);
            app.finish_voice_command(answer);
            assert_eq!(app.pet.voice_phase, VoicePhase::Idle);
            assert!(!app.pet.voice_phase.cue().engaged());
            assert!(app.pet.char_key.is_none());
        }
        app.voice_phase(VoicePhase::Recording);
        app.finish_recording(None);
        assert_eq!(app.pet.voice_phase, VoicePhase::Idle);
        Ok(())
    }
}
