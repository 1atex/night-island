use crate::island::*;

impl App {
    pub(in crate::island) fn click_text(&mut self, x: i32, y: i32) {
        if self.ui.text.qr.take().is_some() {
            return self.repaint_expanded();
        }
        let pill = self.pill_rect(true);
        match view::hit_text(pill, self.scale, x as f32, y as f32, &self.ui.cfg, &self.ui.text) {
            Some(view::TextHit::Chip(i)) => {
                if let Some((op, _)) = texttools::ops(self.settings.dev_tools).get(i) {
                    self.run_text_op(*op);
                }
                self.ui.text.palette_open = false;
                self.ui.text.chip_scroll = 0;
            }
            Some(view::TextHit::Cat(k)) => {
                self.ui.text.cat = k;
                self.repaint_expanded();
            }
            Some(view::TextHit::Palette) => {
                self.ui.text.palette_open = true;
                self.ui.text.chip_scroll = 0;
                self.end_edit(true);
                self.repaint_expanded();
            }
            Some(view::TextHit::PaletteClose) => {
                self.ui.text.palette_open = false;
                self.ui.text.chip_scroll = 0;
                self.repaint_expanded();
            }
            None if view::catalogue_open(&self.ui.cfg, &self.ui.text) => {}
            Some(view::TextHit::Action(i)) => self.run_text_action(i),
            None => {
                self.ui.notes.drag_from = None;
                let (fx, fy) = (x as f32, y as f32);
                let f = view::text_frame(pill, self.scale).0;
                if fx >= f.0 && fx < f.2 && fy >= f.1 && fy < f.3 {
                    self.click_in_field(fx, fy);
                } else {
                    self.end_edit(true);
                }
            }
        }
    }

    pub(in crate::island) fn run_text_op(&mut self, op: texttools::Op) {
        let selection = self.ui.text.input.selected_text();
        let source = selection.clone().unwrap_or_else(|| self.ui.text.input.text());
        if source.is_empty() && !texttools::works_on_empty(op) {
            self.ui.text.status = "Nothing to convert".into();
            return self.repaint_expanded();
        }
        if op == texttools::Op::Count {
            let s = texttools::stats(&source);
            let tr = crate::ui::i18n::tr;
            self.ui.text.status = format!(
                "{} {} · {} {} · {} {} · {} {}",
                s.chars,
                tr("chars"),
                s.words,
                tr("words"),
                s.lines,
                tr("lines"),
                s.bytes,
                tr("bytes")
            );
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
            3 => {
                self.ui.text.input.set_text("");
                self.ui.text.status.clear();
            }
            _ => {
                self.settings.dev_tools = !self.settings.dev_tools;
                self.ui.cfg = self.settings.clone();
                self.settings.save();
                self.ui.text.hover = None;
                self.ui.text.chip_scroll = 0;
            }
        }
        self.repaint_expanded();
    }
}
