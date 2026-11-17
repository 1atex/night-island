use super::winter_model::Ledge;

#[derive(Clone, Copy, Default)]
pub(super) struct Chip {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub radius: f32,
    pub life: f32,
    pub angle: f32,
    pub spin: f32,
    pub source: isize,
    pub previous_y: f32,
}

#[derive(Default)]
pub(super) struct IslandSnow {
    pub ledge: Option<Ledge>,
    pub chips: Vec<Chip>,
    pub coverage: f32,
    expanded: bool,
}

impl IslandSnow {
    pub fn follow(&mut self, ledge: Option<Ledge>, expanded: bool, shake: bool) -> bool {
        let emitted = self.ledge.is_some() && ledge.is_some() && expanded != self.expanded && shake;
        if self.ledge.is_none() {
            self.coverage = 1.0;
        }
        if expanded != self.expanded
            && shake
            && let Some(before) = self.ledge
        {
            for i in 0..(64 - self.chips.len()).min(32) {
                let t = (i as f32 + 0.5) / 32.0;
                let side = (t - 0.5) * 2.0;
                self.chips.push(Chip {
                    x: before.x + t * before.width,
                    y: before.surface_y(before.x + t * before.width) - 1.0,
                    vx: side * (55.0 + (i as f32 * 2.17).sin().abs() * 95.0),
                    vy: if before.y < 16.0 || !expanded {
                        12.0 + (i % 7) as f32 * 3.0
                    } else {
                        -28.0 - (i % 7) as f32 * 6.0
                    },
                    radius: 0.9 + (i as f32 * 1.73).sin().abs() * 2.2,
                    life: 1.0,
                    angle: i as f32 * 0.7,
                    spin: side * (2.0 + (i % 5) as f32),
                    source: -2,
                    previous_y: before.surface_y(before.x + t * before.width) - 1.0,
                });
            }
            self.coverage = if expanded { 0.28 } else { 0.55 };
        }
        self.expanded = expanded;
        self.ledge = ledge;
        if ledge.is_none() {
            self.chips.clear();
        }
        emitted
    }

    pub fn match_cover(&mut self, drift: &super::winter_model::Drift) {
        for chip in self.chips.iter_mut().filter(|c| c.life == 1.0) {
            let bin = (((chip.x - drift.ledge.x) / drift.ledge.width * drift.depths.len() as f32).max(0.0) as usize)
                .min(drift.depths.len() - 1);
            let depth = drift.depths[bin];
            if depth < 0.4 {
                chip.life = 0.0;
                continue;
            }
            chip.radius = chip.radius.min(depth * 0.75);
            chip.y -= depth * 0.5;
            chip.previous_y = chip.y;
        }
        self.chips.retain(|c| c.life > 0.0);
    }

    pub fn step(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.05);
        self.coverage += (1.0 - self.coverage) * (1.0 - (-dt * 1.4).exp());
        advance(&mut self.chips, dt);
    }
}

pub(super) fn advance(chips: &mut Vec<Chip>, dt: f32) {
    let dt = dt.clamp(0.0, 0.05);
    for chip in chips.iter_mut() {
        chip.previous_y = chip.y;
        let drag = (-dt * (0.5 / chip.radius.max(0.5))).exp();
        chip.vx *= drag;
        chip.vy += 320.0 * dt;
        chip.x += chip.vx * dt;
        chip.y += chip.vy * dt;
        chip.angle += chip.spin * dt;
        chip.life = (chip.life - dt * 0.65).max(0.0);
    }
    chips.retain(|c| c.life > 0.0);
}

pub(super) fn land(chips: &mut Vec<Chip>, drifts: &mut [super::winter_model::Drift]) {
    let mut fragments = Vec::new();
    for chip in chips.iter_mut() {
        if chip.vy <= 0.0 {
            continue;
        }
        let hit = drifts
            .iter_mut()
            .filter(|d| {
                d.ledge.id != chip.source
                    && chip.x >= d.ledge.x
                    && chip.x < d.ledge.x + d.ledge.width
                    && chip.previous_y <= d.ledge.surface_y(chip.x)
                    && chip.y >= d.ledge.surface_y(chip.x)
            })
            .min_by(|a, b| a.ledge.surface_y(chip.x).total_cmp(&b.ledge.surface_y(chip.x)));
        let Some(drift) = hit else { continue };
        let bin = (((chip.x - drift.ledge.x) / drift.ledge.width * drift.depths.len() as f32) as usize)
            .min(drift.depths.len() - 1);
        drift.depths[bin] = (drift.depths[bin] + chip.radius * 0.15).min(12.0);
        if chip.radius > 2.0 && chip.life > 0.4 && fragments.len() < 16 {
            fragments.push(Chip {
                y: drift.ledge.surface_y(chip.x) - 1.0,
                previous_y: drift.ledge.surface_y(chip.x) - 1.0,
                vy: -chip.vy * 0.22,
                vx: chip.vx * 0.6,
                spin: -chip.spin * 1.4,
                radius: chip.radius * 0.55,
                life: chip.life * 0.5,
                source: drift.ledge.id,
                ..*chip
            });
        }
        chip.life = 0.0;
    }
    chips.retain(|c| c.life > 0.0);
    chips.extend(fragments.into_iter().take(96usize.saturating_sub(chips.len())));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn opening_shakes_once_and_restores_the_cap_without_resets() {
        let ledge = Some(Ledge { id: -2, x: 100.0, y: 20.0, width: 220.0, ..Default::default() });
        let mut snow = IslandSnow::default();
        snow.follow(ledge, false, true);
        assert_eq!(snow.coverage, 1.0);
        snow.follow(ledge, true, true);
        assert_eq!(snow.chips.len(), 32);
        let x = snow.chips[0].x;
        for _ in 0..20 {
            snow.follow(ledge, true, true);
            snow.step(1.0 / 60.0);
        }
        assert!(snow.chips[0].x < x);
        assert!(snow.coverage > 0.28 && snow.coverage < 1.0);
        for _ in 0..300 {
            snow.step(1.0 / 60.0);
        }
        assert!(snow.chips.is_empty());
        assert!(snow.coverage > 0.99);
        snow.follow(ledge, false, false);
        snow.follow(ledge, true, false);
        assert!(snow.chips.is_empty());
    }
}

#[cfg(test)]
mod transition_tests {
    use super::*;
    #[test]
    fn resting_is_silent_and_opening_and_closing_each_emit_a_single_burst() {
        let ledge = Some(Ledge { id: -2, x: 100.0, y: 8.0, width: 220.0, radius: 16.0 });
        let mut snow = IslandSnow::default();
        assert!(!snow.follow(ledge, false, true));
        for _ in 0..300 {
            snow.follow(ledge, false, true);
            snow.step(1.0 / 60.0);
        }
        assert!(snow.chips.is_empty());
        assert!(snow.follow(ledge, true, true));
        assert_eq!(snow.chips.len(), 32);
        assert!(snow.chips.iter().all(|c| c.vy > 0.0));
        for _ in 0..300 {
            snow.follow(ledge, true, true);
            snow.step(1.0 / 60.0);
        }
        assert!(snow.chips.is_empty());
        assert!(snow.follow(ledge, false, true));
        assert_eq!(snow.chips.len(), 32);
        for _ in 0..300 {
            snow.follow(ledge, false, true);
            snow.step(1.0 / 60.0);
        }
        assert!(snow.chips.is_empty());
    }
}

#[cfg(test)]
mod physics_tests {
    use super::super::winter_model::Snow;
    use super::*;
    #[test]
    fn fragments_rotate_accelerate_and_break_up_on_a_lower_roof() {
        let mut snow = Snow::new(800.0, 600.0, 0, 17);
        snow.ledges(vec![Ledge { id: 77, x: 0.0, y: 100.0, width: 800.0, ..Default::default() }]);
        let mut chips = vec![Chip {
            x: 300.0,
            y: 60.0,
            previous_y: 60.0,
            radius: 3.0,
            vx: 20.0,
            vy: 100.0,
            spin: 4.0,
            life: 1.0,
            source: -2,
            ..Default::default()
        }];
        advance(&mut chips, 0.05);
        assert!(chips[0].angle > 0.0 && chips[0].vy > 100.0);
        for _ in 0..30 {
            advance(&mut chips, 1.0 / 60.0);
            land(&mut chips, &mut snow.drifts);
        }
        assert!(snow.drifts[0].depths.iter().sum::<f32>() > 0.0);
        assert!(chips.iter().all(|c| c.radius < 2.0));
    }
}
