mod detail;

use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};
use windows::core::{Interface, Result, h};

use windows::UI::Color;
use windows::UI::Composition::*;
use windows_numerics::Vector2;

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

const FULL: Vector2 = Vector2 { X: 1.0, Y: 1.0 };

fn surface_brush(c: &Compositor, surface: &CompositionVisualSurface) -> Result<CompositionSurfaceBrush> {
    let brush = c.CreateSurfaceBrushWithSurface(surface)?;
    brush.SetStretch(CompositionStretch::None)?;
    brush.SetHorizontalAlignmentRatio(0.0)?;
    brush.SetVerticalAlignmentRatio(0.0)?;
    Ok(brush)
}

pub struct Lens {
    parent: SpriteVisual,
    root: ContainerVisual,
    frost: ContainerVisual,
    highlights: ContainerVisual,
    owner: Option<ContainerVisual>,
    silhouette: ShapeVisual,
    silhouette_surface: CompositionVisualSurface,
    contours: [CompositionSpriteShape; 3],
    scene_size: Cell<(i32, i32)>,
    material: CompositionPropertySet,
    heights: Option<(f32, f32)>,
    scene: CompositionVisualSurface,
    sample: SpriteVisual,
    face: SpriteVisual,
    sampling_brush: CompositionEffectBrush,
    refracted_layer: ContainerVisual,
    refracted_surface: CompositionVisualSurface,
    matched_edges: CompositionEffectBrush,
    #[cfg(test)]
    bends: Vec<SpriteVisual>,
    pointer: ShapeVisual,
    hover: SpringScalarNaturalMotionAnimation,
    last_pointer: Cell<Option<(i32, i32)>>,
    scale: f32,
}

impl Lens {
    pub fn sync(
        slot: &mut Option<Self>,
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
        *slot = Self::attach(parent, contour, scale)
            .map_err(|error| crate::log_warn!("Liquid Glass unavailable: {error}"))
            .ok();
    }

    pub(super) fn attach(
        parent: &SpriteVisual,
        contour: &CompositionRoundedRectangleGeometry,
        scale: f32,
    ) -> Result<Self> {
        let c = parent.Compositor()?;
        let root = c.CreateContainerVisual()?;
        root.SetRelativeSizeAdjustment(FULL)?;
        let frost = c.CreateContainerVisual()?;
        frost.SetRelativeSizeAdjustment(FULL)?;
        frost.SetOpacity(0.69)?;
        root.Children()?.InsertAtBottom(&frost)?;
        let highlights = c.CreateContainerVisual()?;
        highlights.SetRelativeSizeAdjustment(FULL)?;
        let material = c.CreatePropertySet()?;
        material.InsertScalar(h!("Open"), 1.0)?;
        material.InsertVector2(h!("Cursor"), Vector2::default())?;
        let backdrop = parent.Brush()?;
        let sampling_brush = detail::sampling_brush(&c, &backdrop, scale)?;
        let sample = c.CreateSpriteVisual()?;
        sample.SetSize(FULL)?;
        sample.SetBrush(&sampling_brush)?;
        frost.Children()?.InsertAtBottom(&sample)?;
        let scene = c.CreateVisualSurface()?;
        scene.SetSourceVisual(&sample)?;
        scene.SetSourceSize(FULL)?;
        let face = c.CreateSpriteVisual()?;
        face.SetRelativeSizeAdjustment(FULL)?;
        face.SetBrush(&backdrop)?;
        frost.Children()?.InsertAtTop(&face)?;

        let silhouette = c.CreateShapeVisual()?;
        let outline = c.CreateSpriteShapeWithGeometry(contour)?;
        outline.SetFillBrush(&c.CreateColorBrushWithColor(Color { A: 255, R: 255, G: 255, B: 255 })?)?;
        silhouette.Shapes()?.Append(&outline)?;
        let silhouette_surface = c.CreateVisualSurface()?;
        silhouette_surface.SetSourceVisual(&silhouette)?;
        let body = surface_brush(&c, &silhouette_surface)?;
        let sampled = surface_brush(&c, &scene)?;
        let refracted_layer = c.CreateContainerVisual()?;
        let factory = detail::refraction_factory(&c, scale)?;
        #[cfg(test)]
        let mut bends = Vec::new();
        for (x, y) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
            let shifted = surface_brush(&c, &silhouette_surface)?;
            shifted.SetOffset(Vector2 { X: x * 2.75 * scale, Y: y * 2.75 * scale })?;
            let bend = factory.CreateBrush()?;
            bend.SetSourceParameter(h!("sampled"), &sampled)?;
            bend.SetSourceParameter(h!("body"), &body)?;
            bend.SetSourceParameter(h!("shifted"), &shifted)?;
            let refracted = surface_brush(&c, &scene)?;
            let offset = c.CreateExpressionAnimationWithExpression(h!("direction * (10.0 + 8.0 * material.Open)"))?;
            offset.SetVector2Parameter(h!("direction"), Vector2 { X: -x * scale, Y: -y * scale })?;
            offset.SetReferenceParameter(h!("material"), &material)?;
            refracted.StartAnimation(h!("Offset"), &offset)?;
            bend.SetSourceParameter(h!("refracted"), &refracted)?;
            let sprite = c.CreateSpriteVisual()?;
            sprite.SetRelativeSizeAdjustment(FULL)?;
            sprite.SetBrush(&bend)?;
            refracted_layer.Children()?.InsertAtTop(&sprite)?;
            #[cfg(test)]
            bends.push(sprite);
        }

        let refracted_surface = c.CreateVisualSurface()?;
        refracted_surface.SetSourceVisual(&refracted_layer)?;
        let matched_edges = detail::light_matched_edges(&c)?;
        matched_edges.SetSourceParameter(h!("live"), &backdrop)?;
        matched_edges.SetSourceParameter(h!("edge"), &surface_brush(&c, &refracted_surface)?)?;
        let refraction = c.CreateSpriteVisual()?;
        refraction.SetRelativeSizeAdjustment(FULL)?;
        refraction.SetBrush(&matched_edges)?;
        refraction.SetOpacity(0.73)?;
        frost.Children()?.InsertAtTop(&refraction)?;

        let light = c.CreateColorBrushWithColor(Color { A: 28, R: 234, G: 243, B: 250 })?;
        let rim = c.CreateSpriteShapeWithGeometry(contour)?;
        rim.SetStrokeThickness(scale)?;
        rim.SetStrokeBrush(&light)?;
        rim.SetStrokeLineJoin(CompositionStrokeLineJoin::Round)?;
        let reflection = c.CreateShapeVisual()?;
        reflection.SetRelativeSizeAdjustment(FULL)?;
        reflection.Shapes()?.Append(&rim)?;
        highlights.Children()?.InsertAtTop(&reflection)?;

        let glow = c.CreateRadialGradientBrush()?;
        glow.SetMappingMode(CompositionMappingMode::Absolute)?;
        glow.SetEllipseRadius(Vector2 { X: 94.0 * scale, Y: 94.0 * scale })?;
        for (at, alpha) in [(0.0, 86), (0.55, 26), (1.0, 0)] {
            glow.ColorStops()?.Append(
                &c.CreateColorGradientStopWithOffsetAndColor(at, Color { A: alpha, R: 235, G: 246, B: 255 })?,
            )?;
        }
        let center = c.CreateExpressionAnimationWithExpression(h!("material.Cursor"))?;
        center.SetReferenceParameter(h!("material"), &material)?;
        glow.StartAnimation(h!("EllipseCenter"), &center)?;
        let glint = c.CreateSpriteShapeWithGeometry(contour)?;
        glint.SetStrokeThickness(2.0 * scale)?;
        glint.SetStrokeLineJoin(CompositionStrokeLineJoin::Round)?;
        glint.SetStrokeBrush(&glow)?;
        let pointer = c.CreateShapeVisual()?;
        pointer.SetRelativeSizeAdjustment(FULL)?;
        pointer.Shapes()?.Append(&glint)?;
        pointer.SetOpacity(0.0)?;
        highlights.Children()?.InsertAtTop(&pointer)?;
        root.Children()?.InsertAtTop(&highlights)?;
        let hover = c.CreateSpringScalarAnimation()?;
        hover.SetDampingRatio(1.0)?;
        hover.SetPeriod(std::time::Duration::from_secs_f32(std::f32::consts::TAU / 18.0).into())?;
        parent.Children()?.InsertAtTop(&root)?;
        if let Err(error) = parent.SetBrush(None::<&CompositionBrush>) {
            let _ = parent.Children()?.Remove(&root);
            return Err(error);
        }
        Ok(Self {
            parent: parent.clone(),
            root,
            frost,
            highlights,
            owner: None,
            silhouette,
            silhouette_surface,
            contours: [outline, rim, glint],
            scene_size: Cell::new((0, 0)),
            material,
            heights: None,
            scene,
            sample,
            face,
            sampling_brush,
            refracted_layer,
            refracted_surface,
            matched_edges,
            #[cfg(test)]
            bends,
            pointer,
            hover,
            last_pointer: Cell::new(None),
            scale,
        })
    }

    #[cfg(test)]
    pub(crate) fn show_refraction(&self, on: bool) -> Result<()> {
        for sprite in &self.bends {
            sprite.SetIsVisible(on)?;
        }
        Ok(())
    }

    pub fn set_contour(&self, geometry: &CompositionGeometry) -> Result<()> {
        for shape in &self.contours {
            shape.SetGeometry(geometry)?;
        }
        Ok(())
    }

    pub fn mount_island(
        &mut self,
        owner: &ContainerVisual,
        tint: &SpriteVisual,
        contour: &CompositionRoundedRectangleGeometry,
        heights: (f32, f32),
    ) -> Result<()> {
        if self.heights != Some(heights) {
            let progress = self
                .root
                .Compositor()?
                .CreateExpressionAnimationWithExpression(h!("Clamp((contour.Size.Y - closed) * inverse, 0.0, 1.0)"))?;
            progress.SetReferenceParameter(h!("contour"), contour)?;
            progress.SetScalarParameter(h!("closed"), heights.0)?;
            progress.SetScalarParameter(h!("inverse"), 1.0 / (heights.1 - heights.0).max(1.0))?;
            self.material.StartAnimation(h!("Open"), &progress)?;
            self.heights = Some(heights);
        }

        if self.owner.is_some() {
            return Ok(());
        }
        self.frost.SetOpacity(1.0)?;
        let visible = self.highlights.Compositor()?.CreateExpressionAnimationWithExpression(h!("layer.IsVisible"))?;
        visible.SetReferenceParameter(h!("layer"), &self.parent)?;
        self.highlights.StartAnimation(h!("IsVisible"), &visible)?;
        self.root.Children()?.Remove(&self.highlights)?;
        if let Err(error) = owner.Children()?.InsertAbove(&self.highlights, tint) {
            let _ = self.root.Children()?.InsertAtTop(&self.highlights);
            return Err(error);
        }
        self.owner = Some(owner.clone());
        Ok(())
    }

    pub fn resize(&self, size: (i32, i32)) -> Result<()> {
        let size = (size.0.max(1), size.1.max(1));
        if self.scene_size.get() != size {
            let pixels = Vector2 { X: size.0 as f32, Y: size.1 as f32 };
            self.sample.SetSize(pixels)?;
            self.scene.SetSourceSize(pixels)?;
            self.refracted_layer.SetSize(pixels)?;
            self.refracted_surface.SetSourceSize(pixels)?;
            self.silhouette.SetSize(pixels)?;
            self.silhouette_surface.SetSourceSize(pixels)?;
            self.scene_size.set(size);
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn verify_contour(&self, geometry: &CompositionGeometry, size: (i32, i32)) -> Result<()> {
        let pixels = Vector2 { X: size.0 as f32, Y: size.1 as f32 };
        assert_eq!(self.silhouette_surface.SourceSize()?, pixels);
        assert_eq!(self.scene.SourceSize()?, pixels);
        for shape in &self.contours {
            assert_eq!(shape.Geometry()?, *geometry);
        }
        Ok(())
    }

    pub fn refresh(&self) -> Result<()> {
        let backdrop = self.parent.Compositor()?.CreateHostBackdropBrush()?;
        self.sampling_brush.SetSourceParameter(h!("scene"), &backdrop)?;
        self.matched_edges.SetSourceParameter(h!("live"), &backdrop)?;
        self.face.SetBrush(&backdrop)
    }

    pub fn point(&self, local: Option<(f32, f32)>) {
        let point = local.map(|(x, y)| (x.round() as i32, y.round() as i32));
        let previous = self.last_pointer.replace(point);
        if previous == point {
            return;
        }
        if let Some((x, y)) = point {
            let _ = self.material.InsertVector2(h!("Cursor"), Vector2 { X: x as f32, Y: y as f32 });
        }
        if previous.is_some() != point.is_some() {
            let target = f32::from(point.is_some());
            if super::feedback::motion_allowed()
                && let Ok(value) = windows::Foundation::PropertyValue::CreateSingle(target)
                    .and_then(|v| v.cast::<windows::Foundation::IReference<f32>>())
                && self.hover.SetFinalValue(&value).is_ok()
                && self.pointer.StartAnimation(h!("Opacity"), &self.hover).is_ok()
            {
                return;
            }
            let _ = self.pointer.StopAnimation(h!("Opacity"));
            let _ = self.pointer.SetOpacity(target);
        }
    }
}

impl Drop for Lens {
    fn drop(&mut self) {
        if let Some(owner) = &self.owner
            && let Ok(children) = owner.Children()
        {
            let _ = children.Remove(&self.highlights);
        }
        if let Ok(scene) = self.face.Brush() {
            let _ = self.parent.SetBrush(&scene);
        }
        if let Ok(children) = self.parent.Children() {
            let _ = children.Remove(&self.root);
        }
    }
}
