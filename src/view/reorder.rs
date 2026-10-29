use super::Rect;

pub struct Reorder {
    pub from: usize,
    pub to: usize,
    pub cursor: (f32, f32),
    pub grab: (f32, f32),
    pub pos: Vec<(f32, f32)>,
}

impl Reorder {
    pub fn new(slots: &[Rect], from: usize, cursor: (f32, f32)) -> Self {
        let s = slots[from];
        Self {
            from,
            to: from,
            cursor,
            grab: (cursor.0 - s.0, cursor.1 - s.1),
            pos: slots.iter().map(|r| (r.0, r.1)).collect(),
        }
    }

    pub fn slot_of(&self, j: usize) -> usize {
        if j == self.from {
            self.to
        } else if self.from < self.to && j > self.from && j <= self.to {
            j - 1
        } else if self.to < self.from && j >= self.to && j < self.from {
            j + 1
        } else {
            j
        }
    }

    pub fn nearest(slots: &[Rect], at: (f32, f32)) -> usize {
        let d = |r: &Rect| {
            let (cx, cy) = ((r.0 + r.2) / 2.0, (r.1 + r.3) / 2.0);
            (cx - at.0).powi(2) + (cy - at.1).powi(2)
        };
        slots.iter().enumerate().min_by(|a, b| d(a.1).total_cmp(&d(b.1))).map_or(0, |(i, _)| i)
    }

    pub fn step(&mut self, slots: &[Rect], cursor: (f32, f32)) -> bool {
        self.step_with(
            slots,
            |r, j| {
                let t = slots[r.slot_of(j)];
                (t.0, t.1)
            },
            cursor,
        )
    }

    pub fn step_with(
        &mut self,
        slots: &[Rect],
        target: impl Fn(&Self, usize) -> (f32, f32),
        cursor: (f32, f32),
    ) -> bool {
        self.cursor = cursor;
        if slots.len() != self.pos.len() || self.from >= slots.len() {
            return false;
        }
        let s = slots[self.from];
        let centre = (cursor.0 - self.grab.0 + (s.2 - s.0) / 2.0, cursor.1 - self.grab.1 + (s.3 - s.1) / 2.0);
        self.to = Self::nearest(slots, centre);
        let mut moving = false;
        for j in 0..slots.len() {
            if j == self.from {
                continue;
            }
            let goal = target(self, j);
            let pos = &mut self.pos[j];
            let (dx, dy) = (goal.0 - pos.0, goal.1 - pos.1);
            if dx.abs() < 0.5 && dy.abs() < 0.5 {
                *pos = goal;
            } else {
                *pos = (pos.0 + dx * 0.35, pos.1 + dy * 0.35);
                moving = true;
            }
        }
        moving
    }

    pub fn dragged_rect(&self, size: (f32, f32)) -> Rect {
        let (x, y) = (self.cursor.0 - self.grab.0, self.cursor.1 - self.grab.1);
        (x, y, x + size.0, y + size.1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(n: usize) -> Vec<Rect> {
        (0..n).map(|i| (i as f32 * 50.0, 0.0, i as f32 * 50.0 + 40.0, 40.0)).collect()
    }

    #[test]
    fn the_others_shift_into_the_gap() {
        let mut r = Reorder::new(&row(5), 1, (70.0, 20.0));
        r.to = 3;
        assert_eq!([0, 2, 3, 4].map(|j| r.slot_of(j)), [0, 1, 2, 4]);
        assert_eq!(r.slot_of(1), 3);
        r.from = 3;
        r.to = 1;
        assert_eq!([0, 1, 2, 4].map(|j| r.slot_of(j)), [0, 2, 3, 4]);
    }

    #[test]
    fn dragging_over_a_slot_targets_it_and_the_neighbours_glide() {
        let slots = row(4);
        let mut r = Reorder::new(&slots, 0, (20.0, 20.0));
        let moving = r.step(&slots, (120.0, 20.0));
        assert_eq!(r.to, 2);
        assert!(moving);
        for _ in 0..60 {
            r.step(&slots, (120.0, 20.0));
        }
        assert_eq!(r.pos[1], (0.0, 0.0));
        assert_eq!(r.pos[2], (50.0, 0.0));
    }
}
