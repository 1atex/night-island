use serde::{Deserialize, Serialize};
use windows::Win32::Graphics::DirectWrite::{DWRITE_FONT_WEIGHT_BOLD, DWRITE_TEXT_ALIGNMENT_CENTER};
use windows::core::Result;

use crate::ui::gfx::{Painter, rgba};

pub const BOX_W: f32 = 40.0;
pub const BOX_H: f32 = 30.0;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum Character {
    None,
    Pixel,
    #[default]
    Spark,
    Eyes,
    Cat,
    Robot,
    Blob,
}

impl Character {
    pub const ALL: [Character; 7] = [
        Character::None,
        Character::Pixel,
        Character::Spark,
        Character::Eyes,
        Character::Cat,
        Character::Robot,
        Character::Blob,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Character::None => "No character",
            Character::Pixel => "Pixel",
            Character::Spark => "Spark",
            Character::Eyes => "Eyes",
            Character::Cat => "Cat",
            Character::Robot => "Robot",
            Character::Blob => "Blob",
        }
    }

    pub fn step(self, forward: bool) -> Character {
        let n = Self::ALL.len();
        let i = Self::ALL.iter().position(|c| *c == self).unwrap_or(0);
        Self::ALL[if forward { (i + 1) % n } else { (i + n - 1) % n }]
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Reaction {
    Idle,
    Music,
    Done,
    Attention,
    File,
    Sleep,
}

#[derive(Clone, Copy)]
pub struct Pose {
    pub t: f32,
    pub reaction: Reaction,
    pub look: (f32, f32),
    pub blink: bool,
    pub accent: (f32, f32, f32),
    pub beat: f32,
}

impl Pose {
    pub fn lively(&self) -> bool {
        matches!(self.reaction, Reaction::Music | Reaction::Done | Reaction::Attention | Reaction::File)
    }

    fn bob(&self) -> f32 {
        let t = self.t;
        match self.reaction {
            Reaction::Idle => (t * 1.8).sin() * 0.5,
            Reaction::Music => -self.beat * 1.0 + (t * 9.0).sin() * 0.4,
            Reaction::Done => -(t * 10.0).sin().abs() * 3.2,
            Reaction::Attention => 0.0,
            Reaction::File => (t * 7.0).sin() * 1.0,
            Reaction::Sleep => (t * 1.0).sin() * 0.45,
        }
    }

    fn shake(&self) -> f32 {
        if self.reaction == Reaction::Attention { (self.t * 46.0).sin() * 1.3 } else { 0.0 }
    }

    fn mood(&self, calm: (f32, f32, f32)) -> (f32, f32, f32) {
        match self.reaction {
            Reaction::Idle => calm,
            Reaction::Music => self.accent,
            Reaction::Done => (0.47, 0.9, 0.55),
            Reaction::Attention => (1.0, 0.55, 0.28),
            Reaction::File => (0.5, 0.75, 1.0),
            Reaction::Sleep => (0.58, 0.63, 0.78),
        }
    }
}

type Rgb = (f32, f32, f32);

fn c(rgb: Rgb, a: f32) -> windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F {
    rgba(rgb.0, rgb.1, rgb.2, a)
}

const DARK: Rgb = (0.1, 0.11, 0.15);
const WHITE: Rgb = (0.96, 0.96, 0.98);

pub fn draw(p: &Painter, character: Character, w: f32, pose: &Pose) -> Result<()> {
    let s = w / BOX_W;
    match character {
        Character::None => Ok(()),
        Character::Pixel => draw_pixel(p, s, pose),
        Character::Spark => draw_spark(p, s, pose),
        Character::Eyes => draw_eyes(p, s, pose),
        Character::Cat => draw_cat(p, s, pose),
        Character::Robot => draw_robot(p, s, pose),
        Character::Blob => draw_blob(p, s, pose),
    }
}

#[allow(clippy::too_many_arguments)]
fn eye(p: &Painter, s: f32, pose: &Pose, x: f32, y: f32, rx: f32, ry: f32, color: Rgb) -> Result<()> {
    let (lx, ly) = (pose.look.0 * 1.4, pose.look.1 * 1.0);
    match pose.reaction {
        Reaction::Sleep => p.line(((x - rx) * s, y * s), ((x + rx) * s, y * s), 1.6 * s, c(color, 0.9)),
        Reaction::Done => {
            let w = 1.7 * s;
            p.line(((x - rx) * s, (y + ry * 0.5) * s), (x * s, (y - ry * 0.5) * s), w, c(color, 1.0))?;
            p.line((x * s, (y - ry * 0.5) * s), ((x + rx) * s, (y + ry * 0.5) * s), w, c(color, 1.0))
        }
        _ if pose.blink => p.line(((x - rx) * s, y * s), ((x + rx) * s, y * s), 1.6 * s, c(color, 0.9)),
        Reaction::Attention => p.fill_ellipse((x + lx) * s, (y + ly) * s, rx * 1.15 * s, ry * 1.3 * s, c(color, 1.0)),
        _ => p.fill_ellipse((x + lx) * s, (y + ly) * s, rx * s, ry * s, c(color, 1.0)),
    }
}

fn zzz(p: &Painter, s: f32, pose: &Pose) -> Result<()> {
    if pose.reaction != Reaction::Sleep {
        return Ok(());
    }
    let drift = (pose.t * 0.9).sin() * 1.2;
    p.text(
        "z",
        (28.0 * s, (1.0 + drift) * s, 40.0 * s, (11.0 + drift) * s),
        9.0 * s,
        DWRITE_FONT_WEIGHT_BOLD,
        DWRITE_TEXT_ALIGNMENT_CENTER,
        c(WHITE, 0.7),
    )
}

fn draw_pixel(p: &Painter, s: f32, pose: &Pose) -> Result<()> {
    const BODY: [&str; 8] = [
        "..#.....#..",
        "...#...#...",
        "..#######..",
        ".##.###.##.",
        "###########",
        "#.#######.#",
        "#.#.....#.#",
        "...##.##...",
    ];
    const LEGS_B: &str = "..#.....#..";
    let color = match pose.reaction {
        Reaction::Attention => (0.9, 0.28, 0.24),
        Reaction::Done => (0.43, 0.82, 0.47),
        Reaction::Sleep => (0.55, 0.42, 0.37),
        _ => (0.91, 0.45, 0.32),
    };
    let cell = 2.0 * s;
    let (ox, oy) = ((BOX_W - 22.0) / 2.0 * s + pose.shake() * s, (BOX_H - 16.0) / 2.0 * s + pose.bob() * s);
    let moving = pose.lively();
    let frame_b = moving && ((pose.t * 4.0) as i32) % 2 == 1;
    for (row, line) in BODY.iter().enumerate() {
        let line = if row == 7 && frame_b { LEGS_B } else { line };
        for (col, ch) in line.chars().enumerate() {
            if ch == '#' {
                let (x, y) = (ox + col as f32 * cell, oy + row as f32 * cell);
                p.fill_rect((x, y, x + cell, y + cell), c(color, 1.0))?;
            }
        }
    }
    for col in [3.0f32, 7.0] {
        let (x, y) = (ox + col * cell, oy + 3.0 * cell);
        if pose.blink || pose.reaction == Reaction::Sleep {
            p.fill_rect((x, y, x + cell, y + cell), c(color, 1.0))?;
        } else {
            let (px, py) = (x + cell * 0.25 + pose.look.0 * cell * 0.3, y + cell * 0.25 + pose.look.1 * cell * 0.3);
            p.fill_rect((px, py, px + cell * 0.55, py + cell * 0.55), c(WHITE, 1.0))?;
        }
    }
    zzz(p, s, pose)
}

fn draw_spark(p: &Painter, s: f32, pose: &Pose) -> Result<()> {
    let tint = pose.mood(WHITE);
    let pulse = if pose.reaction == Reaction::Music { pose.beat * 0.9 } else { 0.0 };
    let r = 8.6 + pulse;
    let (cx, cy) = (20.0 + pose.shake(), 15.0 + pose.bob());
    p.fill_ellipse(cx * s, cy * s, (r + 3.0) * s, (r + 3.0) * s, c(tint, 0.22))?;
    let body = (0.55 + 0.45 * tint.0, 0.55 + 0.45 * tint.1, 0.55 + 0.45 * tint.2);
    p.fill_ellipse(cx * s, cy * s, r * s, r * s, c(body, 1.0))?;
    eye(p, s, pose, cx - 3.4, cy - 0.6, 1.5, 2.5, DARK)?;
    eye(p, s, pose, cx + 3.4, cy - 0.6, 1.5, 2.5, DARK)?;
    zzz(p, s, pose)
}

fn draw_eyes(p: &Painter, s: f32, pose: &Pose) -> Result<()> {
    let tint = if pose.reaction == Reaction::Attention { (1.0, 0.5, 0.45) } else { WHITE };
    let squash = if pose.reaction == Reaction::Music { 1.0 - 0.35 * pose.beat } else { 1.0 };
    let (cy, dx) = (15.0 + pose.bob() * 0.5, pose.shake());
    for sign in [-1.0f32, 1.0] {
        let x = 20.0 + sign * 7.0 + dx;
        match pose.reaction {
            Reaction::Sleep => p.line(((x - 3.0) * s, cy * s), ((x + 3.0) * s, cy * s), 1.8 * s, c(tint, 0.85))?,
            Reaction::Done => {
                p.line(((x - 3.2) * s, (cy + 2.0) * s), (x * s, (cy - 2.4) * s), 2.0 * s, c(tint, 1.0))?;
                p.line((x * s, (cy - 2.4) * s), ((x + 3.2) * s, (cy + 2.0) * s), 2.0 * s, c(tint, 1.0))?;
            }
            _ => {
                let h = if pose.blink {
                    1.2
                } else {
                    12.0 * squash * if pose.reaction == Reaction::Attention { 1.2 } else { 1.0 }
                };
                let (lx, ly) = (pose.look.0 * 2.4, pose.look.1 * 1.8);
                p.fill_round_rect(
                    ((x + lx - 3.0) * s, (cy + ly - h / 2.0) * s, (x + lx + 3.0) * s, (cy + ly + h / 2.0) * s),
                    3.0 * s,
                    c(tint, 1.0),
                )?;
            }
        }
    }
    zzz(p, s, pose)
}

fn draw_cat(p: &Painter, s: f32, pose: &Pose) -> Result<()> {
    let (cx, cy) = (20.0 + pose.shake(), 17.0 + pose.bob());
    let fur = (0.97, 0.97, 0.97);
    let pink = (0.98, 0.66, 0.72);
    let wiggle = if pose.reaction == Reaction::Music { (pose.t * 12.0).sin() * 1.0 } else { 0.0 };
    for sign in [-1.0f32, 1.0] {
        let ear =
            [(cx + sign * 10.5, cy - 5.0), (cx + sign * 8.5 + wiggle * sign, cy - 14.0), (cx + sign * 3.0, cy - 8.5)];
        p.fill_triangle(
            (ear[0].0 * s, ear[0].1 * s),
            (ear[1].0 * s, ear[1].1 * s),
            (ear[2].0 * s, ear[2].1 * s),
            c(fur, 1.0),
        )?;
        let inner = [
            (cx + sign * 9.0, cy - 6.5),
            (cx + sign * 8.0 + wiggle * sign * 0.6, cy - 11.5),
            (cx + sign * 5.0, cy - 8.5),
        ];
        p.fill_triangle(
            (inner[0].0 * s, inner[0].1 * s),
            (inner[1].0 * s, inner[1].1 * s),
            (inner[2].0 * s, inner[2].1 * s),
            c(pink, 0.8),
        )?;
    }
    p.fill_ellipse(cx * s, cy * s, 11.5 * s, 9.2 * s, c(fur, 1.0))?;
    eye(p, s, pose, cx - 4.6, cy - 0.8, 1.5, 2.3, DARK)?;
    eye(p, s, pose, cx + 4.6, cy - 0.8, 1.5, 2.3, DARK)?;
    p.fill_triangle(
        ((cx - 1.4) * s, (cy + 2.6) * s),
        ((cx + 1.4) * s, (cy + 2.6) * s),
        (cx * s, (cy + 4.0) * s),
        c(pink, 1.0),
    )?;
    for sign in [-1.0f32, 1.0] {
        p.line(
            ((cx + sign * 5.0) * s, (cy + 3.4) * s),
            ((cx + sign * 11.0) * s, (cy + 2.4) * s),
            0.8 * s,
            c((0.7, 0.7, 0.72), 0.8),
        )?;
        p.line(
            ((cx + sign * 5.0) * s, (cy + 4.4) * s),
            ((cx + sign * 10.5) * s, (cy + 5.6) * s),
            0.8 * s,
            c((0.7, 0.7, 0.72), 0.8),
        )?;
    }
    zzz(p, s, pose)
}

fn draw_robot(p: &Painter, s: f32, pose: &Pose) -> Result<()> {
    let (cx, cy) = (20.0 + pose.shake(), 17.0 + pose.bob());
    let metal = (0.66, 0.69, 0.75);
    p.fill_round_rect(((cx - 11.0) * s, (cy - 8.0) * s, (cx + 11.0) * s, (cy + 8.0) * s), 4.5 * s, c(metal, 1.0))?;
    p.fill_round_rect(
        ((cx - 8.6) * s, (cy - 5.6) * s, (cx + 8.6) * s, (cy + 5.6) * s),
        3.0 * s,
        c((0.14, 0.16, 0.21), 1.0),
    )?;
    for sign in [-1.0f32, 1.0] {
        p.fill_round_rect(
            ((cx + sign * 11.0 - 1.0) * s, (cy - 2.5) * s, (cx + sign * 11.0 + 1.0) * s, (cy + 2.5) * s),
            0.8 * s,
            c(metal, 1.0),
        )?;
    }
    let beam = match pose.reaction {
        Reaction::Attention => (1.0, 0.35, 0.3),
        Reaction::Done => (0.5, 0.95, 0.6),
        Reaction::Sleep => (0.25, 0.4, 0.38),
        _ => (0.35, 0.92, 0.7),
    };
    for sign in [-1.0f32, 1.0] {
        let x = cx + sign * 4.4 + pose.look.0 * 1.2;
        let y = cy - 0.6 + pose.look.1 * 0.9;
        if pose.blink || pose.reaction == Reaction::Sleep {
            p.line(((x - 1.9) * s, y * s), ((x + 1.9) * s, y * s), 1.5 * s, c(beam, 0.9))?;
        } else {
            p.fill_round_rect(((x - 2.0) * s, (y - 2.4) * s, (x + 2.0) * s, (y + 2.4) * s), 1.2 * s, c(beam, 1.0))?;
        }
    }
    p.line((cx * s, (cy - 8.0) * s), (cx * s, (cy - 12.5) * s), 1.3 * s, c(metal, 1.0))?;
    let tip = if pose.reaction == Reaction::Music { pose.mood(WHITE) } else { (1.0, 0.72, 0.28) };
    let glow = if pose.reaction == Reaction::Music { 1.8 + pose.beat * 1.4 } else { 1.8 };
    p.fill_ellipse(cx * s, (cy - 13.2) * s, glow * s, glow * s, c(tip, 1.0))
}

fn draw_blob(p: &Painter, s: f32, pose: &Pose) -> Result<()> {
    let beat = if pose.reaction == Reaction::Music { pose.beat } else { 0.0 };
    let (cx, cy) = (20.0 + pose.shake(), 17.0 + pose.bob());
    let (rx, ry) = (11.0 * (1.0 + 0.09 * beat), 9.2 * (1.0 - 0.13 * beat));
    let skin = match pose.reaction {
        Reaction::Attention => (1.0, 0.55, 0.5),
        Reaction::Done => (0.7, 0.92, 0.6),
        Reaction::Sleep => (0.76, 0.62, 0.7),
        _ => (0.97, 0.63, 0.68),
    };
    p.fill_ellipse(cx * s, cy * s, rx * s, ry * s, c(skin, 1.0))?;
    p.fill_ellipse((cx - 4.5) * s, (cy - 4.2) * s, 3.6 * s, 2.0 * s, c(WHITE, 0.35))?;
    for sign in [-1.0f32, 1.0] {
        p.fill_ellipse((cx + sign * 7.0) * s, (cy + 3.2) * s, 2.3 * s, 1.5 * s, c((0.93, 0.4, 0.5), 0.55))?;
    }
    eye(p, s, pose, cx - 4.2, cy - 0.6, 1.5, 2.4, DARK)?;
    eye(p, s, pose, cx + 4.2, cy - 0.6, 1.5, 2.4, DARK)?;
    p.fill_ellipse(cx * s, (cy + 3.6) * s, 1.3 * s, 0.9 * s, c((0.55, 0.2, 0.3), 0.9))?;
    zzz(p, s, pose)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stepping_wraps_in_both_directions() {
        assert_eq!(Character::None.step(false), Character::Blob);
        assert_eq!(Character::Blob.step(true), Character::None);
        assert_eq!(Character::Pixel.step(true), Character::Spark);
        assert_eq!(Character::Pixel.step(false), Character::None);
    }

    #[test]
    fn every_character_has_a_name_and_round_trips_through_json() {
        for ch in Character::ALL {
            assert!(!ch.name().is_empty());
            let json = serde_json::to_string(&ch).unwrap();
            assert_eq!(serde_json::from_str::<Character>(&json).unwrap(), ch);
        }
        assert_eq!(serde_json::to_string(&Character::Spark).unwrap(), "\"spark\"");
    }
}
