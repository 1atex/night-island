use crate::data::chats::ChatInfo;
use crate::services::agy::{Kind, Msg};
use crate::services::ai::ChatTurn;
use crate::services::attach::{Attachment, Kind as FileKind};
use crate::view::*;

#[derive(Default)]
pub struct ChatUi {
    pub turns: Vec<ChatTurn>,
    pub msgs: Vec<Msg>,
    pub error: Option<String>,
    pub input: TextEdit,
    pub pending: bool,
    pub keyed: bool,
    pub hist: f32,
    pub generation: u32,
    pub cache: std::cell::RefCell<(String, i32, usize, Vec<f32>)>,
    pub max: std::cell::Cell<f32>,
    pub input_h: std::cell::Cell<f32>,
    pub chat_id: Option<i64>,
    pub title: String,
    pub saved: usize,
    pub list_open: bool,
    pub list: Vec<ChatInfo>,
    pub list_scroll: usize,
    pub files: Vec<Attachment>,
    pub drop_hot: bool,
    pub loaded: bool,
    pub sel: Option<((usize, u32), (usize, u32))>,
    pub geom: std::cell::RefCell<Vec<MsgGeom>>,
}

impl ChatUi {
    pub fn rebuild(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.msgs = self
            .turns
            .iter()
            .map(|t| Msg {
                kind: if t.assistant { Kind::Assistant } else { Kind::User },
                doc: crate::ui::md::parse(&shown_text(t)),
            })
            .collect();
        if let Some(why) = &self.error {
            let text = crate::ui::i18n::tr(why).into_owned();
            self.msgs.push(Msg { kind: Kind::Error, doc: crate::ui::md::parse(&text) });
        }
        self.hist = 0.0;
        self.sel = None;
    }

    pub fn selection(&self) -> Option<TextRange> {
        let (a, b) = self.sel?;
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        (lo != hi).then_some((lo, hi))
    }

    pub fn select_all(&mut self) {
        let Some(last) = self.msgs.len().checked_sub(1) else { return };
        self.sel = Some(((0, 0), (last, self.msgs[last].doc.text.encode_utf16().count() as u32)));
    }

    pub fn selected_text(&self) -> Option<String> {
        let (lo, hi) = self.selection()?;
        let mut parts = Vec::new();
        for index in lo.0..=hi.0 {
            let units: Vec<u16> = self.msgs.get(index)?.doc.text.encode_utf16().collect();
            let start = if index == lo.0 { lo.1 as usize } else { 0 };
            let end = if index == hi.0 { (hi.1 as usize).min(units.len()) } else { units.len() };
            if end > start {
                parts.push(String::from_utf16_lossy(&units[start..end]));
            }
        }
        let text = parts.join("\n\n");
        (!text.trim().is_empty()).then_some(text)
    }
}

pub fn chat_text_at(ui: &UiState, px: f32, py: f32) -> Option<(usize, u32)> {
    use windows::Win32::Graphics::DirectWrite::{DWRITE_HIT_TEST_METRICS, DWRITE_TEXT_METRICS};
    let geom = ui.chat.geom.borrow();
    let end_of = |index: usize| ui.chat.msgs.get(index).map_or(0, |m| m.doc.text.encode_utf16().count() as u32);
    let first = geom.first()?;
    let mut previous: Option<usize> = None;
    for g in geom.iter() {
        if py < g.y {
            return Some(previous.map_or((first.index, 0), |i| (i, end_of(i))));
        }
        let mut metrics = DWRITE_TEXT_METRICS::default();
        unsafe {
            let _ = g.layout.GetMetrics(&mut metrics);
        }
        if py <= g.y + metrics.height {
            let (mut trailing, mut inside) = (windows::core::BOOL(0), windows::core::BOOL(0));
            let mut hit = DWRITE_HIT_TEST_METRICS::default();
            unsafe { g.layout.HitTestPoint(px - g.x, py - g.y, &mut trailing, &mut inside, &mut hit) }.ok()?;
            let at = hit.textPosition + if trailing.as_bool() { hit.length } else { 0 };
            return Some((g.index, at.min(end_of(g.index))));
        }
        previous = Some(g.index);
    }
    previous.map(|i| (i, end_of(i)))
}

fn shown_text(turn: &ChatTurn) -> String {
    let mut out = String::new();
    for f in &turn.attachments {
        out.push_str(&format!("\u{1F4CE} `{}`\n\n", f.name.replace('`', "'")));
    }
    out.push_str(&turn.text);
    out
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ChatHit {
    Input,
    Send,
    New,
    Chats,
    Attach,
    Row(usize),
    RowDelete(usize),
    ChipRemove(usize),
    Setup,
    History,
}

pub(in crate::view) fn chat_buttons(pill: Rect, scale: f32) -> (Rect, Rect, Rect) {
    let (_, r) = agy_side(pill, scale);
    let (top, bottom) = (pill.1 + 58.0 * scale, pill.1 + 86.0 * scale);
    let new = (r - 64.0 * scale, top, r, bottom);
    let chats = (new.0 - 6.0 * scale - 74.0 * scale, top, new.0 - 6.0 * scale, bottom);
    let attach = (chats.0 - 6.0 * scale - 74.0 * scale, top, chats.0 - 6.0 * scale, bottom);
    (new, chats, attach)
}

pub(in crate::view) fn chat_reserve(files: usize, scale: f32) -> f32 {
    if files == 0 { 0.0 } else { 32.0 * scale }
}

pub fn chat_view(pill: Rect, scale: f32, input_h: f32, files: usize) -> Rect {
    let mut view = agy_view(pill, scale, input_h);
    view.3 -= chat_reserve(files, scale);
    view
}

pub(in crate::view) fn chat_chip(pill: Rect, scale: f32, input_h: f32, i: usize, n: usize) -> Rect {
    let (l, r) = agy_side(pill, scale);
    let gap = 6.0 * scale;
    let width = ((r - l - gap * (n.max(1) - 1) as f32) / n.max(1) as f32).min(160.0 * scale);
    let top = agy_input(pill, scale, input_h).0.1 - 32.0 * scale;
    let left = l + i as f32 * (width + gap);
    (left, top, left + width, top + 26.0 * scale)
}

fn chip_remove(chip: Rect, scale: f32) -> Rect {
    (chip.2 - 24.0 * scale, chip.1, chip.2, chip.3)
}

fn row_delete(row: Rect, scale: f32) -> Rect {
    (row.2 - 38.0 * scale, row.1 + 6.0 * scale, row.2 - 6.0 * scale, row.3 - 6.0 * scale)
}

pub fn chat_rows() -> usize {
    agy_rows()
}

pub fn hit_chat(pill: Rect, scale: f32, px: f32, py: f32, ui: &UiState) -> Option<ChatHit> {
    if !ui.chat.keyed {
        return Some(ChatHit::Setup);
    }
    let (new, chats, attach) = chat_buttons(pill, scale);
    if inside(new, px, py) {
        return Some(ChatHit::New);
    }
    if inside(chats, px, py) {
        return Some(ChatHit::Chats);
    }
    if inside(attach, px, py) {
        return Some(ChatHit::Attach);
    }
    if ui.chat.list_open {
        let shown = ui.chat.list.len().saturating_sub(ui.chat.list_scroll).min(chat_rows());
        for i in 0..shown {
            let row = agy_row(pill, scale, i);
            if inside(row_delete(row, scale), px, py) {
                return Some(ChatHit::RowDelete(i));
            }
            if inside(row, px, py) {
                return Some(ChatHit::Row(i));
            }
        }
        return Some(ChatHit::History);
    }
    let input_h = ui.chat.input_h.get();
    let n = ui.chat.files.len();
    for i in 0..n {
        if inside(chip_remove(chat_chip(pill, scale, input_h, i, n), scale), px, py) {
            return Some(ChatHit::ChipRemove(i));
        }
    }
    if inside(agy_input(pill, scale, input_h).0, px, py) {
        return Some(ChatHit::Input);
    }
    if inside(agy_send_button(pill, scale), px, py) {
        return Some(ChatHit::Send);
    }
    inside(chat_view(pill, scale, input_h, n), px, py).then_some(ChatHit::History)
}

fn chat_target(ui: &UiState) -> String {
    let ai = &ui.cfg.ai;
    format!("{} · {}", ai.provider.name(), ai.model(ai.provider))
}

fn short(name: &str, max: usize) -> String {
    if name.chars().count() <= max {
        return name.to_string();
    }
    let head: String = name.chars().take(max.saturating_sub(1)).collect();
    format!("{head}\u{2026}")
}

fn header_button(p: &Painter, rect: Rect, label: &str, hover: bool, active: bool, scale: f32) -> Result<()> {
    let fill =
        if active { acc(if hover { 0.5 } else { 0.35 }) } else { rgba(1.0, 1.0, 1.0, if hover { 0.18 } else { 0.08 }) };
    p.fill_round_rect(rect, 14.0 * scale, fill)?;
    p.text(label, rect, 12.5 * scale, DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_TEXT_ALIGNMENT_CENTER, WHITE)
}

fn paint_list(p: &Painter, pill: Rect, scale: f32, ui: &UiState) -> Result<()> {
    if ui.chat.list.is_empty() {
        let view = agy_view(pill, scale, 0.0);
        return p.text(
            "No saved chats yet",
            view,
            15.0 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_CENTER,
            rgba(1.0, 1.0, 1.0, 0.4),
        );
    }
    for (i, chat) in ui.chat.list.iter().skip(ui.chat.list_scroll).take(chat_rows()).enumerate() {
        let row = agy_row(pill, scale, i);
        let hovered = ui.home.hover_act == Some(1220 + i);
        let open = ui.chat.chat_id == Some(chat.id);
        let fill = if open { acc(0.2) } else { rgba(1.0, 1.0, 1.0, if hovered { 0.1 } else { 0.05 }) };
        p.fill_round_rect(row, 12.0 * scale, fill)?;
        p.text(
            &chat.title,
            (row.0 + 14.0 * scale, row.1, row.2 - 130.0 * scale, row.3),
            13.0 * scale,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            WHITE,
        )?;
        p.text(
            &time_ago(chat.ts),
            (row.2 - 124.0 * scale, row.1, row.2 - 44.0 * scale, row.3),
            11.5 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_TRAILING,
            rgba(1.0, 1.0, 1.0, 0.45),
        )?;
        let del = row_delete(row, scale);
        if ui.home.hover_act == Some(1240 + i) {
            p.fill_round_rect(del, 10.0 * scale, rgba(1.0, 0.35, 0.35, 0.35))?;
        }
        p.glyph('\u{E711}', del, 12.0 * scale, rgba(1.0, 1.0, 1.0, 0.7))?;
    }
    Ok(())
}

fn paint_chips(p: &Painter, pill: Rect, scale: f32, ui: &UiState) -> Result<()> {
    let n = ui.chat.files.len();
    for (i, file) in ui.chat.files.iter().enumerate() {
        let chip = chat_chip(pill, scale, ui.chat.input_h.get(), i, n);
        p.fill_round_rect(chip, 10.0 * scale, acc(0.18))?;
        let glyph = match file.kind {
            FileKind::Image => '\u{E91B}',
            FileKind::Pdf => '\u{E8A5}',
            FileKind::Text => '\u{E8A5}',
        };
        p.glyph(glyph, (chip.0 + 4.0 * scale, chip.1, chip.0 + 24.0 * scale, chip.3), 12.0 * scale, WHITE)?;
        p.text(
            &short(&file.name, 22),
            (chip.0 + 26.0 * scale, chip.1, chip.2 - 24.0 * scale, chip.3),
            11.5 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            WHITE,
        )?;
        let remove = chip_remove(chip, scale);
        if ui.home.hover_act == Some(1260 + i) {
            p.fill_round_rect(remove, 10.0 * scale, rgba(1.0, 0.35, 0.35, 0.35))?;
        }
        p.glyph('\u{E711}', remove, 10.0 * scale, rgba(1.0, 1.0, 1.0, 0.75))?;
    }
    Ok(())
}

pub(in crate::view) fn paint_chat(p: &Painter, pill: Rect, scale: f32, ui: &UiState) -> Result<()> {
    let (x, y, w, _) = pill;
    if !ui.chat.keyed {
        p.text(
            "Set up an AI service in the settings to chat",
            (x, y + 120.0 * scale, x + w, y + 200.0 * scale),
            15.0 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_CENTER,
            rgba(1.0, 1.0, 1.0, 0.6),
        )?;
        let button = (x + w / 2.0 - 90.0 * scale, y + 206.0 * scale, x + w / 2.0 + 90.0 * scale, y + 242.0 * scale);
        p.fill_round_rect(button, 14.0 * scale, acc(if ui.home.hover_act == Some(1210) { 0.5 } else { 0.35 }))?;
        return p.text(
            "Open AI settings",
            button,
            13.0 * scale,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_CENTER,
            WHITE,
        );
    }
    let (l, r) = agy_side(pill, scale);
    let (_, inner) = agy_input(pill, scale, ui.chat.input_h.get());
    if let Ok(layout) =
        crate::ui::gfx::make_layout(p.dwrite(), &ui.chat.input.text(), inner.2 - inner.0, AGY_FONT * scale)
    {
        ui.chat.input_h.set(p.layout_height(&layout) + 16.0 * scale);
    }

    let (new, chats, attach) = chat_buttons(pill, scale);
    let title = if ui.chat.title.is_empty() { "Chat" } else { ui.chat.title.as_str() };
    let title_right = attach.0 - 12.0 * scale;
    p.text(
        title,
        (l + 2.0 * scale, y + 56.0 * scale, title_right, y + 74.0 * scale),
        14.0 * scale,
        DWRITE_FONT_WEIGHT_BOLD,
        DWRITE_TEXT_ALIGNMENT_LEADING,
        WHITE,
    )?;
    p.text(
        &chat_target(ui),
        (l + 2.0 * scale, y + 72.0 * scale, title_right, y + 88.0 * scale),
        11.0 * scale,
        DWRITE_FONT_WEIGHT_NORMAL,
        DWRITE_TEXT_ALIGNMENT_LEADING,
        rgba(1.0, 1.0, 1.0, 0.5),
    )?;
    let hover = |id| ui.home.hover_act == Some(id);
    header_button(p, new, "New", hover(1201), false, scale)?;
    header_button(p, chats, "Chats", hover(1202), ui.chat.list_open, scale)?;
    header_button(p, attach, "Attach", hover(1203), !ui.chat.files.is_empty(), scale)?;

    if ui.chat.list_open {
        return paint_list(p, pill, scale, ui);
    }
    let n = ui.chat.files.len();
    if ui.chat.msgs.is_empty() && !ui.chat.pending {
        let view = chat_view(pill, scale, ui.chat.input_h.get(), n);
        p.text(
            "Ask anything",
            view,
            15.0 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_CENTER,
            rgba(1.0, 1.0, 1.0, 0.4),
        )?;
    } else {
        let history = History {
            msgs: &ui.chat.msgs,
            cache: &ui.chat.cache,
            id: format!("{}:{}", ui.chat.chat_id.unwrap_or(0), ui.chat.generation),
            hist: ui.chat.hist,
            max: &ui.chat.max,
            input_h: ui.chat.input_h.get(),
            reserve: chat_reserve(n, scale),
            geom: Some(&ui.chat.geom),
            sel: ui.chat.selection(),
        };
        paint_history(p, pill, scale, &history, ui.chat.pending)?;
    }
    paint_chips(p, pill, scale, ui)?;
    let (frame, text_area) = agy_input(pill, scale, ui.chat.input_h.get());
    paint_editor(
        p,
        frame,
        text_area,
        &ui.chat.input,
        ui.editing && ui.tab == Tab::Chat && ui.chat.sel.is_none(),
        ui.caret_on,
        "Message…",
        AGY_FONT,
        scale,
    )?;
    let button = agy_send_button(pill, scale);
    let hover = ui.home.hover_act == Some(1200) && !ui.chat.pending;
    p.fill_round_rect(
        button,
        12.0 * scale,
        acc(if ui.chat.pending {
            0.15
        } else if hover {
            0.5
        } else {
            0.35
        }),
    )?;
    p.text(
        if ui.chat.pending { "…" } else { "Send" },
        button,
        13.0 * scale,
        DWRITE_FONT_WEIGHT_SEMI_BOLD,
        DWRITE_TEXT_ALIGNMENT_CENTER,
        rgba(1.0, 1.0, 1.0, if ui.chat.pending { 0.5 } else { 1.0 }),
    )?;
    if ui.chat.drop_hot {
        let area = (l, y + 94.0 * scale, r, y + 368.0 * scale);
        p.fill_round_rect(area, 16.0 * scale, rgba(0.0, 0.0, 0.0, 0.55))?;
        p.stroke_round_rect(
            (area.0 + 2.0 * scale, area.1 + 2.0 * scale, area.2 - 2.0 * scale, area.3 - 2.0 * scale),
            16.0 * scale,
            2.0 * scale,
            acc(0.9),
        )?;
        p.text(
            "Drop files to attach",
            area,
            16.0 * scale,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_CENTER,
            WHITE,
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chat(texts: &[&str]) -> ChatUi {
        let mut ui = ChatUi::default();
        for (i, t) in texts.iter().enumerate() {
            ui.turns.push(ChatTurn::text(i % 2 == 1, *t));
        }
        ui.rebuild();
        ui
    }

    #[test]
    fn selected_text_comes_from_one_or_several_messages() {
        let mut ui = chat(&["Hello there", "General Kenobi"]);
        ui.sel = Some(((0, 6), (0, 11)));
        assert_eq!(ui.selected_text().as_deref(), Some("there"));
        ui.sel = Some(((0, 6), (1, 7)));
        assert_eq!(ui.selected_text().as_deref(), Some("there\n\nGeneral"));
        ui.sel = Some(((1, 7), (0, 6)));
        assert_eq!(ui.selection(), Some(((0, 6), (1, 7))), "dragging backwards selects the same text");
        assert_eq!(ui.selected_text().as_deref(), Some("there\n\nGeneral"));
    }

    #[test]
    fn a_click_without_a_drag_selects_nothing() {
        let mut ui = chat(&["Hello"]);
        ui.sel = Some(((0, 3), (0, 3)));
        assert_eq!(ui.selection(), None);
        assert_eq!(ui.selected_text(), None);
    }

    #[test]
    fn select_all_takes_every_message_and_new_messages_clear_it() {
        let mut ui = chat(&["one", "two", "three"]);
        ui.select_all();
        assert_eq!(ui.selected_text().as_deref(), Some("one\n\ntwo\n\nthree"));
        ui.rebuild();
        assert_eq!(ui.sel, None);
        assert_eq!(chat(&[]).selected_text(), None);
    }

    #[test]
    fn text_with_a_two_unit_character_and_markup_is_cut_by_utf16_units() {
        let mut ui = chat(&["\u{1F600} **bold** end"]);
        let all = ui.msgs[0].doc.text.encode_utf16().count() as u32;
        ui.sel = Some(((0, 3), (0, all)));
        assert_eq!(ui.selected_text().as_deref(), Some("bold end"));
    }
}
