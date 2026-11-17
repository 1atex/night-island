use windows::UI::Composition::*;
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_BEZIER_SEGMENT, D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_END_CLOSED,
};
use windows::Win32::Graphics::Direct2D::{ID2D1Factory, ID2D1GeometrySink};
use windows::core::Result;
use windows_numerics::Vector2;

use super::winter_model::Drift;

pub(super) struct Cap {
    pub id: isize,
    pub shape: CompositionSpriteShape,
    geometry: CompositionPathGeometry,
    depths: Vec<f32>,
    painted: Vec<f32>,
    extent: (f32, f32),
    #[cfg(test)]
    pub area: f32,
}

impl Cap {
    pub fn new(c: &Compositor, brush: &CompositionColorBrush, drift: &Drift) -> Result<Self> {
        let geometry = c.CreatePathGeometry()?;
        let shape = c.CreateSpriteShapeWithGeometry(&geometry)?;
        shape.SetFillBrush(brush)?;
        Ok(Self {
            id: drift.ledge.id,
            shape,
            geometry,
            depths: vec![0.0; drift.depths.len()],
            painted: vec![-1.0; drift.depths.len()],
            extent: (0.0, 0.0),
            #[cfg(test)]
            area: 0.0,
        })
    }

    pub fn update(
        &mut self,
        factory: &ID2D1Factory,
        drift: &Drift,
        dt: f32,
        coverage: f32,
        cursor: (f32, f32),
    ) -> Result<()> {
        let ledge = drift.ledge;
        for (i, current) in self.depths.iter_mut().enumerate() {
            let start = i.saturating_sub(2);
            let end = (i + 3).min(drift.depths.len());
            let mut target = drift.depths[start..end].iter().sum::<f32>() / (end - start) as f32;
            if ledge.id == -2 {
                let x = ledge.x + (i as f32 + 0.5) * ledge.width / drift.depths.len() as f32;
                let distance = (x - cursor.0).hypot(ledge.y - cursor.1);
                target *= coverage * (1.0 - 0.12 * (1.0 - distance / 60.0).clamp(0.0, 1.0));
            }
            *current += (target - *current) * (1.0 - (-dt * 14.0).exp());
        }
        self.shape.SetOffset(Vector2 { X: ledge.x, Y: ledge.y })?;
        if self.extent == (ledge.width, ledge.radius)
            && self.depths.iter().zip(&self.painted).all(|(a, b)| (a - b).abs() < 0.01)
        {
            return Ok(());
        }
        self.extent = (ledge.width, ledge.radius);
        self.painted.clone_from(&self.depths);
        let count = (ledge.width / 5.0).ceil().clamp(8.0, 256.0) as usize;
        let mut samples: Vec<f32> = (0..=count).map(|i| i as f32 * ledge.width / count as f32).collect();
        for t in [0.125, 0.25, 0.5, 0.75, 1.0] {
            samples.push(ledge.radius * t);
            samples.push(ledge.width - ledge.radius * t);
        }
        samples.sort_by(f32::total_cmp);
        samples.dedup_by(|a, b| (*a - *b).abs() < 0.01);
        let points: Vec<Vector2> = samples
            .iter()
            .copied()
            .map(|x| {
                let position =
                    (x / ledge.width * (self.depths.len() - 1) as f32).clamp(0.0, (self.depths.len() - 1) as f32);
                let bin = position.floor() as usize;
                let t = position.fract();
                let t = t * t * (3.0 - 2.0 * t);
                let depth =
                    self.depths[bin] + (self.depths[(bin + 1).min(self.depths.len() - 1)] - self.depths[bin]) * t;
                let taper = (x.min(ledge.width - x) / (ledge.radius.max(6.0))).clamp(0.0, 1.0);
                Vector2 { X: x, Y: ledge.surface_y(ledge.x + x) - ledge.y - depth * taper }
            })
            .collect();
        unsafe {
            let path = factory.CreatePathGeometry()?;
            let sink: ID2D1GeometrySink = path.Open()?;
            sink.BeginFigure(points[0], D2D1_FIGURE_BEGIN_FILLED);
            for pair in points.windows(2) {
                let step = (pair[1].X - pair[0].X) * 0.4;
                sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                    point1: Vector2 { X: pair[0].X + step, Y: pair[0].Y },
                    point2: Vector2 { X: pair[1].X - step, Y: pair[1].Y },
                    point3: pair[1],
                });
            }
            let lower: Vec<Vector2> = samples
                .iter()
                .rev()
                .copied()
                .map(|x| Vector2 { X: x, Y: ledge.surface_y(ledge.x + x) - ledge.y })
                .collect();
            sink.AddLines(&[lower[0]]);
            for pair in lower.windows(2) {
                let step = (pair[1].X - pair[0].X) * 0.4;
                sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                    point1: Vector2 { X: pair[0].X + step, Y: pair[0].Y },
                    point2: Vector2 { X: pair[1].X - step, Y: pair[1].Y },
                    point3: pair[1],
                });
            }
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            sink.Close()?;
            #[cfg(test)]
            {
                self.area = path.ComputeArea(None, 0.1)?;
            }
            self.geometry.SetPath(&super::gfx::composition_path(&path)?)?;
        }
        self.shape.SetOffset(Vector2 { X: ledge.x, Y: ledge.y })
    }
}
