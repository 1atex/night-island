use super::winter_model::Ledge;

#[derive(Clone, Copy)]
pub(super) struct Chip {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub radius: f32,
    pub life: f32,
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
                    vx: side * (38.0 + (i % 5) as f32 * 9.0),
                    vy: if before.y < 16.0 || !expanded {
                        12.0 + (i % 7) as f32 * 3.0
                    } else {
                        -28.0 - (i % 7) as f32 * 6.0
                    },
                    radius: 1.3 + (i % 4) as f32 * 0.55,
                    life: 1.0,
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

    pub fn step(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.05);
        self.coverage += (1.0 - self.coverage) * (1.0 - (-dt * 1.4).exp());
        for chip in &mut self.chips {
            chip.vy += 155.0 * dt;
            chip.x += chip.vx * dt;
            chip.y += chip.vy * dt;
            chip.life = (chip.life - dt * 0.9).max(0.0);
        }
        self.chips.retain(|c| c.life > 0.0);
    }
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
