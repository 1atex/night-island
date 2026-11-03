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

fn liquid_body_geometry(factory: &ID2D1Factory, body: &LiquidBody) -> Result<ID2D1PathGeometry> {
    let (x, y, rw, rh) = body.rect;
    let sideways = matches!(body.edge, Hang::Left | Hang::Right);
    let (w, h) = if sideways { (rh, rw) } else { (rw, rh) };
    let r = body.radius.min(w.min(h) / 2.0).max(0.0);
    let half = body.half.min((w / 2.0 - r).max(0.0));
    let center = (w / 2.0 + body.center).clamp(r + half, w - r - half);
    let (a, b) = (center - half, center + half);
    let depth = body.depth.max(0.0);
    let point = |u: f32, v: f32| {
        let (px, py) = match body.edge {
            Hang::Down => (x + u, y + v),
            Hang::Up => (x + rw - u, y + rh - v),
            Hang::Right => (x + v, y + rh - u),
            Hang::Left => (x + rw - v, y + u),
        };
        windows_numerics::Vector2 { X: px, Y: py }
    };
    let k = 0.552_284_8;
    unsafe {
        let geometry = factory.CreatePathGeometry()?;
        let sink = geometry.Open()?;
        let curve = |p1: (f32, f32), p2: (f32, f32), p3: (f32, f32)| {
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: point(p1.0, p1.1),
                point2: point(p2.0, p2.1),
                point3: point(p3.0, p3.1),
            });
        };
        sink.BeginFigure(point(r, 0.0), D2D1_FIGURE_BEGIN_FILLED);
        sink.AddLine(point(w - r, 0.0));
        curve((w - r + r * k, 0.0), (w, r - r * k), (w, r));
        sink.AddLine(point(w, h - r));
        curve((w, h - r + r * k), (w - r + r * k, h), (w - r, h));
        sink.AddLine(point(b, h));
        curve((b - half * 0.55, h), (center + half * 0.28, h + depth), (center, h + depth));
        curve((center - half * 0.28, h + depth), (a + half * 0.55, h), (a, h));
        sink.AddLine(point(r, h));
        curve((r - r * k, h), (0.0, h - r + r * k), (0.0, h - r));
        sink.AddLine(point(0.0, r));
        curve((0.0, r - r * k), (r - r * k, 0.0), (r, 0.0));
        sink.EndFigure(D2D1_FIGURE_END_CLOSED);
        sink.Close()?;
        Ok(geometry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            let rgba: Vec<u8> = data.chunks_exact(4).flat_map(|p| [p[2], p[1], p[0], p[3]]).collect();
            let mut encoder = png::Encoder::new(
                std::fs::File::create(std::path::PathBuf::from(dir).join("unified-liquid.png")).unwrap(),
                340,
                280,
            );
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.write_header().unwrap().write_image_data(&rgba).unwrap();
        }
        Ok(())
    }
}
