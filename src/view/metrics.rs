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

const TILE_H: f32 = 70.0;
const TILE_GAP: f32 = 8.0;
const TOP: f32 = 62.0;

fn tile(pill: Rect, scale: f32, slot: usize) -> Rect {
    let (x, y, w, _) = pill;
    let side = (SIDE_PAD - 4.0) * scale;
    let gap = TILE_GAP * scale;
    let width = (w - 2.0 * side - gap) / 2.0;
    let (col, row) = ((slot % 2) as f32, (slot / 2) as f32);
    let left = x + side + col * (width + gap);
    let top = y + TOP * scale + row * (TILE_H * scale + gap);
    (left, top, left + width, top + TILE_H * scale)
}

fn paint_graph(p: &Painter, area: Rect, scale: f32, values: &[f32], metric: Metric, color: D2D1Color) -> Result<()> {
    let ceiling = if metric.is_percent() { 100.0 } else { values.iter().copied().fold(1000.0, f32::max) };
    let step = (area.2 - area.0) / HISTORY as f32;
    let first = HISTORY - values.len().min(HISTORY);
    for (i, v) in values.iter().enumerate() {
        let h = ((v / ceiling).clamp(0.0, 1.0) * (area.3 - area.1)).max(1.0 * scale);
        let left = area.0 + (first + i) as f32 * step;
        p.fill_rect((left, area.3 - h, left + (step - 0.6 * scale).max(1.0), area.3), color)?;
    }
    Ok(())
}

type D2D1Color = windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;

pub(in crate::view) fn paint_system(p: &Painter, pill: Rect, scale: f32, ui: &UiState) -> Result<()> {
    let (x, y, w, _) = pill;
    let shown: Vec<Metric> = ui.cfg.metrics_tab.iter().filter_map(|k| Metric::from_key(k)).collect();
    let Some(sample) = &ui.metrics.sample else {
        return p.text(
            if shown.is_empty() { "No figures chosen: pick some in Settings → System" } else { "Reading…" },
            (x, y + 120.0 * scale, x + w, y + 280.0 * scale),
            14.0 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_CENTER,
            rgba(1.0, 1.0, 1.0, 0.5),
        );
    };
    if shown.is_empty() {
        return p.text(
            "No figures chosen: pick some in Settings → System",
            (x, y + 120.0 * scale, x + w, y + 280.0 * scale),
            14.0 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_CENTER,
            rgba(1.0, 1.0, 1.0, 0.5),
        );
    }
    for (slot, m) in shown.iter().take(8).enumerate() {
        let t = tile(pill, scale, slot);
        p.fill_round_rect(t, 16.0 * scale, rgba(1.0, 1.0, 1.0, 0.06))?;
        let index = Metric::ALL.iter().position(|a| a == m).unwrap_or(0);
        let value = m.value(sample).unwrap_or(0.0);
        let hot = m.is_percent() && value >= 90.0;
        let warm = m.is_percent() && value >= 70.0;
        let color = if hot {
            rgba(1.0, 0.42, 0.38, 1.0)
        } else if warm {
            rgba(1.0, 0.75, 0.3, 1.0)
        } else {
            let (r, g, b) = crate::ui::theme::accent_u8();
            rgba(f32::from(r) / 255.0, f32::from(g) / 255.0, f32::from(b) / 255.0, 1.0)
        };
        let pad = 14.0 * scale;
        p.text(
            m.label(),
            (t.0 + pad, t.1 + 8.0 * scale, t.0 + 140.0 * scale, t.1 + 26.0 * scale),
            11.5 * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            rgba(1.0, 1.0, 1.0, 0.55),
        )?;
        p.text(
            &m.text(sample),
            (t.0 + pad, t.1 + 24.0 * scale, t.0 + 150.0 * scale, t.1 + 52.0 * scale),
            22.0 * scale,
            DWRITE_FONT_WEIGHT_BOLD,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            WHITE,
        )?;
        if let Some(detail) = m.detail(sample) {
            p.text(
                &detail,
                (t.0 + pad, t.1 + 50.0 * scale, t.0 + 150.0 * scale, t.1 + 66.0 * scale),
                11.0 * scale,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                rgba(1.0, 1.0, 1.0, 0.45),
            )?;
        }
        if *m != Metric::Uptime {
            let graph = (t.0 + 150.0 * scale, t.1 + 12.0 * scale, t.2 - pad, t.3 - 12.0 * scale);
            paint_graph(p, graph, scale, &ui.metrics.history[index], *m, color)?;
        }
    }
    Ok(())
}
