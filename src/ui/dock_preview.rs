use std::sync::Once;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FONT_WEIGHT_NORMAL, DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_TEXT_ALIGNMENT_CENTER,
    DWRITE_TEXT_ALIGNMENT_LEADING,
};
use windows::Win32::Graphics::Dwm::{
    DWMWA_BORDER_COLOR, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmSetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{BeginPaint, EndPaint, InvalidateRect, PAINTSTRUCT};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_ESCAPE};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{Result, w};

use crate::platform::{
    desktop_dock as desktop,
    dock_actions::{Client, MediaAction, Snapshot},
    window_preview::Thumbnail,
};
use crate::ui::{
    desktop_dock::Entry,
    gfx::{Gpu, Painter, rgba},
    i18n::tr,
};
use crate::view::Rect;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Action {
    Dismiss,
    Open,
    PreviousWindow,
    NextWindow,
    CloseWindow,
    PreviousPlayer,
    NextPlayer,
    PreviousTrack,
    PlayPause,
    NextTrack,
    Quieter,
    Louder,
    Mute,
    Microphone,
    Deafen,
    SetupDiscord,
}

struct Button {
    rect: Rect,
    label: String,
    glyph: Option<char>,
    action: Action,
    enabled: bool,
    selected: bool,
}

#[derive(Clone, Copy)]
struct Layout {
    width: f32,
    height: f32,
    image: Rect,
    caption: Rect,
    media_y: f32,
    audio_y: f32,
    discord_y: f32,
}

impl Layout {
    fn new(work: RECT, scale: f32, media: bool, audio: bool, discord: bool) -> Self {
        let width = (work.right - work.left) as f32 / scale;
        let height = (work.bottom - work.top) as f32 / scale;
        let width = (width - 16.0).clamp(160.0, 340.0);
        let extras = 92.0 * if media { 1.0 } else { 0.0 }
            + 48.0 * if audio { 1.0 } else { 0.0 }
            + 84.0 * if discord { 1.0 } else { 0.0 };
        let image_height = (height - 108.0 - extras).clamp(40.0, 168.0);
        let image = (12.0, 44.0, width - 12.0, 44.0 + image_height);
        let caption = (12.0, image.3 + 4.0, width - 12.0, image.3 + 36.0);
        let media_y = caption.3 + 4.0;
        let audio_y = media_y + if media { 92.0 } else { 0.0 };
        let discord_y = audio_y + if audio { 48.0 } else { 0.0 };
        Self {
            width,
            height: discord_y + if discord { 84.0 } else { 0.0 } + 12.0,
            image,
            caption,
            media_y,
            audio_y,
            discord_y,
        }
    }
}

struct State {
    hwnd: HWND,
    parent: HWND,
    gpu: Gpu,
    entry: Entry,
    actions: Client,
    snapshot: Snapshot,
    thumbnail: Option<Thumbnail>,
    thumbnail_visible: bool,
    window_index: usize,
    media_id: Option<u64>,
    scale: f32,
    work: RECT,
    anchor: Rect,
    origin: (i32, i32),
    layout: Layout,
    buttons: Vec<Button>,
    hover: Option<usize>,
    pressed: Option<usize>,
    visible: bool,
    last_inside: Instant,
    dismissed: bool,
    thumbnail_attempt: Instant,
    error: Option<(&'static str, Instant)>,
}

pub struct Preview {
    state: Box<State>,
}

fn inside(r: Rect, p: (f32, f32)) -> bool {
    p.0 >= r.0 && p.0 < r.2 && p.1 >= r.1 && p.1 < r.3
}

fn scaled(r: Rect, s: f32) -> Rect {
    (r.0 * s, r.1 * s, r.2 * s, r.3 * s)
}

fn fit(source: (i32, i32), area: Rect) -> RECT {
    let scale = ((area.2 - area.0) / source.0.max(1) as f32).min((area.3 - area.1) / source.1.max(1) as f32);
    let w = (source.0 as f32 * scale).round().max(1.0);
    let h = (source.1 as f32 * scale).round().max(1.0);
    let x = (area.0 + (area.2 - area.0 - w) / 2.0).round();
    let y = (area.1 + (area.3 - area.1 - h) / 2.0).round();
    RECT { left: x as i32, top: y as i32, right: (x + w) as i32, bottom: (y + h) as i32 }
}

impl State {
    fn invalidate(&self) {
        unsafe {
            let _ = InvalidateRect(Some(self.hwnd), None, false);
        }
    }

    fn media_index(&self) -> Option<usize> {
        self.media_id
            .and_then(|id| self.snapshot.media.iter().position(|m| m.id == id))
            .or_else(|| (!self.snapshot.media.is_empty()).then_some(0))
    }

    fn button(&mut self, rect: Rect, label: &str, glyph: Option<char>, action: Action, enabled: bool, selected: bool) {
        self.buttons.push(Button { rect, label: tr(label).into_owned(), glyph, action, enabled, selected });
    }

    fn arrange(&mut self) {
        self.hover = None;
        self.pressed = None;
        if self.media_id.is_none_or(|id| !self.snapshot.media.iter().any(|m| m.id == id)) {
            self.media_id = self.snapshot.media.first().map(|m| m.id);
        }
        self.layout = Layout::new(
            self.work,
            self.scale,
            !self.snapshot.media.is_empty(),
            self.snapshot.audio.is_some(),
            self.snapshot.discord.is_some(),
        );
        let l = self.layout;
        self.buttons.clear();
        self.button(
            (l.width - 40.0, 6.0, l.width - 6.0, 40.0),
            "Close preview",
            Some('\u{E8BB}'),
            Action::Dismiss,
            true,
            false,
        );
        if !self.entry.windows.is_empty() {
            self.button(l.image, "Open window", None, Action::Open, true, false);
            let many = self.entry.windows.len() > 1;
            self.button(
                (12.0, l.caption.1, 44.0, l.caption.3),
                "Previous window",
                Some('\u{E76B}'),
                Action::PreviousWindow,
                many,
                false,
            );
            self.button(
                (l.width - 82.0, l.caption.1, l.width - 50.0, l.caption.3),
                "Next window",
                Some('\u{E76C}'),
                Action::NextWindow,
                many,
                false,
            );
            self.button(
                (l.width - 44.0, l.caption.1, l.width - 12.0, l.caption.3),
                "Close window",
                Some('\u{E8BB}'),
                Action::CloseWindow,
                true,
                false,
            );
        }
        if let Some(i) = self.media_index() {
            let media = &self.snapshot.media[i];
            let (prev, toggle, next, playing) = (media.can_previous, media.can_toggle, media.can_next, media.playing);
            let y = l.media_y + 48.0;
            let slot = (l.width - 32.0) / 3.0;
            self.button(
                (12.0, y, 12.0 + slot, y + 38.0),
                "Previous track",
                Some('\u{E892}'),
                Action::PreviousTrack,
                prev,
                false,
            );
            self.button(
                (16.0 + slot, y, 16.0 + 2.0 * slot, y + 38.0),
                if playing { "Pause" } else { "Play" },
                Some(if playing { '\u{E769}' } else { '\u{E768}' }),
                Action::PlayPause,
                toggle,
                false,
            );
            self.button(
                (20.0 + 2.0 * slot, y, l.width - 12.0, y + 38.0),
                "Next track",
                Some('\u{E893}'),
                Action::NextTrack,
                next,
                false,
            );
            if self.snapshot.media.len() > 1 {
                self.button(
                    (l.width - 78.0, l.media_y, l.width - 46.0, l.media_y + 36.0),
                    "Previous player",
                    Some('\u{E76B}'),
                    Action::PreviousPlayer,
                    true,
                    false,
                );
                self.button(
                    (l.width - 44.0, l.media_y, l.width - 12.0, l.media_y + 36.0),
                    "Next player",
                    Some('\u{E76C}'),
                    Action::NextPlayer,
                    true,
                    false,
                );
            }
        }
        if let Some(audio) = &self.snapshot.audio {
            let muted = audio.muted;
            let y = l.audio_y + 6.0;
            self.button(
                (12.0, y, 50.0, y + 36.0),
                if muted { "Unmute application" } else { "Mute application" },
                Some(if muted { '\u{E74F}' } else { '\u{E767}' }),
                Action::Mute,
                true,
                muted,
            );
            self.button(
                (l.width - 90.0, y, l.width - 54.0, y + 36.0),
                "Quieter",
                Some('\u{E738}'),
                Action::Quieter,
                true,
                false,
            );
            self.button(
                (l.width - 48.0, y, l.width - 12.0, y + 36.0),
                "Louder",
                Some('\u{E710}'),
                Action::Louder,
                true,
                false,
            );
        }
        if let Some(voice) = &self.snapshot.discord {
            let (connected, available, muted, deaf) = (voice.connected, voice.available, voice.mute, voice.deaf);
            let y = l.discord_y + 32.0;
            if connected {
                let mid = l.width / 2.0;
                self.button(
                    (12.0, y, mid - 3.0, y + 42.0),
                    if muted { "Unmute microphone" } else { "Mute microphone" },
                    Some('\u{E720}'),
                    Action::Microphone,
                    available,
                    muted,
                );
                self.button(
                    (mid + 3.0, y, l.width - 12.0, y + 42.0),
                    if deaf { "Undeafen" } else { "Deafen" },
                    Some('\u{E7F6}'),
                    Action::Deafen,
                    available,
                    deaf,
                );
            } else {
                self.button(
                    (12.0, y, l.width - 12.0, y + 42.0),
                    "Connect Discord in settings",
                    Some('\u{E713}'),
                    Action::SetupDiscord,
                    true,
                    false,
                );
            }
        }
        let size = ((l.width * self.scale).ceil() as i32, (l.height * self.scale).ceil() as i32);
        self.origin = position(self.work, self.anchor, size, self.scale);
        unsafe {
            let _ = SetWindowPos(
                self.hwnd,
                Some(HWND_TOPMOST),
                self.origin.0,
                self.origin.1,
                size.0,
                size.1,
                SWP_NOACTIVATE,
            );
        }
        self.update_thumbnail();
        self.invalidate();
    }

    fn rebuild_thumbnail(&mut self) {
        self.thumbnail_attempt = Instant::now();
        self.thumbnail.take();
        self.thumbnail_visible = false;
        if let Some(source) = self.entry.windows.get(self.window_index) {
            self.thumbnail = Thumbnail::new(self.hwnd, source);
        }
        self.update_thumbnail();
    }

    fn update_thumbnail(&mut self) {
        self.thumbnail_visible = self.thumbnail.as_mut().is_some_and(|thumbnail| {
            let Some(size) = thumbnail.source_size() else { return false };
            thumbnail.update(fit((size.cx, size.cy), scaled(self.layout.image, self.scale)), self.visible)
        });
        if !self.thumbnail_visible {
            self.thumbnail.take();
        }
    }

    fn paint(&self, p: &Painter) -> Result<()> {
        let s = self.scale;
        let l = self.layout;
        let text = |value: &str, rect: Rect, size: f32, strong: bool, muted: bool| {
            p.text(
                value,
                scaled(rect, s),
                size * s,
                if strong { DWRITE_FONT_WEIGHT_SEMI_BOLD } else { DWRITE_FONT_WEIGHT_NORMAL },
                DWRITE_TEXT_ALIGNMENT_LEADING,
                if muted { rgba(0.63, 0.64, 0.69, 1.0) } else { rgba(0.96, 0.96, 0.98, 1.0) },
            )
        };
        p.fill_rect((0.0, 0.0, l.width * s, l.height * s), rgba(0.065, 0.068, 0.08, 1.0))?;
        p.stroke_round_rect(
            (0.5, 0.5, l.width * s - 0.5, l.height * s - 0.5),
            8.0 * s,
            1.0,
            rgba(0.24, 0.25, 0.28, 1.0),
        )?;
        if let Some(icon) = &self.entry.icon {
            p.image(scaled((12.0, 12.0, 32.0, 32.0), s), 0.0, icon.side, &icon.bgra)?;
        }
        text(&self.entry.pin.name, (40.0, 4.0, l.width - 48.0, 40.0), 13.0, true, false)?;
        p.fill_round_rect(scaled(l.image, s), 6.0 * s, rgba(0.035, 0.037, 0.045, 1.0))?;
        if !self.thumbnail_visible {
            let label = if self.thumbnail.as_ref().is_some_and(|t| t.is_minimized())
                || self.entry.windows.get(self.window_index).is_some_and(|w| w.minimized)
            {
                "Window minimized"
            } else {
                "Preview unavailable"
            };
            p.text(
                &tr(label),
                scaled(l.image, s),
                12.0 * s,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_TEXT_ALIGNMENT_CENTER,
                rgba(0.6, 0.61, 0.67, 1.0),
            )?;
        }
        if let Some(window) = self.entry.windows.get(self.window_index) {
            text(
                &format!("{}/{} · {}", self.window_index + 1, self.entry.windows.len(), window.title),
                (48.0, l.caption.1, l.width - 88.0, l.caption.3),
                11.0,
                false,
                true,
            )?;
        }
        if let Some(i) = self.media_index() {
            let media = &self.snapshot.media[i];
            let right = l.width - if self.snapshot.media.len() > 1 { 86.0 } else { 12.0 };
            text(&media.title, (14.0, l.media_y, right, l.media_y + 23.0), 13.0, true, false)?;
            let subtitle = if media.artist.is_empty() { &media.source } else { &media.artist };
            let subtitle = if self.snapshot.media.len() > 1 {
                format!("{}/{} · {subtitle}", i + 1, self.snapshot.media.len())
            } else {
                subtitle.clone()
            };
            text(&subtitle, (14.0, l.media_y + 23.0, right, l.media_y + 43.0), 11.0, false, true)?;
        }
        if let Some(audio) = &self.snapshot.audio {
            text(
                &format!("{} · {}%", tr("Application volume"), (audio.volume * 100.0).round() as u32),
                (58.0, l.audio_y, l.width - 96.0, l.audio_y + 48.0),
                11.0,
                false,
                true,
            )?;
        }
        if let Some(voice) = &self.snapshot.discord {
            let status = if !voice.connected {
                tr("Discord voice controls require connection")
            } else if voice.error.is_some() {
                tr("Discord voice controls unavailable")
            } else if !voice.available {
                tr("Waiting for Discord voice settings")
            } else if voice.channel.is_empty() {
                std::borrow::Cow::Borrowed("Discord")
            } else {
                std::borrow::Cow::Owned(format!("Discord · {}", voice.channel))
            };
            text(&status, (14.0, l.discord_y + 2.0, l.width - 12.0, l.discord_y + 30.0), 11.0, false, true)?;
        }
        for (i, button) in self.buttons.iter().enumerate() {
            if button.action == Action::Open {
                continue;
            }
            let hot = self.hover == Some(i) && button.enabled;
            let background = if button.selected {
                rgba(0.45, 0.18, 0.24, 1.0)
            } else if hot {
                rgba(0.23, 0.24, 0.29, 1.0)
            } else {
                rgba(0.12, 0.13, 0.16, 1.0)
            };
            p.fill_round_rect(scaled(button.rect, s), 8.0 * s, background)?;
            let color = rgba(0.94, 0.94, 0.97, if button.enabled { 1.0 } else { 0.25 });
            let has_text = matches!(button.action, Action::Microphone | Action::Deafen | Action::SetupDiscord);
            if let Some(glyph) = button.glyph {
                let rect = if has_text {
                    (button.rect.0 + 6.0, button.rect.1, button.rect.0 + 32.0, button.rect.3)
                } else {
                    button.rect
                };
                p.glyph(glyph, scaled(rect, s), 15.0 * s, color)?;
            }
            if has_text {
                p.text(
                    &button.label,
                    scaled((button.rect.0 + 34.0, button.rect.1, button.rect.2 - 6.0, button.rect.3), s),
                    10.0 * s,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_TEXT_ALIGNMENT_LEADING,
                    color,
                )?;
            }
        }
        if let Some((message, _)) = self.error {
            let rect = (48.0, l.caption.1, l.width - 88.0, l.caption.3);
            p.fill_rect(scaled(rect, s), rgba(0.3, 0.12, 0.14, 1.0))?;
            text(&tr(message), rect, 11.0, false, false)?;
        }
        Ok(())
    }

    fn hit(&self, point: (f32, f32)) -> Option<usize> {
        self.buttons.iter().position(|b| b.enabled && inside(b.rect, point))
    }

    fn hide(&mut self) {
        self.visible = false;
        self.thumbnail.take();
        self.thumbnail_visible = false;
        self.actions.clear();
        self.snapshot = Snapshot::default();
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    fn action(&mut self, action: Action) -> Option<desktop::Window> {
        match action {
            Action::Dismiss => {
                self.dismissed = true;
                self.hide();
            }
            Action::Open => {
                let window = self.entry.windows.get(self.window_index).cloned();
                self.dismissed = true;
                self.hide();
                return window;
            }
            Action::PreviousWindow | Action::NextWindow => {
                let count = self.entry.windows.len();
                if count > 0 {
                    self.window_index =
                        (self.window_index + if action == Action::NextWindow { 1 } else { count - 1 }) % count;
                    self.rebuild_thumbnail();
                }
            }
            Action::CloseWindow => {
                if let Some(window) = self.entry.windows.get(self.window_index)
                    && !desktop::change_window(window, desktop::WindowAction::Close)
                {
                    self.error = Some(("Window is no longer available", Instant::now()));
                }
            }
            Action::PreviousPlayer | Action::NextPlayer => {
                if let Some(i) = self.media_index() {
                    let count = self.snapshot.media.len();
                    self.media_id = Some(
                        self.snapshot.media[(i + if action == Action::NextPlayer { 1 } else { count - 1 }) % count].id,
                    );
                }
            }
            Action::PreviousTrack | Action::PlayPause | Action::NextTrack => {
                if let Some(i) = self.media_index() {
                    self.actions.control(
                        self.snapshot.media[i].id,
                        match action {
                            Action::PreviousTrack => MediaAction::Previous,
                            Action::NextTrack => MediaAction::Next,
                            _ => MediaAction::PlayPause,
                        },
                    );
                }
            }
            Action::Quieter | Action::Louder => {
                if let Some(audio) = &mut self.snapshot.audio {
                    audio.volume = (audio.volume + if action == Action::Louder { 0.1 } else { -0.1 }).clamp(0.0, 1.0);
                    self.actions.set_volume(audio.volume);
                }
            }
            Action::Mute => {
                if let Some(audio) = &mut self.snapshot.audio {
                    audio.muted = !audio.muted;
                    self.actions.set_muted(audio.muted);
                }
            }
            Action::Microphone => {
                if let Some(voice) = &self.snapshot.discord {
                    self.actions.set_discord_mute(!voice.mute);
                }
            }
            Action::Deafen => {
                if let Some(voice) = &self.snapshot.discord {
                    self.actions.set_discord_deaf(!voice.deaf);
                }
            }
            Action::SetupDiscord => {
                unsafe {
                    let _ =
                        PostMessageW(Some(self.parent), crate::ui::desktop_dock::WM_DESKTOP_DOCK, WPARAM(3), LPARAM(0));
                }
                self.dismissed = true;
                self.hide();
            }
        }
        if self.visible {
            self.arrange();
        }
        None
    }
}

fn position(work: RECT, anchor: Rect, size: (i32, i32), scale: f32) -> (i32, i32) {
    let x = (((anchor.0 + anchor.2) / 2.0) as i32 - size.0 / 2)
        .clamp(work.left + 4, (work.right - size.0 - 4).max(work.left + 4));
    let y = (anchor.1 as i32 - size.1 - (10.0 * scale) as i32)
        .clamp(work.top + 4, (work.bottom - size.1 - 4).max(work.top + 4));
    (x, y)
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
                let state = &*ptr;
                let mut paint = PAINTSTRUCT::default();
                let dc = BeginPaint(hwnd, &mut paint);
                let size = (
                    (state.layout.width * state.scale).ceil() as i32,
                    (state.layout.height * state.scale).ceil() as i32,
                );
                let _ = state.gpu.paint_dc(dc, size, |p| {
                    let _ = state.paint(p);
                });
                let _ = EndPaint(hwnd, &paint);
                LRESULT(0)
            }
            WM_MOUSEMOVE => {
                let state = &mut *ptr;
                let point = (l.0 as i16 as f32 / state.scale, (l.0 >> 16) as i16 as f32 / state.scale);
                let hover = state.hit(point);
                if hover != state.hover {
                    state.hover = hover;
                    state.invalidate();
                }
                LRESULT(0)
            }
            WM_LBUTTONDOWN => {
                let state = &mut *ptr;
                state.pressed = state.hit((l.0 as i16 as f32 / state.scale, (l.0 >> 16) as i16 as f32 / state.scale));
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                let activate = {
                    let state = &mut *ptr;
                    let hit = state.hit((l.0 as i16 as f32 / state.scale, (l.0 >> 16) as i16 as f32 / state.scale));
                    let pressed = state.pressed.take();
                    if let Some(i) = hit.filter(|_| hit == pressed) {
                        state.action(state.buttons[i].action)
                    } else {
                        None
                    }
                };
                if let Some(window) = activate {
                    desktop::activate(&window, false);
                }
                LRESULT(0)
            }
            WM_CLOSE => {
                (*ptr).dismissed = true;
                (*ptr).hide();
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, message, w, l),
        }
    }
}

impl Preview {
    pub fn new(
        parent: HWND,
        gpu: &Gpu,
        entry: &Entry,
        anchor: Rect,
        work: RECT,
        scale: f32,
        capture_hidden: bool,
    ) -> Result<Self> {
        static CLASS: Once = Once::new();
        let instance = unsafe { GetModuleHandleW(None)? };
        CLASS.call_once(|| unsafe {
            RegisterClassW(&WNDCLASSW {
                lpfnWndProc: Some(proc),
                hInstance: instance.into(),
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                lpszClassName: w!("NightIsland.DockPreview"),
                ..Default::default()
            });
        });
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                w!("NightIsland.DockPreview"),
                w!("Night Island Preview"),
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
        unsafe {
            let corner = DWMWCP_ROUND;
            let border = 0x003c3c40u32;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                (&raw const corner).cast(),
                size_of_val(&corner) as u32,
            );
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_BORDER_COLOR,
                (&raw const border).cast(),
                size_of_val(&border) as u32,
            );
        }
        let state = Box::new(State {
            hwnd,
            parent,
            gpu: gpu.popup(),
            entry: entry.clone(),
            actions: Client::new(),
            snapshot: Snapshot::default(),
            thumbnail: None,
            thumbnail_visible: false,
            window_index: entry.windows.iter().position(|w| w.active).unwrap_or(0),
            media_id: None,
            scale,
            work,
            anchor,
            origin: (0, 0),
            layout: Layout::new(work, scale, false, false, false),
            buttons: Vec::new(),
            hover: None,
            pressed: None,
            visible: false,
            last_inside: Instant::now(),
            dismissed: false,
            thumbnail_attempt: Instant::now(),
            error: None,
        });
        let mut preview = Self { state };
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, (&raw mut *preview.state) as isize);
        }
        preview.set_capture_hidden(capture_hidden);
        preview.show(entry, anchor, work, scale);
        Ok(preview)
    }

    pub fn show(&mut self, entry: &Entry, anchor: Rect, work: RECT, scale: f32) {
        self.state.hide();
        self.state.entry = entry.clone();
        self.state.window_index = entry.windows.iter().position(|w| w.active).unwrap_or(0);
        self.state.media_id = None;
        self.state.anchor = anchor;
        self.state.work = work;
        self.state.scale = scale;
        self.state.visible = true;
        self.state.dismissed = false;
        self.state.last_inside = Instant::now();
        self.state.pressed = None;
        self.state.hover = None;
        self.state.error = None;
        let target = entry
            .windows
            .first()
            .map(|w| w.path.clone())
            .unwrap_or_else(|| desktop::resolved_target(&entry.pin.target));
        self.state.actions.request(&target, &entry.windows.iter().map(|w| w.handle).collect::<Vec<_>>());
        self.state.arrange();
        self.state.rebuild_thumbnail();
        unsafe {
            let _ = ShowWindow(self.state.hwnd, SW_SHOWNOACTIVATE);
        }
    }

    pub fn sync_entry(&mut self, entry: &Entry) {
        if !self.state.visible || self.state.entry.pin.target != entry.pin.target {
            return;
        }
        if self.state.entry.windows == entry.windows {
            return;
        }
        let selected = self.state.entry.windows.get(self.state.window_index).map(|w| w.handle);
        self.state.entry = entry.clone();
        self.state.window_index =
            selected.and_then(|handle| entry.windows.iter().position(|w| w.handle == handle)).unwrap_or(0);
        if entry.windows.is_empty() {
            self.state.hide();
            return;
        }
        let path = entry.windows[0].path.clone();
        self.state.actions.request(&path, &entry.windows.iter().map(|w| w.handle).collect::<Vec<_>>());
        self.state.snapshot = self.state.actions.snapshot();
        self.state.arrange();
        self.state.rebuild_thumbnail();
    }

    pub fn allow_reopen(&mut self) {
        self.state.dismissed = false;
    }

    pub fn is_open(&self) -> bool {
        self.state.visible
    }
    pub fn dismissed(&self) -> bool {
        self.state.dismissed
    }
    pub fn target(&self) -> &str {
        &self.state.entry.pin.target
    }
    pub fn hide(&mut self) {
        if self.state.visible {
            self.state.hide();
        }
    }

    pub fn contains(&self, point: POINT) -> bool {
        self.state.visible
            && inside(
                (
                    self.state.origin.0 as f32,
                    self.state.origin.1 as f32,
                    self.state.origin.0 as f32 + self.state.layout.width * self.state.scale,
                    self.state.origin.1 as f32 + self.state.layout.height * self.state.scale,
                ),
                (point.x as f32, point.y as f32),
            )
    }

    pub fn tick(&mut self, point: POINT, over_source: bool) {
        if !self.state.visible {
            return;
        }
        if unsafe { GetAsyncKeyState(VK_ESCAPE.0 as i32) } < 0 {
            self.state.dismissed = true;
            self.state.hide();
            return;
        }
        if over_source || self.contains(point) {
            self.state.last_inside = Instant::now();
        } else if self.state.last_inside.elapsed() > Duration::from_millis(450) {
            self.state.hide();
            return;
        }
        let snapshot = self.state.actions.snapshot();
        if snapshot != self.state.snapshot {
            self.state.snapshot = snapshot;
            self.state.arrange();
        }
        if self.state.thumbnail.is_none() && self.state.thumbnail_attempt.elapsed() >= Duration::from_secs(1) {
            self.state.rebuild_thumbnail();
        }
        let previous = self.state.thumbnail_visible;
        self.state.update_thumbnail();
        if previous != self.state.thumbnail_visible {
            self.state.invalidate();
        }
        if self.state.error.is_some_and(|(_, at)| at.elapsed() > Duration::from_secs(3)) {
            self.state.error = None;
            self.state.invalidate();
        }
    }

    pub fn set_capture_hidden(&self, hidden: bool) {
        unsafe {
            let _ = SetWindowDisplayAffinity(self.state.hwnd, if hidden { WDA_EXCLUDEFROMCAPTURE } else { WDA_NONE });
        }
    }
}

impl Drop for Preview {
    fn drop(&mut self) {
        self.state.thumbnail.take();
        unsafe {
            SetWindowLongPtrW(self.state.hwnd, GWLP_USERDATA, 0);
            let _ = DestroyWindow(self.state.hwnd);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_fits_windows_without_stretching_and_stays_on_negative_monitor() {
        let work = RECT { left: -1920, top: 0, right: 0, bottom: 1040 };
        for scale in [1.0, 1.5, 2.0] {
            let layout = Layout::new(work, scale, true, true, true);
            let size = ((layout.width * scale).ceil() as i32, (layout.height * scale).ceil() as i32);
            let at = position(work, (-100.0, 950.0, -40.0, 1010.0), size, scale);
            assert!(at.0 >= work.left && at.0 + size.0 <= work.right);
            assert!(at.1 >= work.top && at.1 + size.1 <= work.bottom);
            let area = scaled(layout.image, scale);
            for source in [(1920, 1080), (800, 1200), (3440, 1440)] {
                let rect = fit(source, area);
                assert!(rect.left as f32 >= area.0 - 1.0 && rect.right as f32 <= area.2 + 1.0);
                assert!(rect.top as f32 >= area.1 - 1.0 && rect.bottom as f32 <= area.3 + 1.0);
                let ratio = (rect.right - rect.left) as f32 / (rect.bottom - rect.top) as f32;
                assert!((ratio - source.0 as f32 / source.1 as f32).abs() < 0.08);
            }
        }
    }
}

#[cfg(test)]
#[path = "dock_preview_tests.rs"]
mod native_tests;
