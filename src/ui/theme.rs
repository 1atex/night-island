use std::sync::atomic::{AtomicU32, Ordering};

use crate::data::settings::Settings;

static PANEL: [AtomicU32; 3] = [AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0)];
static ACCENT: [AtomicU32; 3] = [AtomicU32::new(0x3F266666), AtomicU32::new(0x3F333333), AtomicU32::new(0x3F800000)];

fn load(a: &[AtomicU32; 3]) -> (f32, f32, f32) {
    (
        f32::from_bits(a[0].load(Ordering::Relaxed)),
        f32::from_bits(a[1].load(Ordering::Relaxed)),
        f32::from_bits(a[2].load(Ordering::Relaxed)),
    )
}

fn store(a: &[AtomicU32; 3], c: (f32, f32, f32)) {
    a[0].store(c.0.to_bits(), Ordering::Relaxed);
    a[1].store(c.1.to_bits(), Ordering::Relaxed);
    a[2].store(c.2.to_bits(), Ordering::Relaxed);
}

pub fn set(panel: (f32, f32, f32), accent: (f32, f32, f32)) {
    store(&PANEL, panel);
    store(&ACCENT, accent);
}

pub fn panel() -> (f32, f32, f32) {
    load(&PANEL)
}

pub fn accent() -> (f32, f32, f32) {
    load(&ACCENT)
}

fn to8(c: (f32, f32, f32)) -> (u8, u8, u8) {
    let b = |v: f32| (v * 255.0).round().clamp(0.0, 255.0) as u8;
    (b(c.0), b(c.1), b(c.2))
}

pub fn panel_u8() -> (u8, u8, u8) {
    to8(panel())
}

pub fn accent_u8() -> (u8, u8, u8) {
    to8(accent())
}

pub fn hsv(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    let (r, g, b) = crate::platform::media::hsv_to_rgb(h, s, v);
    (f32::from(r) / 255.0, f32::from(g) / 255.0, f32::from(b) / 255.0)
}

pub fn colors(s: &Settings) -> ((f32, f32, f32), (f32, f32, f32)) {
    (hsv(s.panel_hue, s.panel_sat, s.panel_val), hsv(s.accent_hue, s.accent_sat, s.accent_val))
}

pub const PRESETS: [(&str, [f32; 6]); 8] = [
    ("Night", [0.0, 0.0, 0.0, 231.0, 0.35, 1.0]),
    ("Graphite", [240.0, 0.08, 0.10, 220.0, 0.10, 0.85]),
    ("Ocean", [212.0, 0.90, 0.12, 205.0, 0.65, 1.0]),
    ("Forest", [140.0, 0.80, 0.09, 140.0, 0.50, 0.90]),
    ("Rose", [340.0, 0.80, 0.12, 340.0, 0.50, 1.0]),
    ("Violet", [270.0, 0.85, 0.14, 275.0, 0.50, 1.0]),
    ("Amber", [35.0, 0.90, 0.11, 38.0, 0.70, 1.0]),
    ("Mono", [0.0, 0.0, 0.0, 0.0, 0.0, 1.0]),
];

pub const PANEL_SWATCHES: [[f32; 3]; 10] = [
    [0.0, 0.0, 0.0],
    [0.0, 0.0, 0.07],
    [240.0, 0.12, 0.11],
    [212.0, 0.90, 0.13],
    [180.0, 0.80, 0.10],
    [140.0, 0.80, 0.10],
    [35.0, 0.90, 0.12],
    [0.0, 0.85, 0.13],
    [320.0, 0.70, 0.13],
    [270.0, 0.85, 0.15],
];

pub const ACCENT_SWATCHES: [[f32; 3]; 10] = [
    [231.0, 0.35, 1.0],
    [205.0, 0.65, 1.0],
    [180.0, 0.70, 0.95],
    [140.0, 0.50, 0.90],
    [90.0, 0.65, 0.90],
    [50.0, 0.75, 1.0],
    [30.0, 0.75, 1.0],
    [5.0, 0.70, 1.0],
    [330.0, 0.50, 1.0],
    [275.0, 0.50, 1.0],
];

pub fn set_panel_swatch(s: &mut Settings, i: usize) {
    if let Some(v) = PANEL_SWATCHES.get(i) {
        (s.panel_hue, s.panel_sat, s.panel_val) = (v[0], v[1], v[2]);
    }
}

pub fn set_accent_swatch(s: &mut Settings, i: usize) {
    if let Some(v) = ACCENT_SWATCHES.get(i) {
        (s.accent_hue, s.accent_sat, s.accent_val) = (v[0], v[1], v[2]);
    }
}

pub fn apply_preset(s: &mut Settings, index: usize) {
    if let Some((_, v)) = PRESETS.get(index) {
        (s.panel_hue, s.panel_sat, s.panel_val) = (v[0], v[1], v[2]);
        (s.accent_hue, s.accent_sat, s.accent_val) = (v[3], v[4], v[5]);
    }
}

pub fn preset_colors(index: usize) -> ((f32, f32, f32), (f32, f32, f32)) {
    let v = PRESETS.get(index).map_or(PRESETS[0].1, |p| p.1);
    (hsv(v[0], v[1], v[2]), hsv(v[3], v[4], v[5]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_look_is_black_and_periwinkle() {
        let (panel, accent) = colors(&Settings::default());
        assert_eq!(to8(panel), (0, 0, 0));
        let (r, g, b) = to8(accent);
        assert!(r.abs_diff(166) <= 3 && g.abs_diff(179) <= 3 && b == 255, "{r},{g},{b}");
    }

    #[test]
    fn presets_stay_dark_enough_for_white_text() {
        for (name, v) in PRESETS {
            let (r, g, b) = hsv(v[0], v[1], v[2]);
            assert!(0.2126 * r + 0.7152 * g + 0.0722 * b < 0.2, "{name} is too bright");
        }
    }
}
