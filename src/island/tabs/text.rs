use crate::island::*;

impl App {
    pub(in crate::island) fn click_text(&mut self, x: i32, y: i32) {
        if self.ui.text.qr.take().is_some() {
            return self.repaint_expanded();
        }
        let pill = self.pill_rect(true);
        match view::hit_text(pill, self.scale, x as f32, y as f32) {
            Some(view::TextHit::Chip(i)) => self.run_text_op(texttools::OPS[i].0),
            Some(view::TextHit::Action(i)) => self.run_text_action(i),
            None => self.click_note(x, y),
        }
    }

    pub(in crate::island) fn run_text_op(&mut self, op: texttools::Op) {
        let selection = self.ui.text.input.selected_text();
        let source = selection.clone().unwrap_or_else(|| self.ui.text.input.text());
        if source.is_empty() {
            self.ui.text.status = "Nothing to convert".into();
            return self.repaint_expanded();
        }
        if op == texttools::Op::Qr {
            match texttools::qr_modules(&source) {
                Some(qr) => self.ui.text.qr = Some(qr),
                None => self.ui.text.status = "Too much text for a QR code".into(),
            }
            return self.repaint_expanded();
        }
        match texttools::apply(op, &source) {
            Ok(result) => {
                if selection.is_some() {
                    self.ui.text.input.insert(&result);
                } else {
                    self.ui.text.input.set_text(&result);
                }
                self.ui.text.status = "Done".into();
            }
            Err(message) => self.ui.text.status = message.into(),
        }
        self.repaint_expanded();
    }

    pub(in crate::island) fn run_text_action(&mut self, index: usize) {
        match index {
            0 => match clips::sys::read_text(self.hwnd) {
                Some(text) => {
                    self.ui.text.input.set_text(&text);
                    self.ui.text.status = "Pasted from the clipboard".into();
                }
                None => self.ui.text.status = "The clipboard has no text".into(),
            },
            1 => {
                let text = self.ui.text.input.selected_text().unwrap_or_else(|| self.ui.text.input.text());
                clips::sys::write_text(self.hwnd, &text);
                self.ui.text.status = "Copied".into();
            }
            2 => {
                let text = self.ui.text.input.selected_text().unwrap_or_else(|| self.ui.text.input.text());
                if text.is_empty() {
                    self.ui.text.status = "Nothing to paste".into();
                } else {
                    self.end_edit(true);
                    clips::sys::write_text(self.hwnd, &text);
                    self.plain_paste();
                }
            }
            _ => {
                self.ui.text.input.set_text("");
                self.ui.text.status.clear();
            }
        }
        self.repaint_expanded();
    }
}
