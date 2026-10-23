use crate::services::agy::{Kind, Msg};
use crate::services::ai::ChatTurn;
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
}

impl ChatUi {
    pub fn rebuild(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.msgs = self
            .turns
            .iter()
            .map(|t| Msg {
                kind: if t.assistant { Kind::Assistant } else { Kind::User },
                doc: crate::ui::md::parse(&t.text),
            })
            .collect();
        if let Some(why) = &self.error {
            let text = crate::ui::i18n::tr(why).into_owned();
            self.msgs.push(Msg { kind: Kind::Error, doc: crate::ui::md::parse(&text) });
        }
        self.hist = 0.0;
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ChatHit {
    Input,
    Send,
    New,
    Setup,
    History,
}

pub(in crate::view) fn chat_new_button(pill: Rect, scale: f32) -> Rect {
    let (_, r) = agy_side(pill, scale);
    (r - 108.0 * scale, pill.1 + 58.0 * scale, r, pill.1 + 86.0 * scale)
}

pub fn hit_chat(pill: Rect, scale: f32, px: f32, py: f32, ui: &UiState) -> Option<ChatHit> {
    if !ui.chat.keyed {
        return Some(ChatHit::Setup);
    }
    let input_h = ui.chat.input_h.get();
    if inside(chat_new_button(pill, scale), px, py) {
        return Some(ChatHit::New);
    }
    if inside(agy_input(pill, scale, input_h).0, px, py) {
        return Some(ChatHit::Input);
    }
    if inside(agy_send_button(pill, scale), px, py) {
        return Some(ChatHit::Send);
    }
    inside(agy_view(pill, scale, input_h), px, py).then_some(ChatHit::History)
}

fn chat_target(ui: &UiState) -> String {
    let ai = &ui.cfg.ai;
    format!("{} · {}", ai.provider.name(), ai.model(ai.provider))
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

    p.text(
        "Chat",
        (l + 2.0 * scale, y + 56.0 * scale, r - 120.0 * scale, y + 74.0 * scale),
        14.0 * scale,
        DWRITE_FONT_WEIGHT_BOLD,
        DWRITE_TEXT_ALIGNMENT_LEADING,
        WHITE,
    )?;
    p.text(
        &chat_target(ui),
        (l + 2.0 * scale, y + 72.0 * scale, r - 120.0 * scale, y + 88.0 * scale),
        11.0 * scale,
        DWRITE_FONT_WEIGHT_NORMAL,
        DWRITE_TEXT_ALIGNMENT_LEADING,
        rgba(1.0, 1.0, 1.0, 0.5),
    )?;
    let new = chat_new_button(pill, scale);
    let new_hover = ui.home.hover_act == Some(1201);
    p.fill_round_rect(new, 14.0 * scale, rgba(1.0, 1.0, 1.0, if new_hover { 0.18 } else { 0.08 }))?;
    p.text("New chat", new, 12.5 * scale, DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_TEXT_ALIGNMENT_CENTER, WHITE)?;

    if ui.chat.msgs.is_empty() && !ui.chat.pending {
        let view = agy_view(pill, scale, ui.chat.input_h.get());
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
            id: ui.chat.generation.to_string(),
            hist: ui.chat.hist,
            max: &ui.chat.max,
            input_h: ui.chat.input_h.get(),
        };
        paint_history(p, pill, scale, &history, ui.chat.pending)?;
    }
    let (frame, text_area) = agy_input(pill, scale, ui.chat.input_h.get());
    paint_editor(
        p,
        frame,
        text_area,
        &ui.chat.input,
        ui.editing && ui.tab == Tab::Chat,
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
    )
}
