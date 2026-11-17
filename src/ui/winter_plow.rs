use super::winter_model::Snow;
use windows::UI::Color;
use windows::UI::Composition::*;
use windows::core::{Interface, Result};
use windows_numerics::{Vector2, Vector3};

#[derive(Clone, Copy)]
pub(super) struct Pose {
    pub x: f32,
    pub y: f32,
    pub scale: f32,
    pub direction: f32,
    pub surface: isize,
    pub travel: f32,
}

struct Route {
    surface: isize,
    progress: f32,
    direction: f32,
}

pub(super) struct Plow {
    route: Option<Route>,
    wait: f32,
    last: Option<isize>,
    turn: usize,
}

impl Default for Plow {
    fn default() -> Self {
        Self { route: None, wait: 5.0, last: None, turn: 0 }
    }
}

impl Plow {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn step(&mut self, snow: &mut Snow, eligible: &[isize], dt: f32, interval: u32) -> Option<Pose> {
        let dt = dt.clamp(0.0, 0.05);
        self.wait = (self.wait.min(interval.clamp(15, 600) as f32) - dt).max(0.0);
        if let Some(route) = &self.route
            && !eligible.contains(&route.surface)
        {
            self.route = None;
            self.wait = 1.0;
        }
        if self.route.is_none() && self.wait <= 0.0 {
            let candidates: Vec<isize> = eligible
                .iter()
                .copied()
                .filter(|id| {
                    (Some(*id) != self.last || eligible.len() == 1)
                        && snow.drifts.iter().any(|d| d.ledge.id == *id && d.depths.iter().any(|v| *v > 0.4))
                })
                .collect();
            if candidates.is_empty() {
                self.wait = 1.0;
                return None;
            }
            let surface = candidates[self.turn % candidates.len()];
            self.route =
                Some(Route { surface, progress: 0.0, direction: if self.turn.is_multiple_of(2) { 1.0 } else { -1.0 } });
            self.turn = self.turn.wrapping_add(1);
            self.last = Some(surface);
            self.wait = interval.clamp(15, 600) as f32;
        }
        let route = self.route.as_mut()?;
        let drift = snow.drifts.iter_mut().find(|d| d.ledge.id == route.surface)?;
        let tiny = route.surface == -2;
        let scale = if tiny { 0.5 } else { 1.0 };
        let width = drift.ledge.width;
        route.progress += dt * if tiny { 45.0 } else { 90.0 } / (width + 36.0 * scale);
        if route.progress >= 1.0 {
            self.route = None;
            return None;
        }
        let along = route.progress * (width + 36.0 * scale) - 18.0 * scale;
        let x = drift.ledge.x + if route.direction > 0.0 { along } else { width - along };
        let blade = x + route.direction * 13.0 * scale;
        let bins = drift.depths.len() as f32;
        for (i, depth) in drift.depths.iter_mut().enumerate() {
            let center = drift.ledge.x + (i as f32 + 0.5) * width / bins;
            if (center - blade).abs() < width / bins + 4.0 * scale {
                *depth = depth.min(if tiny { 0.35 } else { 0.05 });
            }
        }
        Some(Pose {
            x,
            y: drift.ledge.surface_y(x.clamp(drift.ledge.x, drift.ledge.x + width)),
            scale,
            direction: route.direction,
            surface: route.surface,
            travel: along,
        })
    }
}

pub(super) struct Vehicle {
    pub root: ShapeVisual,
    spokes: Vec<CompositionSpriteShape>,
}

fn panel(
    c: &Compositor,
    layer: &ShapeVisual,
    rect: (f32, f32, f32, f32),
    radius: f32,
    color: Color,
) -> Result<CompositionSpriteShape> {
    let g = c.CreateRoundedRectangleGeometry()?;
    g.SetOffset(Vector2 { X: rect.0, Y: rect.1 })?;
    g.SetSize(Vector2 { X: rect.2 - rect.0, Y: rect.3 - rect.1 })?;
    g.SetCornerRadius(Vector2 { X: radius, Y: radius })?;
    let shape = c.CreateSpriteShapeWithGeometry(&g)?;
    shape.SetFillBrush(&c.CreateColorBrushWithColor(color)?)?;
    layer.Shapes()?.Append(&shape)?;
    Ok(shape)
}

impl Vehicle {
    pub fn new(c: &Compositor) -> Result<Self> {
        let root = c.CreateShapeVisual()?;
        root.SetIsVisible(false)?;
        root.SetSize(Vector2 { X: 40.0, Y: 24.0 })?;
        let orange = Color { A: 255, R: 241, G: 171, B: 65 };
        panel(c, &root, (-12.0, -11.0, 10.0, -3.0), 2.0, Color { A: 255, R: 107, G: 151, B: 177 })?;
        panel(c, &root, (0.0, -17.0, 10.0, -6.0), 2.0, Color { A: 255, R: 167, G: 203, B: 220 })?;
        panel(c, &root, (2.0, -15.0, 8.0, -10.0), 1.0, Color { A: 255, R: 32, G: 58, B: 76 })?;
        panel(c, &root, (3.0, -19.0, 7.0, -17.0), 1.0, orange)?;
        panel(c, &root, (10.0, -5.0, 17.0, -2.0), 1.0, orange)?;
        panel(c, &root, (15.0, -8.0, 18.0, 0.0), 1.0, Color { A: 255, R: 231, G: 241, B: 247 })?;
        let mut spokes = Vec::new();
        for x in [-8.0, 7.0] {
            let g = c.CreateEllipseGeometry()?;
            g.SetRadius(Vector2 { X: 3.2, Y: 3.2 })?;
            let wheel = c.CreateSpriteShapeWithGeometry(&g)?;
            wheel.SetFillBrush(&c.CreateColorBrushWithColor(Color { A: 255, R: 34, G: 44, B: 57 })?)?;
            wheel.SetOffset(Vector2 { X: x, Y: -2.0 })?;
            root.Shapes()?.Append(&wheel)?;
            let spoke = panel(c, &root, (-0.6, -2.0, 0.6, 2.0), 0.3, Color { A: 255, R: 191, G: 212, B: 226 })?;
            spoke.SetOffset(Vector2 { X: x, Y: -2.0 })?;
            spokes.push(spoke);
        }
        for shape in root.Shapes()? {
            let shape = shape.cast::<CompositionSpriteShape>()?;
            let offset = shape.Offset()?;
            shape.SetOffset(Vector2 { X: offset.X + 18.0, Y: offset.Y + 19.0 })?;
        }
        Ok(Self { root, spokes })
    }

    pub fn place(&self, pose: Option<Pose>) -> Result<()> {
        self.root.SetIsVisible(pose.is_some())?;
        if let Some(pose) = pose {
            self.root.SetScale(Vector3 { X: pose.scale * pose.direction, Y: pose.scale, Z: 1.0 })?;
            self.root.SetOffset(Vector3 {
                X: pose.x - 18.0 * pose.scale * pose.direction,
                Y: (pose.y - 19.0 * pose.scale).max(1.0),
                Z: 0.0,
            })?;
            for spoke in &self.spokes {
                spoke.SetRotationAngle(pose.travel / 3.2)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::winter_model::Ledge;
    use super::*;
    #[test]
    fn scheduled_routes_clean_snow_and_visit_other_windows() {
        let mut snow = Snow::new(800.0, 600.0, 0, 42);
        snow.starter_cover = true;
        snow.ledges(vec![
            Ledge { id: 42, x: 100.0, y: 300.0, width: 180.0, ..Default::default() },
            Ledge { id: -2, x: 300.0, y: 30.0, width: 200.0, ..Default::default() },
        ]);
        let mut plow = Plow::default();
        let before = snow.drifts[0].depths.iter().sum::<f32>();
        let mut visited = Vec::new();
        for _ in 0..2400 {
            if let Some(pose) = plow.step(&mut snow, &[42, -2], 1.0 / 60.0, 15) {
                if visited.last() != Some(&pose.surface) {
                    visited.push(pose.surface);
                }
                if pose.surface == -2 {
                    assert_eq!(pose.scale, 0.5);
                }
            }
        }
        assert!(snow.drifts[0].depths.iter().sum::<f32>() < before * 0.2);
        assert!(visited.contains(&42) && visited.contains(&-2));
        assert!(plow.step(&mut snow, &[], 1.0 / 60.0, 15).is_none());
    }
}
