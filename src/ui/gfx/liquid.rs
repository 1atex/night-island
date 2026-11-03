use super::*;
use crate::data::settings::Hang;

pub struct LiquidBody {
    pub rect: (f32, f32, f32, f32),
    pub radius: f32,
    pub edge: Hang,
    pub center: f32,
    pub half: f32,
    pub depth: f32,
}

impl Gpu {
    pub fn liquid_body(&self, body: &LiquidBody) -> Result<ID2D1PathGeometry> {
        liquid_body_geometry(&self.factory, body)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct ElasticPull {
    pub point: (f32, f32),
    pub normal: (f32, f32),
    pub spread: f32,
    pub depth: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ElasticBody {
    pub rect: (f32, f32, f32, f32),
    pub radius: f32,
    pub pull: ElasticPull,
}

struct Outline {
    rect: (f32, f32, f32, f32),
    radius: f32,
    lengths: [f32; 8],
    perimeter: f32,
}

impl Outline {
    fn new(rect: (f32, f32, f32, f32), radius: f32) -> Self {
        let (_, _, w, h) = rect;
        let r = radius.min(w.min(h) / 2.0).max(0.0);
        let arc = r * std::f32::consts::FRAC_PI_2;
        let lengths = [w - 2.0 * r, arc, h - 2.0 * r, arc, w - 2.0 * r, arc, h - 2.0 * r, arc];
        Self { rect, radius: r, lengths, perimeter: lengths.iter().sum() }
    }

    fn at(&self, t: f32) -> ((f32, f32), (f32, f32)) {
        let (x, y, w, h) = self.rect;
        let r = self.radius;
        let mut distance = t.rem_euclid(1.0) * self.perimeter;
        let mut segment = 7;
        for (i, length) in self.lengths.iter().enumerate() {
            if distance <= *length && *length > 0.0 {
                segment = i;
                break;
            }
            distance -= length;
        }
        let u = (distance / self.lengths[segment].max(0.001)).clamp(0.0, 1.0);
        let arc = |cx: f32, cy: f32, start: f32| {
            let angle = start + u * std::f32::consts::FRAC_PI_2;
            let normal = (angle.cos(), angle.sin());
            ((cx + r * normal.0, cy + r * normal.1), normal)
        };
        let ((px, py), normal) = match segment {
            0 => ((r + u * (w - 2.0 * r), 0.0), (0.0, -1.0)),
            1 => arc(w - r, r, -std::f32::consts::FRAC_PI_2),
            2 => ((w, r + u * (h - 2.0 * r)), (1.0, 0.0)),
            3 => arc(w - r, h - r, 0.0),
            4 => ((w - r - u * (w - 2.0 * r), h), (0.0, 1.0)),
            5 => arc(r, h - r, std::f32::consts::FRAC_PI_2),
            6 => ((0.0, h - r - u * (h - 2.0 * r)), (-1.0, 0.0)),
            _ => arc(r, r, std::f32::consts::PI),
        };
        ((x + px, y + py), normal)
    }

    fn position(&self, pull: ElasticPull) -> f32 {
        let (x, y, w, h) = self.rect;
        let r = self.radius;
        let (nx, ny) = pull.normal;
        let (segment, distance) = if ny <= -0.9999 {
            (0, (pull.point.0 - x - r).clamp(0.0, self.lengths[0]))
        } else if nx >= 0.9999 {
            (2, (pull.point.1 - y - r).clamp(0.0, self.lengths[2]))
        } else if ny >= 0.9999 {
            (4, (x + w - r - pull.point.0).clamp(0.0, self.lengths[4]))
        } else if nx <= -0.9999 {
            (6, (y + h - r - pull.point.1).clamp(0.0, self.lengths[6]))
        } else if nx >= 0.0 && ny < 0.0 {
            (1, r * (ny.atan2(nx) + std::f32::consts::FRAC_PI_2))
        } else if nx >= 0.0 {
            (3, r * ny.atan2(nx))
        } else if ny >= 0.0 {
            (5, r * (ny.atan2(nx) - std::f32::consts::FRAC_PI_2))
        } else {
            (7, r * (ny.atan2(nx) + std::f32::consts::PI))
        };
        (self.lengths[..segment].iter().sum::<f32>() + distance) / self.perimeter.max(1.0)
    }
}

impl ElasticPull {
    pub fn follow(self, target: Self, rect: (f32, f32, f32, f32), radius: f32, blend: f32) -> Self {
        let outline = Outline::new(rect, radius);
        let from = outline.position(self);
        let delta = (outline.position(target) - from + 0.5).rem_euclid(1.0) - 0.5;
        let (point, normal) = outline.at(from + delta * blend.clamp(0.0, 1.0));
        Self { point, normal, ..target }
    }
}

impl LiquidBody {
    pub fn elastic(&self) -> ElasticBody {
        let (x, y, w, h) = self.rect;
        let (point, normal) = match self.edge {
            Hang::Down => ((x + w / 2.0 + self.center, y + h), (0.0, 1.0)),
            Hang::Up => ((x + w / 2.0 - self.center, y), (0.0, -1.0)),
            Hang::Right => ((x + w, y + h / 2.0 - self.center), (1.0, 0.0)),
            Hang::Left => ((x, y + h / 2.0 + self.center), (-1.0, 0.0)),
        };
        ElasticBody {
            rect: self.rect,
            radius: self.radius,
            pull: ElasticPull { point, normal, spread: self.half, depth: self.depth },
        }
    }
}

pub fn cursor_pull(
    rect: (f32, f32, f32, f32),
    radius: f32,
    cursor: (f32, f32),
    reach: f32,
    strength: f32,
    blocked: Option<Hang>,
) -> ElasticPull {
    let (x, y, w, h) = rect;
    let r = radius.min(w.min(h) / 2.0);
    let q = (cursor.0.clamp(x + r, x + w - r), cursor.1.clamp(y + r, y + h - r));
    let delta = (cursor.0 - q.0, cursor.1 - q.1);
    let length = delta.0.hypot(delta.1);
    let (point, mut normal, distance) = if length > 0.001 {
        let normal = (delta.0 / length, delta.1 / length);
        ((q.0 + normal.0 * r, q.1 + normal.1 * r), normal, (length - r).max(0.0))
    } else {
        let options = [
            (cursor.0 - x, (x, cursor.1), (-1.0, 0.0)),
            (x + w - cursor.0, (x + w, cursor.1), (1.0, 0.0)),
            (cursor.1 - y, (cursor.0, y), (0.0, -1.0)),
            (y + h - cursor.1, (cursor.0, y + h), (0.0, 1.0)),
        ];
        let (_, point, normal) = options.into_iter().min_by(|a, b| a.0.total_cmp(&b.0)).unwrap();
        (point, normal, 0.0)
    };
    match blocked {
        Some(Hang::Down) => normal.1 = normal.1.min(0.0),
        Some(Hang::Up) => normal.1 = normal.1.max(0.0),
        Some(Hang::Right) => normal.0 = normal.0.min(0.0),
        Some(Hang::Left) => normal.0 = normal.0.max(0.0),
        None => {}
    }
    let magnitude = normal.0.hypot(normal.1);
    let t = (1.0 - distance / reach.max(1.0)).clamp(0.0, 1.0);
    let depth = if magnitude > 0.001 { strength * t * t * (3.0 - 2.0 * t) } else { 0.0 };
    if magnitude > 0.001 {
        normal = (normal.0 / magnitude, normal.1 / magnitude);
    }
    ElasticPull { point, normal, spread: 44.0, depth }
}

fn liquid_body_geometry(factory: &ID2D1Factory, body: &LiquidBody) -> Result<ID2D1PathGeometry> {
    elastic_geometry(factory, &body.elastic())
}

fn elastic_geometry(factory: &ID2D1Factory, body: &ElasticBody) -> Result<ID2D1PathGeometry> {
    let outline = Outline::new(body.rect, body.radius);
    let (_, _, w, h) = body.rect;
    let at = |t: f32| {
        let ((px, py), outward) = outline.at(t);
        let delta = (px - body.pull.point.0, py - body.pull.point.1);
        let tangent = delta.0 * -body.pull.normal.1 + delta.1 * body.pull.normal.0;
        let normal = delta.0 * body.pull.normal.0 + delta.1 * body.pull.normal.1;
        let fade = (1.0 + normal / w.min(h).clamp(1.0, 80.0)).clamp(0.0, 1.0);
        let alignment = (outward.0 * body.pull.normal.0 + outward.1 * body.pull.normal.1).max(0.0);
        let weight = (-3.0 * (tangent / body.pull.spread.max(1.0)).powi(2)).exp()
            * fade
            * fade
            * (3.0 - 2.0 * fade)
            * alignment
            * alignment;
        (px + outward.0 * body.pull.depth * weight, py + outward.1 * body.pull.depth * weight)
    };
    let tangent = |t: f32| {
        let a = at(t - 0.0001);
        let b = at(t + 0.0001);
        ((b.0 - a.0) / 0.0002, (b.1 - a.1) / 0.0002)
    };
    let vector = |p: (f32, f32)| windows_numerics::Vector2 { X: p.0, Y: p.1 };
    unsafe {
        let geometry = factory.CreatePathGeometry()?;
        let sink = geometry.Open()?;
        sink.BeginFigure(vector(at(0.0)), D2D1_FIGURE_BEGIN_FILLED);
        for i in 0..128 {
            let t0 = i as f32 / 128.0;
            let t1 = (i + 1) as f32 / 128.0;
            let a = at(t0);
            let b = at(t1);
            let da = tangent(t0);
            let db = tangent(t1);
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: vector((a.0 + da.0 / 384.0, a.1 + da.1 / 384.0)),
                point2: vector((b.0 - db.0 / 384.0, b.1 - db.1 / 384.0)),
                point3: vector(b),
            });
        }
        sink.EndFigure(D2D1_FIGURE_END_CLOSED);
        sink.Close()?;
        Ok(geometry)
    }
}

impl Gpu {
    pub fn elastic_body(&self, body: &ElasticBody) -> Result<ID2D1PathGeometry> {
        elastic_geometry(&self.factory, body)
    }

    pub fn outside_body(
        &self,
        size: (i32, i32),
        body: &ElasticBody,
        transform: &windows_numerics::Matrix3x2,
    ) -> Result<ID2D1PathGeometry> {
        use windows::Win32::Graphics::Direct2D::D2D1_COMBINE_MODE_EXCLUDE;
        unsafe {
            let bounds = self.factory.CreateRectangleGeometry(&D2D_RECT_F {
                left: 0.0,
                top: 0.0,
                right: size.0 as f32,
                bottom: size.1 as f32,
            })?;
            let body = self.elastic_body(body)?;
            let visible = self.factory.CreatePathGeometry()?;
            let sink = visible.Open()?;
            bounds.CombineWithGeometry(&body, D2D1_COMBINE_MODE_EXCLUDE, Some(transform), 0.1, &sink)?;
            sink.Close()?;
            Ok(visible)
        }
    }
}

impl Painter<'_> {
    pub fn liquid_highlight(&self, size: (i32, i32), pull: &ElasticPull, color: D2D1_COLOR_F) -> Result<()> {
        use windows::Win32::Graphics::Direct2D::Common::D2D1_GRADIENT_STOP;
        use windows::Win32::Graphics::Direct2D::{
            D2D1_EXTEND_MODE_CLAMP, D2D1_GAMMA_2_2, D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES,
        };
        let stops = [
            D2D1_GRADIENT_STOP { position: 0.0, color: D2D1_COLOR_F { a: 0.0, ..color } },
            D2D1_GRADIENT_STOP { position: 1.0, color: D2D1_COLOR_F { a: 0.2, ..color } },
        ];
        unsafe {
            let gradient = self.dc.CreateGradientStopCollection(&stops, D2D1_GAMMA_2_2, D2D1_EXTEND_MODE_CLAMP)?;
            let line = D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES {
                startPoint: windows_numerics::Vector2 { X: pull.point.0, Y: pull.point.1 },
                endPoint: windows_numerics::Vector2 {
                    X: pull.point.0 + pull.normal.0 * pull.depth.max(1.0),
                    Y: pull.point.1 + pull.normal.1 * pull.depth.max(1.0),
                },
            };
            let brush = self.dc.CreateLinearGradientBrush(&line, None, &gradient)?;
            self.dc.FillRectangle(
                &D2D_RECT_F { left: 0.0, top: 0.0, right: size.0 as f32, bottom: size.1 as f32 },
                &brush,
            );
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changing_sides_keeps_the_pull_on_the_surface_and_never_cuts_into_the_pill() -> Result<()> {
        let factory: ID2D1Factory = unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)? };
        for scale in [0.75, 1.0, 2.0] {
            let rect = (80.0 * scale, 80.0 * scale, 240.0 * scale, 34.0 * scale);
            let radius = 17.0 * scale;
            let base = elastic_geometry(&factory, &ElasticBody { rect, radius, pull: Default::default() })?;
            let mut pull = cursor_pull(rect, radius, (100.0 * scale, 145.0 * scale), 160.0 * scale, 40.0 * scale, None);
            for cursor in [(100.0, 50.0), (350.0, 97.0), (50.0, 97.0), (200.0, 145.0), (100.0, 50.0)] {
                let target =
                    cursor_pull(rect, radius, (cursor.0 * scale, cursor.1 * scale), 160.0 * scale, 40.0 * scale, None);
                for _ in 0..12 {
                    pull = pull.follow(target, rect, radius, 0.2);
                    let q = (
                        pull.point.0.clamp(rect.0 + radius, rect.0 + rect.2 - radius),
                        pull.point.1.clamp(rect.1 + radius, rect.1 + rect.3 - radius),
                    );
                    assert!(((pull.point.0 - q.0).hypot(pull.point.1 - q.1) - radius).abs() < 0.02);
                    let shape = elastic_geometry(&factory, &ElasticBody { rect, radius, pull })?;
                    for ix in 1..48 {
                        for iy in 1..17 {
                            let point = windows_numerics::Vector2 {
                                X: rect.0 + ix as f32 * 5.0 * scale,
                                Y: rect.1 + iy as f32 * 2.0 * scale,
                            };
                            if unsafe { base.FillContainsPoint(point, None, 0.01)? }.as_bool() {
                                assert!(
                                    unsafe { shape.FillContainsPoint(point, None, 0.01)? }.as_bool(),
                                    "inward fold at {point:?}, pull {pull:?}"
                                );
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    #[test]
    fn cursor_can_pull_the_short_caps_and_corners_of_a_horizontal_island() -> Result<()> {
        let factory: ID2D1Factory = unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)? };
        let rect = (80.0, 80.0, 240.0, 34.0);
        for (cursor, normal) in [
            ((50.0, 97.0), (-1.0, 0.0)),
            ((350.0, 97.0), (1.0, 0.0)),
            ((200.0, 50.0), (0.0, -1.0)),
            ((200.0, 144.0), (0.0, 1.0)),
        ] {
            let pull = cursor_pull(rect, 17.0, cursor, 160.0, 24.0, None);
            assert!(pull.depth > 0.0);
            assert!((pull.normal.0 - normal.0).abs() < 0.001 && (pull.normal.1 - normal.1).abs() < 0.001);
            let shape = elastic_geometry(&factory, &ElasticBody { rect, radius: 17.0, pull })?;
            let probe = windows_numerics::Vector2 {
                X: pull.point.0 + pull.normal.0 * pull.depth * 0.8,
                Y: pull.point.1 + pull.normal.1 * pull.depth * 0.8,
            };
            assert!(unsafe { shape.FillContainsPoint(probe, None, 0.01)? }.as_bool());
        }
        let corner = cursor_pull(rect, 17.0, (340.0, 60.0), 160.0, 24.0, None);
        assert!(corner.normal.0 > 0.0 && corner.normal.1 < 0.0);
        let blocked = cursor_pull(rect, 17.0, (200.0, 50.0), 160.0, 24.0, Some(Hang::Up));
        assert_eq!(blocked.depth, 0.0);
        assert_eq!(cursor_pull(rect, 17.0, (1000.0, 1000.0), 160.0, 24.0, None).depth, 0.0);
        Ok(())
    }

    #[test]
    fn liquid_is_one_connected_body_in_every_direction_and_scale() -> Result<()> {
        let factory: ID2D1Factory = unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)? };
        for scale in [0.75, 1.0, 1.25, 1.5, 2.0] {
            for edge in [Hang::Down, Hang::Up, Hang::Left, Hang::Right] {
                let side = matches!(edge, Hang::Left | Hang::Right);
                let (w, h) = if side { (40.0 * scale, 240.0 * scale) } else { (240.0 * scale, 40.0 * scale) };
                for depth in [0.0, 2.0, 24.0, 40.0] {
                    let shape = liquid_body_geometry(
                        &factory,
                        &LiquidBody {
                            rect: (80.0, 80.0, w, h),
                            radius: 20.0 * scale,
                            edge,
                            center: 0.0,
                            half: 44.0 * scale,
                            depth: depth * scale,
                        },
                    )?;
                    unsafe {
                        assert_eq!(shape.GetFigureCount()?, 1);
                        let bounds = shape.GetBounds(None)?;
                        let expected = match edge {
                            Hang::Down => (80.0, 80.0, 80.0 + w, 80.0 + h + depth * scale),
                            Hang::Up => (80.0, 80.0 - depth * scale, 80.0 + w, 80.0 + h),
                            Hang::Right => (80.0, 80.0, 80.0 + w + depth * scale, 80.0 + h),
                            Hang::Left => (80.0 - depth * scale, 80.0, 80.0 + w, 80.0 + h),
                        };
                        for (value, want) in [
                            (bounds.left, expected.0),
                            (bounds.top, expected.1),
                            (bounds.right, expected.2),
                            (bounds.bottom, expected.3),
                        ] {
                            assert!((value - want).abs() < 0.05);
                        }
                        for step in 0..=60 {
                            let distance = -10.0 * scale + (10.0 + depth - 0.5) * scale * step as f32 / 60.0;
                            let (x, y) = match edge {
                                Hang::Down => (80.0 + w / 2.0, 80.0 + h + distance),
                                Hang::Up => (80.0 + w / 2.0, 80.0 - distance),
                                Hang::Left => (80.0 - distance, 80.0 + h / 2.0),
                                Hang::Right => (80.0 + w + distance, 80.0 + h / 2.0),
                            };
                            assert!(
                                shape
                                    .FillContainsPoint(windows_numerics::Vector2 { X: x, Y: y }, None, 0.01)?
                                    .as_bool(),
                                "gap at scale {scale}, depth {depth}, step {step}"
                            );
                        }
                    }
                }
            }
        }
        Ok(())
    }

    #[test]
    #[ignore = "Requires an interactive Windows desktop and an isolated Composition runtime"]
    fn unified_liquid_clip_morphs_and_renders_without_a_separate_base() -> Result<()> {
        use crate::ui::popup::PopupSurface;
        use windows::UI::Composition::{AnimationIterationBehavior, Compositor};
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
        let sample = ElasticBody {
            rect: (40.0, 40.0, 260.0, 34.0),
            radius: 17.0,
            pull: ElasticPull { point: (170.0, 74.0), normal: (0.0, 1.0), spread: 44.0, depth: 28.0 },
        };
        let transform =
            windows_numerics::Matrix3x2 { M11: 1.0, M22: 1.0, M31: -57.0, M32: -71.0, ..Default::default() };
        let outside = gpu.outside_body((226, 47), &sample, &transform)?;
        unsafe {
            assert!(!outside.FillContainsPoint(windows_numerics::Vector2 { X: 113.0, Y: 10.0 }, None, 0.01)?.as_bool());
            assert!(outside.FillContainsPoint(windows_numerics::Vector2 { X: 28.0, Y: 30.0 }, None, 0.01)?.as_bool());
        }
        let mask = compositor.CreatePathGeometry()?;
        mask.SetPath(&composition_path(&outside)?)?;
        compositor.CreateSpriteVisual()?.SetClip(&compositor.CreateGeometricClipWithGeometry(&mask)?)?;
        let shape = compositor.CreatePathGeometry()?;
        let clip = compositor.CreateGeometricClipWithGeometry(&shape)?;
        let body = compositor.CreateSpriteVisual()?;
        body.SetClip(&clip)?;
        let animation = compositor.CreatePathKeyFrameAnimation()?;
        for (key, depth, center) in [(0.0, 2.0, -6.0), (0.5, 5.0, 6.0), (1.0, 2.0, -6.0)] {
            let geometry = gpu.liquid_body(&LiquidBody {
                rect: (30.0, 20.0, 260.0, 40.0),
                radius: 20.0,
                edge: Hang::Down,
                center,
                half: 44.0,
                depth,
            })?;
            let path = composition_path(&geometry)?;
            shape.SetPath(&path)?;
            animation.InsertKeyFrame(key, &path)?;
        }
        animation.SetDuration(std::time::Duration::from_millis(5200).into())?;
        animation.SetIterationBehavior(AnimationIterationBehavior::Forever)?;
        shape.StartAnimation(
            windows::core::h!("Path"),
            &animation.cast::<windows::UI::Composition::CompositionAnimation>()?,
        )?;
        shape.StopAnimation(windows::core::h!("Path"))?;
        let surface = PopupSurface::new((340, 280))?;
        surface.paint(&gpu, |p| {
            for (row, (depth, center)) in [(2.0, 0.0), (22.0, -45.0), (32.0, 50.0)].into_iter().enumerate() {
                let geometry = gpu.liquid_body(&LiquidBody {
                    rect: (40.0, 18.0 + row as f32 * 85.0, 260.0, 40.0),
                    radius: 20.0,
                    edge: Hang::Down,
                    center,
                    half: 44.0,
                    depth,
                })?;
                unsafe {
                    let brush = p.dc.CreateSolidColorBrush(&rgba(0.13, 0.13, 0.14, 1.0), None)?;
                    p.dc.FillGeometry(&geometry, &brush, None);
                }
            }
            Ok(())
        })?;
        if let Some(dir) = std::env::var_os("NI_LIQUID_PREVIEW") {
            let data = surface.pixels();
            let png_pixels: Vec<u8> = data.chunks_exact(4).flat_map(|p| [p[2], p[1], p[0], p[3]]).collect();
            let mut encoder = png::Encoder::new(
                std::fs::File::create(std::path::PathBuf::from(&dir).join("unified-liquid.png")).unwrap(),
                340,
                280,
            );
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.write_header().unwrap().write_image_data(&png_pixels).unwrap();
            let preview = PopupSurface::new((780, 300))?;
            preview.paint(&gpu, |p| {
                let mut ui = crate::view::UiState::new();
                ui.media.playing = true;
                ui.media.title = "I'm Tired".into();
                ui.cfg.music_eq = false;
                ui.cfg.music_cover = false;
                ui.weather = Some(crate::services::weather::Weather {
                    place: "Preview".into(),
                    temp: 6.0,
                    wind: 2.0,
                    humidity: 70.0,
                    sky: crate::services::weather::Sky::Rain,
                    night: true,
                    hourly: Vec::new(),
                    daily: Vec::new(),
                    city_missing: false,
                });
                for (i, side) in crate::data::settings::WeatherSide::ALL.into_iter().enumerate() {
                    ui.cfg.pill_weather_side = side;
                    let x = 30.0 + i as f32 * 390.0;
                    let geometry = gpu.elastic_body(&ElasticBody {
                        rect: (x, 20.0, 330.0, 34.0),
                        radius: 17.0,
                        pull: Default::default(),
                    })?;
                    unsafe {
                        let brush = p.dc.CreateSolidColorBrush(&rgba(0.1, 0.1, 0.11, 1.0), None)?;
                        p.dc.FillGeometry(&geometry, &brush, None);
                    }
                    crate::view::paint_collapsed(p, (x, 20.0, 330.0, 34.0), 1.0, &ui)?;
                }
                for (i, (x, y, cursor)) in [
                    (50.0, 112.0, (18.0, 129.0)),
                    (440.0, 112.0, (735.0, 129.0)),
                    (50.0, 222.0, (180.0, 282.0)),
                    (440.0, 222.0, (730.0, 196.0)),
                ]
                .into_iter()
                .enumerate()
                {
                    let rect = (x, y, 260.0, 34.0);
                    let pull = cursor_pull(rect, 17.0, cursor, 160.0, 30.0, None);
                    let geometry = gpu.elastic_body(&ElasticBody { rect, radius: 17.0, pull })?;
                    unsafe {
                        let brush = p.dc.CreateSolidColorBrush(&rgba(0.12, 0.12, 0.13, 1.0), None)?;
                        p.dc.FillGeometry(&geometry, &brush, None);
                    }
                    p.text(
                        ["Left edge", "Right edge", "Bottom edge", "Corner"][i],
                        (x, y + 4.0, x + 260.0, y + 30.0),
                        12.0,
                        windows::Win32::Graphics::DirectWrite::DWRITE_FONT_WEIGHT_NORMAL,
                        windows::Win32::Graphics::DirectWrite::DWRITE_TEXT_ALIGNMENT_CENTER,
                        rgba(1.0, 1.0, 1.0, 0.65),
                    )?;
                }
                Ok(())
            })?;
            let data: Vec<u8> = preview.pixels().chunks_exact(4).flat_map(|p| [p[2], p[1], p[0], p[3]]).collect();
            let mut encoder = png::Encoder::new(
                std::fs::File::create(std::path::PathBuf::from(&dir).join("perimeter-and-weather.png")).unwrap(),
                780,
                300,
            );
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.write_header().unwrap().write_image_data(&data).unwrap();
            let transitions = PopupSurface::new((720, 260))?;
            transitions.paint(&gpu, |p| {
                let mut ui = crate::view::UiState::new();
                ui.char_inset = 30.0;
                ui.cfg.pill_weather = false;
                ui.cfg.pill_battery = crate::data::settings::BatteryStyle::Hidden;
                for (i, frames) in [0, 3, 12, 48].into_iter().enumerate() {
                    let rect = (40.0 + (i % 2) as f32 * 360.0, 60.0 + (i / 2) as f32 * 130.0, 240.0, 34.0);
                    let mut pull = cursor_pull(rect, 17.0, (rect.0 + 35.0, rect.1 + 64.0), 160.0, 40.0, None);
                    let target = cursor_pull(rect, 17.0, (rect.0 + 35.0, rect.1 - 20.0), 160.0, 40.0, None);
                    for _ in 0..frames {
                        pull = pull.follow(target, rect, 17.0, 0.2);
                    }
                    let geometry = gpu.elastic_body(&ElasticBody { rect, radius: 17.0, pull })?;
                    unsafe {
                        let brush = p.dc.CreateSolidColorBrush(&rgba(0.06, 0.07, 0.09, 1.0), None)?;
                        p.dc.FillGeometry(&geometry, &brush, None);
                    }
                    crate::view::paint_collapsed(p, rect, 1.0, &ui)?;
                    let offset = p.push_offset(rect.0 + 6.0, rect.1 + 2.0);
                    crate::ui::characters::draw(
                        p,
                        crate::ui::characters::Character::Cat,
                        40.0,
                        &crate::ui::characters::Pose {
                            t: 0.0,
                            reaction: crate::ui::characters::Reaction::Idle,
                            look: (0.0, -1.0),
                            blink: false,
                            accent: (0.5, 0.6, 0.9),
                            beat: 0.0,
                        },
                    )?;
                    p.set_transform(&offset);
                }
                Ok(())
            })?;
            let data: Vec<u8> = transitions.pixels().chunks_exact(4).flat_map(|p| [p[2], p[1], p[0], p[3]]).collect();
            let mut encoder = png::Encoder::new(
                std::fs::File::create(std::path::PathBuf::from(&dir).join("hover-transition.png")).unwrap(),
                720,
                260,
            );
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.write_header().unwrap().write_image_data(&data).unwrap();
        }
        Ok(())
    }
}
