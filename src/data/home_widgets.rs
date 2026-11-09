use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WidgetSize {
    #[default]
    Tile,
    Wide2,
    Tall2,
    Wide4,
    Tall4,
}

impl WidgetSize {
    pub const ALL: [Self; 5] = [Self::Tile, Self::Wide2, Self::Tall2, Self::Wide4, Self::Tall4];
    pub fn dimensions(self) -> (u8, u8) {
        match self {
            Self::Tile => (1, 1),
            Self::Wide2 => (2, 1),
            Self::Tall2 => (1, 2),
            Self::Wide4 => (2, 2),
            Self::Tall4 => (1, 4),
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Tile => "1",
            Self::Wide2 => "2 ↔",
            Self::Tall2 => "2 ↕",
            Self::Wide4 => "4 ↔",
            Self::Tall4 => "4 ↕",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placement {
    pub id: u8,
    pub size: WidgetSize,
    pub column: u8,
    pub row: u8,
}

pub fn pack(items: &[(u8, WidgetSize)], occupied: u8) -> Option<Vec<Placement>> {
    if items.len() > 8
        || items
            .iter()
            .map(|(_, s)| {
                let (w, h) = s.dimensions();
                u32::from(w * h)
            })
            .sum::<u32>()
            + occupied.count_ones()
            > 8
    {
        return None;
    }
    fn place(items: &[(u8, WidgetSize)], used: u8, out: &mut Vec<Placement>) -> bool {
        let Some(&(id, size)) = items.first() else { return true };
        let (w, h) = size.dimensions();
        for row in 0..=4 - h {
            for column in 0..=2 - w {
                let mut bits = 0u8;
                for y in row..row + h {
                    for x in column..column + w {
                        bits |= 1 << (y * 2 + x);
                    }
                }
                if bits & used != 0 {
                    continue;
                }
                out.push(Placement { id, size, column, row });
                if place(&items[1..], used | bits, out) {
                    return true;
                }
                out.pop();
            }
        }
        false
    }
    let mut out = Vec::with_capacity(items.len());
    place(items, occupied, &mut out).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn widgets_reflow_without_overlap_and_leave_reserved_cells_free() {
        for size in WidgetSize::ALL {
            for occupied in [0, 1, 3, 15] {
                let items = [(1, size), (2, WidgetSize::Tile), (3, WidgetSize::Tile)];
                if let Some(layout) = pack(&items, occupied) {
                    let mut used = occupied;
                    for entry in layout {
                        let (w, h) = entry.size.dimensions();
                        for y in entry.row..entry.row + h {
                            for x in entry.column..entry.column + w {
                                let bit = 1 << (y * 2 + x);
                                assert_eq!(used & bit, 0);
                                used |= bit;
                            }
                        }
                    }
                }
            }
        }
        assert!(pack(&[(1, WidgetSize::Tall4), (2, WidgetSize::Wide2)], 0).is_none());
        assert!(pack(&[(1, WidgetSize::Tall4), (2, WidgetSize::Tall4)], 0).is_some());
        assert!(pack(&[(1, WidgetSize::Wide4), (2, WidgetSize::Wide4), (3, WidgetSize::Tile)], 0).is_none());
    }
}
