use crate::island::*;

impl App {
    pub(in crate::island) fn toggle_recording(&mut self) {
        if self.recording.is_some() {
            self.stop_recording();
        } else {
            self.capture(capture::Mode::Record);
        }
    }

    pub(in crate::island) fn begin_recording(&mut self, rect: windows::Win32::Foundation::RECT) {
        if self.settings.capture_self_hidden && !demo::active("NI_KEEP_VISIBLE") {
            unsafe {
                let _ = SetWindowDisplayAffinity(self.hwnd, WDA_EXCLUDEFROMCAPTURE);
            }
        }
        let opts = recorder::Options {
            format: self.settings.record_format,
            fps: self.settings.record_fps,
            system_audio: self.settings.record_system_audio,
            mic: self.settings.record_mic,
        };
        self.recording = Some(recorder::start(self.hwnd, rect, recorder::default_path(opts.format), opts));
        self.ui.recording_secs = Some(0);
        self.repaint_expanded();
        self.set_timer(Timer::Rec, 1000);
        self.update_eq();
        self.repaint_collapsed();
    }

    pub(in crate::island) fn stop_recording(&mut self) {
        if let Some(r) = &self.recording {
            r.stop();
            self.kill_timer(Timer::Rec);
            self.show_notice('\u{E714}', "Saving the recording…");
        }
    }

    pub(in crate::island) fn on_rec_timer(&mut self) {
        if let Some(r) = &self.recording {
            self.ui.recording_secs = Some(r.started.elapsed().as_secs() as u32);
            self.repaint_collapsed();
            self.set_timer(Timer::Rec, 1000);
        }
    }

    pub(in crate::island) fn on_recorded(&mut self) {
        self.recording = None;
        self.ui.recording_secs = None;
        self.repaint_expanded();
        self.kill_timer(Timer::Rec);
        self.apply_capture();
        match recorder::take_result() {
            Some(recorder::Finished::Saved(path, _took, notes)) => {
                let text = match notes.first() {
                    Some(note) => format!("Saved · {note}"),
                    None => "Recording saved".to_string(),
                };
                self.show_notice_for('\u{E714}', &text, if notes.is_empty() { 3000 } else { 5000 });
                self.notice_link = Some(path);
            }
            Some(recorder::Finished::Failed(why)) => {
                let why: String = why.chars().take(60).collect();
                self.show_notice_for('\u{E783}', &format!("Recording failed: {why}"), 5000);
            }
            None => self.show_notice('\u{E783}', "Recording failed"),
        }
        self.update_eq();
        self.repaint_collapsed();
    }

    pub(in crate::island) fn capture(&mut self, mode: capture::Mode) {
        capture::start(self.hwnd, mode);
    }

    pub(in crate::island) fn on_capture_done(&mut self) {
        match capture::take_outcome() {
            Some(capture::Outcome::Screenshot(path)) => {
                let text = if path.is_some() { "Screenshot copied and saved" } else { "Screenshot copied" };
                self.show_notice('\u{E722}', text);
            }
            Some(capture::Outcome::Color(hex)) => self.show_notice('\u{EB52}', &format!("{hex} copied")),
            Some(capture::Outcome::Ocr(bgra, w, h)) => {
                self.show_notice('\u{E8D2}', "Reading text…");
                recognize::spawn(recognize::Kind::Ocr, self.hwnd, bgra, w, h);
            }
            Some(capture::Outcome::Qr(bgra, w, h)) => {
                self.show_notice('\u{E8D2}', "Looking for a QR code…");
                recognize::spawn(recognize::Kind::Qr, self.hwnd, bgra, w, h);
            }
            Some(capture::Outcome::Record(rect)) => self.begin_recording(rect),
            Some(capture::Outcome::Failed) => self.show_notice('\u{E783}', "Capture failed"),
            Some(capture::Outcome::Cancelled) | None => {}
        }
    }

    pub(in crate::island) fn on_recognized(&mut self) {
        let Some(result) = recognize::take_result() else { return };
        match (result.kind, result.text) {
            (kind, Some(text)) => {
                clips::sys::write_text(self.hwnd, &text);
                let label = match kind {
                    recognize::Kind::Ocr => format!("Text copied ({} characters)", text.chars().count()),
                    recognize::Kind::Qr => "QR code copied".to_string(),
                };
                self.show_notice('\u{E8C8}', &label);
            }
            (recognize::Kind::Ocr, None) => self.show_notice('\u{E783}', "No text found"),
            (recognize::Kind::Qr, None) => self.show_notice('\u{E783}', "No QR code found"),
        }
    }

    pub(in crate::island) fn click_record(&mut self, x: i32, y: i32) {
        let pill = self.pill_rect(true);
        match view::hit_record(pill, self.scale, x as f32, y as f32) {
            Some(view::RecHit::Start) => {
                self.collapse();
                self.toggle_recording();
            }
            Some(view::RecHit::Folder) => {
                let dir = recorder::default_path(self.settings.record_format);
                if let Some(dir) = dir.parent() {
                    let _ = std::fs::create_dir_all(dir);
                    cmd::shell_open(&dir.to_string_lossy(), None);
                }
            }
            None => self.click_settings(x, y),
        }
    }
}
