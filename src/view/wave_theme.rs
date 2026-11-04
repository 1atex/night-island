use crate::services::yandex::palette::{Palette, Rgb};
use crate::ui::gfx::rgba;
use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct WaveColours {
    pub background: Rgb,
    pub card: Rgb,
    pub accent: Rgb,
    pub text: Rgb,
}

impl WaveColours {
    pub fn new(palette: Palette, fallback: Rgb) -> Self {
        let average = palette.average.unwrap_or(fallback);
        let background = dim(average, 50.0);
        let card = dim(palette.mini_player.unwrap_or(average), 74.0);
        let text = readable(palette.wave_text.unwrap_or((255, 255, 255)), card, 4.5);
        Self { background, card, accent: readable(palette.accent.unwrap_or(fallback), card, 3.0), text }
    }

    pub fn blend(self, other: Self, t: f32) -> Self {
        let mut result = Self {
            background: mix(self.background, other.background, t),
            card: mix(self.card, other.card, t),
            accent: mix(self.accent, other.accent, t),
            text: mix(self.text, other.text, t),
        };
        result.text = readable(result.text, result.card, 4.5);
        result.accent = readable(result.accent, result.card, 3.0);
        result
    }
}

pub fn wave_colour(rgb: Rgb, alpha: f32) -> D2D1_COLOR_F {
    rgba(rgb.0 as f32 / 255.0, rgb.1 as f32 / 255.0, rgb.2 as f32 / 255.0, alpha)
}

fn dim(rgb: Rgb, max: f32) -> Rgb {
    let factor = (max / f32::from(rgb.0.max(rgb.1).max(rgb.2).max(1))).min(1.0);
    ((rgb.0 as f32 * factor) as u8, (rgb.1 as f32 * factor) as u8, (rgb.2 as f32 * factor) as u8)
}

fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let blend = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t.clamp(0.0, 1.0)).round() as u8;
    (blend(a.0, b.0), blend(a.1, b.1), blend(a.2, b.2))
}

fn luminance(rgb: Rgb) -> f32 {
    let channel = |c: u8| {
        let c = c as f32 / 255.0;
        if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    channel(rgb.0) * 0.2126 + channel(rgb.1) * 0.7152 + channel(rgb.2) * 0.0722
}

fn contrast(a: Rgb, b: Rgb) -> f32 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

fn readable(fg: Rgb, bg: Rgb, ratio: f32) -> Rgb {
    (0..=10)
        .map(|i| mix(fg, (255, 255, 255), i as f32 / 10.0))
        .find(|c| contrast(*c, bg) >= ratio)
        .unwrap_or((255, 255, 255))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn light_dark_and_saturated_palettes_keep_text_readable_through_transitions() {
        let samples = [(255, 255, 255), (0, 0, 0), (240, 20, 100), (0, 255, 20), (10, 20, 255)];
        for a in samples {
            for b in samples {
                let make = |c| {
                    WaveColours::new(
                        Palette { average: Some(c), mini_player: Some(c), wave_text: Some(c), accent: Some(c) },
                        c,
                    )
                };
                for i in 0..=20 {
                    let c = make(a).blend(make(b), i as f32 / 20.0);
                    assert!(contrast(c.text, c.card) >= 4.5);
                    assert!(contrast(c.accent, c.card) >= 3.0);
                }
            }
        }
    }
}
