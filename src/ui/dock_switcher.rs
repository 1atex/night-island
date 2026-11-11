use std::cell::{Cell, RefCell};
use std::sync::Arc;
use std::time::Instant;

use crate::data::{desktop_dock::DockSettings, settings::AnimCurve};
use crate::platform::{
    apps::Icon,
    desktop_dock::{self as desktop, Window},
    window_preview::Thumbnail,
};
use crate::ui::{
    desktop_dock::paint_material,
    dock_preview::fit,
    feedback::{Feedback, motion_allowed},
    gfx::{Gpu, rgba},
    popup::{PopupBackdrop, PopupSurface},
    reveal::Reveal,
};
use crate::view::Rect;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FONT_WEIGHT_NORMAL, DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_TEXT_ALIGNMENT_CENTER,
    DWRITE_TEXT_ALIGNMENT_LEADING,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_SHIFT};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{Result, w};

#[derive(Clone)]
pub(super) enum Anchor {
    Dock(i32),
    Keyboard(super::dock_alt_tab::Cycle),
}

pub(super) struct Placement {
    pub work: RECT,
    pub anchor: Anchor,
    pub scale: f32,
    pub hidden: bool,
}

pub(super) const CHOSEN: u32 = WM_APP + 119;
pub(super) const DONE: u32 = WM_APP + 117;

pub(super) struct Item {
    pub window: Window,
    pub icon: Option<Arc<Icon>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Hit {
    Window(usize),
    Close(usize),
    Prev,
    Next,
}

struct Layout {
    size: (i32, i32),
    count: usize,
    tile: f32,
    scale: f32,
    overview: bool,
    spans: Vec<(f32, f32)>,
}

impl Layout {
    fn new(work: RECT, scale: f32, windows: usize) -> Self {
        let room = ((work.right - work.left) as f32 / scale - 12.0).max(1.0);
        let height = ((work.bottom - work.top) as f32 / scale - 12.0).clamp(1.0, 210.0);
        let count = (((room - 30.0) / 208.0).floor() as usize).clamp(1, 5).min(windows.max(1));
        let tile = ((room - 30.0) / count as f32 - 8.0).clamp(1.0, 244.0);
        let size =
            (((30.0 + (tile + 8.0) * count as f32).min(room) * scale).round() as i32, (height * scale).round() as i32);
        Self { size, count, tile, scale, overview: false, spans: Vec::new() }
    }
    fn overview(work: RECT, scale: f32, windows: usize) -> Self {
        let room = (((work.right - work.left) as f32 / scale) * 0.94).max(1.0);
        let height = (((work.bottom - work.top) as f32 / scale) * 0.38).clamp(1.0, 352.0);
        let count = (((room - 30.0) / 320.0) as usize).clamp(1, 6).min(windows.max(1));
        let tile = ((room - 30.0) / count as f32 - 8.0).clamp(1.0, 450.0);
        let size =
            (((30.0 + (tile + 8.0) * count as f32).min(room) * scale).round() as i32, (height * scale).round() as i32);
        Self { size, count, tile, scale, overview: true, spans: Vec::new() }
    }
    fn arrange_page(&mut self, aspects: &[f32]) {
        if !self.overview || aspects.is_empty() {
            return;
        }
        let image_height = (self.size.1 as f32 / self.scale - 108.0).max(1.0);
        let widths: Vec<_> = aspects.iter().map(|a| (a * image_height + 12.0).clamp(134.0, 465.0)).collect();
        let total = widths.iter().sum::<f32>();
        let gaps = (widths.len() - 1) as f32 * 8.0;
        let room = (self.size.0 as f32 / self.scale - 38.0 - gaps).max(1.0);
        let factor = (room / total).min(1.0);
        let mut x = (self.size.0 as f32 / self.scale - total * factor - gaps) / 2.0;
        self.spans = widths
            .into_iter()
            .map(|width| {
                let left = x;
                x += width * factor + 8.0;
                (left, width * factor)
            })
            .collect();
    }
    fn card(&self, slot: usize) -> Rect {
        let s = self.scale;
        let (left, width) =
            self.spans.get(slot).copied().unwrap_or((19.0 + (self.tile + 8.0) * slot as f32, self.tile));
        ((left * s).round(), (48.0 * s).round(), ((left + width) * s).round(), (self.size.1 as f32 - 19.0 * s).round())
    }
    fn image(&self, slot: usize) -> Rect {
        let (l, t, r, b) = self.card(slot);
        let s = self.scale;
        (l + 6.0 * s, t + 32.0 * s, r - 6.0 * s, b - 6.0 * s)
    }
    fn thumbnail(&self, slot: usize, source: (i32, i32)) -> RECT {
        let area = self.image(slot);
        if !self.overview {
            return fit(source, area);
        }
        let cx = (area.0 + area.2) / 2.0;
        let cy = (area.1 + area.3) / 2.0;
        let hw = (area.2 - area.0).min(source.0 as f32) / 2.0;
        let hh = (area.3 - area.1).min(source.1 as f32) / 2.0;
        fit(source, (cx - hw, cy - hh, cx + hw, cy + hh))
    }

    fn close(&self, slot: usize) -> Rect {
        let (_, t, r, _) = self.card(slot);
        let s = self.scale;
        (r - 28.0 * s, t + 3.0 * s, r - 4.0 * s, t + 29.0 * s)
    }
    fn page_button(&self, next: bool) -> Rect {
        let s = self.scale;
        let r = self.size.0 as f32 - (if next { 18.0 } else { 49.0 }) * s;
        (r - 29.0 * s, 13.0 * s, r, 43.0 * s)
    }
}

fn contains(r: Rect, p: (f32, f32)) -> bool {
    p.0 >= r.0 && p.0 < r.2 && p.1 >= r.1 && p.1 < r.3
}
struct State {
    hwnd: HWND,
    dock: HWND,
    gpu: Gpu,
    surface: PopupSurface,
    backdrop: Option<PopupBackdrop>,
    config: DockSettings,
    focus_motion: Feedback,
    backdrop_at: Cell<Option<(i32, i32)>>,
    layout: Layout,
    origin: (i32, i32),
    items: Vec<Item>,
    thumbnails: Vec<Option<Thumbnail>>,
    selected: usize,
    hover: Option<Hit>,
    pressed: Option<Hit>,
    title: String,
    reveal: Reveal,
    closing: bool,
    choice: Option<Window>,
    cycle: Option<super::dock_alt_tab::Cycle>,
}
impl State {
    fn start(&self) -> usize {
        self.selected / self.layout.count * self.layout.count
    }
    fn pages(&self) -> usize {
        self.items.len().div_ceil(self.layout.count)
    }
    fn hit(&self, p: (f32, f32)) -> Option<Hit> {
        if self.pages() > 1 {
            for (next, hit) in [(false, Hit::Prev), (true, Hit::Next)] {
                if contains(self.layout.page_button(next), p) {
                    return Some(hit);
                }
            }
        }
        for (slot, i) in (self.start()..self.items.len()).take(self.layout.count).enumerate() {
            if contains(self.layout.close(slot), p) {
                return Some(Hit::Close(i));
            }
            if contains(self.layout.card(slot), p) {
                return Some(Hit::Window(i));
            }
        }
        None
    }
    fn bind(&mut self) {
        self.thumbnails.clear();
        let sizes: Vec<_> = self
            .items
            .iter()
            .skip(self.start())
            .take(self.layout.count)
            .map(|item| {
                let thumbnail = Thumbnail::new(self.hwnd, &item.window);
                let size = thumbnail.as_ref().and_then(Thumbnail::source_size);
                self.thumbnails.push(thumbnail);
                size
            })
            .collect();
        if self.layout.overview {
            self.layout
                .arrange_page(&sizes.iter().map(|s| s.map_or(1.6, |s| s.cx as f32 / s.cy as f32)).collect::<Vec<_>>());
        }
        for (slot, (thumb, size)) in self.thumbnails.iter_mut().zip(sizes).enumerate() {
            if let (Some(t), Some(size)) = (thumb.as_mut(), size)
                && !t.update(self.layout.thumbnail(slot, (size.cx, size.cy)), true)
            {
                *thumb = None;
            }
        }
    }
    fn paint(&self) -> Result<()> {
        let s = self.layout.scale;
        let (w, h) = (self.layout.size.0 as f32, self.layout.size.1 as f32);
        self.surface.paint(&self.gpu, |p| {
            crate::view::paint_popup_shadow(p, (w, h), s)?;
            paint_material(p, (9.0 * s, 9.0 * s, w - 9.0 * s, h - 9.0 * s), 24.0 * s, s, &self.config)?;
            let pages = self.pages();
            let label = if pages > 1 {
                format!("{}   ·   {} / {}", self.title, self.selected / self.layout.count + 1, pages)
            } else {
                self.title.clone()
            };
            p.text(
                &label,
                (22.0 * s, 14.0 * s, w - if pages > 1 { 84.0 * s } else { 20.0 * s }, 43.0 * s),
                14.0 * s,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                rgba(1.0, 1.0, 1.0, 0.92),
            )?;
            if self.layout.overview && w / s >= 410.0 {
                p.text(
                    "Release Alt to switch",
                    (w * 0.42, 14.0 * s, w - if pages > 1 { 90.0 } else { 23.0 } * s, 43.0 * s),
                    12.0 * s,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    windows::Win32::Graphics::DirectWrite::DWRITE_TEXT_ALIGNMENT_TRAILING,
                    rgba(1.0, 1.0, 1.0, 0.68),
                )?;
            }
            if pages > 1 {
                for (next, hit, glyph) in [(false, Hit::Prev, '\u{E76B}'), (true, Hit::Next, '\u{E76C}')] {
                    let r = self.layout.page_button(next);
                    p.fill_round_rect(
                        r,
                        10.0 * s,
                        rgba(1.0, 1.0, 1.0, if self.hover == Some(hit) { 0.18 } else { 0.08 }),
                    )?;
                    p.glyph(glyph, r, 12.0 * s, rgba(1.0, 1.0, 1.0, 0.85))?;
                }
            }
            if self.items.is_empty() {
                p.text(
                    "No open windows",
                    (15.0 * s, 48.0 * s, w - 15.0 * s, h - 18.0 * s),
                    14.0 * s,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_TEXT_ALIGNMENT_CENTER,
                    rgba(1.0, 1.0, 1.0, 0.65),
                )?;
            }
            for (slot, i) in (self.start()..self.items.len()).take(self.layout.count).enumerate() {
                let item = &self.items[i];
                let r = self.layout.card(slot);
                let selected = i == self.selected;
                let face = if self.layout.overview {
                    if selected { rgba(0.17, 0.17, 0.19, 0.98) } else { rgba(0.105, 0.105, 0.12, 0.98) }
                } else {
                    rgba(1.0, 1.0, 1.0, if selected { 0.115 } else { 0.06 })
                };
                p.fill_round_rect(r, 13.0 * s, face)?;
                let icon_side = (18.0 * s).round();
                let (ix, iy) = ((r.0 + 9.0 * s).round(), (r.1 + 7.0 * s).round());
                let icon = (ix, iy, ix + icon_side, iy + icon_side);
                if let Some(icon_data) = &item.icon {
                    p.image(icon, 3.0 * s, icon_data.side, &icon_data.bgra)?;
                } else {
                    p.glyph('\u{E737}', icon, 14.0 * s, rgba(1.0, 1.0, 1.0, 0.65))?;
                }
                let caption = ((icon.2 + 6.0 * s).round(), r.1, (r.2 - 30.0 * s).round(), (r.1 + 31.0 * s).round());
                p.push_clip(caption);
                let title = p.text(
                    &item.window.title,
                    caption,
                    if self.layout.overview { 13.0 } else { 12.0 } * s,
                    DWRITE_FONT_WEIGHT_SEMI_BOLD,
                    DWRITE_TEXT_ALIGNMENT_LEADING,
                    rgba(1.0, 1.0, 1.0, 0.93),
                );
                p.pop_clip();
                title?;
                let close = self.layout.close(slot);
                if self.hover == Some(Hit::Close(i)) {
                    p.fill_round_rect(close, 8.0 * s, rgba(0.9, 0.28, 0.28, 0.35))?;
                }
                if selected || matches!(self.hover, Some(Hit::Window(n) | Hit::Close(n)) if n == i) {
                    p.glyph('\u{E711}', close, 10.0 * s, rgba(1.0, 1.0, 1.0, 0.7))?;
                }
                if self.thumbnails.get(slot).is_none_or(Option::is_none) {
                    p.text(
                        if item.window.minimized { "Window minimized" } else { "Preview unavailable" },
                        self.layout.image(slot),
                        12.0 * s,
                        DWRITE_FONT_WEIGHT_NORMAL,
                        DWRITE_TEXT_ALIGNMENT_CENTER,
                        rgba(1.0, 1.0, 1.0, 0.6),
                    )?;
                }
            }
            if !self.items.is_empty() {
                let slot = self.selected % self.layout.count;
                let x = self.focus_motion.follow(1, slot as f32, slot as f32, 130);
                let x = x.clamp(0.0, (self.items.len() - self.start() - 1).min(self.layout.count - 1) as f32);
                let (l, t, r, b) = self.layout.card(x.floor() as usize);
                let (next_l, _, next_r, _) = self.layout.card(x.ceil() as usize);
                let (red, green, blue) = crate::ui::theme::accent();
                p.stroke_round_rect(
                    (l + (next_l - l) * x.fract(), t, r + (next_r - r) * x.fract(), b),
                    13.0 * s,
                    1.8 * s,
                    rgba(0.48 + red * 0.52, 0.48 + green * 0.52, 0.48 + blue * 0.52, 0.95),
                )?;
            }
            Ok(())
        })?;
        self.frame()
    }
    fn frame(&self) -> Result<()> {
        let now = Instant::now();
        let t = self.reveal.value(now);
        let at = if self.layout.overview {
            self.origin
        } else {
            (self.origin.0, self.origin.1 + ((1.0 - t) * 9.0 * self.layout.scale).round() as i32)
        };
        let moving = self.reveal.active(now) || self.focus_motion.active();
        unsafe {
            if moving {
                SetTimer(Some(self.hwnd), 1, if self.config.economy { 33 } else { 15 }, None);
            } else {
                let _ = KillTimer(Some(self.hwnd), 1);
                if self.closing {
                    let _ = PostMessageW(Some(self.dock), DONE, WPARAM(self.hwnd.0 as usize), LPARAM(0));
                    return Ok(());
                }
            }
        }
        self.surface.present(self.hwnd, Some(POINT { x: at.0, y: at.1 }), (255.0 * t) as u8)?;
        for thumbnail in self.thumbnails.iter().flatten() {
            thumbnail.set_opacity((255.0 * t) as u8);
        }
        if let Some(backdrop) = &self.backdrop {
            if unsafe { IsWindowVisible(self.hwnd).as_bool() } && self.backdrop_at.replace(Some(at)) != Some(at) {
                backdrop.show(at, self.layout.size, self.layout.scale, true);
            }
            backdrop.set_opacity(t);
        }
        Ok(())
    }
    fn animate(&mut self, show: bool) {
        let ms = if self.config.animations && motion_allowed() {
            if self.layout.overview {
                if show { 95 } else { 65 }
            } else if show {
                165
            } else {
                105
            }
        } else {
            0
        };
        self.reveal.retarget(show, ms, AnimCurve::Smooth, Instant::now());
        let _ = self.frame();
    }
    fn finish(&mut self, choice: Option<Window>) {
        if !self.closing {
            if let Some(cycle) = &self.cycle {
                let index =
                    choice.as_ref().and_then(|window| cycle.windows.iter().position(|w| w.handle == window.handle));
                if let Some(window) = cycle.finish(index)
                    && index.is_some()
                {
                    desktop::switch_window(window);
                }
            } else if choice.is_some() {
                self.choice = choice;
                unsafe {
                    let _ = PostMessageW(Some(self.dock), CHOSEN, WPARAM(self.hwnd.0 as usize), LPARAM(0));
                }
            }
            self.focus_motion.set_enabled(false);
            self.closing = true;
            self.animate(false);
        }
    }
    fn select(&mut self, index: usize) {
        if self.items.is_empty() {
            return;
        }
        let page = self.start();
        self.selected = index.min(self.items.len() - 1);
        if self.start() != page {
            self.reset_focus_motion();
            self.bind();
        }
        let _ = self.paint();
    }
    fn reset_focus_motion(&self) {
        self.focus_motion.set_enabled(false);
        self.focus_motion.set_enabled(!self.layout.overview && self.config.animations && motion_allowed());
    }
    fn step(&mut self, amount: isize) {
        if !self.items.is_empty() {
            self.hover = None;
            self.select((self.selected as isize + amount).rem_euclid(self.items.len() as isize) as usize);
        }
    }
    fn page(&mut self, next: bool) {
        let pages = self.pages();
        if pages > 1 {
            self.hover = None;
            let page = self.selected / self.layout.count;
            self.select((page + if next { 1 } else { pages - 1 }) % pages * self.layout.count);
            if let Some(cycle) = &self.cycle
                && let Some(item) = self.items.get(self.selected)
                && let Some(i) = cycle.windows.iter().position(|w| w.handle == item.window.handle)
            {
                cycle.select(i);
            }
        }
    }
    fn choose(&mut self) {
        if let Some(item) = self.items.get(self.selected) {
            self.finish(Some(item.window.clone()));
        }
    }
    fn input(&mut self, msg: u32, w: WPARAM, l: LPARAM) {
        if self.closing {
            return;
        }
        let hit = self.hit((l.0 as i16 as f32, (l.0 >> 16) as i16 as f32));
        match msg {
            WM_MOUSEMOVE if self.hover != hit => {
                self.hover = hit;
                if !self.layout.overview
                    && let Some(Hit::Window(i) | Hit::Close(i)) = hit
                {
                    self.select(i);
                } else {
                    let _ = self.paint();
                }
            }
            WM_LBUTTONDOWN => self.pressed = hit,
            WM_LBUTTONUP => {
                if self.pressed.take() != hit {
                    return;
                }
                match hit {
                    Some(Hit::Window(i)) => {
                        self.selected = i;
                        self.choose();
                    }
                    Some(Hit::Close(i)) => {
                        desktop::change_window(&self.items[i].window, desktop::WindowAction::Close);
                    }
                    Some(Hit::Prev) => self.page(false),
                    Some(Hit::Next) => self.page(true),
                    None => {}
                }
            }
            WM_MOUSEWHEEL => self.page(((w.0 >> 16) as i16) < 0),
            WM_KEYDOWN => match w.0 {
                27 => self.finish(None),
                13 => self.choose(),
                37 => self.step(-1),
                39 => self.step(1),
                9 => self.step(if unsafe { GetKeyState(VK_SHIFT.0 as i32) } < 0 { -1 } else { 1 }),
                33 => self.page(false),
                34 => self.page(true),
                _ => {}
            },
            WM_ACTIVATE if !self.layout.overview && w.0 & 0xffff == WA_INACTIVE as usize => self.finish(None),
            WM_CLOSE => self.finish(None),
            _ => {}
        }
    }
}

extern "system" fn proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const RefCell<State>;
        if !ptr.is_null() {
            match msg {
                WM_MOUSEACTIVATE if (*ptr).try_borrow().is_ok_and(|s| s.layout.overview) => {
                    return LRESULT(MA_NOACTIVATE as isize);
                }
                WM_TIMER if w.0 == 1 => {
                    if let Ok(s) = (*ptr).try_borrow() {
                        if s.focus_motion.active() {
                            let _ = s.paint();
                        } else {
                            let _ = s.frame();
                        }
                    }
                    return LRESULT(0);
                }
                WM_MOUSEMOVE | WM_LBUTTONDOWN | WM_LBUTTONUP | WM_MOUSEWHEEL | WM_KEYDOWN | WM_ACTIVATE | WM_CLOSE => {
                    if let Ok(mut s) = (*ptr).try_borrow_mut() {
                        s.input(msg, w, l);
                    }
                    return LRESULT(0);
                }
                _ => {}
            }
        }
        DefWindowProcW(hwnd, msg, w, l)
    }
}
pub(super) struct Switcher {
    hwnd: HWND,
    state: Box<RefCell<State>>,
}
impl Switcher {
    pub fn new(
        dock: HWND,
        gpu: &Gpu,
        items: Vec<Item>,
        title: String,
        placement: Placement,
        config: &DockSettings,
    ) -> Result<Self> {
        let Placement { work, anchor, scale, hidden } = placement;
        let overview = matches!(anchor, Anchor::Keyboard(_));
        let layout =
            if overview { Layout::overview(work, scale, items.len()) } else { Layout::new(work, scale, items.len()) };
        let surface = PopupSurface::new(layout.size)?;
        let origin = (
            work.left + (work.right - work.left - layout.size.0) / 2,
            match anchor {
                Anchor::Dock(above) => {
                    (above - layout.size.1).clamp(work.top, (work.bottom - layout.size.1).max(work.top))
                }
                Anchor::Keyboard(_) => work.top + (work.bottom - work.top - layout.size.1) / 2,
            },
        );
        let instance = unsafe { GetModuleHandleW(None)? };
        static CLASS: std::sync::Once = std::sync::Once::new();
        CLASS.call_once(|| unsafe {
            RegisterClassW(&WNDCLASSW {
                lpfnWndProc: Some(proc),
                hInstance: instance.into(),
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                lpszClassName: w!("NightIsland.WindowSwitcher"),
                ..Default::default()
            });
        });
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_LAYERED
                    | WS_EX_TOOLWINDOW
                    | WS_EX_TOPMOST
                    | if overview { WS_EX_NOACTIVATE } else { WINDOW_EX_STYLE(0) },
                w!("NightIsland.WindowSwitcher"),
                w!("Night Island"),
                WS_POPUP,
                origin.0,
                origin.1,
                layout.size.0,
                layout.size.1,
                Some(dock),
                None,
                Some(instance.into()),
                None,
            )?
        };
        let selected = match &anchor {
            Anchor::Keyboard(cycle) => cycle.selected().unwrap_or(0),
            Anchor::Dock(_) => items.iter().position(|item| item.window.active).unwrap_or(0),
        };
        let backdrop =
            if config.blur { gpu.compositor().ok().and_then(|c| PopupBackdrop::new(hwnd, &c).ok()) } else { None };
        let state = Box::new(RefCell::new(State {
            hwnd,
            dock,
            gpu: gpu.popup(),
            surface,
            backdrop,
            config: config.clone(),
            focus_motion: Feedback::with_capacity(1),
            backdrop_at: Cell::new(None),
            layout,
            origin,
            items,
            thumbnails: Vec::new(),
            selected,
            hover: None,
            pressed: None,
            title,
            reveal: Reveal::new(0.0),
            closing: false,
            choice: None,
            cycle: match anchor {
                Anchor::Keyboard(cycle) => Some(cycle),
                Anchor::Dock(_) => None,
            },
        }));
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, (&*state as *const RefCell<State>) as isize);
        }
        let popup = Self { hwnd, state };
        popup.set_capture(hidden);
        {
            let mut s = popup.state.borrow_mut();
            s.reset_focus_motion();
            s.bind();
            s.paint()?;
        }
        if popup.state.borrow().cycle.as_ref().is_some_and(|cycle| cycle.selected().is_none()) {
            return Err(windows::core::Error::from(windows::Win32::Foundation::E_ABORT));
        }
        unsafe {
            let _ = ShowWindow(hwnd, if overview { SW_SHOWNOACTIVATE } else { SW_SHOW });
            if !overview {
                let _ = SetForegroundWindow(hwnd);
            }
        }
        popup.state.borrow_mut().animate(true);
        Ok(popup)
    }
    pub fn retain_icon_paths(&self, paths: &mut Vec<String>) {
        paths.extend(self.state.borrow().items.iter().map(|item| item.window.path.clone()));
    }
    pub fn load_icons(&self, cache: &mut crate::platform::apps::DockIconCache) {
        let mut s = self.state.borrow_mut();
        if s.closing {
            return;
        }
        let start = s.start();
        let count = s.layout.count;
        let size = (20.0 * s.layout.scale).ceil() as u32;
        let mut changed = false;
        for item in s.items.iter_mut().skip(start).take(count) {
            if item.icon.is_none()
                && let Some(icon) = cache.get(&item.window.path, size)
            {
                item.icon = Some(icon);
                changed = true;
            }
        }
        if changed {
            let _ = s.paint();
        }
    }
    pub fn keyboard(&self) -> bool {
        self.state.borrow().layout.overview
    }
    pub fn sync_selection(&self, cycle: &super::dock_alt_tab::Cycle) -> bool {
        let mut s = self.state.borrow_mut();
        if s.closing || !s.cycle.as_ref().is_some_and(|own| own.same(cycle)) {
            return false;
        }
        if let Some(i) = cycle.selected()
            && let Some(index) = s.items.iter().position(|item| item.window.handle == cycle.windows[i].handle)
            && s.selected != index
        {
            s.select(index);
        }
        true
    }
    #[cfg(test)]
    pub fn selected(&self) -> Option<Window> {
        let s = self.state.borrow();
        if s.closing {
            return None;
        }
        s.items.get(s.selected).map(|i| i.window.clone())
    }
    #[cfg(test)]
    pub fn selected_point(&self) -> (i32, i32) {
        let state = self.state.borrow();
        let (l, t, r, b) = state.layout.image(state.selected % state.layout.count);
        (((l + r) / 2.0) as i32, ((t + b) / 2.0) as i32)
    }
    pub fn dismiss(&self) {
        self.state.borrow_mut().finish(None);
    }
    pub fn prune(&self) {
        let mut s = self.state.borrow_mut();
        if s.closing {
            return;
        }
        let selected = s.items.get(s.selected).map(|item| item.window.handle);
        let len = s.items.len();
        s.items.retain(|item| unsafe { IsWindowVisible(HWND(item.window.handle as _)).as_bool() });
        if s.items.len() != len {
            s.selected = s
                .items
                .iter()
                .position(|item| Some(item.window.handle) == selected)
                .unwrap_or_else(|| s.selected.min(s.items.len().saturating_sub(1)));
            s.hover = None;
            s.pressed = None;
            s.bind();
            let _ = s.paint();
        }
    }
    pub fn set_capture(&self, hidden: bool) {
        unsafe {
            let _ = SetWindowDisplayAffinity(self.hwnd, if hidden { WDA_EXCLUDEFROMCAPTURE } else { WDA_NONE });
        }
        if let Some(backdrop) = &self.state.borrow().backdrop {
            backdrop.set_capture_hidden(hidden);
        }
    }
    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }
    pub fn take_choice(&self) -> Option<Window> {
        self.state.borrow_mut().choice.take()
    }
}
impl Drop for Switcher {
    fn drop(&mut self) {
        unsafe {
            SetWindowLongPtrW(self.hwnd, GWLP_USERDATA, 0);
            self.state.get_mut().thumbnails.clear();
            self.state.get_mut().backdrop.take();
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overview_previews_never_enlarge_the_source_bitmap() {
        let work = RECT { left: 0, top: 0, right: 2560, bottom: 1440 };
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let mut layout = Layout::overview(work, scale, 2);
            layout.arrange_page(&[1.5, 16.0 / 9.0]);
            for (slot, size) in [(150, 100), (1920, 1080)].into_iter().enumerate() {
                let r = layout.thumbnail(slot, size);
                assert!(r.right - r.left <= size.0 && r.bottom - r.top <= size.1);
                if slot == 0 {
                    assert_eq!((r.right - r.left, r.bottom - r.top), size);
                }
                assert!(r.left >= 0 && r.right <= layout.size.0 && r.bottom <= layout.size.1);
            }
        }
    }

    #[test]
    fn pages_fit_the_work_area_at_each_monitor_scale() {
        for (width, height, scale) in [(1920, 1080, 1.0), (3840, 2160, 2.0), (1280, 720, 1.5), (720, 1080, 2.0)] {
            let work = RECT { left: -width, top: -height, right: 0, bottom: 0 };
            for windows in [0, 1, 3, 25] {
                let layout = Layout::new(work, scale, windows);
                assert!((1..=windows.clamp(1, 5)).contains(&layout.count));
                assert!(layout.size.0 <= width && layout.size.1 <= height);
                let last = layout.image(layout.count - 1);
                assert!(last.0 >= 0.0 && last.0 < last.2 && last.2 < layout.size.0 as f32);
                assert!(last.1 >= 0.0 && last.1 < last.3 && last.3 < layout.size.1 as f32);
            }
        }
    }
}
