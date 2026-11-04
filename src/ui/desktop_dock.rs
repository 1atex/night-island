use std::collections::HashMap;
use std::sync::{Arc, Once};
use std::time::{Duration, Instant};

use windows::UI::Composition::Desktop::DesktopWindowTarget;
use windows::UI::Composition::{Compositor, ContainerVisual};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::DirectWrite::{DWRITE_FONT_WEIGHT_NORMAL, DWRITE_TEXT_ALIGNMENT_CENTER};
use windows::Win32::Graphics::Gdi::{CombineRgn, CreateRoundRectRgn, DeleteObject, RGN_OR, SetWindowRgn};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::WinRT::Composition::ICompositorDesktopInterop;
use windows::Win32::UI::Controls::WM_MOUSELEAVE;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
};
use windows::Win32::UI::Shell::{DragAcceptFiles, DragFinish, DragQueryFileW, HDROP};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{Interface, Result, w};
use windows_numerics::Vector2;

use crate::data::desktop_dock::{DockSettings, Pin, key};
use crate::platform::{apps, desktop_dock as desktop};
use crate::ui::feedback::{Feedback, motion_allowed};
use crate::ui::gfx::{Gpu, Layer, rgba};
use crate::view::Rect;

pub const WM_DESKTOP_DOCK: u32 = WM_APP + 70;
const PULSE: usize = 1;
const FRAME: usize = 2;

#[derive(Clone)]
pub struct Entry {
    pub pin: Pin,
    pub pinned: bool,
    pub windows: Vec<desktop::Window>,
    icon: Option<Arc<apps::Icon>>,
}

pub enum Request {
    Save,
    Launch(Entry),
    Menu(Option<Entry>),
    Add,
}

struct State {
    hwnd: HWND,
    parent: HWND,
    config: DockSettings,
    gpu: Gpu,
    content: Layer,
    _target: DesktopWindowTarget,
    _root: ContainerVisual,
    entries: Vec<Entry>,
    scan: Vec<desktop::Window>,
    targets: HashMap<String, String>,
    monitor: desktop::Monitor,
    scale: f32,
    size: (i32, i32),
    origin: (i32, i32),
    rects: Vec<(usize, Rect)>,
    panel: Rect,
    feedback: Feedback,
    pointer: Option<(f32, f32)>,
    hover: Option<usize>,
    pressed: Option<(usize, f32)>,
    dragging: bool,
    leave_at: Instant,
    scanned_at: Instant,
    hidden: bool,
    suppressed: bool,
    offset: usize,
    request: Option<Request>,
    menu_open: bool,
}

pub struct Dock {
    state: Box<State>,
}

fn inside(rect: Rect, point: (f32, f32)) -> bool {
    point.0 >= rect.0 && point.0 < rect.2 && point.1 >= rect.1 && point.1 < rect.3
}

pub fn capacity(width: f32, scale: f32, icon: f32, zoom: f32) -> usize {
    (((width / scale - 48.0 - icon * (zoom - 1.0) * 3.0) / (icon + 12.0)).floor() as usize).clamp(2, 64)
}

impl State {
    fn post(&mut self, request: Request) {
        if self.request.is_none() {
            self.request = Some(request);
            unsafe {
                let _ = PostMessageW(Some(self.parent), WM_DESKTOP_DOCK, WPARAM(0), LPARAM(0));
            }
        }
    }

    fn refresh(&mut self, force: bool) {
        if self.pressed.is_some() || self.menu_open {
            return;
        }
        let scan = desktop::windows();
        self.scanned_at = Instant::now();
        if !force && scan == self.scan {
            return;
        }
        self.scan = scan;
        let previous: Vec<String> = self.entries.iter().map(|e| key(&e.pin.target)).collect();
        let icons: HashMap<_, _> = self.entries.iter().map(|e| (e.pin.target.clone(), e.icon.clone())).collect();
        let icon_for = |path: &str| icons.get(path).cloned().unwrap_or_else(|| apps::large_icon_for_path(path));
        let mut entries: Vec<Entry> = self
            .config
            .pins
            .iter()
            .map(|pin| {
                let resolved = self
                    .targets
                    .entry(pin.target.clone())
                    .or_insert_with(|| key(&desktop::resolved_target(&pin.target)));
                Entry {
                    pin: pin.clone(),
                    pinned: true,
                    windows: self.scan.iter().filter(|w| key(&w.path) == *resolved).cloned().collect(),
                    icon: icon_for(&pin.target),
                }
            })
            .collect();
        if self.config.show_running {
            for window in &self.scan {
                if entries.iter().any(|e| e.windows.iter().any(|w| w.handle == window.handle)) {
                    continue;
                }
                let target = key(&window.path);
                if let Some(entry) = entries.iter_mut().find(|e| key(&e.pin.target) == target) {
                    entry.windows.push(window.clone());
                } else {
                    entries.push(Entry {
                        pin: Pin::from_path(&window.path),
                        pinned: false,
                        windows: vec![window.clone()],
                        icon: icon_for(&window.path),
                    });
                }
            }
            let pinned = self.config.pins.len();
            entries[pinned..]
                .sort_by_key(|e| previous.iter().position(|k| *k == key(&e.pin.target)).unwrap_or(usize::MAX));
        }
        self.entries = entries;
        self.targets.retain(|target, _| self.config.pins.iter().any(|p| p.target == *target));
        self.offset = self.offset.min(self.entries.len().saturating_sub(1));
        self.layout();
        self.render();
    }

    fn layout(&mut self) {
        let monitors = desktop::monitors();
        if let Some(monitor) = monitors.iter().find(|m| m.device == self.config.monitor).or_else(|| monitors.first()) {
            self.monitor = monitor.clone();
        }
        let area = self.monitor.work;
        let icon = self.config.icon_size as f32;
        let count =
            (self.entries.len() + 1).min(capacity((area.right - area.left) as f32, self.scale, icon, self.config.zoom));
        let width =
            ((count as f32 * (icon + 12.0) + 40.0 + icon * (self.config.zoom - 1.0) * 3.0) * self.scale).ceil() as i32;
        let height = ((icon * self.config.zoom + 64.0 + self.config.margin as f32) * self.scale).ceil() as i32;
        self.size = (width.min(area.right - area.left).max(1), height);
        self.origin = (area.left + (area.right - area.left - self.size.0) / 2, area.bottom - height);
        unsafe {
            let _ = SetWindowPos(
                self.hwnd,
                Some(HWND_TOPMOST),
                self.origin.0,
                self.origin.1,
                self.size.0,
                self.size.1,
                SWP_NOACTIVATE,
            );
        }
        let _ = self.content.resize(self.size);
    }

    fn hit(&self, point: (f32, f32)) -> Option<usize> {
        self.rects.iter().find(|(_, rect)| inside(*rect, point)).map(|(index, _)| *index)
    }

    fn render(&mut self) {
        if self.suppressed {
            unsafe {
                let _ = KillTimer(Some(self.hwnd), FRAME);
            }
            return;
        }
        let s = self.scale;
        let icon = self.config.icon_size as f32 * s;
        let width = self.size.0 as f32;
        let height = self.size.1 as f32;
        let slots = capacity(
            (self.monitor.work.right - self.monitor.work.left) as f32,
            s,
            self.config.icon_size as f32,
            self.config.zoom,
        )
        .saturating_sub(1);
        let count = self.entries.len().saturating_sub(self.offset).min(slots);
        let pitch = icon + 12.0 * s;
        let base_left = (width - (count + 1) as f32 * pitch) / 2.0;
        let hide = self.feedback.sample(10_000, if self.hidden { 1.0 } else { 0.0 }, 0.0, 220);
        let bottom = height - (self.config.margin as f32 + 12.0) * s;
        let shift = hide * (icon * self.config.zoom + (self.config.margin as f32 + 24.0) * s);
        let mut sizes = Vec::new();
        for n in 0..=count {
            let i = if n == count { self.entries.len() } else { self.offset + n };
            let strength = self.pointer.filter(|_| self.config.zoom > 1.0).map_or(0.0, |p| {
                let d = ((p.0 - (base_left + (n as f32 + 0.5) * pitch)) / (pitch * 1.8)).abs();
                if p.1 < bottom - icon * self.config.zoom - 10.0 * s { 0.0 } else { (1.0 - d).max(0.0).powi(2) }
            });
            let zoom = self.feedback.sample(i as u64, strength, 0.0, 150);
            sizes.push(icon * (1.0 + (self.config.zoom - 1.0) * zoom));
        }
        let total = sizes.iter().sum::<f32>() + (count as f32 + 2.0) * 10.0 * s;
        let left = (width - total) / 2.0;
        self.panel =
            (left - 4.0 * s, bottom - icon - 10.0 * s + shift, left + total + 4.0 * s, bottom + 10.0 * s + shift);
        self.rects.clear();
        let mut x = left + 10.0 * s;
        for (n, side) in sizes.iter().enumerate() {
            let i = if n == count { self.entries.len() } else { self.offset + n };
            self.rects.push((i, (x, bottom - side + shift, x + side, bottom + shift)));
            x += side + 10.0 * s;
        }
        let hint = self.hover.and_then(|i| self.entries.get(i)).map(|e| e.pin.name.clone());
        let label = self.label_rect();
        let _ = self.content.paint(&self.gpu, |p| self.paint_content(p, hide));
        unsafe {
            let r = if hide > 0.99 { (0.0, height - 4.0 * s, width, height) } else { self.panel };
            let region = CreateRoundRectRgn(
                r.0 as i32,
                r.1 as i32,
                r.2.ceil() as i32,
                r.3.ceil() as i32,
                (20.0 * s) as i32,
                (20.0 * s) as i32,
            );
            if hide <= 0.99 {
                for (_, rect) in &self.rects {
                    let part = CreateRoundRectRgn(
                        (rect.0 - 3.0 * s) as i32,
                        (rect.1 - 3.0 * s) as i32,
                        (rect.2 + 3.0 * s).ceil() as i32,
                        (rect.3 + 3.0 * s).ceil() as i32,
                        (12.0 * s) as i32,
                        (12.0 * s) as i32,
                    );
                    CombineRgn(Some(region), Some(part), Some(region), RGN_OR);
                    let _ = DeleteObject(part.into());
                }
                if hint.is_some() {
                    let part =
                        CreateRoundRectRgn(label.0 as i32, label.1 as i32, label.2 as i32, label.3 as i32, 12, 12);
                    CombineRgn(Some(region), Some(part), Some(region), RGN_OR);
                    let _ = DeleteObject(part.into());
                }
            }
            if SetWindowRgn(self.hwnd, Some(region), false) == 0 {
                let _ = DeleteObject(region.into());
            }
            if self.feedback.active() {
                SetTimer(Some(self.hwnd), FRAME, if self.config.economy { 33 } else { 16 }, None);
            } else {
                let _ = KillTimer(Some(self.hwnd), FRAME);
            }
        }
    }

    fn label_rect(&self) -> Rect {
        let s = self.scale;
        let width = self.size.0 as f32;
        let len = self.hover.and_then(|i| self.entries.get(i)).map_or(0, |e| e.pin.name.chars().count());
        let label_width = ((len as f32 * 7.0 + 24.0).clamp(64.0, 280.0) * s).min(width - 8.0 * s);
        let center =
            self.rects.iter().find(|(i, _)| Some(*i) == self.hover).map_or(width / 2.0, |(_, r)| (r.0 + r.2) / 2.0);
        let left = (center - label_width / 2.0).clamp(4.0 * s, (width - label_width - 4.0 * s).max(4.0 * s));
        (left, 8.0 * s, left + label_width, 34.0 * s)
    }

    fn paint_content(&self, p: &crate::ui::gfx::Painter, hide: f32) {
        let s = self.scale;
        let icon = self.config.icon_size as f32 * s;
        let width = self.size.0 as f32;
        let height = self.size.1 as f32;
        let bottom = height - (self.config.margin as f32 + 12.0) * s;
        let shift = hide * (icon * self.config.zoom + (self.config.margin as f32 + 24.0) * s);
        let slots = capacity(
            (self.monitor.work.right - self.monitor.work.left) as f32,
            s,
            self.config.icon_size as f32,
            self.config.zoom,
        )
        .saturating_sub(1);
        let hint = self.hover.and_then(|i| self.entries.get(i)).map(|e| e.pin.name.clone());
        let label = self.label_rect();

        if hide > 0.99 {
            let _ = p.fill_round_rect(
                (width / 2.0 - 24.0 * s, height - 3.0 * s, width / 2.0 + 24.0 * s, height),
                1.5 * s,
                rgba(0.7, 0.7, 0.75, 0.45),
            );
            return;
        }
        let (r, g, b) = crate::ui::theme::panel_u8();
        let _ = p.fill_round_rect(
            self.panel,
            22.0 * s,
            rgba(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, self.config.opacity),
        );
        let _ = p.stroke_round_rect(self.panel, 22.0 * s, s, rgba(1.0, 1.0, 1.0, 0.15));
        for &(i, rect) in &self.rects {
            let active = self.entries.get(i).is_some_and(|e| e.windows.iter().any(|w| w.active));
            if self.hover == Some(i) || active {
                let _ = p.fill_round_rect(
                    (rect.0 - 3.0 * s, rect.1 - 3.0 * s, rect.2 + 3.0 * s, rect.3 + 3.0 * s),
                    13.0 * s,
                    rgba(1.0, 1.0, 1.0, if active { 0.12 } else { 0.06 }),
                );
            }
            if self.dragging && self.hover == Some(i) && self.entries.get(i).is_some_and(|e| e.pinned) {
                let _ = p.line(
                    (rect.0 - 5.0 * s, bottom - icon + shift),
                    (rect.0 - 5.0 * s, bottom + shift),
                    2.0 * s,
                    rgba(0.65, 0.8, 1.0, 0.9),
                );
            }
            let shrink = if self.pressed.is_some_and(|(index, _)| index == i) { 2.0 * s } else { 0.0 };
            let image = (rect.0 + shrink, rect.1 + shrink, rect.2 - shrink, rect.3 - shrink);
            if let Some(entry) = self.entries.get(i) {
                if let Some(icon) = &entry.icon {
                    let _ = p.image(image, 0.0, icon.side, &icon.bgra);
                } else {
                    let _ = p.glyph('\u{E8A5}', image, icon * 0.65, rgba(0.85, 0.85, 0.9, 1.0));
                }
                if !entry.windows.is_empty() {
                    let cx = (rect.0 + rect.2) / 2.0;
                    let _ = p.fill_ellipse(
                        cx,
                        bottom + shift + 6.0 * s,
                        if active { 3.0 } else { 2.0 } * s,
                        2.0 * s,
                        rgba(1.0, 1.0, 1.0, if active { 0.95 } else { 0.5 }),
                    );
                }
                if i > 0 && !entry.pinned && self.entries[i - 1].pinned {
                    let _ = p.line(
                        (rect.0 - 5.0 * s, bottom - icon * 0.7 + shift),
                        (rect.0 - 5.0 * s, bottom - icon * 0.3 + shift),
                        s,
                        rgba(1.0, 1.0, 1.0, 0.2),
                    );
                }
            } else {
                let _ = p.glyph('\u{E710}', image, icon * 0.42, rgba(0.75, 0.75, 0.8, 0.9));
            }
        }
        if let Some(text) = &hint
            && hide < 0.01
        {
            let _ = p.fill_round_rect(label, 9.0 * s, rgba(0.06, 0.06, 0.07, 0.97));
            let _ = p.text(
                text,
                label,
                12.0 * s,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_TEXT_ALIGNMENT_CENTER,
                rgba(0.95, 0.95, 0.97, 1.0),
            );
        }
        if self.entries.len() > slots && hide < 0.01 {
            let _ = p.fill_ellipse(
                self.panel.2 - 5.0 * s,
                (self.panel.1 + self.panel.3) / 2.0,
                1.5 * s,
                5.0 * s,
                rgba(1.0, 1.0, 1.0, 0.4),
            );
        }
    }

    fn pulse(&mut self) {
        self.feedback.set_enabled(motion_allowed());
        if self.scanned_at.elapsed() >= Duration::from_secs(2) {
            self.refresh(false);
        }
        let suppressed = self.config.hide_fullscreen && desktop::fullscreen(&self.monitor);
        if suppressed != self.suppressed {
            self.suppressed = suppressed;
            self.pointer = None;
            self.hover = None;
            unsafe {
                let _ = ShowWindow(self.hwnd, if suppressed { SW_HIDE } else { SW_SHOWNOACTIVATE });
            }
            self.render();
        }
        let mut pt = POINT::default();
        unsafe {
            let _ = GetCursorPos(&mut pt);
        }
        let point = ((pt.x - self.origin.0) as f32, (pt.y - self.origin.1) as f32);
        let near = inside(
            (0.0, self.panel.1.min(self.size.1 as f32 - 12.0 * self.scale), self.size.0 as f32, self.size.1 as f32),
            point,
        );
        if near || self.menu_open || self.pressed.is_some() {
            self.leave_at = Instant::now();
        }
        let hidden = self.config.auto_hide
            && !near
            && !self.menu_open
            && self.pressed.is_none()
            && self.leave_at.elapsed() > Duration::from_millis(650);
        if hidden != self.hidden {
            self.hidden = hidden;
            self.render();
        }
    }
}

extern "system" fn proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
        if ptr.is_null() {
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }
        match msg {
            WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
            WM_ERASEBKGND => LRESULT(1),
            WM_TIMER => {
                let state = &mut *ptr;
                if wparam.0 == FRAME {
                    state.render();
                } else if wparam.0 == PULSE {
                    state.pulse();
                }
                LRESULT(0)
            }
            WM_MOUSEMOVE => {
                let state = &mut *ptr;
                let point = (lparam.0 as i16 as f32, (lparam.0 >> 16) as i16 as f32);
                state.pointer = Some(point);
                state.hover = state.hit(point);
                state.leave_at = Instant::now();
                state.hidden = false;
                if let Some((_, start)) = state.pressed {
                    state.dragging |= (point.0 - start).abs() > 7.0 * state.scale;
                }
                let _ = TrackMouseEvent(&mut TRACKMOUSEEVENT {
                    cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                });
                state.render();
                LRESULT(0)
            }
            WM_MOUSELEAVE => {
                let state = &mut *ptr;
                state.pointer = None;
                state.hover = None;
                state.leave_at = Instant::now();
                state.render();
                LRESULT(0)
            }
            WM_LBUTTONDOWN => {
                let state = &mut *ptr;
                let point = (lparam.0 as i16 as f32, (lparam.0 >> 16) as i16 as f32);
                state.pressed = state.hit(point).map(|i| (i, point.0));
                state.dragging = false;
                state.render();
                SetCapture(hwnd);
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                let state = &mut *ptr;
                let point = (lparam.0 as i16 as f32, (lparam.0 >> 16) as i16 as f32);
                let pressed = state.pressed.take();
                let hit = state.hit(point);
                if let Some((from, _)) = pressed {
                    if state.dragging {
                        if let Some(to) = hit
                            && from < state.config.pins.len()
                            && to < state.config.pins.len()
                            && from != to
                        {
                            let pin = state.config.pins.remove(from);
                            state.config.pins.insert(to, pin);
                            state.post(Request::Save);
                            state.refresh(true);
                        }
                    } else if hit == Some(from) {
                        if let Some(entry) = state.entries.get(from).cloned() {
                            state.post(Request::Launch(entry));
                        } else {
                            state.post(Request::Add);
                        }
                    }
                }
                state.dragging = false;
                state.render();
                let _ = ReleaseCapture();
                LRESULT(0)
            }
            WM_CAPTURECHANGED => {
                let state = &mut *ptr;
                state.pressed = None;
                state.dragging = false;
                state.render();
                LRESULT(0)
            }
            WM_RBUTTONUP => {
                let state = &mut *ptr;
                let point = (lparam.0 as i16 as f32, (lparam.0 >> 16) as i16 as f32);
                let entry = state.hit(point).and_then(|i| state.entries.get(i)).cloned();
                state.menu_open = true;
                state.post(Request::Menu(entry));
                LRESULT(0)
            }
            WM_MOUSEWHEEL => {
                let state = &mut *ptr;
                let down = ((wparam.0 >> 16) as i16) < 0;
                let slots = capacity(
                    (state.monitor.work.right - state.monitor.work.left) as f32,
                    state.scale,
                    state.config.icon_size as f32,
                    state.config.zoom,
                )
                .saturating_sub(1);
                state.offset = if down {
                    (state.offset + 1).min(state.entries.len().saturating_sub(slots))
                } else {
                    state.offset.saturating_sub(1)
                };
                state.hover = None;
                state.pointer = None;
                state.render();
                LRESULT(0)
            }
            WM_DROPFILES => {
                let state = &mut *ptr;
                let drop = HDROP(wparam.0 as *mut _);
                let mut changed = false;
                for i in 0..DragQueryFileW(drop, u32::MAX, None).min(32) {
                    let n = DragQueryFileW(drop, i, None);
                    if n == 0 || n > 2048 {
                        continue;
                    }
                    let mut text = vec![0u16; n as usize + 1];
                    let len = DragQueryFileW(drop, i, Some(&mut text)) as usize;
                    changed |= state.config.add(Pin::from_path(&String::from_utf16_lossy(&text[..len])));
                }
                DragFinish(drop);
                if changed {
                    state.post(Request::Save);
                    state.refresh(true);
                }
                LRESULT(0)
            }
            WM_DISPLAYCHANGE | WM_SETTINGCHANGE => {
                let state = &mut *ptr;
                state.layout();
                state.render();
                LRESULT(0)
            }
            WM_DPICHANGED => {
                let _ = PostMessageW(Some(hwnd), WM_APP + 71, WPARAM(0), LPARAM(0));
                LRESULT(0)
            }
            m if m == WM_APP + 71 => {
                let state = &mut *ptr;
                let scale = GetDpiForWindow(hwnd) as f32 / 96.0;
                if (scale - state.scale).abs() > 0.01 {
                    state.scale = scale;
                    state.layout();
                    state.render();
                }
                LRESULT(0)
            }
            WM_CLOSE => LRESULT(0),
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

impl Dock {
    pub fn new(parent: HWND, compositor: &Compositor, gpu: &Gpu, config: &DockSettings) -> Result<Self> {
        Self::create(parent, compositor, gpu, config, true)
    }

    fn create(parent: HWND, compositor: &Compositor, gpu: &Gpu, config: &DockSettings, visible: bool) -> Result<Self> {
        static CLASS: Once = Once::new();
        let instance = unsafe { GetModuleHandleW(None)? };
        CLASS.call_once(|| unsafe {
            RegisterClassW(&WNDCLASSW {
                lpfnWndProc: Some(proc),
                hInstance: instance.into(),
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                lpszClassName: w!("NightIsland.DesktopDock"),
                ..Default::default()
            });
        });
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                w!("NightIsland.DesktopDock"),
                w!("Night Island Dock"),
                WS_POPUP,
                0,
                0,
                1,
                1,
                None,
                None,
                Some(instance.into()),
                None,
            )?
        };
        struct Guard(HWND);
        impl Drop for Guard {
            fn drop(&mut self) {
                unsafe {
                    let _ = DestroyWindow(self.0);
                }
            }
        }
        let guard = Guard(hwnd);
        let target = unsafe { compositor.cast::<ICompositorDesktopInterop>()?.CreateDesktopWindowTarget(hwnd, true)? };
        let root = compositor.CreateContainerVisual()?;
        root.SetRelativeSizeAdjustment(Vector2 { X: 1.0, Y: 1.0 })?;
        let content = gpu.layer(compositor, (1, 1))?;
        root.Children()?.InsertAtTop(&content.sprite)?;
        target.SetRoot(&root)?;
        let monitor = desktop::monitors().into_iter().next().ok_or_else(windows::core::Error::empty)?;
        let mut state = Box::new(State {
            hwnd,
            parent,
            config: config.clone(),
            gpu: gpu.popup(),
            content,
            _target: target,
            _root: root,
            entries: Vec::new(),
            scan: Vec::new(),
            targets: HashMap::new(),
            monitor,
            scale: unsafe { GetDpiForWindow(hwnd) } as f32 / 96.0,
            size: (1, 1),
            origin: (0, 0),
            rects: Vec::new(),
            panel: (0.0, 0.0, 1.0, 1.0),
            feedback: Feedback::default(),
            pointer: None,
            hover: None,
            pressed: None,
            dragging: false,
            leave_at: Instant::now(),
            scanned_at: Instant::now(),
            hidden: false,
            suppressed: false,
            offset: 0,
            request: None,
            menu_open: false,
        });
        state.feedback.set_enabled(motion_allowed());
        state.refresh(true);
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, (&raw mut *state) as isize);
            DragAcceptFiles(hwnd, true);
            SetTimer(Some(hwnd), PULSE, 200, None);
            if visible {
                let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            }
        }
        std::mem::forget(guard);
        Ok(Self { state })
    }

    pub fn update(&mut self, config: &DockSettings) {
        self.state.config = config.clone();
        self.state.refresh(true);
        self.state.pulse();
    }

    pub fn refresh_windows(&mut self) {
        self.state.refresh(false);
    }

    pub fn set_capture_hidden(&self, hidden: bool) {
        unsafe {
            let _ = SetWindowDisplayAffinity(self.state.hwnd, if hidden { WDA_EXCLUDEFROMCAPTURE } else { WDA_NONE });
        }
    }

    pub fn config(&self) -> DockSettings {
        self.state.config.clone()
    }
    pub fn take_request(&mut self) -> Option<Request> {
        self.state.request.take()
    }
    pub fn close_menu(&mut self) {
        self.state.menu_open = false;
        self.state.leave_at = Instant::now();
        self.state.pointer = None;
        self.state.hover = None;
        self.state.render();
    }
}

impl Drop for Dock {
    fn drop(&mut self) {
        unsafe {
            let _ = KillTimer(Some(self.state.hwnd), PULSE);
            let _ = KillTimer(Some(self.state.hwnd), FRAME);
            SetWindowLongPtrW(self.state.hwnd, GWLP_USERDATA, 0);
            let _ = DestroyWindow(self.state.hwnd);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "Requires an isolated Windows Composition runtime"]
    fn native_dock_regions_dpi_reorder_import_and_cleanup() -> Result<()> {
        use windows::Win32::Graphics::Gdi::{CreateRectRgn, GetWindowRgn, PtInRegion};
        use windows::Win32::System::WinRT::{
            CreateDispatcherQueueController, DQTAT_COM_NONE, DQTYPE_THREAD_CURRENT, DispatcherQueueOptions,
            RO_INIT_SINGLETHREADED, RoInitialize, RoUninitialize,
        };
        unsafe {
            RoInitialize(RO_INIT_SINGLETHREADED)?;
        }
        struct Runtime;
        impl Drop for Runtime {
            fn drop(&mut self) {
                unsafe {
                    RoUninitialize();
                }
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
        let compositor = Compositor::new()?;
        let gpu = Gpu::new(&compositor)?;
        let parent = unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW,
                w!("STATIC"),
                w!("Dock test owner"),
                WS_POPUP,
                0,
                0,
                1,
                1,
                None,
                None,
                Some(GetModuleHandleW(None)?.into()),
                None,
            )?
        };
        let mut config = DockSettings { enabled: true, show_running: false, ..Default::default() };
        if let Some(path) = std::env::var_os("NI_DOCK_IMPORT_FILE") {
            config.pins = crate::data::desktop_dock::import_mydock(&std::fs::read(path).unwrap()).unwrap();
            assert!(config.pins.len() >= 2);
        } else {
            config.add(Pin::from_path(r"C:\Windows\System32\notepad.exe"));
            config.add(Pin::from_path(r"C:\Windows\System32\calc.exe"));
        }
        let mut dock = Dock::create(parent, &compositor, &gpu, &config, false)?;
        let hwnd = dock.state.hwnd;
        for scale in [1.0, 1.5, 2.0] {
            dock.state.scale = scale;
            dock.state.feedback.set_enabled(false);
            dock.state.layout();
            dock.state.render();
            let panel = dock.state.panel;
            unsafe {
                let region = CreateRectRgn(0, 0, 0, 0);
                assert!(GetWindowRgn(hwnd, region).0 > 0);
                assert!(!PtInRegion(region, 0, 0).as_bool());
                assert!(
                    PtInRegion(region, ((panel.0 + panel.2) / 2.0) as i32, (panel.3 - 4.0 * scale) as i32).as_bool()
                );
                let _ = DeleteObject(region.into());
            }
            let rect = dock.state.rects[0].1;
            let x = (rect.0 + rect.2) / 2.0;
            let y = (rect.1 + rect.3) / 2.0;
            dock.state.pointer = Some((x, y));
            dock.state.hover = Some(0);
            dock.state.render();
            assert!(dock.state.rects[0].1.2 - dock.state.rects[0].1.0 > config.icon_size as f32 * scale);
            if let Some(folder) = std::env::var_os("NI_DOCK_PREVIEW") {
                let folder = std::path::PathBuf::from(folder);
                std::fs::create_dir_all(&folder).unwrap();
                let surface = crate::ui::popup::PopupSurface::new(dock.state.size)?;
                surface.paint(&gpu, |p| {
                    p.fill_rect(
                        (0.0, 0.0, dock.state.size.0 as f32, dock.state.size.1 as f32),
                        rgba(0.025, 0.06, 0.12, 1.0),
                    )?;
                    dock.state.paint_content(p, 0.0);
                    Ok(())
                })?;
                let mut pixels = surface.pixels();
                for px in pixels.chunks_exact_mut(4) {
                    px.swap(0, 2);
                }
                let mut encoder = png::Encoder::new(
                    std::fs::File::create(folder.join(format!("dock-{scale:.1}.png"))).unwrap(),
                    dock.state.size.0 as u32,
                    dock.state.size.1 as u32,
                );
                encoder.set_color(png::ColorType::Rgba);
                encoder.set_depth(png::BitDepth::Eight);
                encoder.write_header().unwrap().write_image_data(&pixels).unwrap();
            }
            dock.state.pointer = None;
            dock.state.hover = None;
            dock.state.hidden = true;
            dock.state.render();
            unsafe {
                let region = CreateRectRgn(0, 0, 0, 0);
                assert!(GetWindowRgn(hwnd, region).0 > 0);
                assert!(!PtInRegion(region, dock.state.size.0 / 2, dock.state.size.1 - 20).as_bool());
                let mut bounds = windows::Win32::Foundation::RECT::default();
                windows::Win32::Graphics::Gdi::GetRgnBox(region, &mut bounds);
                assert!(
                    PtInRegion(region, dock.state.size.0 / 2, dock.state.size.1 - 2).as_bool(),
                    "scale={scale} size={:?} region={bounds:?}",
                    dock.state.size
                );
                let _ = DeleteObject(region.into());
            }
            assert!(!dock.state.feedback.active());
            dock.state.hidden = false;
            dock.state.render();
        }
        if let Some(folder) = std::env::var_os("NI_DOCK_PREVIEW") {
            let mut ui = crate::view::UiState::new();
            ui.cfg.desktop_dock = config.clone();
            ui.sw.group = crate::view::GROUPS.iter().position(|g| g.name == "Desktop dock").unwrap();
            let size = crate::view::SW_SIZE;
            for (name, scroll) in [("settings", 0.0), ("settings-bottom", crate::view::max_scroll(size, 1.0, &ui))] {
                ui.sw.scroll = scroll;
                let surface = crate::ui::popup::PopupSurface::new((size.0 as i32, size.1 as i32))?;
                surface.paint(&gpu, |p| crate::view::paint_settings_window(p, size, 1.0, &ui))?;
                let mut pixels = surface.pixels();
                for px in pixels.chunks_exact_mut(4) {
                    px.swap(0, 2);
                }
                let file = std::path::PathBuf::from(&folder).join(format!("{name}.png"));
                let mut encoder = png::Encoder::new(std::fs::File::create(file).unwrap(), size.0 as u32, size.1 as u32);
                encoder.set_color(png::ColorType::Rgba);
                encoder.set_depth(png::BitDepth::Eight);
                encoder.write_header().unwrap().write_image_data(&pixels).unwrap();
            }
        }
        let before = dock.config().pins;
        let a = dock.state.rects[0].1;
        let b = dock.state.rects[1].1;
        let pack = |r: Rect| {
            LPARAM(((((r.1 + r.3) / 2.0) as i32 as u32) << 16 | (((r.0 + r.2) / 2.0) as i32 as u32 & 0xffff)) as isize)
        };
        unsafe {
            SendMessageW(hwnd, WM_LBUTTONDOWN, Some(WPARAM(0)), Some(pack(a)));
            SendMessageW(hwnd, WM_MOUSEMOVE, Some(WPARAM(0)), Some(pack(b)));
            SendMessageW(hwnd, WM_LBUTTONUP, Some(WPARAM(0)), Some(pack(b)));
        }
        assert!(matches!(dock.take_request(), Some(Request::Save)));
        assert_eq!(dock.config().pins[1], before[0]);
        assert_eq!(dock.config().pins[0], before[1]);
        let mut config = dock.config();
        config.pins.clear();
        config.monitor = "disconnected-monitor".into();
        dock.update(&config);
        assert_eq!(dock.state.rects.len(), 1);
        assert!(dock.state.monitor.primary);
        drop(dock);
        unsafe {
            assert!(!IsWindow(Some(hwnd)).as_bool());
            DestroyWindow(parent)?;
        }
        Ok(())
    }

    #[test]
    fn overflow_capacity_leaves_room_for_magnification_and_add_button() {
        for scale in [1.0, 1.5, 2.0] {
            for icon in [32.0, 48.0, 72.0] {
                let count = capacity(1280.0, scale, icon, 1.7);
                let width = (count as f32 * (icon + 12.0) + 40.0 + icon * 0.7 * 3.0) * scale;
                assert!(width <= 1280.0);
                assert!(count >= 2);
            }
        }
    }
}
