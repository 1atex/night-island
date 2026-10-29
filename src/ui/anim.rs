use crate::data::settings::{AnimCurve, TabAnim};

pub fn bezier_points(curve: AnimCurve) -> Option<(f32, f32, f32, f32)> {
    match curve {
        AnimCurve::Spring => Some((0.34, 1.45, 0.64, 1.0)),
        AnimCurve::Smooth => Some((0.45, 0.0, 0.55, 1.0)),
        AnimCurve::Out => Some((0.16, 1.0, 0.3, 1.0)),
        AnimCurve::Linear => None,
    }
}

pub fn ease(curve: AnimCurve, t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let Some((x1, y1, x2, y2)) = bezier_points(curve) else { return t };
    let at = |a: f32, b: f32, s: f32| 3.0 * a * (1.0 - s) * (1.0 - s) * s + 3.0 * b * (1.0 - s) * s * s + s * s * s;
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for _ in 0..24 {
        let mid = (lo + hi) / 2.0;
        if at(x1, x2, mid) < t {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    at(y1, y2, (lo + hi) / 2.0)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub alpha: f32,
    pub dx: f32,
    pub dy: f32,
    pub scale: f32,
}

pub fn tab_looks(kind: TabAnim, p: f32, dir: f32) -> (Look, Look) {
    let rest = Look { alpha: 1.0, dx: 0.0, dy: 0.0, scale: 1.0 };
    let q = p.clamp(0.0, 1.0);
    let fade_in = q;
    let fade_out = (1.0 - q * 1.6).clamp(0.0, 1.0);
    match kind {
        TabAnim::None => (Look { alpha: 0.0, ..rest }, rest),
        TabAnim::Fade => (Look { alpha: fade_out, ..rest }, Look { alpha: fade_in, ..rest }),
        TabAnim::Slide => (
            Look { alpha: fade_out, dx: -dir * 0.10 * p, ..rest },
            Look { alpha: fade_in, dx: dir * 0.10 * (1.0 - p), ..rest },
        ),
        TabAnim::Zoom => (
            Look { alpha: fade_out, scale: 1.0 + 0.05 * p, ..rest },
            Look { alpha: fade_in, scale: 0.94 + 0.06 * p, ..rest },
        ),
        TabAnim::Rise => {
            (Look { alpha: fade_out, dy: -0.04 * p, ..rest }, Look { alpha: fade_in, dy: 0.08 * (1.0 - p), ..rest })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curves_start_at_zero_and_end_at_one() {
        for c in [AnimCurve::Spring, AnimCurve::Smooth, AnimCurve::Out, AnimCurve::Linear] {
            assert!(ease(c, 0.0).abs() < 1e-3);
            assert!((ease(c, 1.0) - 1.0).abs() < 1e-3);
        }
        assert!((ease(AnimCurve::Linear, 0.25) - 0.25).abs() < 1e-6);
        assert!(ease(AnimCurve::Spring, 0.6) > 1.0, "the spring overshoots");
    }

    #[test]
    fn a_finished_change_shows_only_the_new_tab() {
        for k in [TabAnim::Fade, TabAnim::Slide, TabAnim::Zoom, TabAnim::Rise] {
            let (old, new) = tab_looks(k, 1.0, 1.0);
            assert_eq!(old.alpha, 0.0);
            assert_eq!(new, Look { alpha: 1.0, dx: 0.0, dy: 0.0, scale: 1.0 });
        }
    }
}
