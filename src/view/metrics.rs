use crate::platform::metrics::{Metric, Sample};
use crate::view::*;

pub const HISTORY: usize = 60;

#[derive(Default)]
pub struct MetricsUi {
    pub sample: Option<Sample>,
    pub history: [Vec<f32>; 7],
    pub pill: String,
}

impl MetricsUi {
    pub fn push(&mut self, sample: Sample, pill_keys: &[String]) {
        for (i, m) in Metric::ALL.into_iter().enumerate() {
            let list = &mut self.history[i];
            list.push(m.value(&sample).unwrap_or(0.0));
            if list.len() > HISTORY {
                list.remove(0);
            }
        }
        self.pill = pill_text(&sample, pill_keys);
        self.sample = Some(sample);
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

fn pill_value(m: Metric, s: &Sample) -> String {
    let Some(v) = m.value(s) else { return "  —".into() };
    match m {
        Metric::NetDown | Metric::NetUp => {
            if v < 1_000_000.0 {
                format!("{:>4}K", (v / 1000.0).round() as i32)
            } else {
                format!("{:>4.1}M", v / 1_000_000.0)
            }
        }
        Metric::Uptime => crate::platform::metrics::uptime_text(s.uptime).replace(" min", "m").replace(' ', ""),
        _ => format!("{:>3}%", v.round() as i32),
    }
}

pub fn pill_text(s: &Sample, keys: &[String]) -> String {
    keys.iter()
        .filter_map(|k| Metric::from_key(k))
        .map(|m| format!("{} {}", m.short(), pill_value(m, s)))
        .collect::<Vec<_>>()
        .join("  ")
}

pub fn pill_chars(keys: &[String]) -> usize {
    let one = |m: Metric| match m {
        Metric::NetDown | Metric::NetUp => 1 + 1 + 5,
        Metric::Uptime => 2 + 1 + 6,
        _ => 3 + 1 + 4,
    };
    let metrics: Vec<Metric> = keys.iter().filter_map(|k| Metric::from_key(k)).collect();
    metrics.iter().map(|m| one(*m)).sum::<usize>() + 2 * metrics.len().saturating_sub(1)
}

pub fn pill_metrics_on(cfg: &Settings) -> bool {
    cfg.metrics_pill.iter().any(|k| Metric::from_key(k).is_some())
}

pub(in crate::view) fn paint_metrics_row(p: &Painter, pill: Rect, scale: f32, ui: &UiState) -> Result<()> {
    let (x, y, w, h) = pill;
    let shown: Vec<Metric> = ui.cfg.metrics_tab.iter().filter_map(|k| Metric::from_key(k)).take(4).collect();
    let Some(sample) = ui.metrics.sample.as_ref().filter(|_| !shown.is_empty()) else { return Ok(()) };
    let side = SIDE_PAD * scale;
    let gap = CTL_GAP * scale;
    let top = y + (62.0 + CTL_BIG_H + CTL_GAP + CTL_SMALL_H + CTL_GAP) * scale;
    let bottom = y + h - 14.0 * scale;
    if bottom - top < 40.0 * scale {
        return Ok(());
    }
    let tile_w = (w - 2.0 * side - 3.0 * gap) / 4.0;
    for (slot, m) in shown.iter().enumerate() {
        let left = x + side + slot as f32 * (tile_w + gap);
        let t = (left, top, left + tile_w, bottom.min(top + 66.0 * scale));
        p.fill_round_rect(t, 16.0 * scale, rgba(1.0, 1.0, 1.0, 0.06))?;
        let value = m.value(sample).unwrap_or(0.0);
        let color = if m.is_percent() && value >= 90.0 {
            rgba(1.0, 0.42, 0.38, 1.0)
        } else if m.is_percent() && value >= 70.0 {
            rgba(1.0, 0.75, 0.3, 1.0)
        } else {
            let (r, g, b) = crate::ui::theme::accent_u8();
            rgba(f32::from(r) / 255.0, f32::from(g) / 255.0, f32::from(b) / 255.0, 1.0)
        };
        let pad = 12.0 * scale;
        p.text(
            m.label(),
            (t.0 + pad, t.1 + 6.0 * scale, t.2 - pad, t.1 + 24.0 * scale),
            11.5 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            rgba(1.0, 1.0, 1.0, 0.55),
        )?;
        p.text(
            &m.text(sample),
            (t.0 + pad, t.1 + 22.0 * scale, t.2 - pad, t.1 + 46.0 * scale),
            18.0 * scale,
            DWRITE_FONT_WEIGHT_BOLD,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            WHITE,
        )?;
        if let Some(detail) = m.detail(sample) {
            p.text(
                &detail,
                (t.0 + pad, t.1 + 44.0 * scale, t.2 - pad, t.1 + 62.0 * scale),
                10.5 * scale,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                rgba(1.0, 1.0, 1.0, 0.45),
            )?;
        } else if m.is_percent() {
            let bar = (t.0 + pad, t.3 - 10.0 * scale, t.2 - pad, t.3 - 6.0 * scale);
            p.fill_round_rect(bar, 2.0 * scale, rgba(1.0, 1.0, 1.0, 0.1))?;
            let fill = bar.0 + (bar.2 - bar.0) * (value / 100.0).clamp(0.0, 1.0);
            p.fill_round_rect((bar.0, bar.1, fill.max(bar.0 + 2.0 * scale), bar.3), 2.0 * scale, color)?;
        }
    }
    Ok(())
}
