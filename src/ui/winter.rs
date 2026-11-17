use std::time::{Duration, Instant};

use windows::UI::Color;
use windows::UI::Composition::Desktop::DesktopWindowTarget;
use windows::UI::Composition::*;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::WinRT::Composition::ICompositorDesktopInterop;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, ReleaseCapture, SetCapture, VK_MENU};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{Interface, Result, w};
use windows_numerics::{Vector2, Vector3};

use super::winter_model::{Ledge, Snow};
use crate::platform::desktop_dock::{self as desktop, Monitor};

#[cfg(test)]
#[path = "winter_tests.rs"]
mod tests;

use crate::data::winter::WinterSettings;

pub struct Winter {
    #[allow(clippy::vec_box)]
    scenes: Vec<Box<Scene>>,
    settings: WinterSettings,
    refresh: Instant,
    frame: Instant,
    topology: Instant,
    compositor: Compositor,
}

struct Dot {
    _geometry: CompositionEllipseGeometry,
    shape: CompositionSpriteShape,
}

struct Scene {
    hwnd: HWND,
    bounds: RECT,
    scale: f32,
    snow: Snow,
    _target: DesktopWindowTarget,
    root: ContainerVisual,
    flakes: Vec<Dot>,
    piles: ShapeVisual,
    pile_dots: Vec<Dot>,
    balls: ShapeVisual,
    ball_dots: Vec<[Dot; 4]>,
    compositor: Compositor,
    white: CompositionColorBrush,
    ink: CompositionColorBrush,
    interactive: bool,
    suppressed: bool,
}

unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let pointer = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut Scene;
    if !pointer.is_null() {
        let scene = unsafe { &mut *pointer };
        let mut cursor = POINT::default();
        let _ = unsafe { GetCursorPos(&mut cursor) };
        let x = (cursor.x - scene.bounds.left) as f32 / scene.scale;
        let y = (cursor.y - scene.bounds.top) as f32 / scene.scale;
        match msg {
            WM_NCHITTEST => {
                return LRESULT(
                    if scene.interactive && unsafe { GetAsyncKeyState(VK_MENU.0 as i32) } < 0 && scene.snow.hit(x, y) {
                        HTCLIENT as isize
                    } else {
                        HTTRANSPARENT as isize
                    },
                );
            }
            WM_MOUSEACTIVATE => return LRESULT(MA_NOACTIVATE as isize),
            WM_LBUTTONDOWN => {
                scene.snow.grab(x, y);
                unsafe {
                    SetCapture(hwnd);
                }
                return LRESULT(0);
            }
            WM_LBUTTONUP | WM_CANCELMODE => {
                scene.snow.release();
                unsafe {
                    let _ = ReleaseCapture();
                }
                return LRESULT(0);
            }
            WM_CAPTURECHANGED => {
                scene.snow.release();
                return LRESULT(0);
            }
            WM_ERASEBKGND => return LRESULT(1),
            _ => {}
        }
    }
    unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
}

impl Scene {
    fn new(c: &Compositor, monitor: Monitor, settings: WinterSettings) -> Result<Box<Self>> {
        static CLASS: std::sync::Once = std::sync::Once::new();
        let instance = unsafe { GetModuleHandleW(None)? };
        CLASS.call_once(|| unsafe {
            RegisterClassW(&WNDCLASSW {
                lpfnWndProc: Some(proc),
                hInstance: instance.into(),
                lpszClassName: w!("NightIsland.Winter"),
                ..Default::default()
            });
        });
        let bounds = monitor.bounds;
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_NOREDIRECTIONBITMAP
                    | WS_EX_LAYERED
                    | WS_EX_TOPMOST
                    | WS_EX_TOOLWINDOW
                    | WS_EX_NOACTIVATE
                    | WS_EX_TRANSPARENT,
                w!("NightIsland.Winter"),
                w!("Night Island Snow"),
                WS_POPUP,
                bounds.left,
                bounds.top,
                bounds.right - bounds.left,
                bounds.bottom - bounds.top,
                None,
                None,
                Some(instance.into()),
                None,
            )?
        };
        unsafe {
            if let Err(error) =
                SetLayeredWindowAttributes(hwnd, windows::Win32::Foundation::COLORREF(0), 255, LWA_ALPHA)
            {
                let _ = DestroyWindow(hwnd);
                return Err(error);
            }
        }
        let result = (|| {
            let scale = unsafe { windows::Win32::UI::HiDpi::GetDpiForWindow(hwnd) } as f32 / 96.0;
            let root = c.CreateContainerVisual()?;
            root.SetScale(Vector3 { X: scale, Y: scale, Z: 1.0 })?;
            root.SetSize(Vector2 {
                X: (bounds.right - bounds.left) as f32 / scale,
                Y: (bounds.bottom - bounds.top) as f32 / scale,
            })?;
            let target = unsafe { c.cast::<ICompositorDesktopInterop>()?.CreateDesktopWindowTarget(hwnd, true)? };
            target.SetRoot(&root)?;
            let white = c.CreateColorBrushWithColor(Color { A: 255, R: 238, G: 247, B: 255 })?;
            let ink = c.CreateColorBrushWithColor(Color { A: 255, R: 40, G: 54, B: 69 })?;
            let snow = Snow::new(
                (bounds.right - bounds.left) as f32 / scale,
                (bounds.bottom - bounds.top) as f32 / scale,
                if settings.dense { 220 } else { 90 },
                (bounds.left as u64).wrapping_add(73471),
            );
            let flakes_layer = c.CreateShapeVisual()?;
            flakes_layer.SetRelativeSizeAdjustment(Vector2 { X: 1.0, Y: 1.0 })?;
            let mut flakes = Vec::new();
            for flake in &snow.flakes {
                let dot = dot(c, &white, flake.radius, flake.radius)?;
                dot.shape.SetOffset(Vector2 { X: flake.x, Y: flake.y })?;
                flakes_layer.Shapes()?.Append(&dot.shape)?;
                flakes.push(dot);
            }
            root.Children()?.InsertAtTop(&flakes_layer)?;
            let piles = c.CreateShapeVisual()?;
            piles.SetRelativeSizeAdjustment(Vector2 { X: 1.0, Y: 1.0 })?;
            root.Children()?.InsertAtTop(&piles)?;
            let balls = c.CreateShapeVisual()?;
            balls.SetRelativeSizeAdjustment(Vector2 { X: 1.0, Y: 1.0 })?;
            root.Children()?.InsertAtTop(&balls)?;
            Ok(Box::new(Self {
                hwnd,
                bounds,
                scale,
                snow,
                _target: target,
                root,
                flakes,
                piles,
                pile_dots: Vec::new(),
                balls,
                ball_dots: Vec::new(),
                compositor: c.clone(),
                white,
                ink,
                interactive: settings.interactive,
                suppressed: false,
            }))
        })();
        match result {
            Ok(mut scene) => {
                unsafe {
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, (&raw mut *scene) as isize);
                    let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                }
                Ok(scene)
            }
            Err(error) => {
                unsafe {
                    let _ = DestroyWindow(hwnd);
                }
                Err(error)
            }
        }
    }

    fn ledges(
        &mut self,
        island: (f32, f32, f32, f32),
        island_visible: bool,
        on_windows: bool,
        dock: Option<(f32, f32, f32)>,
    ) {
        let s = self.scale;
        let mut ledges = vec![Ledge {
            id: -1,
            x: 0.0,
            y: (self.bounds.bottom - self.bounds.top) as f32 / s,
            width: (self.bounds.right - self.bounds.left) as f32 / s,
        }];
        let mut add = |id, x: f32, y: f32, width: f32| {
            if y >= self.bounds.top as f32
                && y < self.bounds.bottom as f32
                && x + width > self.bounds.left as f32
                && x < self.bounds.right as f32
            {
                let left = x.max(self.bounds.left as f32);
                let right = (x + width).min(self.bounds.right as f32);
                ledges.push(Ledge {
                    id,
                    x: (left - self.bounds.left as f32) / s,
                    y: (y - self.bounds.top as f32) / s,
                    width: (right - left) / s,
                });
            }
        };
        if island_visible {
            add(-2, island.0 + 10.0, island.1 + 2.0, (island.2 - 20.0).max(1.0));
        }
        if let Some((x, y, width)) = dock {
            add(-3, x, y, width);
        }
        if on_windows {
            let mut front: Vec<RECT> = Vec::new();
            for window in desktop::cached_windows().into_iter().take(20) {
                let hwnd = HWND(window.handle as *mut _);
                let mut r = RECT::default();
                if window.minimized
                    || unsafe { IsIconic(hwnd).as_bool() }
                    || !unsafe { IsWindowVisible(hwnd).as_bool() }
                    || unsafe { GetWindowRect(hwnd, &mut r) }.is_err()
                {
                    continue;
                }
                let _ = unsafe {
                    windows::Win32::Graphics::Dwm::DwmGetWindowAttribute(
                        hwnd,
                        windows::Win32::Graphics::Dwm::DWMWA_EXTENDED_FRAME_BOUNDS,
                        (&raw mut r).cast(),
                        size_of::<RECT>() as u32,
                    )
                };
                if r.right - r.left < 120 || r.bottom - r.top < 80 {
                    continue;
                }
                if !unsafe { IsZoomed(hwnd).as_bool() } {
                    let mut spans = vec![(r.left + 8, r.right - 8)];
                    for cover in &front {
                        if cover.top > r.top + 4 || cover.bottom <= r.top {
                            continue;
                        }
                        spans = spans
                            .into_iter()
                            .flat_map(|(a, b)| {
                                let mut remaining = Vec::new();
                                if cover.right <= a || cover.left >= b {
                                    remaining.push((a, b));
                                } else {
                                    if cover.left > a {
                                        remaining.push((a, cover.left));
                                    }
                                    if cover.right < b {
                                        remaining.push((cover.right, b));
                                    }
                                }
                                remaining
                            })
                            .collect();
                    }
                    for (part, (left, right)) in spans.into_iter().take(8).enumerate() {
                        if right - left > 24 {
                            add(
                                window.handle.wrapping_mul(16).wrapping_add(part as isize),
                                left as f32,
                                r.top as f32,
                                (right - left) as f32,
                            );
                        }
                    }
                }
                front.push(r);
            }
        }
        self.snow.ledges(ledges);
    }

    fn render(&mut self, dt: f32, settings: WinterSettings, refresh: bool) -> Result<()> {
        let mut cursor = POINT::default();
        let _ = unsafe { GetCursorPos(&mut cursor) };
        let alt = settings.interactive && unsafe { GetAsyncKeyState(VK_MENU.0 as i32) } < 0;
        let catch = alt
            && self.snow.hit(
                (cursor.x - self.bounds.left) as f32 / self.scale,
                (cursor.y - self.bounds.top) as f32 / self.scale,
            );
        unsafe {
            let style = GetWindowLongPtrW(self.hwnd, GWL_EXSTYLE);
            let next = if catch || self.snow.held.is_some() {
                style & !(WS_EX_TRANSPARENT.0 as isize)
            } else {
                style | WS_EX_TRANSPARENT.0 as isize
            };
            if style != next {
                SetWindowLongPtrW(self.hwnd, GWL_EXSTYLE, next);
            }
        }
        if !alt && self.snow.held.is_some() {
            self.snow.release();
            unsafe {
                let _ = PostMessageW(Some(self.hwnd), WM_CANCELMODE, WPARAM(0), LPARAM(0));
            }
        }
        self.snow
            .drag((cursor.x - self.bounds.left) as f32 / self.scale, (cursor.y - self.bounds.top) as f32 / self.scale);
        self.snow.step(dt, settings.accumulate);
        for (flake, dot) in self.snow.flakes.iter().zip(&self.flakes) {
            dot.shape.SetOffset(Vector2 { X: flake.x, Y: flake.y })?;
        }
        if refresh || self.snow.held.is_some() {
            let mut active = 0;
            for drift in &self.snow.drifts {
                let width = drift.ledge.width / drift.depths.len() as f32;
                for (i, depth) in drift.depths.iter().enumerate().filter(|(_, v)| **v > 0.1) {
                    if active == self.pile_dots.len() {
                        let bead = dot(&self.compositor, &self.white, 1.0, 1.0)?;
                        self.piles.Shapes()?.Append(&bead.shape)?;
                        self.pile_dots.push(bead);
                    }
                    let bead = &self.pile_dots[active];
                    let start = i.saturating_sub(2);
                    let end = (i + 3).min(drift.depths.len());
                    let smooth = drift.depths[start..end].iter().sum::<f32>() / (end - start) as f32;
                    bead._geometry.SetRadius(Vector2 { X: width * 1.4, Y: smooth })?;
                    bead.shape.SetOffset(Vector2 {
                        X: drift.ledge.x + (i as f32 + 0.5) * width,
                        Y: drift.ledge.y - 0.5 * depth,
                    })?;
                    active += 1;
                }
            }
            for bead in &self.pile_dots[active..] {
                bead._geometry.SetRadius(Vector2::default())?;
            }
        }
        for (index, ball) in self.snow.balls.iter().enumerate() {
            if index == self.ball_dots.len() {
                let nose = self.compositor.CreateColorBrushWithColor(Color { A: 255, R: 244, G: 143, B: 67 })?;
                let beads = [
                    dot(&self.compositor, &self.white, 1.0, 1.0)?,
                    dot(&self.compositor, &self.ink, 1.5, 1.5)?,
                    dot(&self.compositor, &self.ink, 1.5, 1.5)?,
                    dot(&self.compositor, &nose, 1.0, 1.0)?,
                ];
                for bead in &beads {
                    self.balls.Shapes()?.Append(&bead.shape)?;
                }
                self.ball_dots.push(beads);
            }
            let beads = &self.ball_dots[index];
            beads[0]._geometry.SetRadius(Vector2 { X: ball.radius, Y: ball.radius })?;
            beads[0].shape.SetOffset(Vector2 { X: ball.x, Y: ball.y })?;
            let head = !self
                .snow
                .balls
                .iter()
                .any(|b| b.y < ball.y && (b.x - ball.x).abs() < 3.0 && ball.y - b.y < ball.radius + b.radius + 3.0)
                && self.snow.balls.iter().any(|b| {
                    b.radius > ball.radius
                        && (b.x - ball.x).abs() < 3.0
                        && b.y > ball.y
                        && b.y - ball.y < b.radius + ball.radius + 3.0
                });
            for (bead, dx) in beads[1..3].iter().zip([-0.28, 0.28]) {
                bead._geometry
                    .SetRadius(Vector2 { X: if head { 1.5 } else { 0.0 }, Y: if head { 1.5 } else { 0.0 } })?;
                bead.shape.SetOffset(Vector2 { X: ball.x + ball.radius * dx, Y: ball.y - ball.radius * 0.15 })?;
            }
            beads[3]._geometry.SetRadius(Vector2 {
                X: if head { ball.radius * 0.28 } else { 0.0 },
                Y: if head { 2.0 } else { 0.0 },
            })?;
            beads[3].shape.SetOffset(Vector2 { X: ball.x + ball.radius * 0.15, Y: ball.y + ball.radius * 0.18 })?;
        }
        Ok(())
    }
}

fn dot(c: &Compositor, brush: &CompositionColorBrush, x: f32, y: f32) -> Result<Dot> {
    let geometry = c.CreateEllipseGeometry()?;
    geometry.SetRadius(Vector2 { X: x, Y: y })?;
    let shape = c.CreateSpriteShapeWithGeometry(&geometry)?;
    shape.SetFillBrush(brush)?;
    Ok(Dot { _geometry: geometry, shape })
}

impl Winter {
    pub fn new(c: &Compositor, settings: WinterSettings) -> Result<Self> {
        let mut scenes = Vec::new();
        for monitor in desktop::monitors() {
            scenes.push(Scene::new(c, monitor, settings)?);
        }
        Ok(Self {
            scenes,
            settings,
            refresh: Instant::now() - Duration::from_secs(1),
            frame: Instant::now(),
            topology: Instant::now(),
            compositor: c.clone(),
        })
    }

    pub fn configure(&mut self, settings: WinterSettings) -> bool {
        if self.settings.dense != settings.dense {
            return false;
        }
        self.settings = settings;
        for scene in &mut self.scenes {
            scene.interactive = settings.interactive;
        }
        true
    }

    pub fn tick(&mut self, island: (f32, f32, f32, f32), visible: bool, dock: Option<(f32, f32, f32)>) -> Result<()> {
        if self.topology.elapsed() >= Duration::from_secs(2) {
            self.topology = Instant::now();
            let monitors = desktop::monitors();
            if monitors.len() != self.scenes.len()
                || monitors.iter().zip(&self.scenes).any(|(m, s)| {
                    m.bounds != s.bounds
                        || (unsafe { windows::Win32::UI::HiDpi::GetDpiForWindow(s.hwnd) } as f32 / 96.0 - s.scale).abs()
                            > 0.01
                })
            {
                let mut scenes = Vec::new();
                for monitor in monitors {
                    scenes.push(Scene::new(&self.compositor, monitor, self.settings)?);
                }
                self.scenes = scenes;
            }
        }
        let dt = self.frame.elapsed().as_secs_f32();
        self.frame = Instant::now();
        let refresh = self.refresh.elapsed() >= Duration::from_millis(150);
        if refresh {
            self.refresh = Instant::now();
        }
        let foreground = unsafe { GetForegroundWindow() };
        let mut rect = RECT::default();
        let mut class = [0u16; 128];
        let len = unsafe { GetClassNameW(foreground, &mut class) } as usize;
        let shell = matches!(String::from_utf16_lossy(&class[..len]).as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd");
        let valid = !shell && unsafe { GetWindowRect(foreground, &mut rect) }.is_ok();
        for scene in &mut self.scenes {
            let full = valid
                && rect.left <= scene.bounds.left
                && rect.top <= scene.bounds.top
                && rect.right >= scene.bounds.right
                && rect.bottom >= scene.bounds.bottom;
            if full != scene.suppressed {
                scene.suppressed = full;
                scene.root.SetIsVisible(!full)?;
                if full {
                    unsafe {
                        SetWindowLongPtrW(
                            scene.hwnd,
                            GWL_EXSTYLE,
                            GetWindowLongPtrW(scene.hwnd, GWL_EXSTYLE) | WS_EX_TRANSPARENT.0 as isize,
                        );
                    }
                }
            }
            if full {
                if scene.snow.held.is_some() {
                    scene.snow.release();
                    unsafe {
                        let _ = PostMessageW(Some(scene.hwnd), WM_CANCELMODE, WPARAM(0), LPARAM(0));
                    }
                }
                continue;
            }
            if refresh {
                scene.ledges(island, visible, self.settings.windows, dock);
            }
            scene.render(dt, self.settings, refresh)?;
        }
        Ok(())
    }
}

impl Drop for Scene {
    fn drop(&mut self) {
        unsafe {
            SetWindowLongPtrW(self.hwnd, GWLP_USERDATA, 0);
            let _ = DestroyWindow(self.hwnd);
        }
    }
}
