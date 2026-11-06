use windows::Win32::Graphics::DirectWrite::{DWRITE_FONT_WEIGHT_BOLD, DWRITE_TEXT_ALIGNMENT_CENTER};
use windows::core::Result;

use super::{BOX_H, BOX_W, Character, DARK, Pose, Reaction, Rgb, WHITE, c};
use crate::ui::gfx::Painter;

pub(super) fn draw(p: &Painter, s: f32, pose: &Pose, character: Character) -> Result<()> {
    match character {
        Character::PixelOld => draw_pixel(p, s, pose),
        Character::SparkOld => draw_spark(p, s, pose),
        Character::EyesOld => draw_eyes(p, s, pose),
        Character::CatOld => draw_cat(p, s, pose),
        Character::RobotOld => draw_robot(p, s, pose),
        _ => draw_blob(p, s, pose),
    }
}

fn bob(pose: &Pose) -> f32 {
    let t = pose.t;
    match pose.reaction {
        Reaction::Idle => (t * 1.8).sin() * 0.5,
        Reaction::Music => -pose.beat * 1.0 + (t * 9.0).sin() * 0.4,
        Reaction::Done => -(t * 10.0).sin().abs() * 3.2,
        Reaction::Attention => 0.0,
        Reaction::File => (t * 7.0).sin() * 1.0,
        Reaction::Sleep => (t * 1.0).sin() * 0.45,
    }
}

fn shake(pose: &Pose) -> f32 {
    if pose.reaction == Reaction::Attention { (pose.t * 46.0).sin() * 1.3 } else { 0.0 }
}

fn mood(pose: &Pose, calm: Rgb) -> Rgb {
    match pose.reaction {
        Reaction::Idle => calm,
        Reaction::Music => pose.accent,
        Reaction::Done => (0.47, 0.9, 0.55),
        Reaction::Attention => (1.0, 0.55, 0.28),
        Reaction::File => (0.5, 0.75, 1.0),
        Reaction::Sleep => (0.58, 0.63, 0.78),
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
    let (ox, oy) = ((BOX_W - 22.0) / 2.0 * s + shake(pose) * s, (BOX_H - 16.0) / 2.0 * s + bob(pose) * s);
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
    let tint = mood(pose, WHITE);
    let pulse = if pose.reaction == Reaction::Music { pose.beat * 0.9 } else { 0.0 };
    let r = 8.6 + pulse;
    let (cx, cy) = (20.0 + shake(pose), 15.0 + bob(pose));
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
    let (cy, dx) = (15.0 + bob(pose) * 0.5, shake(pose));
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
    let (cx, cy) = (20.0 + shake(pose), 17.0 + bob(pose));
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
    let (cx, cy) = (20.0 + shake(pose), 17.0 + bob(pose));
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
    let tip = if pose.reaction == Reaction::Music { mood(pose, WHITE) } else { (1.0, 0.72, 0.28) };
    let glow = if pose.reaction == Reaction::Music { 1.8 + pose.beat * 1.4 } else { 1.8 };
    p.fill_ellipse(cx * s, (cy - 13.2) * s, glow * s, glow * s, c(tip, 1.0))
}

fn draw_blob(p: &Painter, s: f32, pose: &Pose) -> Result<()> {
    let beat = if pose.reaction == Reaction::Music { pose.beat } else { 0.0 };
    let (cx, cy) = (20.0 + shake(pose), 17.0 + bob(pose));
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
