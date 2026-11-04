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
    Bear,
    Custom,
}

impl Character {
    pub const ALL: [Character; 9] = [
        Character::None,
        Character::Pixel,
        Character::Spark,
        Character::Eyes,
        Character::Cat,
        Character::Robot,
        Character::Blob,
        Character::Bear,
        Character::Custom,
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
            Character::Bear => "Bear",
            Character::Custom => "Custom pet",
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
    pub motion: super::pet_motion::Motion,
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
            Reaction::Idle => (t * 1.8).sin() * 0.15,
            Reaction::Music => -self.beat * 1.0 + (t * 9.0).sin() * 0.4,
            Reaction::Done => -(t * 5.0).sin().abs() * 1.1,
            Reaction::Attention => 0.0,
            Reaction::File => (t * 7.0).sin() * 1.0,
            Reaction::Sleep => (t * 1.0).sin() * 0.45,
        }
    }

    fn shake(&self) -> f32 {
        if self.reaction == Reaction::Attention { (self.t * 8.0).sin() * 0.5 } else { 0.0 }
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

#[cfg(test)]
pub fn draw(p: &Painter, character: Character, w: f32, pose: &Pose) -> Result<()> {
    draw_with_design(p, character, w, pose, None)
}

pub fn draw_with_design(
    p: &Painter,
    character: Character,
    w: f32,
    pose: &Pose,
    design: Option<&super::pet_design::PetDesign>,
) -> Result<()> {
    let s = w / BOX_W;
    let mut pose = *pose;
    if pose.motion.still {
        pose.t = 0.0;
        pose.beat = 0.0;
        pose.look = (0.0, 0.0);
        pose.blink = false;
        pose.motion = super::pet_motion::Motion { still: true, ..Default::default() };
    }
    let breathing = if pose.motion.still { 0.0 } else { (pose.t * 1.65).sin() * 0.015 };
    let (sx, sy) = (1.0 + breathing - pose.motion.stretch * 0.45, 1.0 - breathing + pose.motion.stretch);
    let angle = pose.motion.lean;
    let (sin, cos) = angle.sin_cos();
    let (cx, cy) = (20.0 * s, 24.0 * s);
    let mut transform = windows_numerics::Matrix3x2 {
        M11: sx * cos,
        M12: sx * sin,
        M21: -sy * sin,
        M22: sy * cos,
        M31: cx - cx * sx * cos + cy * sy * sin,
        M32: cy - cx * sx * sin - cy * sy * cos,
    };
    let (left, top, right, bottom) = drawing_bounds(character, &pose, design);
    let mut bounds = (f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
    for (x, y) in [(left, top), (right, top), (left, bottom), (right, bottom)] {
        let px = x * s * transform.M11 + y * s * transform.M21 + transform.M31;
        let py = x * s * transform.M12 + y * s * transform.M22 + transform.M32;
        bounds = (bounds.0.min(px), bounds.1.min(py), bounds.2.max(px), bounds.3.max(py));
    }
    let fit = (39.0 * s / (bounds.2 - bounds.0)).min(29.0 * s / (bounds.3 - bounds.1)).min(1.0);
    transform = transform * windows_numerics::Matrix3x2::scale(fit, fit);
    transform.M31 += (0.5 * s - bounds.0 * fit).max(0.0) + (39.5 * s - bounds.2 * fit).min(0.0);
    transform.M32 += (0.5 * s - bounds.1 * fit).max(0.0) + (29.5 * s - bounds.3 * fit).min(0.0);
    let old = p.push_offset(0.0, 0.0);
    p.set_transform(&(transform * old));
    let result = match character {
        Character::None => Ok(()),
        Character::Pixel => draw_pixel(p, s, &pose),
        Character::Spark => draw_spark(p, s, &pose),
        Character::Eyes => draw_eyes(p, s, &pose),
        Character::Cat => draw_cat(p, s, &pose),
        Character::Robot => draw_robot(p, s, &pose),
        Character::Blob => draw_blob(p, s, &pose),
        Character::Bear => draw_designed(p, s, &pose, super::pet_design::PetDesign::bear()),
        Character::Custom => draw_designed(p, s, &pose, design.unwrap_or_else(|| super::pet_design::PetDesign::bear())),
    };
    p.set_transform(&old);
    result?;
    if pose.motion.warmth > 0.015 && character != Character::None {
        let alpha = pose.motion.warmth * 0.8;
        let (x, y) = (32.0 * s, (3.5 + (pose.t * 2.0).sin() * 0.6) * s);
        p.fill_ellipse(x - s, y, 1.2 * s, 1.2 * s, rgba(1.0, 0.56, 0.62, alpha))?;
        p.fill_ellipse(x + s, y, 1.2 * s, 1.2 * s, rgba(1.0, 0.56, 0.62, alpha))?;
        p.fill_triangle((x - 2.0 * s, y), (x + 2.0 * s, y), (x, y + 2.8 * s), rgba(1.0, 0.56, 0.62, alpha))?;
    }
    Ok(())
}

fn drawing_bounds(
    character: Character,
    pose: &Pose,
    design: Option<&super::pet_design::PetDesign>,
) -> (f32, f32, f32, f32) {
    use super::pet_design::PetBody;
    let (cx, cy) = (20.0 + pose.shake(), 17.0 + pose.bob());
    let mut bounds = match character {
        Character::Spark => {
            let radius = 11.6 + if pose.reaction == Reaction::Music { pose.beat * 0.9 } else { 0.0 };
            (cx - radius, cy - 2.0 - radius, cx + radius, cy - 2.0 + radius)
        }
        Character::Cat => (cx - 11.5, cy - 14.0, cx + 11.5, cy + 9.2),
        Character::Robot => {
            let glow = if pose.reaction == Reaction::Music { 1.8 + pose.beat * 1.4 } else { 1.8 };
            (cx - 12.0, cy - 13.2 - glow, cx + 12.0, cy + 8.0)
        }
        Character::Bear | Character::Custom => {
            let default = super::pet_design::PetDesign::bear();
            let pet = if character == Character::Bear { default } else { design.unwrap_or(default) };
            let (cx, cy) = (20.0 + pose.look.0 * 0.4, 13.6 + pose.bob() * 0.5);
            let width = 9.4 * pet.width;
            let half = width.max(width * 0.76 + 5.2 * pet.ear_size);
            let top = match pet.body {
                PetBody::Bear => cy - 6.8 - 4.0 * pet.ear_size - 0.35,
                PetBody::Cat | PetBody::Fox => (cy - 6.8 - 4.5 * pet.ear_size - 0.35).max(0.8),
                PetBody::Rabbit => cy - 7.6 - 5.0 * pet.ear_size - 0.35,
                PetBody::Robot => cy - 10.6 - 0.35,
                PetBody::Ghost => cy - 7.9,
            };
            (cx - half, top, cx + half.max(if pet.tail { width + 5.9 } else { 0.0 }), cy + 14.1)
        }
        _ => (5.0, 3.0 + pose.bob(), 35.0, 28.0 + pose.bob()),
    };
    if pose.reaction == Reaction::Sleep {
        bounds = (bounds.0.min(28.0), bounds.1.min(-0.2), bounds.2.max(40.0), bounds.3.max(12.2));
    }
    bounds
}

#[allow(clippy::too_many_arguments)]
fn eye(p: &Painter, s: f32, pose: &Pose, x: f32, y: f32, rx: f32, ry: f32, color: Rgb) -> Result<()> {
    let (lx, ly) = (pose.look.0 * 1.4, pose.look.1 * 1.0);
    let ry = ry * (1.0 - pose.motion.lid * 0.94);
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
                    12.0 * squash
                        * (1.0 - pose.motion.lid * 0.94)
                        * if pose.reaction == Reaction::Attention { 1.2 } else { 1.0 }
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
    let wiggle = (pose.t * 2.4).sin() * (0.25 + pose.motion.warmth * 0.55) + pose.look.0 * 0.3;
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
            p.fill_round_rect(
                (
                    (x - 2.0) * s,
                    (y - 2.4 * (1.0 - pose.motion.lid * 0.94)) * s,
                    (x + 2.0) * s,
                    (y + 2.4 * (1.0 - pose.motion.lid * 0.94)) * s,
                ),
                1.2 * s,
                c(beam, 1.0),
            )?;
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

fn draw_designed(p: &Painter, s: f32, pose: &Pose, design: &super::pet_design::PetDesign) -> Result<()> {
    use super::pet_design::{PetAccessory, PetBody, PetPattern};
    let rgb = |value: [u8; 3]| (value[0] as f32 / 255.0, value[1] as f32 / 255.0, value[2] as f32 / 255.0);
    let (fur, belly, eyes, accent) = (rgb(design.fur), rgb(design.belly), rgb(design.eyes), rgb(design.accent));
    let (cx, cy) = (20.0 + pose.look.0 * 0.4, 13.6 + pose.bob() * 0.5);
    let width = 9.4 * design.width;
    let breathe = (pose.t * 1.65).sin();
    if design.tail {
        let wag = (pose.t * (3.5 + pose.motion.warmth * 3.0)).sin() * 1.2;
        p.fill_ellipse((cx + width + 2.5) * s, (cy + 8.0 + wag) * s, 3.5 * s, 2.3 * s, c(fur, 1.0))?;
        p.fill_ellipse((cx + width + 4.4) * s, (cy + 7.4 + wag) * s, 1.5 * s, 1.5 * s, c(belly, 1.0))?;
    }
    for sign in [-1.0, 1.0] {
        let ex = cx + sign * width * 0.76;
        let ey = cy - 6.8 + sign * pose.look.0 * 0.35;
        match design.body {
            PetBody::Bear => {
                p.fill_ellipse(ex * s, ey * s, 4.1 * design.ear_size * s, 4.0 * design.ear_size * s, c(fur, 1.0))?;
                p.fill_ellipse(ex * s, ey * s, 2.3 * design.ear_size * s, 2.2 * design.ear_size * s, c(accent, 0.8))?;
            }
            PetBody::Cat | PetBody::Fox => {
                let tip = (ey - 4.5 * design.ear_size).max(0.8);
                p.fill_triangle(
                    ((ex - 3.6) * s, (ey + 4.0) * s),
                    ((ex + sign * 1.1) * s, tip * s),
                    ((ex + 3.6) * s, (ey + 4.0) * s),
                    c(fur, 1.0),
                )?;
                p.fill_triangle(
                    ((ex - 2.0) * s, (ey + 2.3) * s),
                    ((ex + sign * 0.7) * s, (tip + 2.0) * s),
                    ((ex + 2.0) * s, (ey + 2.3) * s),
                    c(accent, 0.85),
                )?;
            }
            PetBody::Rabbit => {
                p.fill_ellipse(ex * s, (ey - 0.8) * s, 2.3 * s, 5.0 * design.ear_size * s, c(fur, 1.0))?;
                p.fill_ellipse(ex * s, (ey - 1.2) * s, 1.0 * s, 3.3 * design.ear_size * s, c(accent, 0.85))?;
            }
            PetBody::Robot => {
                p.line((ex * s, ey * s), (ex * s, (ey - 2.0) * s), s, c(belly, 1.0))?;
                p.fill_ellipse(ex * s, (ey - 2.5) * s, 1.3 * s, 1.3 * s, c(accent, 1.0))?;
            }
            PetBody::Ghost => {}
        }
    }
    if design.body == PetBody::Ghost {
        p.fill_round_rect(((cx - width) * s, cy * s, (cx + width) * s, (cy + 9.0) * s), 3.5 * s, c(fur, 1.0))?;
        for offset in [-0.65, 0.0, 0.65] {
            p.fill_ellipse(
                (cx + width * offset) * s,
                (cy + 9.0 + breathe * offset * 0.45) * s,
                width * 0.35 * s,
                3.2 * s,
                c(fur, 1.0),
            )?;
        }
    } else {
        p.fill_ellipse(cx * s, (cy + 7.3) * s, (width * 0.78) * s, (6.8 + breathe * 0.12) * s, c(fur, 1.0))?;
        p.fill_ellipse(cx * s, (cy + 7.5) * s, width * 0.48 * s, 4.5 * s, c(belly, 1.0))?;
        for sign in [-1.0, 1.0] {
            p.fill_ellipse((cx + sign * width * 0.58) * s, (cy + 12.0) * s, 3.0 * s, 1.9 * s, c(fur, 1.0))?;
            let paw = if pose.motion.warmth > 0.2 { (pose.t * 5.0).sin() * pose.motion.warmth } else { breathe * 0.15 };
            p.fill_ellipse((cx + sign * width * 0.84) * s, (cy + 6.0 + paw) * s, 2.1 * s, 3.1 * s, c(fur, 1.0))?;
        }
    }
    if design.body == PetBody::Robot {
        p.fill_round_rect(((cx - width) * s, (cy - 6.6) * s, (cx + width) * s, (cy + 5.6) * s), 3.0 * s, c(fur, 1.0))?;
    } else {
        p.fill_ellipse(cx * s, cy * s, width * s, 7.9 * s, c(fur, 1.0))?;
    }
    match design.pattern {
        PetPattern::Patches => {
            p.fill_ellipse((cx - width * 0.45) * s, (cy - 2.1) * s, 3.2 * s, 4.2 * s, c(belly, 0.75))?
        }
        PetPattern::Spots => {
            for (x, y) in [(-6.5, -3.5), (5.9, -4.0), (7.0, 1.4)] {
                p.fill_ellipse((cx + x) * s, (cy + y) * s, 1.5 * s, 1.2 * s, c(belly, 0.8))?;
            }
        }
        PetPattern::Stripes => {
            for x in [-3.2, 0.0, 3.2] {
                p.line(((cx + x) * s, (cy - 6.4) * s), ((cx + x * 0.65) * s, (cy - 3.5) * s), 1.4 * s, c(belly, 0.85))?;
            }
        }
        PetPattern::Solid => {}
    }
    if !matches!(design.body, PetBody::Ghost | PetBody::Robot) {
        p.fill_ellipse(cx * s, (cy + 3.5) * s, 4.5 * s, 3.0 * s, c(belly, 1.0))?;
    }
    eye(p, s, pose, cx - 3.9, cy - 0.8, 1.05, 1.65, eyes)?;
    eye(p, s, pose, cx + 3.9, cy - 0.8, 1.05, 1.65, eyes)?;
    if design.body == PetBody::Ghost {
        p.fill_ellipse(cx * s, (cy + 4.0) * s, 1.3 * s, (1.2 + pose.motion.warmth * 0.4) * s, c(eyes, 0.9))?;
    } else if design.body == PetBody::Robot {
        p.line(((cx - 2.0) * s, (cy + 3.4) * s), ((cx + 2.0) * s, (cy + 3.4) * s), s, c(accent, 1.0))?;
    } else {
        p.fill_ellipse(cx * s, (cy + 2.8) * s, 1.55 * s, 1.0 * s, c(eyes, 1.0))?;
        for sign in [-1.0, 1.0] {
            p.line((cx * s, (cy + 3.5) * s), ((cx + sign * 1.5) * s, (cy + 4.4) * s), 0.65 * s, c(eyes, 0.8))?;
        }
    }
    for sign in [-1.0, 1.0] {
        p.fill_ellipse(
            (cx + sign * 6.1) * s,
            (cy + 2.3) * s,
            1.8 * s,
            0.9 * s,
            c(accent, 0.32 + pose.motion.warmth * 0.25),
        )?;
    }
    match design.accessory {
        PetAccessory::Scarf => {
            p.fill_round_rect(((cx - 6.5) * s, (cy + 5.8) * s, (cx + 6.5) * s, (cy + 8.0) * s), s, c(accent, 1.0))?;
            p.fill_round_rect(((cx + 2.5) * s, (cy + 7.0) * s, (cx + 5.0) * s, (cy + 11.7) * s), s, c(accent, 1.0))?;
        }
        PetAccessory::Bow => {
            for sign in [-1.0, 1.0] {
                p.fill_triangle(
                    (cx * s, (cy + 7.0) * s),
                    ((cx + sign * 3.3) * s, (cy + 5.5) * s),
                    ((cx + sign * 3.3) * s, (cy + 8.6) * s),
                    c(accent, 1.0),
                )?;
            }
        }
        PetAccessory::Star => {
            p.fill_triangle(
                (cx * s, (cy + 6.6) * s),
                ((cx - 2.5) * s, (cy + 10.4) * s),
                ((cx + 2.5) * s, (cy + 10.4) * s),
                c(accent, 1.0),
            )?;
            p.fill_triangle(
                ((cx - 2.5) * s, (cy + 7.9) * s),
                ((cx + 2.5) * s, (cy + 7.9) * s),
                (cx * s, (cy + 11.3) * s),
                c(accent, 1.0),
            )?;
        }
        PetAccessory::None => {}
    }
    zzz(p, s, pose)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stepping_wraps_in_both_directions() {
        assert_eq!(Character::None.step(false), Character::Custom);
        assert_eq!(Character::Custom.step(true), Character::None);
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

#[cfg(test)]
#[path = "character_render_tests.rs"]
mod render_tests;
