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

use super::winter_island::IslandSnow;
use super::winter_model::{Ledge, Snow};
use crate::platform::desktop_dock::{self as desktop, Monitor};

#[cfg(test)]
#[path = "winter_tests.rs"]
mod tests;

use crate::data::winter::WinterSettings;
use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;
use windows::Win32::Graphics::Direct2D::{
    D2D1_COMBINE_MODE_EXCLUDE, D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1CreateFactory, ID2D1Factory, ID2D1Geometry,
};

#[path = "winter_windows.rs"]
mod occlusion;

use occlusion::{WindowEdge, window_edges};

pub struct Winter {
    #[allow(clippy::vec_box)]
    scenes: Vec<Box<Scene>>,
    settings: WinterSettings,
    refresh: Instant,
    frame: Instant,
    topology: Instant,
    compositor: Compositor,
    windows: Vec<WindowEdge>,
    revision: u32,
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
    _root: ContainerVisual,
    flakes: Vec<Dot>,
    flakes_layer: ShapeVisual,
    piles: ShapeVisual,
    caps: Vec<super::winter_cap::Cap>,
    rounded_windows: Vec<(RECT, f32)>,
    balls: ShapeVisual,
    ball_dots: Vec<[Dot; 7]>,
    compositor: Compositor,
    white: CompositionColorBrush,
    ink: CompositionColorBrush,
    interactive: bool,
    suppressed: bool,
    presented: bool,
    mask: CompositionPathGeometry,
    factory: ID2D1Factory,
    exclusions: Vec<RECT>,
    cap_mask: CompositionPathGeometry,
    cap_regions: Vec<(f32, f32, f32, f32)>,
    island: IslandSnow,
    chips: Vec<(Dot, CompositionColorBrush)>,
    chip_layer: ShapeVisual,
    chip_mask: CompositionPathGeometry,
    protected_island: Option<RECT>,
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
                    if scene.interactive
                        && !scene.blocked(cursor)
                        && unsafe { GetAsyncKeyState(VK_MENU.0 as i32) } < 0
                        && scene.snow.hit(x, y)
                    {
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
            let mask = c.CreatePathGeometry()?;
            let clip = c.CreateGeometricClipWithGeometry(&mask)?;
            let factory: ID2D1Factory = unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)? };
            let white = c.CreateColorBrushWithColor(Color { A: 255, R: 238, G: 247, B: 255 })?;
            let ink = c.CreateColorBrushWithColor(Color { A: 255, R: 40, G: 54, B: 69 })?;
            let mut snow = Snow::new(
                (bounds.right - bounds.left) as f32 / scale,
                (bounds.bottom - bounds.top) as f32 / scale,
                if settings.dense { 220 } else { 90 },
                (bounds.left as u64).wrapping_add(73471),
            );
            snow.starter_cover = settings.starter_cover;
            snow.island_cap = settings.island_cap;
            let flakes_layer = c.CreateShapeVisual()?;
            flakes_layer.SetRelativeSizeAdjustment(Vector2 { X: 1.0, Y: 1.0 })?;
            let mut flakes = Vec::new();
            for flake in &snow.flakes {
                let dot = dot(c, &white, flake.radius, flake.radius)?;
                dot.shape.SetOffset(Vector2 { X: flake.x, Y: flake.y })?;
                flakes_layer.Shapes()?.Append(&dot.shape)?;
                flakes.push(dot);
            }
            flakes_layer.SetClip(&clip)?;
            root.Children()?.InsertAtTop(&flakes_layer)?;
            let piles = c.CreateShapeVisual()?;
            piles.SetRelativeSizeAdjustment(Vector2 { X: 1.0, Y: 1.0 })?;
            let cap_mask = c.CreatePathGeometry()?;
            piles.SetClip(&c.CreateGeometricClipWithGeometry(&cap_mask)?)?;
            root.Children()?.InsertAtTop(&piles)?;
            let chip_layer = c.CreateShapeVisual()?;
            chip_layer.SetRelativeSizeAdjustment(Vector2 { X: 1.0, Y: 1.0 })?;
            let chip_mask = c.CreatePathGeometry()?;
            chip_layer.SetClip(&c.CreateGeometricClipWithGeometry(&chip_mask)?)?;
            root.Children()?.InsertAtTop(&chip_layer)?;
            let balls = c.CreateShapeVisual()?;
            balls.SetRelativeSizeAdjustment(Vector2 { X: 1.0, Y: 1.0 })?;
            balls.SetClip(&clip)?;
            root.Children()?.InsertAtTop(&balls)?;
            Ok(Box::new(Self {
                hwnd,
                bounds,
                scale,
                snow,
                _target: target,
                _root: root,
                flakes,
                flakes_layer,
                piles,
                caps: Vec::new(),
                rounded_windows: Vec::new(),
                balls,
                ball_dots: Vec::new(),
                compositor: c.clone(),
                white,
                ink,
                interactive: settings.interactive,
                suppressed: false,
                presented: false,
                mask,
                factory,
                exclusions: Vec::new(),
                cap_mask,
                cap_regions: Vec::new(),
                island: IslandSnow::default(),
                chips: Vec::new(),
                chip_layer,
                chip_mask,
                protected_island: None,
            }))
        })();
        match result {
            Ok(mut scene) => {
                unsafe {
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, (&raw mut *scene) as isize);
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
        dock: Option<(f32, f32, f32, f32, f32)>,
        windows: &[WindowEdge],
        island_radius: f32,
    ) {
        let s = self.scale;
        let mut ledges = vec![Ledge {
            id: -1,
            x: 0.0,
            y: (self.bounds.bottom - self.bounds.top) as f32 / s,
            width: (self.bounds.right - self.bounds.left) as f32 / s,
            ..Default::default()
        }];
        let mut add = |id, x: f32, y: f32, width: f32, radius: f32| {
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
                    radius: radius / s,
                });
            }
        };
        if island_visible {
            add(-2, island.0, island.1, island.2, island_radius);
        }
        if let Some((x, y, width, _, radius)) = dock {
            add(-3, x, y, width, radius);
        }
        if on_windows {
            for window in windows {
                if !window.maximized {
                    let r = window.rect;
                    add(window.handle, r.left as f32, r.top as f32, (r.right - r.left).max(1) as f32, window.radius);
                }
            }
        }
        self.snow.ledges(ledges);
    }

    fn update_caps(&mut self, windows: &[WindowEdge]) -> Result<()> {
        let mut regions = Vec::new();
        for drift in &self.snow.drifts {
            let d = drift.ledge;
            let left = d.x * self.scale + self.bounds.left as f32;
            let right = (d.x + d.width) * self.scale + self.bounds.left as f32;
            let roof = d.y * self.scale + self.bounds.top as f32;
            let front: Vec<RECT> = if d.id == -1 {
                windows.iter().map(|w| w.rect).collect()
            } else if d.id >= 0 {
                windows.iter().take_while(|w| w.handle != d.id).map(|w| w.rect).collect()
            } else {
                Vec::new()
            };
            for (a, b) in occlusion::exposed_spans(left, right, roof, &front) {
                regions.push((
                    (a - self.bounds.left as f32) / self.scale,
                    d.y - 32.0,
                    (b - self.bounds.left as f32) / self.scale,
                    d.y + d.radius + 0.5,
                ));
            }
        }
        if self.cap_regions == regions && self.cap_mask.Path().is_ok() {
            return Ok(());
        }
        unsafe {
            use windows::Win32::Graphics::Direct2D::Common::{
                D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_END_CLOSED, D2D1_FILL_MODE_WINDING,
            };
            let path = self.factory.CreatePathGeometry()?;
            let sink = path.Open()?;
            sink.SetFillMode(D2D1_FILL_MODE_WINDING);
            for &(left, top, right, bottom) in &regions {
                sink.BeginFigure(Vector2 { X: left, Y: top }, D2D1_FIGURE_BEGIN_FILLED);
                sink.AddLines(&[
                    Vector2 { X: right, Y: top },
                    Vector2 { X: right, Y: bottom },
                    Vector2 { X: left, Y: bottom },
                ]);
                sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            }
            sink.Close()?;
            self.cap_mask.SetPath(&super::gfx::composition_path(&path)?)?;
        }
        self.cap_regions = regions;
        Ok(())
    }

    fn update_mask(&mut self, exclusions: Vec<RECT>) -> Result<()> {
        if exclusions == self.exclusions && self.mask.Path().is_ok() {
            return Ok(());
        }
        unsafe {
            let stage = self.factory.CreateRectangleGeometry(&D2D_RECT_F {
                left: 0.0,
                top: 0.0,
                right: (self.bounds.right - self.bounds.left) as f32 / self.scale,
                bottom: (self.bounds.bottom - self.bounds.top) as f32 / self.scale,
            })?;
            let mut outside: ID2D1Geometry = stage.cast()?;

            for rect in &exclusions {
                if rect.right <= self.bounds.left
                    || rect.left >= self.bounds.right
                    || rect.bottom <= self.bounds.top
                    || rect.top >= self.bounds.bottom
                {
                    continue;
                }
                let radius = self
                    .rounded_windows
                    .iter()
                    .find(|(r, _)| r.left == rect.left && r.right == rect.right && r.bottom == rect.bottom)
                    .map_or(0.0, |(_, r)| *r / self.scale);
                let hole = self.factory.CreateRoundedRectangleGeometry(
                    &windows::Win32::Graphics::Direct2D::D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: (rect.left - self.bounds.left) as f32 / self.scale,
                            top: (rect.top - self.bounds.top) as f32 / self.scale,
                            right: (rect.right - self.bounds.left) as f32 / self.scale,
                            bottom: (rect.bottom - self.bounds.top) as f32 / self.scale,
                        },
                        radiusX: radius,
                        radiusY: radius,
                    },
                )?;
                let path = self.factory.CreatePathGeometry()?;
                let sink = path.Open()?;
                outside.CombineWithGeometry(&hole, D2D1_COMBINE_MODE_EXCLUDE, None, 0.25, &sink)?;
                sink.Close()?;
                outside = path.cast()?;
            }
            let path = self.factory.CreatePathGeometry()?;
            let sink = path.Open()?;
            outside.Simplify(
                windows::Win32::Graphics::Direct2D::D2D1_GEOMETRY_SIMPLIFICATION_OPTION_CUBICS_AND_LINES,
                None,
                0.25,
                &sink,
            )?;
            sink.Close()?;
            self.mask.SetPath(&super::gfx::composition_path(&path)?)?;
            if let Some(rect) = self.protected_island {
                let protected = self.factory.CreateRectangleGeometry(&D2D_RECT_F {
                    left: (rect.left - self.bounds.left) as f32 / self.scale,
                    top: (rect.top - self.bounds.top) as f32 / self.scale,
                    right: (rect.right - self.bounds.left) as f32 / self.scale,
                    bottom: (rect.bottom - self.bounds.top) as f32 / self.scale,
                })?;
                let reveal = self.factory.CreatePathGeometry()?;
                let sink = reveal.Open()?;
                outside.CombineWithGeometry(
                    &protected,
                    windows::Win32::Graphics::Direct2D::D2D1_COMBINE_MODE_UNION,
                    None,
                    0.25,
                    &sink,
                )?;
                sink.Close()?;
                self.chip_mask.SetPath(&super::gfx::composition_path(&reveal)?)?;
            } else {
                self.chip_mask.SetPath(&super::gfx::composition_path(&path)?)?;
            }
        }
        self.exclusions = exclusions;
        Ok(())
    }

    fn blocked(&self, point: POINT) -> bool {
        self.exclusions.iter().any(|r| point.x >= r.left && point.x < r.right && point.y >= r.top && point.y < r.bottom)
            && !self.cap_regions.iter().any(|&(l, t, r, b)| {
                let x = (point.x - self.bounds.left) as f32 / self.scale;
                let y = (point.y - self.bounds.top) as f32 / self.scale;
                x >= l && x < r && y >= t && y < b
            })
    }

    fn render(&mut self, dt: f32, settings: WinterSettings, _refresh: bool) -> Result<()> {
        let mut cursor = POINT::default();
        let _ = unsafe { GetCursorPos(&mut cursor) };
        let alt = settings.interactive && unsafe { GetAsyncKeyState(VK_MENU.0 as i32) } < 0;
        let catch = alt
            && !self.blocked(cursor)
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
        self.snow.dynamics = settings.physics;
        self.snow.starter_cover = settings.starter_cover;
        self.snow.island_cap = settings.island_cap;
        self.snow.step(dt, settings.accumulate);
        self.island.step(dt);
        for (flake, dot) in self.snow.flakes.iter().zip(&self.flakes) {
            dot.shape.SetOffset(Vector2 { X: flake.x, Y: flake.y })?;
        }
        let changed = self.caps.len() != self.snow.drifts.len()
            || self.caps.iter().any(|c| !self.snow.drifts.iter().any(|d| d.ledge.id == c.id));
        if changed {
            self.caps.retain(|c| self.snow.drifts.iter().any(|d| d.ledge.id == c.id));
            for drift in &self.snow.drifts {
                if !self.caps.iter().any(|c| c.id == drift.ledge.id) {
                    self.caps.push(super::winter_cap::Cap::new(&self.compositor, &self.white, drift)?);
                }
            }
            self.piles.Shapes()?.Clear()?;
            for cap in &self.caps {
                self.piles.Shapes()?.Append(&cap.shape)?;
            }
        }
        let cursor =
            ((cursor.x - self.bounds.left) as f32 / self.scale, (cursor.y - self.bounds.top) as f32 / self.scale);
        for cap in &mut self.caps {
            if let Some(drift) = self.snow.drifts.iter().find(|d| d.ledge.id == cap.id) {
                cap.update(
                    &self.factory,
                    drift,
                    dt,
                    if cap.id == -2 { self.island.coverage.max(0.28) } else { 1.0 },
                    cursor,
                )?;
            }
        }
        for (index, ball) in self.snow.balls.iter().enumerate() {
            if index == self.ball_dots.len() {
                let nose = self.compositor.CreateColorBrushWithColor(Color { A: 255, R: 244, G: 143, B: 67 })?;
                let grain = self.compositor.CreateColorBrushWithColor(Color { A: 80, R: 255, G: 255, B: 255 })?;
                let beads = [
                    dot(&self.compositor, &self.white, 1.0, 1.0)?,
                    dot(&self.compositor, &self.ink, 1.5, 1.5)?,
                    dot(&self.compositor, &self.ink, 1.5, 1.5)?,
                    dot(&self.compositor, &nose, 1.0, 1.0)?,
                    dot(&self.compositor, &grain, 1.0, 1.0)?,
                    dot(&self.compositor, &grain, 1.0, 1.0)?,
                    dot(&self.compositor, &grain, 1.0, 1.0)?,
                ];
                let skin = self.compositor.CreateRadialGradientBrush()?;
                skin.SetEllipseCenter(Vector2 { X: 0.32, Y: 0.25 })?;
                skin.SetEllipseRadius(Vector2 { X: 0.85, Y: 0.9 })?;
                for (at, color) in [
                    (0.0, Color { A: 255, R: 249, G: 253, B: 255 }),
                    (0.6, Color { A: 255, R: 229, G: 241, B: 251 }),
                    (1.0, Color { A: 255, R: 179, G: 209, B: 234 }),
                ] {
                    skin.ColorStops()?
                        .Append(&self.compositor.CreateColorGradientStopWithOffsetAndColor(at, color)?)?;
                }
                beads[0].shape.SetFillBrush(&skin)?;
                for bead in &beads {
                    self.balls.Shapes()?.Append(&bead.shape)?;
                }
                self.ball_dots.push(beads);
            }
            let beads = &self.ball_dots[index];
            let rx = ball.radius * (1.0 + ball.squash);
            let ry = ball.radius / (1.0 + ball.squash);
            let center_y = ball.y + ball.radius - ry;
            beads[0]._geometry.SetRadius(Vector2 { X: rx, Y: ry })?;
            beads[0].shape.SetOffset(Vector2 { X: ball.x, Y: center_y })?;
            for (i, grain) in beads[4..].iter().enumerate() {
                let angle = ball.angle + i as f32 * 2.1;
                grain.shape.SetOffset(Vector2 {
                    X: ball.x + rx * 0.52 * angle.cos(),
                    Y: center_y + ry * 0.52 * angle.sin(),
                })?;
                grain._geometry.SetRadius(Vector2 { X: ball.radius * 0.07, Y: ball.radius * 0.05 })?;
            }
            let above = self.snow.balls.iter().any(|b| {
                b.parent == Some(index)
                    || (b.y < ball.y && (b.x - ball.x).abs() < 3.0 && ball.y - b.y < ball.radius + b.radius + 3.0)
            });
            let below =
                ball.parent.is_some()
                    || self.snow.balls.iter().any(|b| {
                        (b.x - ball.x).abs() < 3.0 && b.y > ball.y && b.y - ball.y < b.radius + ball.radius + 3.0
                    });
            let head = !above && below;
            for (bead, dx) in beads[1..3].iter().zip([-0.28, 0.28]) {
                bead._geometry
                    .SetRadius(Vector2 { X: if head { 1.5 } else { 0.0 }, Y: if head { 1.5 } else { 0.0 } })?;
                bead.shape.SetOffset(Vector2 { X: ball.x + ball.radius * dx, Y: center_y - ry * 0.15 })?;
            }
            beads[3]._geometry.SetRadius(Vector2 {
                X: if head { ball.radius * 0.28 } else { 0.0 },
                Y: if head { 2.0 } else { 0.0 },
            })?;
            beads[3].shape.SetOffset(Vector2 { X: ball.x + ball.radius * 0.15, Y: center_y + ry * 0.18 })?;
        }
        for (index, chip) in self.island.chips.iter().chain(self.snow.crumbs.iter()).enumerate() {
            if index == self.chips.len() {
                let brush = self.compositor.CreateColorBrushWithColor(Color { A: 255, R: 238, G: 247, B: 255 })?;
                let bead = dot(&self.compositor, &brush, chip.radius, chip.radius)?;
                self.chip_layer.Shapes()?.Append(&bead.shape)?;
                self.chips.push((bead, brush));
            }
            let (bead, brush) = &self.chips[index];
            bead._geometry.SetRadius(Vector2 { X: chip.radius, Y: chip.radius * 0.7 })?;
            bead.shape.SetOffset(Vector2 { X: chip.x, Y: chip.y })?;
            brush.SetColor(Color { A: (chip.life * 255.0) as u8, R: 238, G: 247, B: 255 })?;
        }
        for (bead, _) in &self.chips[self.island.chips.len() + self.snow.crumbs.len()..] {
            bead._geometry.SetRadius(Vector2::default())?;
        }
        if !self.presented {
            unsafe {
                let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
            }
            self.presented = true;
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
            windows: Vec::new(),
            revision: 0,
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

    pub fn tick(
        &mut self,
        island: (f32, f32, f32, f32),
        visible: bool,
        dock: Option<(f32, f32, f32, f32, f32)>,
        expanded: bool,
        island_radius: f32,
    ) -> Result<()> {
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
        let revision = crate::platform::winevent::layout_revision();
        let refresh = self.refresh.elapsed() >= Duration::from_millis(100) || revision != self.revision;
        self.revision = revision;
        if refresh {
            self.refresh = Instant::now();
        }
        if refresh {
            self.windows = window_edges();
        }
        let windows = &self.windows;
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
                scene.flakes_layer.SetIsVisible(!full)?;
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
            if full && scene.snow.held.is_some() {
                scene.snow.release();
                unsafe {
                    let _ = PostMessageW(Some(scene.hwnd), WM_CANCELMODE, WPARAM(0), LPARAM(0));
                }
            }
            {
                scene.ledges(island, visible, self.settings.windows, dock, windows, island_radius);
                scene.rounded_windows = windows.iter().map(|w| (w.rect, w.radius)).collect();
                let ledge = scene.snow.drifts.iter().find(|d| d.ledge.id == -2).map(|d| d.ledge);
                scene.island.follow(ledge, expanded, self.settings.island_shake);
                scene.update_caps(windows)?;
                let mut exclusions: Vec<RECT> =
                    windows.iter().map(|w| RECT { top: w.rect.top + 1, ..w.rect }).collect();
                if visible {
                    exclusions.push(RECT {
                        left: island.0.floor() as i32,
                        top: (island.1 + 2.0).floor() as i32,
                        right: (island.0 + island.2).ceil() as i32,
                        bottom: (island.1 + island.3).ceil() as i32,
                    });
                }
                if let Some((x, y, width, height, _)) = dock {
                    exclusions.push(RECT {
                        left: (x - 12.0 * scene.scale).floor() as i32,
                        top: (y + 2.0).floor() as i32,
                        right: (x + width + 12.0 * scene.scale).ceil() as i32,
                        bottom: (y + height).ceil() as i32,
                    });
                }
                scene.protected_island = visible.then_some(RECT {
                    left: island.0.floor() as i32,
                    top: island.1.floor() as i32,
                    right: (island.0 + island.2).ceil() as i32,
                    bottom: (island.1 + island.3).ceil() as i32,
                });
                scene.update_mask(exclusions)?;
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
