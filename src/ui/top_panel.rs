use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::Once;
use std::time::Instant;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FONT_WEIGHT_NORMAL, DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_TEXT_ALIGNMENT_LEADING,
};
use windows::Win32::Graphics::Gdi::{BeginPaint, EndPaint, PAINTSTRUCT};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::WM_MOUSELEAVE;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetCapture, ReleaseCapture, SetCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent, VK_ESCAPE,
    VK_LBUTTON,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{Result, w};

use crate::data::top_panel::TopPanelSettings;
use crate::platform::{apps, desktop_dock as desktop, system::Radios};
use crate::ui::{
    gfx::{Gpu, Painter, rgba},
    i18n::tr,
    popup::PopupSurface,
};
use crate::view::Rect;

pub const WM_TOP_PANEL: u32 = WM_APP + 90;
const FRAME: usize = 1;
const DISMISS: usize = 2;
const MARGIN: f32 = 10.0;
const HEIGHT: f32 = 34.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Block {
    Application,
    Wifi,
    Bluetooth,
    Volume,
    Battery,
}

#[derive(Clone, Default, PartialEq)]
pub struct Snapshot {
    pub active: Option<desktop::Window>,
    pub windows: Vec<desktop::Window>,
    pub radios: Radios,
    pub volume: Option<u8>,
    pub muted: bool,
    pub device: String,
    pub battery: Option<(u8, bool)>,
}

pub enum Request {
    Settings,
    Open(Block),
    WindowsSettings(Block),
    ToggleWifi,
    ToggleBluetooth,
    Volume(u8),
    Mute,
    Mixer,
    Window(desktop::Window, desktop::WindowAction),
    Activate(desktop::Window),
}

#[derive(Clone, Copy, PartialEq)]
enum Hit {
    Block(Block),
    Settings,
    Close,
    Toggle,
    Volume,
    Mute,
    Mixer,
    WindowsSettings,
    Minimize,
    Maximize,
    CloseWindow,
    Window(usize),
}

#[derive(Clone, Copy)]
enum Role {
    Left,
    Right,
    Card(Block),
}

struct Button {
    rect: Rect,
    hit: Hit,
    enabled: bool,
}

struct State {
    hwnd: HWND,
    parent: HWND,
    id: usize,
    gpu: Gpu,
    role: Role,
    settings: TopPanelSettings,
    snapshot: Snapshot,
    icon: Option<std::sync::Arc<apps::Icon>>,
    surface: RefCell<Option<PopupSurface>>,
    size: (i32, i32),
    origin: (i32, i32),
    scale: f32,
    buttons: Vec<Button>,
    hover: Option<Hit>,
    pressed: Option<Hit>,
    requests: VecDeque<Request>,
    visible: bool,
    fade: Instant,
    opening: bool,
    alpha: u8,
    dirty: bool,
}

struct Wing {
    state: Box<State>,
}

pub struct TopPanel {
    left: Wing,
    right: Wing,
    card: Wing,
    pub(super) settings: TopPanelSettings,
    icons: apps::DockIconCache,
    pub(super) reservation: Option<super::top_panel_space::Reservation>,
    pub(super) monitor: desktop::Monitor,
    pub(super) scale: f32,
    snapshot: Snapshot,
    shown: bool,
}

fn inside(r: Rect, p: (f32, f32)) -> bool {
    p.0 >= r.0 && p.0 < r.2 && p.1 >= r.1 && p.1 < r.3
}
fn overlaps(a: Rect, b: Rect) -> bool {
    a.0 < b.2 && b.0 < a.2 && a.1 < b.3 && b.1 < a.3
}
fn scaled(r: Rect, s: f32) -> Rect {
    (r.0 * s, r.1 * s, r.2 * s, r.3 * s)
}

fn app_name(snapshot: &Snapshot) -> String {
    snapshot
        .active
        .as_ref()
        .map(|window| {
            if window.island_settings {
                return "Night Island".into();
            }
            std::path::Path::new(&window.path).file_stem().unwrap_or_default().to_string_lossy().into_owned()
        })
        .unwrap_or_else(|| tr("Desktop").into_owned())
}

fn block_width(block: Block) -> f32 {
    match block {
        Block::Volume | Block::Battery => 68.0,
        _ => 42.0,
    }
}

fn blocks(settings: &TopPanelSettings) -> Vec<Block> {
    [
        (settings.wifi, Block::Wifi),
        (settings.bluetooth, Block::Bluetooth),
        (settings.volume, Block::Volume),
        (settings.battery, Block::Battery),
    ]
    .into_iter()
    .filter_map(|(on, block)| on.then_some(block))
    .collect()
}

impl State {
    fn post(&mut self, request: Request) {
        if matches!(request, Request::Volume(_)) {
            self.requests.retain(|r| !matches!(r, Request::Volume(_)));
        }
        if self.requests.len() < 32 {
            self.requests.push_back(request);
            unsafe {
                let _ = PostMessageW(Some(self.parent), WM_TOP_PANEL, WPARAM(self.id), LPARAM(0));
            }
        }
    }

    fn dimensions(&self) -> (f32, f32) {
        match self.role {
            Role::Left => (214.0, HEIGHT + 2.0 * MARGIN),
            Role::Right => (
                blocks(&self.settings).iter().copied().map(block_width).sum::<f32>() + 2.0 * MARGIN + 8.0,
                HEIGHT + 2.0 * MARGIN,
            ),
            Role::Card(Block::Application) => (306.0, 210.0 + self.snapshot.windows.len().min(3) as f32 * 32.0),
            Role::Card(Block::Volume) => (306.0, 190.0),
            Role::Card(_) => (306.0, 176.0),
        }
    }

    fn layout(&mut self) {
        let (width, _) = self.dimensions();
        self.buttons.clear();
        let mut add = |rect, hit, enabled| self.buttons.push(Button { rect, hit, enabled });
        match self.role {
            Role::Left => add((10.0, 10.0, width - 10.0, 44.0), Hit::Block(Block::Application), true),
            Role::Right => {
                let mut x = 14.0;
                for block in blocks(&self.settings) {
                    let next = x + block_width(block);
                    add((x, 12.0, next, 42.0), Hit::Block(block), true);
                    x = next;
                }
            }
            Role::Card(block) => {
                add((width - 45.0, 20.0, width - 21.0, 44.0), Hit::Close, true);
                match block {
                    Block::Application => {
                        let enabled = self.snapshot.active.is_some();
                        add((24.0, 88.0, 106.0, 132.0), Hit::Minimize, enabled);
                        add((112.0, 88.0, 194.0, 132.0), Hit::Maximize, enabled);
                        add((200.0, 88.0, 282.0, 132.0), Hit::CloseWindow, enabled);
                        for i in 0..self.snapshot.windows.len().min(3) {
                            let y = 142.0 + i as f32 * 32.0;
                            add((24.0, y, width - 24.0, y + 30.0), Hit::Window(i), true);
                        }
                        let y = 146.0 + self.snapshot.windows.len().min(3) as f32 * 32.0;
                        add((24.0, y + 16.0, width - 24.0, y + 44.0), Hit::Settings, true);
                    }
                    Block::Volume => {
                        add((24.0, 79.0, 58.0, 117.0), Hit::Mute, self.snapshot.volume.is_some());
                        add((65.0, 79.0, width - 57.0, 117.0), Hit::Volume, self.snapshot.volume.is_some());
                        add((24.0, 133.0, 150.0, 169.0), Hit::Mixer, true);
                        add((156.0, 133.0, width - 24.0, 169.0), Hit::WindowsSettings, true);
                    }
                    Block::Wifi | Block::Bluetooth => {
                        let available = if block == Block::Wifi {
                            self.snapshot.radios.wifi
                        } else {
                            self.snapshot.radios.bluetooth
                        };
                        add((24.0, 66.0, width - 24.0, 108.0), Hit::Toggle, available.is_some());
                        add((24.0, 123.0, width - 24.0, 155.0), Hit::WindowsSettings, true);
                    }
                    Block::Battery => add((24.0, 123.0, width - 24.0, 155.0), Hit::WindowsSettings, true),
                }
            }
        }
    }

    fn draw(&self, p: &Painter) -> Result<()> {
        let s = self.scale;
        let (w, h) = self.dimensions();
        let text = |value: &str, r: Rect, size: f32, strong: bool, alpha: f32| {
            p.text(
                value,
                scaled(r, s),
                size * s,
                if strong { DWRITE_FONT_WEIGHT_SEMI_BOLD } else { DWRITE_FONT_WEIGHT_NORMAL },
                DWRITE_TEXT_ALIGNMENT_LEADING,
                rgba(1.0, 1.0, 1.0, alpha),
            )
        };
        let glyph = |value: char, r: Rect, size: f32, alpha: f32| {
            p.glyph(value, scaled(r, s), size * s, rgba(1.0, 1.0, 1.0, alpha))
        };
        match self.role {
            Role::Card(_) => crate::view::paint_popup_panel(p, (w * s, h * s), s)?,
            _ => {
                let (r, g, b) = crate::ui::theme::panel();
                let panel = scaled((10.0, 10.0, w - 10.0, h - 10.0), s);
                p.fill_round_rect(panel, 17.0 * s, rgba(r, g, b, 0.98))?;
                p.stroke_round_rect(panel, 17.0 * s, s, rgba(1.0, 1.0, 1.0, 0.09))?;
            }
        }
        for button in &self.buttons {
            if self.hover == Some(button.hit) && button.enabled {
                p.fill_round_rect(scaled(button.rect, s), 12.0 * s, rgba(1.0, 1.0, 1.0, 0.085))?;
            }
        }
        match self.role {
            Role::Left => {
                if let Some(icon) = &self.icon {
                    p.image(scaled((22.0, 19.0, 38.0, 35.0), s), 0.0, icon.side, &icon.bgra)?;
                } else {
                    glyph('\u{E737}', (22.0, 18.0, 40.0, 36.0), 15.0, 0.7)?;
                }
                text(&app_name(&self.snapshot), (48.0, 14.0, w - 40.0, 40.0), 12.0, true, 0.92)?;
                glyph('\u{E70D}', (w - 34.0, 19.0, w - 18.0, 35.0), 9.0, 0.45)?;
            }
            Role::Right => {
                for button in &self.buttons {
                    let Hit::Block(block) = button.hit else { continue };
                    let x = button.rect.0;
                    let (icon, value, active) = match block {
                        Block::Wifi => ('\u{E701}', String::new(), self.snapshot.radios.wifi == Some(true)),
                        Block::Bluetooth => ('\u{E702}', String::new(), self.snapshot.radios.bluetooth == Some(true)),
                        Block::Volume => (
                            if self.snapshot.muted { '\u{E74F}' } else { '\u{E767}' },
                            self.snapshot.volume.map(|v| v.to_string()).unwrap_or_else(|| "—".into()),
                            true,
                        ),
                        Block::Battery => (
                            if self.snapshot.battery.is_some() { '\u{E83F}' } else { '\u{E945}' },
                            self.snapshot.battery.map(|(v, _)| format!("{v}%")).unwrap_or_else(|| "AC".into()),
                            true,
                        ),
                        _ => continue,
                    };
                    let gx = if value.is_empty() { x + 12.0 } else { x + 7.0 };
                    glyph(icon, (gx, 18.0, gx + 18.0, 36.0), 15.0, if active { 0.92 } else { 0.4 })?;
                    if !value.is_empty() {
                        text(&value, (gx + 23.0, 14.0, button.rect.2 - 3.0, 40.0), 11.0, false, 0.8)?;
                    }
                }
            }
            Role::Card(block) => {
                let title = match block {
                    Block::Application => app_name(&self.snapshot),
                    Block::Wifi => "Wi-Fi".into(),
                    Block::Bluetooth => "Bluetooth".into(),
                    Block::Volume => tr("System volume").into_owned(),
                    Block::Battery => tr("Battery and power").into_owned(),
                };
                text(&title, (26.0, 20.0, w - 55.0, 46.0), 14.0, true, 0.96)?;
                glyph('\u{E711}', (w - 43.0, 23.0, w - 23.0, 43.0), 11.0, 0.5)?;
                match block {
                    Block::Application => {
                        text(
                            self.snapshot
                                .active
                                .as_ref()
                                .map(|w| w.title.as_str())
                                .unwrap_or(&tr("No active application")),
                            (26.0, 49.0, w - 26.0, 75.0),
                            11.0,
                            false,
                            0.5,
                        )?;
                        for (x, icon, label, danger) in [
                            (24.0, '\u{E921}', "Minimize window", false),
                            (112.0, '\u{E922}', "Maximize window", false),
                            (200.0, '\u{E711}', "Close", true),
                        ] {
                            p.fill_round_rect(
                                scaled((x, 88.0, x + 82.0, 132.0), s),
                                12.0 * s,
                                if danger { rgba(1.0, 0.3, 0.3, 0.09) } else { rgba(1.0, 1.0, 1.0, 0.06) },
                            )?;
                            glyph(
                                icon,
                                (x + 31.0, 91.0, x + 51.0, 111.0),
                                12.0,
                                if self.snapshot.active.is_some() { 0.85 } else { 0.25 },
                            )?;
                            let label = match label {
                                "Minimize window" => "Minimize",
                                "Maximize window" => "Maximize",
                                other => other,
                            };
                            text(&tr(label), (x + 6.0, 109.0, x + 78.0, 130.0), 9.0, false, 0.6)?;
                        }
                        for (i, window) in self.snapshot.windows.iter().take(3).enumerate() {
                            let y = 142.0 + i as f32 * 32.0;
                            glyph('\u{E737}', (29.0, y + 7.0, 44.0, y + 22.0), 12.0, 0.5)?;
                            text(&window.title, (54.0, y, w - 30.0, y + 30.0), 11.0, false, 0.8)?;
                        }
                        let y = 162.0 + self.snapshot.windows.len().min(3) as f32 * 32.0;
                        text(&tr("Top panel"), (34.0, y, w - 34.0, y + 28.0), 11.0, false, 0.5)?;
                    }
                    Block::Volume => {
                        text(
                            &if self.snapshot.device.is_empty() {
                                tr("Unavailable").into_owned()
                            } else {
                                self.snapshot.device.clone()
                            },
                            (26.0, 48.0, w - 26.0, 71.0),
                            11.0,
                            false,
                            0.5,
                        )?;
                        glyph(
                            if self.snapshot.muted { '\u{E74F}' } else { '\u{E767}' },
                            (29.0, 89.0, 49.0, 109.0),
                            16.0,
                            0.8,
                        )?;
                        let level = self.snapshot.volume.unwrap_or(0) as f32 / 100.0;
                        let x = 69.0 + level * (w - 132.0);
                        p.fill_round_rect(scaled((69.0, 97.0, w - 63.0, 101.0), s), 2.0 * s, rgba(1.0, 1.0, 1.0, 0.2))?;
                        p.fill_round_rect(scaled((69.0, 97.0, x, 101.0), s), 2.0 * s, rgba(1.0, 1.0, 1.0, 0.9))?;
                        p.fill_ellipse(x * s, 99.0 * s, 5.0 * s, 5.0 * s, rgba(1.0, 1.0, 1.0, 1.0))?;
                        text(
                            &self.snapshot.volume.map(|v| v.to_string()).unwrap_or_else(|| "—".into()),
                            (w - 48.0, 85.0, w - 22.0, 113.0),
                            12.0,
                            false,
                            0.6,
                        )?;
                        for (x, label) in [(24.0, "Mixer"), (156.0, "Settings")] {
                            p.fill_round_rect(
                                scaled((x, 133.0, x + 126.0, 169.0), s),
                                12.0 * s,
                                rgba(1.0, 1.0, 1.0, 0.06),
                            )?;
                            text(&tr(label), (x + 12.0, 133.0, x + 116.0, 169.0), 11.0, false, 0.85)?;
                        }
                    }
                    Block::Wifi | Block::Bluetooth => {
                        let value = if block == Block::Wifi {
                            self.snapshot.radios.wifi
                        } else {
                            self.snapshot.radios.bluetooth
                        };
                        p.fill_round_rect(
                            scaled((24.0, 66.0, w - 24.0, 108.0), s),
                            14.0 * s,
                            rgba(1.0, 1.0, 1.0, 0.06),
                        )?;
                        text(
                            &tr(match value {
                                Some(true) => "Enabled",
                                Some(false) => "Disabled",
                                None => "Radio unavailable",
                            }),
                            (38.0, 66.0, w - 89.0, 108.0),
                            12.0,
                            false,
                            0.85,
                        )?;
                        let color = crate::ui::theme::accent();
                        p.fill_round_rect(
                            scaled((w - 78.0, 76.0, w - 40.0, 98.0), s),
                            11.0 * s,
                            if value == Some(true) {
                                rgba(color.0, color.1, color.2, 0.85)
                            } else {
                                rgba(1.0, 1.0, 1.0, 0.14)
                            },
                        )?;
                        p.fill_ellipse(
                            (if value == Some(true) { w - 51.0 } else { w - 67.0 }) * s,
                            87.0 * s,
                            7.0 * s,
                            7.0 * s,
                            rgba(1.0, 1.0, 1.0, 0.9),
                        )?;
                        text(&tr("Open Windows settings"), (34.0, 123.0, w - 34.0, 155.0), 11.0, false, 0.6)?;
                    }
                    Block::Battery => {
                        let (value, label) = match self.snapshot.battery {
                            Some((v, charging)) => (
                                format!("{v}%"),
                                if charging {
                                    if v == 100 { "Power connected" } else { "Charging" }
                                } else {
                                    "On battery"
                                },
                            ),
                            None => ("AC".into(), "Power connected"),
                        };
                        text(&value, (26.0, 57.0, 119.0, 111.0), 32.0, true, 0.96)?;
                        text(&tr(label), (126.0, 66.0, w - 26.0, 106.0), 12.0, false, 0.6)?;
                        text(&tr("Open Windows settings"), (34.0, 123.0, w - 34.0, 155.0), 11.0, false, 0.6)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn render(&self) {
        let mut surface = self.surface.borrow_mut();
        if surface.is_none() {
            *surface = PopupSurface::new(self.size).ok();
        }
        if let Some(surface) = &*surface
            && surface.paint(&self.gpu, |p| self.draw(p)).is_ok()
        {
            let _ = surface.present(self.hwnd, None, self.alpha);
        }
    }

    fn hide(&mut self) {
        self.visible = false;
        self.pressed = None;
        unsafe {
            let _ = KillTimer(Some(self.hwnd), FRAME);
            let _ = KillTimer(Some(self.hwnd), DISMISS);
            let _ = PostMessageW(Some(self.hwnd), WM_APP + 93, WPARAM(0), LPARAM(0));
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    fn volume_at(&mut self, x: f32) {
        let value = (((x - 69.0) / (self.dimensions().0 - 132.0)).clamp(0.0, 1.0) * 100.0).round() as u8;
        self.snapshot.volume = Some(value);
        self.snapshot.muted = value == 0;
        self.post(Request::Volume(value));
        self.render();
    }

    fn activate(&mut self, hit: Hit) {
        match hit {
            Hit::Block(block) => self.post(Request::Open(block)),
            Hit::Settings => {
                self.post(Request::Settings);
                self.hide();
            }
            Hit::Close => self.hide(),
            Hit::Toggle => {
                if let Role::Card(block) = self.role {
                    self.post(if block == Block::Wifi { Request::ToggleWifi } else { Request::ToggleBluetooth });
                }
            }
            Hit::Mute => self.post(Request::Mute),
            Hit::Mixer => {
                self.post(Request::Mixer);
                self.hide();
            }
            Hit::WindowsSettings => {
                if let Role::Card(block) = self.role {
                    self.post(Request::WindowsSettings(block));
                    self.hide();
                }
            }
            Hit::Minimize | Hit::Maximize | Hit::CloseWindow => {
                if let Some(window) = self.snapshot.active.clone() {
                    let action = match hit {
                        Hit::Minimize => desktop::WindowAction::Minimize,
                        Hit::CloseWindow => desktop::WindowAction::Close,
                        _ => {
                            if unsafe { IsZoomed(HWND(window.handle as *mut _)).as_bool() } {
                                desktop::WindowAction::Restore
                            } else {
                                desktop::WindowAction::Maximize
                            }
                        }
                    };
                    self.post(Request::Window(window, action));
                    self.hide();
                }
            }
            Hit::Window(index) => {
                if let Some(window) = self.snapshot.windows.get(index).cloned() {
                    self.post(Request::Activate(window));
                    self.hide();
                }
            }
            Hit::Volume => {}
        }
    }
}

extern "system" fn proc(hwnd: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
        if ptr.is_null() {
            return DefWindowProcW(hwnd, message, w, l);
        }
        match message {
            WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
            WM_ERASEBKGND => LRESULT(1),
            WM_PAINT => {
                let mut paint = PAINTSTRUCT::default();
                BeginPaint(hwnd, &mut paint);
                let _ = EndPaint(hwnd, &paint);
                (*ptr).render();
                LRESULT(0)
            }
            WM_NCHITTEST => {
                let state = &*ptr;
                let point = (
                    (l.0 as i16 as i32 - state.origin.0) as f32 / state.scale,
                    ((l.0 >> 16) as i16 as i32 - state.origin.1) as f32 / state.scale,
                );
                let (width, height) = state.dimensions();
                let hit = crate::ui::popup::hit_panel(point, (width, height), 1.0);
                LRESULT(if hit { HTCLIENT as isize } else { HTTRANSPARENT as isize })
            }
            WM_MOUSEMOVE => {
                let _ = TrackMouseEvent(&mut TRACKMOUSEEVENT {
                    cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    ..Default::default()
                });
                let state = &mut *ptr;
                let point = (l.0 as i16 as f32 / state.scale, (l.0 >> 16) as i16 as f32 / state.scale);
                if state.pressed == Some(Hit::Volume) {
                    state.volume_at(point.0);
                }
                let hover = state.buttons.iter().find(|b| b.enabled && inside(b.rect, point)).map(|b| b.hit);
                if state.hover != hover {
                    state.hover = hover;
                    state.render();
                }
                LRESULT(0)
            }
            WM_MOUSELEAVE => {
                let state = &mut *ptr;
                if state.hover.take().is_some() {
                    state.render();
                }
                LRESULT(0)
            }
            WM_LBUTTONDOWN => {
                let state = &mut *ptr;
                let point = (l.0 as i16 as f32 / state.scale, (l.0 >> 16) as i16 as f32 / state.scale);
                state.pressed = state.buttons.iter().find(|b| b.enabled && inside(b.rect, point)).map(|b| b.hit);
                if state.pressed == Some(Hit::Volume) {
                    state.volume_at(point.0);
                    SetCapture(hwnd);
                }
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                let state = &mut *ptr;
                let point = (l.0 as i16 as f32 / state.scale, (l.0 >> 16) as i16 as f32 / state.scale);
                let hit = state.buttons.iter().find(|b| b.enabled && inside(b.rect, point)).map(|b| b.hit);
                if let Some(hit) = hit.filter(|hit| Some(*hit) == state.pressed.take()) {
                    state.activate(hit);
                }
                state.pressed = None;
                if GetCapture() == hwnd {
                    let _ = ReleaseCapture();
                }
                LRESULT(0)
            }
            WM_RBUTTONUP => {
                (*ptr).post(Request::Settings);
                LRESULT(0)
            }
            WM_CAPTURECHANGED => {
                (*ptr).pressed = None;
                LRESULT(0)
            }
            m if m == WM_APP + 93 => {
                if GetCapture() == hwnd {
                    let _ = ReleaseCapture();
                }
                LRESULT(0)
            }
            WM_TIMER => {
                let state = &mut *ptr;
                if w.0 == FRAME {
                    let t = (state.fade.elapsed().as_secs_f32() / 0.16).clamp(0.0, 1.0);
                    state.alpha = ((1.0 - (1.0 - t).powi(3)) * 255.0).round() as u8;
                    state.render();
                    if t >= 1.0 {
                        state.opening = false;
                        let _ = KillTimer(Some(hwnd), FRAME);
                    }
                } else if w.0 == DISMISS {
                    let mut point = POINT::default();
                    let _ = GetCursorPos(&mut point);
                    let outside = !inside(
                        (
                            state.origin.0 as f32,
                            state.origin.1 as f32,
                            (state.origin.0 + state.size.0) as f32,
                            (state.origin.1 + state.size.1) as f32,
                        ),
                        (point.x as f32, point.y as f32),
                    );
                    if GetAsyncKeyState(VK_ESCAPE.0 as i32) < 0
                        || (outside && GetAsyncKeyState(VK_LBUTTON.0 as i32) < 0 && state.pressed.is_none())
                    {
                        state.hide();
                    }
                }
                LRESULT(0)
            }
            WM_CLOSE => {
                (*ptr).hide();
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, message, w, l),
        }
    }
}

impl Wing {
    fn new(parent: HWND, gpu: &Gpu, role: Role, id: usize) -> Result<Self> {
        static CLASS: Once = Once::new();
        let instance = unsafe { GetModuleHandleW(None)? };
        CLASS.call_once(|| unsafe {
            RegisterClassW(&WNDCLASSW {
                lpfnWndProc: Some(proc),
                hInstance: instance.into(),
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                lpszClassName: w!("NightIsland.TopPanel"),
                ..Default::default()
            });
        });
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                w!("NightIsland.TopPanel"),
                w!("Night Island Top Panel"),
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
        let mut state = Box::new(State {
            hwnd,
            parent,
            id,
            gpu: gpu.popup(),
            role,
            settings: Default::default(),
            snapshot: Default::default(),
            icon: None,
            surface: RefCell::new(None),
            size: (1, 1),
            origin: (0, 0),
            scale: 1.0,
            buttons: Vec::new(),
            hover: None,
            pressed: None,
            requests: VecDeque::new(),
            visible: false,
            fade: Instant::now(),
            opening: false,
            alpha: 255,
            dirty: true,
        });
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, (&raw mut *state) as isize);
        }
        Ok(Self { state })
    }

    fn place(&mut self, origin: (i32, i32), scale: f32, visible: bool) {
        if !visible {
            if self.state.visible {
                self.state.hide();
            }
            return;
        }
        let fresh = !self.state.visible;
        let (w, h) = self.state.dimensions();
        let size = ((w * scale).ceil() as i32, (h * scale).ceil() as i32);
        let moved = size != self.state.size || origin != self.state.origin;
        if size != self.state.size {
            self.state.surface.take();
        }
        self.state.scale = scale;
        self.state.size = size;
        self.state.origin = origin;
        self.state.visible = true;
        self.state.layout();
        unsafe {
            if moved || fresh {
                let _ = SetWindowPos(
                    self.state.hwnd,
                    Some(HWND_TOPMOST),
                    origin.0,
                    origin.1,
                    size.0,
                    size.1,
                    SWP_NOACTIVATE,
                );
            }
            if fresh {
                self.state.fade = Instant::now();
                self.state.opening = crate::ui::feedback::motion_allowed();
                self.state.alpha = if self.state.opening { 0 } else { 255 };
                if self.state.opening {
                    SetTimer(Some(self.state.hwnd), FRAME, 16, None);
                }
                if matches!(self.state.role, Role::Card(_)) {
                    SetTimer(Some(self.state.hwnd), DISMISS, 50, None);
                }
            }
            if fresh || moved || self.state.dirty {
                self.state.render();
                self.state.dirty = false;
            }
            if fresh {
                let _ = ShowWindow(self.state.hwnd, SW_SHOWNOACTIVATE);
            }
        }
    }
}

impl Drop for Wing {
    fn drop(&mut self) {
        self.state.hide();
        unsafe {
            if GetCapture() == self.state.hwnd {
                let _ = ReleaseCapture();
            }
            SetWindowLongPtrW(self.state.hwnd, GWLP_USERDATA, 0);
            let _ = DestroyWindow(self.state.hwnd);
        }
    }
}

impl TopPanel {
    pub(super) fn parent(&self) -> HWND {
        self.left.state.parent
    }

    pub fn new(parent: HWND, gpu: &Gpu, settings: &TopPanelSettings) -> Result<Self> {
        let monitor = desktop::monitors().into_iter().find(|m| m.primary).ok_or_else(windows::core::Error::empty)?;
        Ok(Self {
            left: Wing::new(parent, gpu, Role::Left, 0)?,
            right: Wing::new(parent, gpu, Role::Right, 1)?,
            card: Wing::new(parent, gpu, Role::Card(Block::Volume), 2)?,
            settings: settings.clone(),
            icons: apps::DockIconCache::new(),
            reservation: None,
            monitor,
            scale: 1.0,
            snapshot: Default::default(),
            shown: false,
        })
    }

    pub fn suppressed(&self) -> bool {
        self.settings.hide_fullscreen && desktop::fullscreen(&self.monitor)
    }

    pub fn repaint(&mut self) {
        for wing in [&mut self.left, &mut self.right, &mut self.card] {
            wing.state.dirty = true;
            if wing.state.visible {
                wing.state.render();
            }
        }
    }

    pub fn configure(&mut self, settings: &TopPanelSettings, scale: f32) {
        self.card.state.hide();
        self.settings = settings.clone();
        self.scale = scale;
        self.reflow();
        for wing in [&mut self.left, &mut self.right, &mut self.card] {
            wing.state.settings = settings.clone();
            wing.state.dirty = true;
        }
    }

    pub fn top(&self) -> f32 {
        self.reservation.as_ref().map_or(self.monitor.work.top, |r| r.rect().top) as f32
    }

    pub fn update(&mut self, snapshot: Snapshot, island: Rect, hidden: bool) {
        let active_changed =
            snapshot.active.as_ref().map(|w| w.handle) != self.snapshot.active.as_ref().map(|w| w.handle);
        if active_changed {
            self.card.state.hide();
        }
        self.snapshot = snapshot;
        let suppressed = hidden || self.suppressed();
        self.shown = !suppressed;
        let icons_changed = self.icons.poll();
        let icon =
            self.snapshot.active.as_ref().and_then(|w| self.icons.get(&w.path, (20.0 * self.scale).ceil() as u32));
        let scale = self.scale;
        let y = self.top() as i32 - (2.0 * scale) as i32;
        for wing in [&mut self.left, &mut self.right, &mut self.card] {
            let dragging = wing.state.pressed == Some(Hit::Volume);
            let volume = wing.state.snapshot.volume;
            wing.state.dirty |= wing.state.snapshot != self.snapshot || icons_changed;
            wing.state.snapshot = self.snapshot.clone();
            if dragging {
                wing.state.snapshot.volume = volume;
            }
            wing.state.icon = icon.clone();
        }
        let left_x = self.monitor.work.left + (6.0 * scale) as i32;
        let right_w = self.right.state.dimensions().0 * scale;
        let right_x = self.monitor.work.right - right_w.ceil() as i32 - (6.0 * scale) as i32;
        let collision = |x: i32, width: f32| {
            overlaps(
                (x as f32, y as f32, x as f32 + width * scale, y as f32 + 54.0 * scale),
                (island.0 - 8.0 * scale, island.1 - 8.0 * scale, island.2 + 8.0 * scale, island.3 + 8.0 * scale),
            )
        };
        self.left.place((left_x, y), scale, !suppressed && self.settings.application && !collision(left_x, 214.0));
        self.right.place(
            (right_x, y),
            scale,
            !suppressed && !blocks(&self.settings).is_empty() && !collision(right_x, self.right.state.dimensions().0),
        );
        if suppressed {
            self.card.state.hide();
        } else if self.card.state.visible {
            let card = &self.card.state;
            let bounds = (
                card.origin.0 as f32,
                card.origin.1 as f32,
                (card.origin.0 + card.size.0) as f32,
                (card.origin.1 + card.size.1) as f32,
            );
            if overlaps(bounds, island) {
                self.card.state.hide();
            } else {
                self.card.place(self.card.state.origin, self.scale, true);
            }
        }
    }

    pub fn open(&mut self, block: Block) {
        if !self.shown {
            return;
        }
        if matches!(self.card.state.role, Role::Card(current) if current == block) && self.card.state.visible {
            self.card.state.hide();
            return;
        }
        self.card.state.hide();
        self.card.state.role = Role::Card(block);
        self.card.state.dirty = true;
        self.card.state.snapshot = self.snapshot.clone();
        let width = self.card.state.dimensions().0 * self.scale;
        let x = if block == Block::Application {
            self.left.state.origin.0
        } else {
            self.right.state.origin.0 + self.right.state.size.0 - width as i32
        };
        let x = x.clamp(self.monitor.work.left, (self.monitor.work.right - width as i32).max(self.monitor.work.left));
        let y = self.top() as i32 + (44.0 * self.scale) as i32;
        self.card.place((x, y), self.scale, true);
    }

    pub fn take_request(&mut self, id: usize) -> Option<Request> {
        match id {
            0 => self.left.state.requests.pop_front(),
            1 => self.right.state.requests.pop_front(),
            2 => self.card.state.requests.pop_front(),
            _ => None,
        }
    }

    pub fn set_capture_hidden(&self, hidden: bool) {
        for wing in [&self.left, &self.right, &self.card] {
            unsafe {
                let _ =
                    SetWindowDisplayAffinity(wing.state.hwnd, if hidden { WDA_EXCLUDEFROMCAPTURE } else { WDA_NONE });
            }
        }
    }
}

impl Drop for TopPanel {
    fn drop(&mut self) {
        self.reservation.take();
    }
}

#[cfg(test)]
#[path = "top_panel_tests.rs"]
mod tests;
