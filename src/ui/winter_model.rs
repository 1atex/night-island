#[derive(Clone, Copy, Debug, Default)]
pub struct Ledge {
    pub id: isize,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub radius: f32,
}

impl Ledge {
    pub fn surface_y(&self, x: f32) -> f32 {
        let radius = self.radius.min(self.width * 0.5).max(0.0);
        let edge = (x - self.x).min(self.x + self.width - x).clamp(0.0, radius);
        self.y + radius - (radius * radius - (radius - edge).powi(2)).max(0.0).sqrt()
    }
}

pub struct Drift {
    pub ledge: Ledge,
    pub depths: Vec<f32>,
    pub motion: (f32, f32),
    pub shed_distance: f32,
}

#[derive(Clone, Copy)]
pub struct Flake {
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub speed: f32,
    pub phase: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Ball {
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub velocity: f32,
    pub vx: f32,
    pub angle: f32,
    pub squash: f32,
    pub parent: Option<usize>,
    pub support: Option<isize>,
}

pub struct Snow {
    pub flakes: Vec<Flake>,
    pub drifts: Vec<Drift>,
    pub balls: Vec<Ball>,
    pub held: Option<usize>,
    pub starter_cover: bool,
    pub island_cap: bool,
    pub dynamics: bool,
    pub crumbs: Vec<super::winter_island::Chip>,
    width: f32,
    height: f32,
    random: u64,
    time: f32,
    last_dt: f32,
    drag_sample: Option<(f32, f32, f32)>,
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
            last_dt: 1.0 / 60.0,
            drag_sample: None,
            dynamics: true,
            crumbs: Vec::new(),
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
                let ease = 1.0 - (-self.last_dt * 12.0).exp();
                let motion = (
                    drift.motion.0 + (dx / self.last_dt - drift.motion.0) * ease,
                    drift.motion.1 + (dy / self.last_dt - drift.motion.1) * ease,
                );
                drift.shed_distance += dx.hypot(dy);
                let impulse = ((motion.0 - drift.motion.0) * 0.16).clamp(-260.0, 260.0);
                for (index, ball) in self.balls.iter_mut().enumerate() {
                    if ball.support == Some(ledge.id) && self.held != Some(index) {
                        ball.x += dx;
                        ball.y += dy;
                        if self.dynamics {
                            ball.vx -= impulse;
                        }
                    }
                }
                if self.dynamics
                    && motion.0.hypot(motion.1) > 120.0
                    && drift.shed_distance > 24.0
                    && self.crumbs.len() < 96
                {
                    drift.shed_distance = 0.0;
                    let count = drift.depths.len() as f32;
                    let spacing = (drift.depths.len() / 5).max(1);
                    for (i, depth) in drift.depths.iter_mut().enumerate().step_by(spacing) {
                        if *depth < 0.5 || self.crumbs.len() >= 96 {
                            continue;
                        }
                        let x = ledge.x + (i as f32 + 0.5) * ledge.width / count;
                        self.crumbs.push(super::winter_island::Chip {
                            x,
                            y: ledge.surface_y(x) - *depth,
                            vx: (-motion.0 * 0.25).clamp(-200.0, 200.0) + (i as f32 * 2.1).sin() * 25.0,
                            vy: (-motion.1 * 0.2).clamp(-100.0, 100.0) - 15.0,
                            radius: (*depth * 0.35).clamp(0.7, 2.2),
                            life: 1.0,
                        });
                        *depth *= 0.9;
                    }
                }
                drift.motion = motion;
                drift.ledge = ledge;
            } else if ledge.width > 1.0 {
                self.drifts.push(Drift {
                    ledge,
                    motion: (0.0, 0.0),
                    shed_distance: 0.0,
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
        if self.dynamics {
            for drift in &mut self.drifts {
                let previous = drift.depths.clone();
                let count = previous.len();
                let width = drift.ledge.width / count as f32;
                for i in 0..count.saturating_sub(1) {
                    let a =
                        previous[i] - (drift.ledge.surface_y(drift.ledge.x + (i as f32 + 0.5) * width) - drift.ledge.y);
                    let b = previous[i + 1]
                        - (drift.ledge.surface_y(drift.ledge.x + (i as f32 + 1.5) * width) - drift.ledge.y);
                    let excess = (a - b).abs() - width * 0.28;
                    if excess > 0.0 {
                        let (from, to) = if a > b { (i, i + 1) } else { (i + 1, i) };
                        let amount = (excess * dt * 2.0).min(previous[from] * 0.2);
                        drift.depths[from] -= amount;
                        drift.depths[to] += amount;
                    }
                }
                if drift.ledge.radius > 0.0 {
                    for (index, side) in [(0, -1.0), (count - 1, 1.0)] {
                        let before = drift.depths[index];
                        drift.depths[index] *= (-dt * 1.7).exp();
                        if before > 0.8
                            && (self.time * 4.0).floor() != ((self.time - dt) * 4.0).floor()
                            && self.crumbs.len() < 96
                        {
                            let x =
                                if side < 0.0 { drift.ledge.x + 1.0 } else { drift.ledge.x + drift.ledge.width - 1.0 };
                            self.crumbs.push(super::winter_island::Chip {
                                x,
                                y: drift.ledge.surface_y(x) - 1.0,
                                vx: side * 22.0,
                                vy: 10.0,
                                radius: 0.8,
                                life: 0.8,
                            });
                        }
                    }
                }
                for depth in &mut drift.depths {
                    *depth = depth.clamp(0.0, 14.0);
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
                    f.x >= d.ledge.x
                        && f.x < d.ledge.x + d.ledge.width
                        && before <= d.ledge.surface_y(f.x)
                        && f.y >= d.ledge.surface_y(f.x)
                })
                .min_by(|a, b| a.ledge.surface_y(f.x).total_cmp(&b.ledge.surface_y(f.x)));
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
        if !self.dynamics {
            for ball in &mut self.balls {
                ball.vx = 0.0;
            }
        }
        super::winter_physics::advance(&mut self.balls, &self.drifts, self.held, (self.width, self.height), dt);
        self.last_dt = dt.max(1.0 / 240.0);
        for crumb in &mut self.crumbs {
            crumb.vy += 220.0 * dt;
            crumb.x += crumb.vx * dt;
            crumb.y += crumb.vy * dt;
            crumb.life = (crumb.life - dt).max(0.0);
        }
        self.crumbs.retain(|c| c.life > 0.0);
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
        self.drag_sample = None;
        self.held = self.balls.iter().rposition(|b| (b.x - x).hypot(b.y - y) < b.radius + 4.0);
        if self.held.is_none() && self.hit(x, y) && self.balls.len() < 12 {
            self.balls.push(Ball { x, y, radius: 4.0, ..Default::default() });
            self.held = Some(self.balls.len() - 1);
        }
        if let Some(index) = self.held {
            self.balls[index].vx = 0.0;
            self.balls[index].velocity = 0.0;
            self.balls[index].parent = None;
            self.balls[index].support = None;
        }
    }

    pub fn drag(&mut self, x: f32, y: f32) {
        let Some(index) = self.held else { return };
        let ball = &mut self.balls[index];
        if let Some((px, py, at)) = self.drag_sample {
            let dt = (self.time - at).max(1.0 / 240.0);
            ball.vx = ball.vx * 0.45 + ((x - px) / dt).clamp(-900.0, 900.0) * 0.55;
            ball.velocity = ball.velocity * 0.45 + ((y - py) / dt).clamp(-900.0, 900.0) * 0.55;
        }
        self.drag_sample = Some((x, y, self.time));
        ball.parent = None;
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
        self.drag_sample = None;
        if !self.dynamics {
            self.balls[index].vx = 0.0;
            self.balls[index].velocity = 0.0;
        }
        let held = self.balls[index];
        if self.dynamics && held.vx.hypot(held.velocity) > 140.0 {
            return;
        }
        let target = self
            .balls
            .iter()
            .enumerate()
            .filter(|(i, b)| {
                *i != index
                    && b.radius >= held.radius * 0.75
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
            self.balls[index].y = drift.ledge.surface_y(held.x) - held.radius;
            self.balls[index].velocity = 0.0;
            self.balls[index].support = Some(drift.ledge.id);
        }
        if let Some(base) = target {
            self.balls[index].x = base.x;
            self.balls[index].y = base.y - base.radius - held.radius + 2.0;
            self.balls[index].support = base.support;
            self.balls[index].velocity = 0.0;
            self.balls[index].vx = 0.0;
            self.balls[index].parent = self.balls.iter().position(|b| b.x == base.x && b.y == base.y);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene() -> Snow {
        let mut snow = Snow::new(800.0, 600.0, 90, 741);
        snow.dynamics = false;
        snow.ledges(vec![
            Ledge { id: 42, x: 100.0, y: 300.0, width: 400.0, ..Default::default() },
            Ledge { id: -1, x: 0.0, y: 600.0, width: 800.0, ..Default::default() },
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
        snow.ledges(vec![Ledge { id: 42, x: 120.0, y: 350.0, width: 400.0, ..Default::default() }]);
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
            Ball { x: 300.0, y: 570.0, radius: 30.0, velocity: 0.0, support: Some(-1), ..Default::default() },
            Ball { x: 311.0, y: 519.0, radius: 20.0, velocity: 0.0, support: None, ..Default::default() },
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
        snow.ledges(vec![Ledge { id: -2, x: 100.0, y: 100.0, width: 300.0, ..Default::default() }]);
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
