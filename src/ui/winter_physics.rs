use super::winter_model::{Ball, Drift};

pub(super) fn advance(balls: &mut [Ball], drifts: &[Drift], held: Option<usize>, size: (f32, f32), dt: f32) {
    let steps = (dt / (1.0 / 120.0)).ceil().clamp(1.0, 8.0) as usize;
    let step = dt / steps as f32;
    for _ in 0..steps {
        for index in 0..balls.len() {
            if held == Some(index) {
                continue;
            }
            let parent = balls[index].parent.and_then(|p| balls.get(p).copied().map(|b| (p, b)));
            let ball = &mut balls[index];
            ball.squash *= (-step * 12.0).exp();
            if let Some((parent_index, base)) = parent {
                let offset = ball.x - base.x;
                if held == Some(parent_index) || offset.abs() > base.radius * 0.6 || (ball.vx - base.vx).abs() > 110.0 {
                    ball.parent = None;
                    ball.support = None;
                } else {
                    ball.vx += (base.x - ball.x) * step * 24.0;
                    ball.vx += (base.vx - ball.vx) * (1.0 - (-step * 8.0).exp());
                    ball.x += ball.vx * step;
                    let distance = (base.radius + ball.radius - 2.0).max(1.0);
                    ball.y = base.y - (distance * distance - (ball.x - base.x).powi(2)).max(0.0).sqrt();
                    ball.velocity = base.velocity;
                    ball.support = base.support;
                    ball.angle += ball.vx * step / ball.radius.max(1.0);
                    continue;
                }
            }
            let before = ball.y + ball.radius;
            ball.velocity += 430.0 * step;
            ball.x += ball.vx * step;
            ball.y += ball.velocity * step;
            ball.support = None;
            if let Some(drift) = drifts
                .iter()
                .filter(|d| {
                    ball.x >= d.ledge.x
                        && ball.x <= d.ledge.x + d.ledge.width
                        && before <= d.ledge.surface_y(ball.x) + 3.0
                        && ball.y + ball.radius >= d.ledge.surface_y(ball.x)
                })
                .min_by(|a, b| a.ledge.surface_y(ball.x).total_cmp(&b.ledge.surface_y(ball.x)))
            {
                let impact = ball.velocity;
                ball.y = drift.ledge.surface_y(ball.x) - ball.radius;
                ball.velocity = if impact > 100.0 { -impact * 0.16 } else { 0.0 };
                ball.squash = ball.squash.max((impact / 900.0).min(0.18));
                ball.support = Some(drift.ledge.id);
                let gradient = drift.ledge.surface_y(ball.x + 0.5) - drift.ledge.surface_y(ball.x - 0.5);
                ball.vx += gradient.clamp(-2.0, 2.0) * step * 230.0;
                ball.vx *= (-step * 3.0).exp();
            }
            if ball.y + ball.radius > size.1 {
                let impact = ball.velocity;
                ball.y = size.1 - ball.radius;
                ball.velocity = if impact > 100.0 { -impact * 0.12 } else { 0.0 };
                ball.vx *= (-step * 4.0).exp();
                ball.squash = ball.squash.max((impact / 900.0).min(0.18));
            }
            if ball.x < ball.radius {
                ball.x = ball.radius;
                ball.vx = ball.vx.abs() * 0.25;
            }
            if ball.x > size.0 - ball.radius {
                ball.x = (size.0 - ball.radius).max(ball.radius);
                ball.vx = -ball.vx.abs() * 0.25;
            }
            ball.angle += ball.vx * step / ball.radius.max(1.0);
        }
        for _ in 0..5 {
            for a in 0..balls.len() {
                for b in a + 1..balls.len() {
                    let (left, right) = balls.split_at_mut(b);
                    let first = &mut left[a];
                    let second = &mut right[0];
                    let dx = second.x - first.x;
                    let dy = second.y - first.y;
                    let distance = dx.hypot(dy).max(0.001);
                    let joined = first.parent == Some(b) || second.parent == Some(a);
                    let contact = (first.radius + second.radius - if joined { 2.0 } else { 0.3 }).max(1.0);
                    if distance >= contact {
                        continue;
                    }
                    let (nx, ny) = if distance < 0.01 { (0.0, 1.0) } else { (dx / distance, dy / distance) };
                    let weight_a = if held == Some(a) { 0.0 } else { 1.0 / first.radius.powi(2).max(1.0) };
                    let weight_b = if held == Some(b) { 0.0 } else { 1.0 / second.radius.powi(2).max(1.0) };
                    let total = (weight_a + weight_b).max(0.00001);
                    let correction = (contact - distance) * 0.8;
                    first.x -= nx * correction * weight_a / total;
                    first.y -= ny * correction * weight_a / total;
                    second.x += nx * correction * weight_b / total;
                    second.y += ny * correction * weight_b / total;
                    let approach = (second.vx - first.vx) * nx + (second.velocity - first.velocity) * ny;
                    if approach < 0.0 {
                        let impulse = -approach * 1.12 / total;
                        first.vx -= nx * impulse * weight_a;
                        first.velocity -= ny * impulse * weight_a;
                        second.vx += nx * impulse * weight_b;
                        second.velocity += ny * impulse * weight_b;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::winter_model::{Ledge, Snow};
    use super::*;
    fn world() -> Snow {
        let mut snow = Snow::new(800.0, 600.0, 0, 19);
        snow.starter_cover = true;
        snow.ledges(vec![
            Ledge { id: 42, x: 200.0, y: 300.0, width: 240.0, radius: 12.0 },
            Ledge { id: -1, x: 0.0, y: 600.0, width: 800.0, radius: 0.0 },
        ]);
        snow
    }
    #[test]
    fn released_snowball_keeps_the_mouse_impulse_and_falls() {
        let mut snow = world();
        snow.balls.push(Ball { x: 300.0, y: 100.0, radius: 18.0, ..Default::default() });
        snow.grab(300.0, 100.0);
        snow.drag(300.0, 100.0);
        snow.step(1.0 / 60.0, true);
        snow.drag(320.0, 90.0);
        snow.release();
        snow.step(1.0 / 60.0, true);
        assert!(snow.balls[0].x > 320.0);
        assert!(snow.balls[0].y < 90.0);
        assert!(snow.balls[0].angle > 0.0);
        for _ in 0..600 {
            snow.step(1.0 / 120.0, true);
        }
        assert!(snow.balls[0].y + snow.balls[0].radius <= 600.1);
        assert!(snow.balls[0].vx.is_finite());
    }
    #[test]
    fn a_ball_rolls_down_a_round_corner_and_window_motion_sheds_snow() {
        let mut snow = world();
        let y = snow.drifts[0].ledge.surface_y(201.0) - 15.0;
        snow.balls.push(Ball { x: 201.0, y, radius: 15.0, support: Some(42), ..Default::default() });
        for _ in 0..120 {
            snow.step(1.0 / 120.0, true);
        }
        assert!(snow.balls[0].x < 200.0);
        assert!(snow.balls[0].y > y);
        let mut snow = world();
        snow.balls.push(Ball { x: 300.0, y: 280.0, radius: 20.0, support: Some(42), ..Default::default() });
        snow.step(1.0 / 60.0, true);
        snow.ledges(vec![Ledge { id: 42, x: 250.0, y: 300.0, width: 240.0, radius: 12.0 }]);
        snow.step(1.0 / 60.0, true);
        assert!(snow.balls[0].vx < 0.0);
        assert!(snow.balls[0].x < 350.0);
        assert!(!snow.crumbs.is_empty());
    }
    #[test]
    fn touching_balls_exchange_momentum_without_interpenetrating() {
        let mut snow = world();
        snow.balls = vec![
            Ball { x: 280.0, y: 100.0, radius: 25.0, vx: 140.0, ..Default::default() },
            Ball { x: 325.0, y: 100.0, radius: 25.0, vx: -60.0, ..Default::default() },
        ];
        snow.step(1.0 / 60.0, true);
        assert!((snow.balls[0].x - snow.balls[1].x).abs() > 49.0);
        assert!(snow.balls[0].vx < 140.0 && snow.balls[1].vx > -60.0);
    }
}
