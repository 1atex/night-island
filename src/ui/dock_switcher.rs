mod strip;

use std::cell::{Cell, RefCell};
use std::sync::Arc;
use std::time::Instant;
use strip::Strip;

use crate::data::{
    desktop_dock::{DockSettings, SwitcherStyle},
    settings::AnimCurve,
};
use crate::platform::{
    apps::Icon,
    desktop_dock::{self as desktop, Window},
    window_preview::Thumbnail,
};
use crate::ui::{
    desktop_dock::paint_material,
    dock_preview::fit,
    feedback::{Feedback, motion_allowed},
    gfx::{Gpu, Painter, rgba},
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
    Dock { above: i32, center: i32 },
    FileDrag { above: i32, center: i32 },
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
    ActualSize,
}

struct Layout {
    size: (i32, i32),
    count: usize,
    tile: f32,
    scale: f32,
    overview: bool,
    strip: Option<Strip>,
    columns: usize,
    rows: usize,
    first_row: usize,
}

impl Layout {
    fn new(work: RECT, scale: f32, windows: usize) -> Self {
        Self::grid(work, scale, windows, false)
    }
    fn overview(work: RECT, scale: f32, windows: usize) -> Self {
        Self::grid(work, scale, windows, true)
    }
    fn grid(work: RECT, scale: f32, windows: usize, overview: bool) -> Self {
        let width = (work.right - work.left) as f32 / scale;
        let height = (work.bottom - work.top) as f32 / scale;
        let room = (width * if overview { 0.94 } else { 0.96 }).max(1.0);
        let n = windows.max(1);
        let max_columns = (((room - 30.0) / 152.0) as usize).clamp(1, 4);
        let columns = n.div_ceil(n.div_ceil(max_columns));
        let total_rows = n.div_ceil(columns);
        let max_h = (height * if overview { 0.80 } else { 0.74 }).max(1.0);
        let rows = (((max_h - 59.0) / 106.0) as usize).max(1).min(total_rows);
        let tile_limit = if total_rows == 1 {
            if overview { 540.0 } else { 244.0 }
        } else {
            let full_row = (room - 30.0) / max_columns as f32 - 8.0;
            (full_row * 0.92).min(if overview { 335.0 } else { 202.0 }).max(1.0)
        };
        let tile = ((room - 30.0) / columns as f32 - 8.0).clamp(1.0, tile_limit);
        let row_height = if overview {
            (tile * 0.59 + 32.0).min(345.0)
        } else if rows == 1 {
            143.0
        } else {
            tile * 0.55 + 32.0
        };
        let h = (59.0 + (row_height + 8.0) * rows as f32).min(max_h);
        let size =
            (((30.0 + (tile + 8.0) * columns as f32).min(room) * scale).round() as i32, (h * scale).round() as i32);
        Self { size, count: rows * columns, tile, scale, overview, strip: None, columns, rows, first_row: 0 }
    }
    fn reveal_row(&mut self, index: usize, len: usize) {
        let row = index / self.columns;
        self.first_row = self
            .first_row
            .clamp((row + 1).saturating_sub(self.rows), row)
            .min(len.div_ceil(self.columns).saturating_sub(self.rows));
    }

    fn keyboard(work: RECT, scale: f32, windows: usize, style: SwitcherStyle) -> Self {
        if style == SwitcherStyle::Overview {
            return Self::overview(work, scale, windows);
        }
        let strip = Strip::new(work, scale, windows, style);
        Self {
            size: strip.size,
            count: strip.capacity,
            tile: 0.0,
            scale,
            overview: true,
            strip: Some(strip),
            columns: 1,
            rows: 1,
            first_row: 0,
        }
    }
    fn card(&self, slot: usize) -> Rect {
        let s = self.scale;
        let left = 19.0 + (self.tile + 8.0) * (slot % self.columns) as f32;
        let pitch = (self.size.1 as f32 / s - 59.0) / self.rows as f32;
        let top = 48.0 + pitch * (slot / self.columns) as f32;
        ((left * s).round(), (top * s).round(), ((left + self.tile) * s).round(), ((top + pitch - 8.0) * s).round())
    }
    fn image(&self, slot: usize) -> Rect {
        let (l, t, r, b) = self.card(slot);
        let s = self.scale;
        (l + 6.0 * s, t + 32.0 * s, r - 6.0 * s, b - 6.0 * s)
    }
    fn thumbnail(&self, slot: usize, source: (i32, i32)) -> RECT {
        let area = self.strip.map_or_else(|| self.image(slot), Strip::preview);
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
    fn actual_size_button(&self) -> Rect {
        let right = self.size.0 as f32 - 21.0 * self.scale;
        (right - 44.0 * self.scale, 13.0 * self.scale, right, 43.0 * self.scale)
    }
}

fn contains(r: Rect, p: (f32, f32)) -> bool {
    p.0 >= r.0 && p.0 < r.2 && p.1 >= r.1 && p.1 < r.3
}
fn source_view(size: (i32, i32), dest: RECT, cursor: Option<(f32, f32)>) -> RECT {
    let Some((x, y)) = cursor else { return RECT { left: 0, top: 0, right: size.0, bottom: size.1 } };
    let w = (dest.right - dest.left).clamp(1, size.0);
    let h = (dest.bottom - dest.top).clamp(1, size.1);
    let left = ((size.0 - w) as f32 * ((x - dest.left as f32) / w as f32).clamp(0.0, 1.0)).round() as i32;
    let top = ((size.1 - h) as f32 * ((y - dest.top as f32) / h as f32).clamp(0.0, 1.0)).round() as i32;
    RECT { left, top, right: left + w, bottom: top + h }
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
    sizes: Vec<Option<windows::Win32::Foundation::SIZE>>,
    actual_size: bool,
    file_drag: bool,
    previous_preview: Option<Thumbnail>,
    preview_mix: Reveal,
    strip_offset: Cell<f32>,
    wheel_delta: i32,
    strip_target: f32,
    pointer: Option<(i32, i32)>,
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
        self.layout.first_row * self.layout.columns
    }
    fn hit(&self, p: (f32, f32)) -> Option<Hit> {
        if let Some(strip) = self.layout.strip {
            return strip.hit(p, self.strip_offset.get(), self.items.len(), self.selected);
        }
        if self.layout.overview && contains(self.layout.actual_size_button(), p) {
            return Some(Hit::ActualSize);
        }
        for (slot, i) in (self.start()..self.items.len()).take(self.layout.count).enumerate() {
            if !self.file_drag && contains(self.layout.close(slot), p) {
                return Some(Hit::Close(i));
            }
            if contains(self.layout.card(slot), p) {
                return Some(Hit::Window(i));
            }
        }
        None
    }
    fn bind(&mut self) {
        self.previous_preview = None;
        self.thumbnails.clear();
        self.sizes.clear();
        let (start, count) = match self.layout.strip {
            Some(strip) if strip.carousel => (self.selected, 1),
            Some(_) => return,
            None => (self.start(), self.layout.count),
        };
        for item in self.items.iter().skip(start).take(count) {
            let thumbnail = Thumbnail::new(self.hwnd, &item.window);
            self.sizes.push(thumbnail.as_ref().and_then(Thumbnail::source_size));
            self.thumbnails.push(thumbnail);
        }
        for (slot, (thumb, size)) in self.thumbnails.iter_mut().zip(&self.sizes).enumerate() {
            if let (Some(t), Some(size)) = (thumb.as_mut(), size)
                && !t.update(self.layout.thumbnail(slot, (size.cx, size.cy)), true)
            {
                *thumb = None;
            }
        }
        if self.actual_size {
            for slot in 0..self.thumbnails.len() {
                self.update_source(slot, None);
            }
        }
    }

    fn update_source(&self, slot: usize, position: Option<(f32, f32)>) {
        if let (Some(Some(thumb)), Some(Some(size))) = (self.thumbnails.get(slot), self.sizes.get(slot)) {
            let dest = self.layout.thumbnail(slot, (size.cx, size.cy));
            let center = ((dest.left + dest.right) as f32 / 2.0, (dest.top + dest.bottom) as f32 / 2.0);
            thumb.set_source_rect(source_view(
                (size.cx, size.cy),
                dest,
                self.actual_size.then_some(position.unwrap_or(center)),
            ));
        }
    }

    fn paint(&self) -> Result<()> {
        let s = self.layout.scale;
        let (w, h) = (self.layout.size.0 as f32, self.layout.size.1 as f32);
        self.surface.paint(&self.gpu, |p| {
            crate::view::paint_popup_shadow(p, (w, h), s)?;
            paint_material(p, (9.0 * s, 9.0 * s, w - 9.0 * s, h - 9.0 * s), 24.0 * s, s, &self.config)?;
            if let Some(strip) = self.layout.strip {
                return self.paint_strip(p, strip);
            }
            let right = if self.layout.overview { self.layout.actual_size_button().0 - 8.0 * s } else { w - 20.0 * s };
            p.text(
                &self.title,
                (22.0 * s, 14.0 * s, right, 43.0 * s),
                14.0 * s,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                rgba(1.0, 1.0, 1.0, 0.92),
            )?;
            if self.layout.overview {
                let button = self.layout.actual_size_button();
                p.fill_round_rect(button, 9.0 * s, rgba(1.0, 1.0, 1.0, if self.actual_size { 0.24 } else { 0.095 }))?;
                p.text(
                    "1:1",
                    button,
                    13.0 * s,
                    DWRITE_FONT_WEIGHT_SEMI_BOLD,
                    DWRITE_TEXT_ALIGNMENT_CENTER,
                    rgba(1.0, 1.0, 1.0, 1.0),
                )?;
            }
            if self.layout.overview && w / s >= 500.0 {
                p.text(
                    "Release Alt to switch",
                    (w * 0.42, 14.0 * s, right, 43.0 * s),
                    12.0 * s,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    windows::Win32::Graphics::DirectWrite::DWRITE_TEXT_ALIGNMENT_TRAILING,
                    rgba(1.0, 1.0, 1.0, 0.68),
                )?;
            }
            let total_rows = self.items.len().div_ceil(self.layout.columns);
            if total_rows > self.layout.rows {
                let track = h - 67.0 * s;
                let top = 48.0 * s + track * self.layout.first_row as f32 / total_rows as f32;
                let bottom = top + track * self.layout.rows as f32 / total_rows as f32;
                p.fill_round_rect((w - 17.0 * s, top, w - 14.0 * s, bottom), 1.5 * s, rgba(1.0, 1.0, 1.0, 0.42))?;
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
                    if selected { rgba(0.17, 0.17, 0.19, 1.0) } else { rgba(0.105, 0.105, 0.12, 1.0) }
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
                let title = if self.layout.overview {
                    p.window_caption(&item.window.title, caption, 15.0 * s)
                } else {
                    p.text(
                        &item.window.title,
                        caption,
                        12.0 * s,
                        DWRITE_FONT_WEIGHT_SEMI_BOLD,
                        DWRITE_TEXT_ALIGNMENT_LEADING,
                        rgba(1.0, 1.0, 1.0, 0.93),
                    )
                };
                p.pop_clip();
                title?;
                let close = self.layout.close(slot);
                if self.hover == Some(Hit::Close(i)) {
                    p.fill_round_rect(close, 8.0 * s, rgba(0.9, 0.28, 0.28, 0.35))?;
                }
                if !self.file_drag && (selected || matches!(self.hover, Some(Hit::Window(n) | Hit::Close(n)) if n == i))
                {
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
                let slot = self.selected - self.start();
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
        let moving = self.reveal.active(now)
            || self.focus_motion.active()
            || self.previous_preview.is_some() && self.preview_mix.active(now);
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
        let mix = if let Some(previous) = &self.previous_preview {
            let mix = self.preview_mix.value(now);
            previous.set_opacity((255.0 * t * (1.0 - mix)) as u8);
            mix
        } else {
            1.0
        };
        for thumbnail in self.thumbnails.iter().flatten() {
            thumbnail.set_opacity((255.0 * t * mix) as u8);
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
    fn select(&mut self, index: usize, scroll: bool) {
        if self.items.is_empty() {
            return;
        }
        let page = self.start();
        let previous = self.selected;
        self.selected = index.min(self.items.len() - 1);
        if let Some(strip) = self.layout.strip {
            if scroll {
                self.strip_target = strip.target(self.selected, self.items.len());
            } else {
                self.strip_target = self.strip_offset.get();
            }
            if previous.abs_diff(self.selected) > strip.capacity.max(1) / 2 + 1 {
                self.reset_focus_motion();
            }
            if strip.carousel && previous != self.selected {
                let old = self.thumbnails.pop().flatten();
                self.bind();
                if self.config.animations
                    && motion_allowed()
                    && !self.config.economy
                    && self.thumbnails.first().is_some_and(Option::is_some)
                {
                    self.previous_preview = old;
                    self.preview_mix = Reveal::new(0.0);
                    self.preview_mix.retarget(true, 120, AnimCurve::Smooth, Instant::now());
                }
            }
        } else {
            self.layout.reveal_row(self.selected, self.items.len());
            if previous / self.layout.columns != self.selected / self.layout.columns {
                self.reset_focus_motion();
            }
            if self.start() != page {
                self.bind();
            }
        }
        let _ = self.paint();
    }
    fn reset_focus_motion(&self) {
        self.focus_motion.set_enabled(false);
        self.focus_motion.set_enabled(
            (!self.layout.overview || self.layout.strip.is_some()) && self.config.animations && motion_allowed(),
        );
    }
    fn step(&mut self, amount: isize) {
        if !self.items.is_empty() {
            self.hover = None;
            self.select_from_pointer(
                (self.selected as isize + amount).rem_euclid(self.items.len() as isize) as usize,
                true,
            );
        }
    }
    fn scroll(&mut self, next: bool) {
        let len = self.items.len();
        if self.layout.strip.is_some() || len <= self.layout.count {
            self.step(if next { 1 } else { -1 });
            return;
        }
        let first = self.layout.first_row;
        let last = len.div_ceil(self.layout.columns) - self.layout.rows;
        let row = if next { (first + 1).min(last) } else { first.saturating_sub(1) };
        if first != row {
            let slot = self.selected - self.start();
            self.layout.first_row = row;
            self.hover = None;
            self.reset_focus_motion();
            self.bind();
            self.select_from_pointer((self.start() + slot).min(len - 1), true);
        }
    }
    fn select_from_pointer(&mut self, index: usize, scroll: bool) {
        if let Some(cycle) = &self.cycle
            && let Some(item) = self.items.get(index)
            && let Some(i) = cycle.windows.iter().position(|w| w.handle == item.window.handle)
        {
            cycle.select(i);
        }
        self.select(index, scroll);
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
            WM_MOUSEMOVE => {
                let position = (l.0 as i16 as i32, (l.0 >> 16) as i16 as i32);
                if self.pointer.replace(position) == Some(position) {
                    return;
                }
                let previous = self.hover;
                self.hover = hit;
                if let Some(Hit::Window(i) | Hit::Close(i)) = hit {
                    if self.actual_size {
                        let point = (position.0 as f32, position.1 as f32);
                        if let Some(strip) = self.layout.strip {
                            if contains(strip.preview(), point) {
                                self.update_source(0, Some(point));
                            }
                        } else {
                            self.update_source(i - self.start(), Some(point));
                        }
                    }
                    if i != self.selected {
                        self.select_from_pointer(i, false);
                        return;
                    }
                }
                if previous != hit {
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
                    Some(Hit::ActualSize) => {
                        self.actual_size = !self.actual_size;
                        for slot in 0..self.thumbnails.len() {
                            self.update_source(slot, None);
                        }
                        let _ = self.paint();
                    }
                    Some(Hit::Prev) => self.scroll(false),
                    Some(Hit::Next) => self.scroll(true),
                    None => {}
                }
            }
            WM_MOUSEWHEEL => {
                self.wheel_delta += i32::from((w.0 >> 16) as i16);
                let steps = self.wheel_delta / 120;
                self.wheel_delta %= 120;
                for _ in 0..steps.unsigned_abs() {
                    self.scroll(steps < 0);
                }
            }
            WM_KEYDOWN => match w.0 {
                27 => self.finish(None),
                13 => self.choose(),
                36 => self.select_from_pointer(0, true),
                35 => self.select_from_pointer(self.items.len().saturating_sub(1), true),
                37 => self.step(-1),
                39 => self.step(1),
                9 => self.step(if unsafe { GetKeyState(VK_SHIFT.0 as i32) } < 0 { -1 } else { 1 }),
                33 => self.scroll(false),
                34 => self.scroll(true),
                _ => {}
            },
            WM_ACTIVATE if !self.layout.overview && !self.file_drag && w.0 & 0xffff == WA_INACTIVE as usize => {
                self.finish(None)
            }
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
                WM_MOUSEACTIVATE if (*ptr).try_borrow().is_ok_and(|s| s.layout.overview || s.file_drag) => {
                    return LRESULT(MA_NOACTIVATE as isize);
                }
                WM_TIMER if w.0 == 1 => {
                    if let Ok(mut s) = (*ptr).try_borrow_mut() {
                        if s.previous_preview.is_some() && !s.preview_mix.active(Instant::now()) {
                            s.previous_preview = None;
                        }
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
    file_target: Option<crate::platform::dnd::FileRegistration>,
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
        let file_drag = matches!(anchor, Anchor::FileDrag { .. });
        let mut layout = if overview {
            Layout::keyboard(work, scale, items.len(), config.switcher_style)
        } else {
            Layout::new(work, scale, items.len())
        };
        let surface = PopupSurface::new(layout.size)?;
        let origin = (
            match &anchor {
                Anchor::Dock { center, .. } | Anchor::FileDrag { center, .. } => {
                    (*center - layout.size.0 / 2).clamp(work.left, (work.right - layout.size.0).max(work.left))
                }
                Anchor::Keyboard(_) if config.switcher_style == SwitcherStyle::SideLeft => {
                    work.left + (10.0 * scale) as i32
                }
                Anchor::Keyboard(_) if config.switcher_style == SwitcherStyle::SideRight => {
                    work.right - layout.size.0 - (10.0 * scale) as i32
                }
                _ => work.left + (work.right - work.left - layout.size.0) / 2,
            },
            match anchor {
                Anchor::Dock { above, .. } | Anchor::FileDrag { above, .. } => {
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
                    | if overview || file_drag { WS_EX_NOACTIVATE } else { WINDOW_EX_STYLE(0) },
                w!("NightIsland.WindowSwitcher"),
                w!("Night Island"),
                WS_POPUP,
                origin.0,
                origin.1,
                layout.size.0,
                layout.size.1,
                if overview { None } else { Some(dock) },
                None,
                Some(instance.into()),
                None,
            )?
        };
        let selected = match &anchor {
            Anchor::Keyboard(cycle) => cycle.selected().unwrap_or(0),
            Anchor::Dock { .. } | Anchor::FileDrag { .. } => {
                items.iter().position(|item| item.window.active).unwrap_or(0)
            }
        };
        if layout.strip.is_none() {
            layout.reveal_row(selected, items.len());
        }
        let strip_target = layout.strip.map_or(0.0, |strip| strip.target(selected, items.len()));
        let backdrop =
            if config.blur { gpu.compositor().ok().and_then(|c| PopupBackdrop::new(hwnd, &c).ok()) } else { None };
        let state = Box::new(RefCell::new(State {
            hwnd,
            dock,
            gpu: gpu.popup(),
            surface,
            backdrop,
            config: config.clone(),
            focus_motion: Feedback::with_capacity(2),
            backdrop_at: Cell::new(None),
            layout,
            origin,
            items,
            thumbnails: Vec::new(),
            sizes: Vec::new(),
            actual_size: false,
            file_drag,
            previous_preview: None,
            preview_mix: Reveal::new(1.0),
            strip_offset: Cell::new(strip_target),
            wheel_delta: 0,
            strip_target,
            pointer: {
                let mut pt = POINT::default();
                unsafe { GetCursorPos(&mut pt) }.ok().map(|_| (pt.x - origin.0, pt.y - origin.1))
            },
            selected,
            hover: None,
            pressed: None,
            title,
            reveal: Reveal::new(0.0),
            closing: false,
            choice: None,
            cycle: match anchor {
                Anchor::Keyboard(cycle) => Some(cycle),
                Anchor::Dock { .. } | Anchor::FileDrag { .. } => None,
            },
        }));
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, (&*state as *const RefCell<State>) as isize);
        }
        let file_target = file_drag.then(|| crate::platform::dnd::FileRegistration::new(hwnd, dock)).flatten();
        let popup = Self { hwnd, state, file_target };
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
            let _ = ShowWindow(hwnd, if overview || file_drag { SW_SHOWNOACTIVATE } else { SW_SHOW });
            if !overview && !file_drag {
                let _ = SetForegroundWindow(hwnd);
            }
        }
        popup.state.borrow_mut().animate(true);
        Ok(popup)
    }
    pub fn drag_window(&self, x: i32, y: i32) -> Option<Window> {
        let mut s = self.state.try_borrow_mut().ok()?;
        if !s.file_drag || s.closing {
            return None;
        }
        let p = ((x - s.origin.0) as f32, (y - s.origin.1) as f32);
        let index = match s.hit(p)? {
            Hit::Close(i) | Hit::Window(i) => i,
            _ => return None,
        };
        if s.hover != Some(Hit::Window(index)) {
            s.hover = Some(Hit::Window(index));
            s.select(index, false);
        }
        s.items.get(index).map(|i| i.window.clone())
    }

    pub fn retain_icon_paths(&self, paths: &mut Vec<String>) {
        paths.extend(self.state.borrow().items.iter().map(|item| item.window.path.clone()));
    }
    pub fn load_icons(&self, cache: &mut crate::platform::apps::DockIconCache) {
        let mut s = self.state.borrow_mut();
        if s.closing {
            return;
        }
        let range = if let Some(strip) = s.layout.strip {
            strip.range(s.strip_target, s.items.len())
        } else {
            s.start()..(s.start() + s.layout.count).min(s.items.len())
        };
        let size = (if s.layout.strip.is_some() { 52.0 } else { 20.0 } * s.layout.scale).ceil() as u32;
        let mut changed = false;
        for item in &mut s.items[range] {
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
            s.select(index, true);
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
    pub fn window_point(&self, handle: isize) -> (i32, i32) {
        let state = self.state.borrow();
        let index = state.items.iter().position(|item| item.window.handle == handle).unwrap();
        let (l, t, r, b) = match state.layout.strip {
            Some(strip) => strip.tile(index as f32, state.strip_offset.get()),
            None => state.layout.image(index - state.start()),
        };
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
            if s.items.is_empty() {
                s.finish(None);
                return;
            }
            if let Some(strip) = s.layout.strip {
                s.strip_target = strip.target(s.selected, s.items.len());
                s.reset_focus_motion();
            } else {
                let (selected, len) = (s.selected, s.items.len());
                s.layout.reveal_row(selected, len);
            }
            if let Some(cycle) = &s.cycle
                && let Some(item) = s.items.get(s.selected)
                && let Some(i) = cycle.windows.iter().position(|w| w.handle == item.window.handle)
            {
                cycle.select(i);
            }
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
        self.file_target.take();
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
    fn actual_size_preserves_one_source_pixel_per_display_pixel_and_reaches_every_edge() {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let layout = Layout::overview(RECT { left: 0, top: 0, right: 1920, bottom: 1080 }, scale, 3);
            let size = (2560, 1440);
            let dest = layout.thumbnail(0, size);
            for edge in [0.0, 0.5, 1.0] {
                let px = (
                    dest.left as f32 + (dest.right - dest.left) as f32 * edge,
                    dest.top as f32 + (dest.bottom - dest.top) as f32 * edge,
                );
                let source = source_view(size, dest, Some(px));
                assert_eq!(
                    (source.right - source.left, source.bottom - source.top),
                    (dest.right - dest.left, dest.bottom - dest.top)
                );
                if edge == 0.0 {
                    assert_eq!((source.left, source.top), (0, 0));
                }
                if edge == 1.0 {
                    assert_eq!((source.right, source.bottom), size);
                }
            }
            assert_eq!(source_view(size, dest, None), RECT { left: 0, top: 0, right: size.0, bottom: size.1 });
        }
    }

    #[test]
    fn overview_previews_never_enlarge_the_source_bitmap() {
        let work = RECT { left: 0, top: 0, right: 2560, bottom: 1440 };
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let layout = Layout::overview(work, scale, 2);
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
    fn fifth_window_starts_a_second_row_and_overflow_scrolls_one_row() {
        for overview in [false, true] {
            let work = RECT { left: -1920, top: 0, right: 0, bottom: 1080 };
            for scale in [1.0, 1.25, 1.5, 2.0] {
                let few = Layout::grid(work, scale, 4, overview);
                assert_eq!(few.rows, 1);
                for len in 5..=8 {
                    let layout = Layout::grid(work, scale, len, overview);
                    assert_eq!(layout.rows, 2);
                    assert!(layout.count >= len && layout.tile <= few.tile);
                    assert!(layout.card(layout.columns).1 > layout.card(0).3);
                }
                let mut large = Layout::grid(work, scale, 128, overview);
                large.reveal_row(large.count, 128);
                assert_eq!(large.first_row, 1);
                large.reveal_row(127, 128);
                assert_eq!(large.first_row + large.rows, 128usize.div_ceil(large.columns));
            }
        }
    }

    #[test]
    fn grid_fits_the_work_area_at_each_monitor_scale() {
        for (width, height, scale) in [(1920, 1080, 1.0), (3840, 2160, 2.0), (1280, 720, 1.5), (720, 1080, 2.0)] {
            let work = RECT { left: -width, top: -height, right: 0, bottom: 0 };
            for windows in [0, 1, 3, 25] {
                let layout = Layout::new(work, scale, windows);
                assert!((1..=4).contains(&layout.columns));
                assert!(layout.size.0 <= width && layout.size.1 <= height);
                let last = layout.image(layout.count - 1);
                assert!(last.0 >= 0.0 && last.0 < last.2 && last.2 < layout.size.0 as f32);
                assert!(last.1 >= 0.0 && last.1 < last.3 && last.3 < layout.size.1 as f32);
            }
        }
    }
}
