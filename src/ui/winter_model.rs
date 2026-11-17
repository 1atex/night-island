#[derive(Clone, Copy, Debug)]
pub struct Ledge {
    pub id: isize,
    pub x: f32,
    pub y: f32,
    pub width: f32,
}

pub struct Drift {
    pub ledge: Ledge,
    pub depths: Vec<f32>,
}

#[derive(Clone, Copy)]
pub struct Flake {
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub speed: f32,
    pub phase: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Ball {
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub velocity: f32,
    pub support: Option<isize>,
}

pub struct Snow {
    pub flakes: Vec<Flake>,
    pub drifts: Vec<Drift>,
    pub balls: Vec<Ball>,
    pub held: Option<usize>,
    pub starter_cover: bool,
    pub island_cap: bool,
    width: f32,
    height: f32,
    random: u64,
    time: f32,
}

impl Snow {
    pub fn new(width: f32, height: f32, count: usize, seed: u64) -> Self {
        let mut snow = Self {
            flakes: Vec::new(),
            drifts: Vec::new(),
            balls: Vec::new(),
            held: None,
            starter_cover: false,
            island_cap: false,
            width,
            height,
            random: seed.max(1),
            time: 0.0,
        };
        for _ in 0..count {
            let flake = snow.flake(true);
            snow.flakes.push(flake);
        }
        snow
    }

    fn random(&mut self) -> f32 {
        self.random ^= self.random << 13;
        self.random ^= self.random >> 7;
        self.random ^= self.random << 17;
        (self.random >> 40) as f32 / (1_u32 << 24) as f32
    }

    fn flake(&mut self, initial: bool) -> Flake {
        Flake {
            x: self.random() * self.width,
            y: if initial { self.random() * self.height } else { -8.0 },
            radius: 1.0 + self.random() * 2.0,
            speed: 22.0 + self.random() * 40.0,
            phase: self.random() * std::f32::consts::TAU,
        }
    }

    pub fn ledges(&mut self, ledges: Vec<Ledge>) {
        self.drifts.retain(|d| ledges.iter().any(|l| l.id == d.ledge.id));
        for ledge in ledges {
            if let Some(drift) = self.drifts.iter_mut().find(|d| d.ledge.id == ledge.id) {
                let dx = ledge.x - drift.ledge.x;
                let dy = ledge.y - drift.ledge.y;
                for (index, ball) in self.balls.iter_mut().enumerate() {
                    if ball.support == Some(ledge.id) && self.held != Some(index) {
                        ball.x += dx;
                        ball.y += dy;
                    }
                }
                drift.ledge = ledge;
            } else if ledge.width > 1.0 {
                self.drifts.push(Drift {
                    ledge,
                    depths: vec![
                        if self.starter_cover || (ledge.id == -2 && self.island_cap) { 2.5 } else { 0.0 };
                        (ledge.width / 12.0).ceil().clamp(2.0, 96.0) as usize
                    ],
                });
            }
        }
        for ball in &mut self.balls {
            if !self
                .drifts
                .iter()
                .any(|d| Some(d.ledge.id) == ball.support && ball.x >= d.ledge.x && ball.x <= d.ledge.x + d.ledge.width)
            {
                ball.support = None;
            }
        }
    }

    pub fn step(&mut self, dt: f32, accumulate: bool) {
        let dt = dt.clamp(0.0, 0.05);
        self.time += dt;
        if self.island_cap
            && let Some(drift) = self.drifts.iter_mut().find(|d| d.ledge.id == -2)
        {
            for (i, depth) in drift.depths.iter_mut().enumerate() {
                let baseline = 3.0 + 0.8 * (i as f32 * 0.7).sin();
                if *depth < baseline {
                    *depth += (baseline - *depth) * (1.0 - (-dt * 2.0).exp());
                }
            }
        }
        for index in 0..self.flakes.len() {
            let f = &mut self.flakes[index];
            let before = f.y;
            f.y += f.speed * dt;
            f.x = (f.x + (self.time * 0.6 + f.phase).sin() * dt * 13.0).rem_euclid(self.width);
            let hit = self
                .drifts
                .iter_mut()
                .filter(|d| {
                    f.x >= d.ledge.x && f.x < d.ledge.x + d.ledge.width && before <= d.ledge.y && f.y >= d.ledge.y
                })
                .min_by(|a, b| a.ledge.y.total_cmp(&b.ledge.y));
            let landed = if let Some(drift) = hit {
                if accumulate {
                    let bin = ((f.x - drift.ledge.x) / drift.ledge.width * drift.depths.len() as f32) as usize;
                    let bin = bin.min(drift.depths.len() - 1);
                    let depth = &mut drift.depths[bin];
                    *depth = (*depth + 1.8).min(12.0);
                }
                true
            } else {
                false
            };
            if landed || f.y > self.height {
                self.flakes[index] = self.flake(false);
            }
        }
        let previous = self.balls.clone();
        for (index, ball) in self.balls.iter_mut().enumerate() {
            if self.held == Some(index) {
                continue;
            }
            let grounded = self.drifts.iter().any(|d| {
                Some(d.ledge.id) == ball.support
                    && ball.x >= d.ledge.x
                    && ball.x <= d.ledge.x + d.ledge.width
                    && (ball.y + ball.radius - d.ledge.y).abs() < 3.0
            });
            let stacked = previous.iter().enumerate().any(|(i, b)| {
                i != index
                    && self.held != Some(i)
                    && b.y > ball.y
                    && (b.x - ball.x).abs() < b.radius
                    && (ball.y + ball.radius - (b.y - b.radius)).abs() < 3.0
            });
            if ball.support.is_some() && (grounded || stacked) {
                continue;
            }
            ball.support = None;
            let before = ball.y + ball.radius;
            ball.velocity += 380.0 * dt;
            ball.y += ball.velocity * dt;
            let ledge = self
                .drifts
                .iter()
                .filter(|d| {
                    ball.x >= d.ledge.x
                        && ball.x <= d.ledge.x + d.ledge.width
                        && before <= d.ledge.y + 1.0
                        && ball.y + ball.radius >= d.ledge.y
                })
                .map(|d| (d.ledge.y, Some(d.ledge.id)));
            let spheres = previous
                .iter()
                .enumerate()
                .filter(|(i, b)| {
                    *i != index
                        && self.held != Some(*i)
                        && b.y > ball.y
                        && (b.x - ball.x).abs() < b.radius
                        && before <= b.y - b.radius + 1.0
                        && ball.y + ball.radius >= b.y - b.radius
                })
                .map(|(_, b)| (b.y - b.radius, b.support));
            if let Some((top, support)) = ledge.chain(spheres).min_by(|a, b| a.0.total_cmp(&b.0)) {
                ball.y = top - ball.radius;
                ball.velocity = 0.0;
                ball.support = support;
            }
            ball.y = ball.y.min(self.height - ball.radius);
        }
        if !accumulate {
            for drift in &mut self.drifts {
                if drift.ledge.id != -2 || !self.island_cap {
                    drift.depths.fill(0.0);
                }
            }
        }
    }

    pub fn hit(&self, x: f32, y: f32) -> bool {
        self.balls.iter().any(|b| (b.x - x).hypot(b.y - y) < b.radius + 4.0)
            || self.drifts.iter().any(|d| {
                x >= d.ledge.x
                    && x <= d.ledge.x + d.ledge.width
                    && y >= d.ledge.y - 16.0
                    && y <= d.ledge.y + 4.0
                    && d.depths[(((x - d.ledge.x) / d.ledge.width * d.depths.len() as f32).floor().max(0.0) as usize)
                        .min(d.depths.len() - 1)]
                        > 0.1
            })
    }

    pub fn grab(&mut self, x: f32, y: f32) {
        self.held = self.balls.iter().rposition(|b| (b.x - x).hypot(b.y - y) < b.radius + 4.0);
        if self.held.is_none() && self.hit(x, y) && self.balls.len() < 12 {
            self.balls.push(Ball { x, y, radius: 4.0, velocity: 0.0, support: None });
            self.held = Some(self.balls.len() - 1);
        }
    }

    pub fn drag(&mut self, x: f32, y: f32) {
        let Some(index) = self.held else { return };
        let ball = &mut self.balls[index];
        ball.x = x.clamp(ball.radius, (self.width - ball.radius).max(ball.radius));
        ball.y = y.clamp(ball.radius, (self.height - ball.radius).max(ball.radius));
        ball.support = None;
        for drift in &mut self.drifts {
            if (y - drift.ledge.y).abs() > ball.radius + 18.0 {
                continue;
            }
            let count = drift.depths.len() as f32;
            for (bin, depth) in drift.depths.iter_mut().enumerate() {
                let center = drift.ledge.x + (bin as f32 + 0.5) * drift.ledge.width / count;
                if (center - x).abs() < ball.radius + 8.0 {
                    ball.radius = (ball.radius.powi(2) + *depth * 18.0).sqrt().min(32.0);
                    *depth = 0.0;
                }
            }
        }
    }

    pub fn release(&mut self) {
        let Some(index) = self.held.take() else { return };
        let held = self.balls[index];
        let target = self
            .balls
            .iter()
            .enumerate()
            .filter(|(i, b)| {
                *i != index
                    && b.radius >= held.radius * 0.85
                    && (b.x - held.x).abs() < b.radius
                    && (held.y - (b.y - b.radius - held.radius)).abs() < 35.0
            })
            .min_by(|(_, a), (_, b)| a.y.total_cmp(&b.y))
            .map(|(_, b)| *b);
        if target.is_none()
            && let Some(drift) = self
                .drifts
                .iter()
                .filter(|d| {
                    held.x >= d.ledge.x
                        && held.x <= d.ledge.x + d.ledge.width
                        && (held.y - d.ledge.y).abs() <= held.radius + 18.0
                })
                .min_by(|a, b| (held.y - a.ledge.y).abs().total_cmp(&(held.y - b.ledge.y).abs()))
        {
            self.balls[index].y = drift.ledge.y - held.radius;
            self.balls[index].velocity = 0.0;
            self.balls[index].support = Some(drift.ledge.id);
        }
        if let Some(base) = target {
            self.balls[index].x = base.x;
            self.balls[index].y = base.y - base.radius - held.radius + 2.0;
            self.balls[index].support = base.support;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene() -> Snow {
        let mut snow = Snow::new(800.0, 600.0, 90, 741);
        snow.ledges(vec![
            Ledge { id: 42, x: 100.0, y: 300.0, width: 400.0 },
            Ledge { id: -1, x: 0.0, y: 600.0, width: 800.0 },
        ]);
        snow
    }

    #[test]
    fn snow_lands_rolls_and_keeps_its_volume_when_the_window_moves() {
        let mut snow = scene();
        snow.flakes[0] = Flake { x: 220.0, y: 299.0, radius: 2.0, speed: 60.0, phase: 0.0 };
        snow.step(0.05, true);
        assert!(snow.flakes[0].y < 0.0);
        assert!(snow.drifts[0].depths.iter().sum::<f32>() > 0.0);
        assert!(!snow.hit(600.0, 300.0));
        snow.drifts[0].depths.fill(10.0);
        let before = snow.drifts[0].depths.iter().sum::<f32>();
        snow.grab(200.0, 296.0);
        snow.drag(200.0, 296.0);
        snow.drag(240.0, 296.0);
        assert!(snow.balls[0].radius > 4.0);
        assert!(snow.drifts[0].depths.iter().sum::<f32>() < before);
        snow.release();
        for _ in 0..60 {
            snow.step(0.033, true);
        }
        assert_eq!(snow.balls[0].support, Some(42));
        let before = snow.balls[0];
        snow.ledges(vec![Ledge { id: 42, x: 120.0, y: 350.0, width: 400.0 }]);
        assert_eq!(snow.balls[0].x, before.x + 20.0);
        assert_eq!(snow.balls[0].y, before.y + 50.0);
        snow.ledges(vec![]);
        assert_eq!(snow.balls[0].support, None);
        snow.step(0.033, true);
        assert!(snow.balls[0].y > before.y + 50.0);
    }

    #[test]
    fn snowman_snaps_and_cancellation_releases_the_ball() {
        let mut snow = scene();
        snow.balls = vec![
            Ball { x: 300.0, y: 570.0, radius: 30.0, velocity: 0.0, support: Some(-1) },
            Ball { x: 311.0, y: 519.0, radius: 20.0, velocity: 0.0, support: None },
        ];
        snow.grab(311.0, 519.0);
        snow.release();
        assert_eq!(snow.held, None);
        assert_eq!(snow.balls[1].x, 300.0);
        assert_eq!(snow.balls[1].y, 522.0);
        for _ in 0..100 {
            snow.step(0.033, true);
        }
        assert_eq!(snow.balls[1].y, 522.0);
        snow.grab(300.0, 522.0);
        snow.release();
        assert!(snow.held.is_none());
        snow.grab(300.0, 570.0);
        snow.drag(700.0, 350.0);
        snow.release();
        snow.step(0.033, true);
        assert_eq!(snow.balls[1].support, None);
        assert!(snow.balls[1].y > 522.0);
    }

    #[test]
    fn long_frames_are_bounded_and_disabling_accumulation_clears_the_drifts() {
        let mut snow = scene();
        snow.drifts[0].depths.fill(12.0);
        for _ in 0..1000 {
            snow.step(10.0, false);
        }
        assert_eq!(snow.flakes.len(), 90);
        assert!(snow.drifts.iter().all(|d| d.depths.iter().all(|v| *v == 0.0)));
        assert!(snow.flakes.iter().all(|f| f.x.is_finite() && f.x >= 0.0 && f.x < 800.0 && f.y <= 600.0));
    }
}

#[cfg(test)]
mod cover_tests {
    use super::*;
    #[test]
    fn a_light_cover_is_collectable_and_the_island_recovers_after_rolling() {
        let mut snow = Snow::new(800.0, 600.0, 90, 91);
        snow.starter_cover = true;
        snow.island_cap = true;
        snow.ledges(vec![Ledge { id: -2, x: 100.0, y: 100.0, width: 300.0 }]);
        assert!(snow.hit(150.0, 98.0));
        snow.grab(150.0, 98.0);
        snow.drag(180.0, 98.0);
        assert!(snow.balls[0].radius > 4.0);
        snow.release();
        snow.drifts[0].depths.fill(0.0);
        snow.step(1.0 / 60.0, false);
        assert!(snow.drifts[0].depths.iter().all(|v| *v > 0.0));
    }
}
