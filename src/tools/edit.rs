use windows::Win32::Graphics::DirectWrite::{DWRITE_HIT_TEST_METRICS, IDWriteTextLayout};
use windows::core::BOOL;

pub const MAX_CHARS: usize = 200_000;

#[derive(Default)]
pub struct TextEdit {
    buf: Vec<char>,
    pub caret: usize,
    pub anchor: usize,
    pub scroll: f32,
}

impl TextEdit {
    pub fn text(&self) -> String {
        self.buf.iter().collect()
    }

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    pub fn set_text(&mut self, text: &str) {
        self.buf = text.chars().take(MAX_CHARS).collect();
        self.caret = self.buf.len();
        self.anchor = self.caret;
        self.scroll = 0.0;
    }

    pub fn selection(&self) -> (usize, usize) {
        (self.caret.min(self.anchor), self.caret.max(self.anchor))
    }

    pub fn has_selection(&self) -> bool {
        self.caret != self.anchor
    }

    pub fn selected_text(&self) -> Option<String> {
        let (a, b) = self.selection();
        (a != b).then(|| self.buf[a..b].iter().collect())
    }

    pub fn utf16_at(&self, index: usize) -> u32 {
        self.buf[..index.min(self.buf.len())].iter().map(|c| c.len_utf16() as u32).sum()
    }

    fn char_at_utf16(&self, offset: u32) -> usize {
        let mut units = 0u32;
        for (i, c) in self.buf.iter().enumerate() {
            if units >= offset {
                return i;
            }
            units += c.len_utf16() as u32;
        }
        self.buf.len()
    }

    pub fn delete_selection(&mut self) -> bool {
        let (a, b) = self.selection();
        if a == b {
            return false;
        }
        self.buf.drain(a..b);
        self.caret = a;
        self.anchor = a;
        true
    }

    pub fn insert(&mut self, text: &str) {
        self.delete_selection();
        let room = MAX_CHARS.saturating_sub(self.buf.len());
        let chars: Vec<char> = text.chars().take(room).collect();
        let n = chars.len();
        self.buf.splice(self.caret..self.caret, chars);
        self.caret += n;
        self.anchor = self.caret;
    }

    pub fn backspace(&mut self, word: bool) {
        if self.delete_selection() {
            return;
        }
        if self.caret == 0 {
            return;
        }
        let to = if word { self.word_left(self.caret) } else { self.caret - 1 };
        self.buf.drain(to..self.caret);
        self.caret = to;
        self.anchor = to;
    }

    pub fn delete(&mut self, word: bool) {
        if self.delete_selection() {
            return;
        }
        if self.caret >= self.buf.len() {
            return;
        }
        let to = if word { self.word_right(self.caret) } else { self.caret + 1 };
        self.buf.drain(self.caret..to);
        self.anchor = self.caret;
    }

    fn word_left(&self, mut i: usize) -> usize {
        while i > 0 && self.buf[i - 1].is_whitespace() {
            i -= 1;
        }
        while i > 0 && !self.buf[i - 1].is_whitespace() {
            i -= 1;
        }
        i
    }

    fn word_right(&self, mut i: usize) -> usize {
        while i < self.buf.len() && !self.buf[i].is_whitespace() {
            i += 1;
        }
        while i < self.buf.len() && self.buf[i].is_whitespace() {
            i += 1;
        }
        i
    }

    pub fn set_caret(&mut self, pos: usize, extend: bool) {
        self.caret = pos.min(self.buf.len());
        if !extend {
            self.anchor = self.caret;
        }
    }

    pub fn left(&mut self, extend: bool, word: bool) {
        if self.has_selection() && !extend {
            let (a, _) = self.selection();
            return self.set_caret(a, false);
        }
        let to = if word { self.word_left(self.caret) } else { self.caret.saturating_sub(1) };
        self.set_caret(to, extend);
    }

    pub fn right(&mut self, extend: bool, word: bool) {
        if self.has_selection() && !extend {
            let (_, b) = self.selection();
            return self.set_caret(b, false);
        }
        let to = if word { self.word_right(self.caret) } else { self.caret + 1 };
        self.set_caret(to, extend);
    }

    pub fn caret_to_start(&mut self, extend: bool) {
        self.set_caret(0, extend);
    }

    pub fn caret_to_end(&mut self, extend: bool) {
        self.set_caret(self.buf.len(), extend);
    }

    pub fn select_all(&mut self) {
        self.anchor = 0;
        self.caret = self.buf.len();
    }

    pub fn vertical(&mut self, layout: &IDWriteTextLayout, dir: i32, extend: bool) {
        let (x, y, h) = caret_xy(layout, self.utf16_at(self.caret));
        let target = if dir < 0 { y - h * 0.5 } else { y + h * 1.5 };
        let pos = if target < 0.0 { 0 } else { hit_point(layout, x, target) };
        self.set_caret(self.char_at_utf16(pos), extend);
    }

    pub fn line_edge(&mut self, layout: &IDWriteTextLayout, end: bool, extend: bool) {
        let (_, y, h) = caret_xy(layout, self.utf16_at(self.caret));
        let x = if end { f32::MAX / 4.0 } else { 0.0 };
        let pos = hit_point(layout, x, y + h / 2.0);
        self.set_caret(self.char_at_utf16(pos), extend);
    }

    pub fn click(&mut self, layout: &IDWriteTextLayout, x: f32, y: f32, extend: bool) {
        let pos = hit_point(layout, x, y);
        self.set_caret(self.char_at_utf16(pos), extend);
    }
}

pub fn caret_xy(layout: &IDWriteTextLayout, pos: u32) -> (f32, f32, f32) {
    let (mut x, mut y) = (0.0f32, 0.0f32);
    let mut m = DWRITE_HIT_TEST_METRICS::default();
    unsafe {
        let _ = layout.HitTestTextPosition(pos, false, &mut x, &mut y, &mut m);
    }
    (x, y, m.height.max(1.0))
}

pub fn hit_point(layout: &IDWriteTextLayout, x: f32, y: f32) -> u32 {
    let (mut trailing, mut inside) = (BOOL(0), BOOL(0));
    let mut m = DWRITE_HIT_TEST_METRICS::default();
    unsafe {
        let _ = layout.HitTestPoint(x, y, &mut trailing, &mut inside, &mut m);
    }
    m.textPosition + u32::from(trailing.as_bool()) * m.length
}

pub fn selection_rects(layout: &IDWriteTextLayout, start: u32, len: u32) -> Vec<(f32, f32, f32, f32)> {
    let mut count = 0u32;
    unsafe {
        let _ = layout.HitTestTextRange(start, len, 0.0, 0.0, None, &mut count);
        if count == 0 {
            return Vec::new();
        }
        let mut metrics = vec![DWRITE_HIT_TEST_METRICS::default(); count as usize];
        if layout.HitTestTextRange(start, len, 0.0, 0.0, Some(&mut metrics), &mut count).is_err() {
            return Vec::new();
        }
        metrics.truncate(count as usize);
        metrics.iter().map(|m| (m.left, m.top, m.width, m.height)).collect()
    }
}

pub fn content_height(layout: &IDWriteTextLayout) -> f32 {
    let mut m = windows::Win32::Graphics::DirectWrite::DWRITE_TEXT_METRICS::default();
    unsafe {
        let _ = layout.GetMetrics(&mut m);
    }
    m.height
}
