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

#[derive(Clone, Copy, Debug)]
pub(super) struct Span {
    pub surface: isize,
    pub left: f32,
    pub right: f32,
}

struct Route {
    span: Span,
    progress: f32,
    direction: f32,
    ball: Option<usize>,
    collected: f32,
}

pub(super) struct Plow {
    route: Option<Route>,
    pending: std::collections::VecDeque<Span>,
    wait: f32,
    turn: usize,
}

impl Default for Plow {
    fn default() -> Self {
        Self { route: None, pending: Default::default(), wait: 5.0, turn: 0 }
    }
}

fn snowy(snow: &Snow, span: Span) -> bool {
    snow.drifts.iter().any(|d| {
        d.ledge.id == span.surface
            && d.depths.iter().enumerate().any(|(i, depth)| {
                let t = (i as f32 + 0.5) / d.depths.len() as f32;
                t >= span.left && t <= span.right && *depth > 0.4
            })
    })
}

fn overlap(a: Span, b: Span) -> Option<Span> {
    let left = a.left.max(b.left);
    let right = a.right.min(b.right);
    (a.surface == b.surface && right > left).then_some(Span { surface: a.surface, left, right })
}

pub(super) fn size(surface: isize, width: f32, percent: u32) -> f32 {
    let base = if surface == -2 { 0.5 } else { (width / 320.0).sqrt().clamp(1.15, 2.6) };
    base * percent.clamp(50, 200) as f32 / 100.0
}

impl Plow {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn step(
        &mut self,
        snow: &mut Snow,
        eligible: &[Span],
        dt: f32,
        settings: crate::data::winter::WinterSettings,
    ) -> Option<Pose> {
        let dt = dt.clamp(0.0, 0.05);
        self.wait = (self.wait.min(settings.plow_interval_secs.clamp(15, 600) as f32) - dt).max(0.0);
        if let Some(route) = &self.route
            && !eligible.iter().any(|s| overlap(*s, route.span).is_some())
        {
            self.route = None;
            self.wait = 0.5;
        }
        if self.route.is_none() && self.wait <= 0.0 {
            if self.pending.is_empty() {
                let mut spans: Vec<Span> = eligible.iter().copied().filter(|s| snowy(snow, *s)).collect();
                spans.sort_by(|a, b| {
                    (a.surface == -1)
                        .cmp(&(b.surface == -1))
                        .then(a.surface.cmp(&b.surface))
                        .then(a.left.total_cmp(&b.left))
                });
                self.pending.extend(spans);
            }
            while let Some(queued) = self.pending.pop_front() {
                let span = eligible
                    .iter()
                    .filter_map(|s| overlap(*s, queued))
                    .max_by(|a, b| (a.right - a.left).total_cmp(&(b.right - b.left)));
                let Some(span) = span.filter(|s| snowy(snow, *s)) else { continue };
                self.route = Some(Route {
                    span,
                    progress: 0.0,
                    direction: if self.turn.is_multiple_of(2) { 1.0 } else { -1.0 },
                    ball: None,
                    collected: 0.0,
                });
                self.turn = self.turn.wrapping_add(1);
                break;
            }
            if self.route.is_none() {
                self.wait = settings.plow_interval_secs.clamp(15, 600) as f32;
                return None;
            }
        }
        let route = self.route.as_mut()?;
        let drift = snow.drifts.iter_mut().find(|d| d.ledge.id == route.span.surface)?;
        let tiny = route.span.surface == -2;
        let scale = size(route.span.surface, drift.ledge.width, settings.plow_size);
        let left = drift.ledge.x + route.span.left * drift.ledge.width;
        let right = drift.ledge.x + route.span.right * drift.ledge.width;
        let width = right - left;
        let speed = if tiny { 45.0 } else { 90.0 * scale.sqrt() };
        route.progress += dt * speed / (width + 36.0 * scale);
        if route.progress >= 1.0 {
            self.route = None;
            self.wait = if self.pending.is_empty() { settings.plow_interval_secs.clamp(15, 600) as f32 } else { 1.2 };
            return None;
        }
        let along = route.progress * (width + 36.0 * scale) - 18.0 * scale;
        let x = if route.direction > 0.0 { left + along } else { right - along };
        let blade = x + route.direction * 18.0 * scale;
        let bins = drift.depths.len() as f32;
        for (i, depth) in drift.depths.iter_mut().enumerate() {
            let center = drift.ledge.x + (i as f32 + 0.5) * drift.ledge.width / bins;
            let t = (i as f32 + 0.5) / bins;
            if center >= left
                && center <= right
                && eligible.iter().any(|s| s.surface == route.span.surface && t >= s.left && t <= s.right)
                && (center - blade).abs() < drift.ledge.width / bins * 0.5 + 4.0 * scale
            {
                let remaining = depth.min(if tiny { 0.35 } else { 0.05 });
                route.collected += (*depth - remaining) * drift.ledge.width / bins;
                *depth = remaining;
                if tiny {
                    snow.island_plowed.resize(bins as usize, 0.0);
                    snow.island_plowed[i] = 12.0;
                }
            }
        }
        let ledge = drift.ledge;
        if settings.plow_snowballs && !tiny {
            if route.ball.is_some_and(|i| snow.held == Some(i) || snow.balls[i].parent.is_some()) {
                route.ball = None;
                route.collected = 0.0;
            }
            if route.ball.is_none() && route.collected > 30.0 && snow.balls.len() < 8 {
                route.ball = Some(snow.balls.len());
                snow.balls.push(super::winter_model::Ball::default());
            }
            if let Some(index) = route.ball {
                let ball = &mut snow.balls[index];
                ball.radius = (route.collected / std::f32::consts::PI * 0.35).sqrt().clamp(3.0, 24.0);
                let bx = blade + route.direction * (ball.radius + 1.0);
                ball.x = bx.clamp(left + ball.radius, (right - ball.radius).max(left + ball.radius));
                ball.y = ledge.surface_y(ball.x) - ball.radius;
                ball.angle += route.direction * speed * dt / ball.radius;
                ball.vx = 0.0;
                ball.velocity = 0.0;
                ball.support = Some(ledge.id);
            }
        } else {
            route.ball = None;
        }
        Some(Pose {
            x,
            y: ledge.surface_y(x.clamp(ledge.x, ledge.x + ledge.width)),
            scale,
            direction: route.direction,
            surface: route.span.surface,
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
                spoke.SetRotationAngle(pose.travel / (3.2 * pose.scale.max(0.1)))?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::winter_model::Ledge;
    use super::*;
    use crate::data::winter::WinterSettings;

    fn world() -> Snow {
        let mut snow = Snow::new(1800.0, 1000.0, 0, 42);
        snow.starter_cover = true;
        snow.ledges(vec![
            Ledge { id: 42, x: 100.0, y: 300.0, width: 1300.0, ..Default::default() },
            Ledge { id: 77, x: 100.0, y: 500.0, width: 1300.0, ..Default::default() },
            Ledge { id: -2, x: 300.0, y: 30.0, width: 200.0, ..Default::default() },
            Ledge { id: -1, x: 0.0, y: 1000.0, width: 1800.0, ..Default::default() },
        ]);
        snow
    }

    #[test]
    fn a_round_visits_both_windows_the_island_and_ground_before_waiting() {
        let mut snow = world();
        let spans = [42, 77, -2, -1].map(|surface| Span { surface, left: 0.0, right: 1.0 });
        let mut plow = Plow::default();
        let mut visited = Vec::new();
        for _ in 0..80 * 60 {
            if let Some(pose) = plow.step(&mut snow, &spans, 1.0 / 60.0, WinterSettings::default()) {
                if visited.last() != Some(&pose.surface) {
                    visited.push(pose.surface);
                }
                if pose.surface == -2 {
                    assert_eq!(pose.scale, 0.5);
                }
                if pose.surface == 42 {
                    assert!(pose.scale >= 2.0);
                }
            }
        }
        assert_eq!(visited, [-2, 42, 77, -1]);
        assert!(snow.drifts.iter().all(|d| d.depths.iter().sum::<f32>() < d.depths.len() as f32 * 0.4));
        assert!(snow.balls.len() >= 3 && snow.balls.len() < 9);
        assert!(snow.balls.iter().all(|b| b.radius > 3.0 && b.radius <= 24.0));
    }

    #[test]
    fn a_partly_covered_window_is_cleaned_only_on_its_exposed_spans() {
        let mut snow = world();
        let spans = [Span { surface: 77, left: 0.0, right: 0.1 }, Span { surface: 77, left: 0.9, right: 1.0 }];
        let mut plow = Plow::default();
        let before = snow.drifts[1].depths.clone();
        for _ in 0..20 * 60 {
            plow.step(&mut snow, &spans, 1.0 / 60.0, WinterSettings::default());
        }
        let after = &snow.drifts[1].depths;
        assert!(after[0] < before[0] && after[after.len() - 1] < before[before.len() - 1]);
        assert_eq!(after[after.len() / 2], before[before.len() / 2]);
    }

    #[test]
    fn collection_grows_a_ball_and_respects_the_particle_budget_and_held_ball() {
        let mut snow = world();
        let spans = [Span { surface: 42, left: 0.0, right: 1.0 }];
        let mut plow = Plow::default();
        for _ in 0..8 * 60 {
            plow.step(&mut snow, &spans, 1.0 / 60.0, WinterSettings::default());
        }
        let radius = snow.balls[0].radius;
        snow.held = Some(0);
        let held = snow.balls[0];
        for _ in 0..3 * 60 {
            plow.step(&mut snow, &spans, 1.0 / 60.0, WinterSettings::default());
        }
        assert_eq!(snow.balls[0].radius, radius);
        assert_eq!(snow.balls[0].x, held.x);
        assert!(snow.balls[1].radius > 3.0);
        while snow.balls.len() < 8 {
            snow.balls.push(Default::default());
        }
        for _ in 0..60 {
            plow.step(&mut snow, &spans, 1.0 / 60.0, WinterSettings::default());
        }
        assert_eq!(snow.balls.len(), 8);
        assert!(plow.step(&mut snow, &[], 1.0 / 60.0, WinterSettings::default()).is_none());
    }

    #[test]
    fn island_sweep_stays_clear_instead_of_regrowing_on_the_next_frame() {
        let mut snow = world();
        snow.island_cap = true;
        snow.ledges(vec![Ledge { id: -2, x: 300.0, y: 30.0, width: 200.0, ..Default::default() }]);
        let spans = [Span { surface: -2, left: 0.0, right: 1.0 }];
        let mut plow = Plow::default();
        for _ in 0..11 * 60 {
            snow.step(1.0 / 60.0, true);
            plow.step(&mut snow, &spans, 1.0 / 60.0, WinterSettings::default());
        }
        assert!(snow.island_plowed.iter().all(|c| *c > 0.0));
        assert!(snow.drifts[0].depths.iter().all(|d| *d < 0.4));
        for _ in 0..2 * 60 {
            snow.step(1.0 / 60.0, true);
        }
        assert!(snow.drifts[0].depths.iter().all(|d| *d < 0.4));
        for _ in 0..15 * 60 {
            snow.step(1.0 / 60.0, true);
        }
        assert!(snow.drifts[0].depths.iter().all(|d| *d > 2.0));
    }

    #[test]
    fn size_is_proportional_bounded_and_can_restore_the_old_scale() {
        assert!(size(42, 1300.0, 100) > size(42, 320.0, 100));
        assert!((size(42, 1300.0, 50) - 1.0).abs() < 0.02);
        assert!(size(42, 100000.0, 100) <= 2.6);
        assert_eq!(size(-2, 1300.0, 100), 0.5);
    }
}
