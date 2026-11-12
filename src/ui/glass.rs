use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};
use windows::core::{Interface, Result, h};

use windows::UI::Color;
use windows::UI::Composition::*;
use windows_numerics::{Vector2, Vector3};

use super::gfx::Gpu;

static ENABLED: AtomicBool = AtomicBool::new(false);

pub fn configure(requested: bool) -> bool {
    let on = requested
        && windows::UI::ViewManagement::UISettings::new()
            .and_then(|settings| settings.AdvancedEffectsEnabled())
            .unwrap_or(false)
        && windows::UI::ViewManagement::AccessibilitySettings::new()
            .and_then(|settings| settings.HighContrast())
            .is_ok_and(|high| !high);
    ENABLED.swap(on, Ordering::Relaxed) != on
}

pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

const RADIUS: f32 = 32.0;
const SIDE: u32 = 68;
const DIRECTIONS: [[f32; 2]; 4] = [[-1.0, 0.0], [1.0, 0.0], [0.0, -1.0], [0.0, 1.0]];

fn optics(x: f32, y: f32) -> [u8; 5] {
    let half = SIDE as f32 * 0.5;
    let (x, y) = (x - half, y - half);
    let (qx, qy) = (x.abs() - half + RADIUS, y.abs() - half + RADIUS);
    let len = qx.max(0.0).hypot(qy.max(0.0));
    let d = len + qx.max(qy).min(0.0) - RADIUS;
    let (nx, ny) = if len > 0.0001 {
        (qx.max(0.0) * x.signum() / len, qy.max(0.0) * y.signum() / len)
    } else if qx > qy {
        (x.signum(), 0.0)
    } else {
        (0.0, y.signum())
    };
    let cover = (0.5 - d / 1.5).clamp(0.0, 1.0);
    let bend = (1.0 + d / 22.0).clamp(0.0, 1.0).powf(2.4) * cover;
    let light = -0.543 * nx - 0.840 * ny;
    let rim = (-(d + 1.0).abs() / 1.1).exp();
    let reflection = cover * rim * (0.10 + 0.58 * light.max(0.0).powi(5) + 0.28 * (-light).max(0.0).powi(8));
    [bend * (-nx).max(0.0), bend * nx.max(0.0), bend * (-ny).max(0.0), bend * ny.max(0.0), reflection]
        .map(|a| (255.0 * a).round() as u8)
}

fn optical_maps() -> &'static [Vec<u8>; 5] {
    static MAPS: std::sync::OnceLock<[Vec<u8>; 5]> = std::sync::OnceLock::new();
    MAPS.get_or_init(|| {
        let mut maps: [Vec<u8>; 5] = std::array::from_fn(|_| Vec::with_capacity((SIDE * SIDE * 4) as usize));
        for y in 0..SIDE {
            for x in 0..SIDE {
                for (map, alpha) in maps.iter_mut().zip(optics(x as f32 + 0.5, y as f32 + 0.5)) {
                    map.extend_from_slice(&[alpha; 4]);
                }
            }
        }
        maps
    })
}

pub struct Lens {
    parent: SpriteVisual,
    root: ContainerVisual,
    sample: SpriteVisual,
    #[cfg(test)]
    bends: Vec<SpriteVisual>,
    pointer: SpriteVisual,
    last_pointer: Cell<Option<(i32, i32)>>,
    scale: f32,
}

impl Lens {
    pub fn sync(
        slot: &mut Option<Self>,
        gpu: &Gpu,
        parent: &SpriteVisual,
        contour: &CompositionRoundedRectangleGeometry,
        scale: f32,
    ) {
        if !enabled() {
            slot.take();
            return;
        }
        if slot.as_ref().is_some_and(|lens| lens.scale == scale) {
            return;
        }
        slot.take();
        *slot = Self::attach(gpu, parent, contour, scale)
            .map_err(|error| {
                crate::log_warn!("Liquid Glass unavailable: {error}");
            })
            .ok();
    }

    pub(super) fn attach(
        gpu: &Gpu,
        parent: &SpriteVisual,
        contour: &CompositionRoundedRectangleGeometry,
        scale: f32,
    ) -> Result<Self> {
        let c = parent.Compositor()?;
        let root = c.CreateContainerVisual()?;
        root.SetRelativeSizeAdjustment(Vector2 { X: 1.0, Y: 1.0 })?;
        let edge = c.CreateContainerVisual()?;
        for (property, expression) in
            [(h!("Size"), h!("contour.Size")), (h!("Offset"), h!("Vector3(contour.Offset.X, contour.Offset.Y, 0)"))]
        {
            let anim = c.CreateExpressionAnimationWithExpression(expression)?;
            anim.SetReferenceParameter(h!("contour"), contour)?;
            edge.StartAnimation(property, &anim)?;
        }
        let inset = c.CreateExpressionAnimationWithExpression(h!("Max(0.02, contour.CornerRadius.X / 32.0)"))?;
        inset.SetReferenceParameter(h!("contour"), contour)?;
        let sample = c.CreateSpriteVisual()?;
        sample.SetRelativeSizeAdjustment(Vector2 { X: 1.0, Y: 1.0 })?;
        sample.SetBrush(&parent.Brush()?)?;
        let frost = c.CreateContainerVisual()?;
        frost.SetRelativeSizeAdjustment(Vector2 { X: 1.0, Y: 1.0 })?;
        frost.SetOpacity(0.27)?;
        frost.Children()?.InsertAtTop(&sample)?;
        root.Children()?.InsertAtBottom(&frost)?;
        root.Children()?.InsertAtTop(&edge)?;
        let surface = c.CreateVisualSurface()?;
        surface.SetSourceVisual(&sample)?;
        for (property, expression) in
            [(h!("SourceOffset"), h!("contour.Offset")), (h!("SourceSize"), h!("contour.Size"))]
        {
            let anim = c.CreateExpressionAnimationWithExpression(expression)?;
            anim.SetReferenceParameter(h!("contour"), contour)?;
            surface.StartAnimation(property, &anim)?;
        }
        #[cfg(test)]
        let mut bends = Vec::new();
        for (i, map) in optical_maps().iter().enumerate() {
            let atlas = gpu.layer(&c, (SIDE as i32, SIDE as i32))?;
            let rect = (0.0, 0.0, SIDE as f32, SIDE as f32);
            let mut upload = Ok(());
            atlas.paint(gpu, |p| upload = p.image(rect, 0.0, SIDE, map))?;
            upload?;
            let grid = c.CreateNineGridBrush()?;
            let source_brush = atlas.sprite.Brush()?.cast::<CompositionSurfaceBrush>()?;
            source_brush.SetStretch(CompositionStretch::Fill)?;
            grid.SetSource(&source_brush)?;
            grid.SetInsets(RADIUS + 1.0)?;
            for prop in [h!("LeftInsetScale"), h!("TopInsetScale"), h!("RightInsetScale"), h!("BottomInsetScale")] {
                grid.StartAnimation(prop, &inset)?;
            }
            let sprite = c.CreateSpriteVisual()?;
            sprite.SetRelativeSizeAdjustment(Vector2 { X: 1.0, Y: 1.0 })?;
            if let Some(dir) = DIRECTIONS.get(i) {
                let translated = c.CreateSurfaceBrushWithSurface(&surface)?;
                translated.SetStretch(CompositionStretch::Fill)?;
                translated.SetOffset(Vector2 { X: dir[0] * 12.0 * scale, Y: dir[1] * 12.0 * scale })?;
                #[cfg(test)]
                bends.push(sprite.clone());
                let mask = c.CreateMaskBrush()?;
                mask.SetSource(&translated)?;
                mask.SetMask(&grid)?;
                sprite.SetBrush(&mask)?;
            } else {
                sprite.SetBrush(&grid)?;
            }
            edge.Children()?.InsertAtTop(&sprite)?;
        }
        let pointer = c.CreateSpriteVisual()?;
        let glow = c.CreateRadialGradientBrush()?;
        for (at, a) in [(0.0, 36), (0.5, 15), (1.0, 0)] {
            glow.ColorStops()?
                .Append(&c.CreateColorGradientStopWithOffsetAndColor(at, Color { A: a, R: 182, G: 215, B: 255 })?)?;
        }
        pointer.SetSize(Vector2 { X: 156.0 * scale, Y: 156.0 * scale })?;
        pointer.SetBrush(&glow)?;
        pointer.SetOpacity(0.0)?;
        edge.Children()?.InsertAtTop(&pointer)?;
        parent.Children()?.InsertAtTop(&root)?;
        if let Err(error) = parent.SetBrush(None::<&CompositionBrush>) {
            let _ = parent.Children()?.Remove(&root);
            return Err(error);
        }
        Ok(Self {
            parent: parent.clone(),
            root,
            sample,
            #[cfg(test)]
            bends,
            pointer,
            last_pointer: Cell::new(None),
            scale,
        })
    }
    #[cfg(test)]
    pub(super) fn show_refraction(&self, on: bool) -> Result<()> {
        for sprite in &self.bends {
            sprite.SetIsVisible(on)?;
        }
        Ok(())
    }

    pub fn refresh(&self) -> Result<()> {
        self.sample.SetBrush(&self.parent.Compositor()?.CreateHostBackdropBrush()?)
    }
    pub fn point(&self, local: Option<(f32, f32)>) {
        let point = local.map(|(x, y)| (x.round() as i32, y.round() as i32));
        let previous = self.last_pointer.replace(point);
        if previous == point {
            return;
        }
        if let Some((x, y)) = point {
            let half = 78.0 * self.scale;
            let _ = self.pointer.SetOffset(Vector3 { X: x as f32 - half, Y: y as f32 - half, Z: 0.0 });
        }
        if previous.is_some() != point.is_some() {
            let target = f32::from(point.is_some());
            if super::feedback::motion_allowed()
                && let Ok(fade) = self.pointer.Compositor().and_then(|c| c.CreateScalarKeyFrameAnimation())
            {
                let _ = fade.InsertKeyFrame(1.0, target);
                let _ = fade.SetDuration(std::time::Duration::from_millis(120).into());
                let _ = self.pointer.StartAnimation(h!("Opacity"), &fade);
            } else {
                let _ = self.pointer.SetOpacity(target);
            }
        }
    }
}

impl Drop for Lens {
    fn drop(&mut self) {
        if let Ok(scene) = self.sample.Brush() {
            let _ = self.parent.SetBrush(&scene);
        }
        if let Ok(children) = self.parent.Children() {
            let _ = children.Remove(&self.root);
        }
    }
}
