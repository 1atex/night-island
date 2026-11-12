mod detail;

use std::cell::{Cell, RefCell};
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
const DIRECTIONS: [(bool, bool); 4] = [(true, false), (true, true), (false, false), (false, true)];

fn optics(x: f32, y: f32) -> [u8; 7] {
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
    let band = ((d + 25.0) / 9.0).clamp(0.0, 1.0);
    let bend = band * band * (3.0 - 2.0 * band) * cover;
    let light = -0.543 * nx - 0.840 * ny;
    let rim = (-(d + 1.0).abs() / 1.1).exp();
    let reflection = cover * rim * (0.14 + 0.68 * light.max(0.0).powi(4) + 0.41 * (-light).max(0.0).powi(5));
    [
        bend * (-nx).max(0.0),
        bend * nx.max(0.0),
        bend * (-ny).max(0.0),
        bend * ny.max(0.0),
        reflection,
        cover * (1.0 + d / 25.0).clamp(0.0, 1.0),
        cover * rim,
    ]
    .map(|a| (255.0 * a).round() as u8)
}

fn optical_maps() -> &'static [Vec<u8>; 7] {
    static MAPS: std::sync::OnceLock<[Vec<u8>; 7]> = std::sync::OnceLock::new();
    MAPS.get_or_init(|| {
        let mut maps: [Vec<u8>; 7] = std::array::from_fn(|_| Vec::with_capacity((SIDE * SIDE * 4) as usize));
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

#[derive(Clone, Copy, PartialEq)]
pub struct CardBounds {
    pub rect: (f32, f32, f32, f32),
    pub radius: f32,
}

struct Card {
    sprite: SpriteVisual,
    source: CompositionSurfaceBrush,
    mask: CompositionNineGridBrush,
    bounds: Option<CardBounds>,
}

struct IslandEdges {
    owner: ContainerVisual,
    overlay: ContainerVisual,
    cards: ContainerVisual,
    atlas: CompositionSurfaceBrush,
    pool: RefCell<Vec<Card>>,
    layout: RefCell<Vec<CardBounds>>,
}

pub struct Lens {
    parent: SpriteVisual,
    root: ContainerVisual,
    edge: ContainerVisual,
    island: Option<IslandEdges>,
    surface: CompositionVisualSurface,
    scene_size: Cell<(i32, i32)>,
    material: CompositionPropertySet,
    heights: Option<(f32, f32)>,
    sample: SpriteVisual,
    base: SpriteVisual,
    detail: Option<CompositionEffectBrush>,
    #[cfg(test)]
    bends: Vec<SpriteVisual>,
    pointer: SpriteVisual,
    hover: SpringScalarNaturalMotionAnimation,
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
        let material = c.CreatePropertySet()?;
        material.InsertScalar(h!("Open"), 1.0)?;
        material.InsertVector2(h!("Cursor"), Vector2::default())?;
        let sample = c.CreateSpriteVisual()?;
        sample.SetSize(Vector2 { X: 1.0, Y: 1.0 })?;
        let scene = parent.Brush()?;
        let detail = detail::detail_brush(&c, &scene, scale).ok();
        if let Some(brush) = &detail {
            let blur = c.CreateExpressionAnimationWithExpression(h!("dpi * (1.6 + 1.2 * material.Open)"))?;
            blur.SetReferenceParameter(h!("material"), &material)?;
            blur.SetScalarParameter(h!("dpi"), scale)?;
            brush.StartAnimation(h!("frost.BlurAmount"), &blur)?;
            sample.SetBrush(brush)?;
        } else {
            sample.SetBrush(&scene)?;
        }
        let frost = c.CreateContainerVisual()?;
        frost.SetRelativeSizeAdjustment(Vector2 { X: 1.0, Y: 1.0 })?;
        frost.SetOpacity(0.69)?;
        frost.Children()?.InsertAtTop(&sample)?;
        let base = if detail.is_some() {
            let base = c.CreateSpriteVisual()?;
            base.SetRelativeSizeAdjustment(Vector2 { X: 1.0, Y: 1.0 })?;
            base.SetBrush(&scene)?;
            frost.Children()?.InsertAtTop(&base)?;
            base
        } else {
            sample.clone()
        };
        root.Children()?.InsertAtBottom(&frost)?;
        root.Children()?.InsertAtTop(&edge)?;
        let surface = c.CreateVisualSurface()?;
        surface.SetSourceVisual(&sample)?;
        surface.SetSourceSize(Vector2 { X: 1.0, Y: 1.0 })?;
        #[cfg(test)]
        let mut bends = Vec::new();
        for (i, map) in optical_maps()[..5].iter().enumerate() {
            let source_brush = Self::atlas(gpu, &c, map)?;
            let grid = c.CreateNineGridBrush()?;
            grid.SetSource(&source_brush)?;
            grid.SetInsets(RADIUS + 1.0)?;
            grid.SetIsCenterHollow(true)?;
            for prop in [h!("LeftInsetScale"), h!("TopInsetScale"), h!("RightInsetScale"), h!("BottomInsetScale")] {
                grid.StartAnimation(prop, &inset)?;
            }
            let sprite = c.CreateSpriteVisual()?;
            sprite.SetRelativeSizeAdjustment(Vector2 { X: 1.0, Y: 1.0 })?;
            if let Some(&(horizontal, far)) = DIRECTIONS.get(i) {
                let translated = c.CreateSurfaceBrushWithSurface(&surface)?;
                translated.SetStretch(CompositionStretch::None)?;
                translated.SetHorizontalAlignmentRatio(0.0)?;
                translated.SetVerticalAlignmentRatio(0.0)?;
                let magnification = 2.8;
                let (sx, sy) = if horizontal { (magnification, 1.0) } else { (1.0, magnification) };
                translated.SetScale(Vector2 { X: sx, Y: sy })?;
                let anchor = match (horizontal, far) {
                    (true, false) => h!(
                        "Vector2(-contour.Offset.X * zoom + (1.0 - zoom) * (width + extra * material.Open), -contour.Offset.Y)"
                    ),
                    (true, true) => h!(
                        "Vector2(-contour.Offset.X * zoom + (1.0 - zoom) * (contour.Size.X - width - extra * material.Open), -contour.Offset.Y)"
                    ),
                    (false, false) => h!(
                        "Vector2(-contour.Offset.X, -contour.Offset.Y * zoom + (1.0 - zoom) * (width + extra * material.Open))"
                    ),
                    (false, true) => h!(
                        "Vector2(-contour.Offset.X, -contour.Offset.Y * zoom + (1.0 - zoom) * (contour.Size.Y - width - extra * material.Open))"
                    ),
                };
                let offset = c.CreateExpressionAnimationWithExpression(anchor)?;
                offset.SetReferenceParameter(h!("contour"), contour)?;
                offset.SetScalarParameter(h!("zoom"), magnification)?;
                let displacement_scale = scale / (1.0 - 1.0 / magnification);
                offset.SetScalarParameter(h!("width"), 12.0 * displacement_scale)?;
                offset.SetScalarParameter(h!("extra"), 4.0 * displacement_scale)?;
                offset.SetReferenceParameter(h!("material"), &material)?;
                translated.StartAnimation(h!("Offset"), &offset)?;
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
        let hover = c.CreateSpringScalarAnimation()?;
        hover.SetDampingRatio(1.0)?;
        hover.SetPeriod(std::time::Duration::from_secs_f32(std::f32::consts::TAU / 18.0).into())?;
        let position = c.CreateExpressionAnimationWithExpression(h!(
            "Vector3(material.Cursor.X - contour.Offset.X - half, material.Cursor.Y - contour.Offset.Y - half, 0.0)"
        ))?;
        position.SetReferenceParameter(h!("material"), &material)?;
        position.SetReferenceParameter(h!("contour"), contour)?;
        position.SetScalarParameter(h!("half"), 78.0 * scale)?;
        pointer.StartAnimation(h!("Offset"), &position)?;
        edge.Children()?.InsertAtTop(&pointer)?;
        let rim = c.CreateNineGridBrush()?;
        rim.SetSource(&Self::atlas(gpu, &c, &optical_maps()[6])?)?;
        rim.SetInsets(RADIUS + 1.0)?;
        rim.SetIsCenterHollow(true)?;
        for property in [h!("LeftInsetScale"), h!("RightInsetScale"), h!("TopInsetScale"), h!("BottomInsetScale")] {
            rim.StartAnimation(property, &inset)?;
        }
        let light = c.CreateRadialGradientBrush()?;
        light.SetMappingMode(CompositionMappingMode::Absolute)?;
        light.SetEllipseRadius(Vector2 { X: 130.0 * scale, Y: 130.0 * scale })?;
        for (at, a) in [(0.0, 200), (0.46, 110), (1.0, 0)] {
            light
                .ColorStops()?
                .Append(&c.CreateColorGradientStopWithOffsetAndColor(at, Color { A: a, R: 235, G: 246, B: 255 })?)?;
        }
        let center = c.CreateExpressionAnimationWithExpression(h!("material.Cursor - contour.Offset"))?;
        center.SetReferenceParameter(h!("material"), &material)?;
        center.SetReferenceParameter(h!("contour"), contour)?;
        light.StartAnimation(h!("EllipseCenter"), &center)?;
        let highlight = c.CreateMaskBrush()?;
        highlight.SetMask(&rim)?;
        highlight.SetSource(&light)?;
        let specular = c.CreateSpriteVisual()?;
        specular.SetRelativeSizeAdjustment(Vector2 { X: 1.0, Y: 1.0 })?;
        specular.SetBrush(&highlight)?;
        let illumination = c.CreateExpressionAnimationWithExpression(h!("pointer.Opacity"))?;
        illumination.SetReferenceParameter(h!("pointer"), &pointer)?;
        specular.StartAnimation(h!("Opacity"), &illumination)?;
        edge.Children()?.InsertAtTop(&specular)?;
        parent.Children()?.InsertAtTop(&root)?;
        if let Err(error) = parent.SetBrush(None::<&CompositionBrush>) {
            let _ = parent.Children()?.Remove(&root);
            return Err(error);
        }
        Ok(Self {
            parent: parent.clone(),
            root,
            edge,
            island: None,
            surface,
            scene_size: Cell::new((1, 1)),
            material,
            heights: None,
            sample,
            base,
            detail,
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
        if let Some(island) = &self.island {
            for card in island.pool.borrow().iter() {
                card.sprite.SetIsVisible(on)?;
            }
        }
        Ok(())
    }

    fn atlas(gpu: &Gpu, c: &Compositor, map: &[u8]) -> Result<CompositionSurfaceBrush> {
        let atlas = gpu.layer(c, (SIDE as i32, SIDE as i32))?;
        let mut upload = Ok(());
        atlas.paint(gpu, |p| upload = p.image((0.0, 0.0, SIDE as f32, SIDE as f32), 0.0, SIDE, map))?;
        upload?;
        let brush = atlas.sprite.Brush()?.cast::<CompositionSurfaceBrush>()?;
        brush.SetStretch(CompositionStretch::Fill)?;
        Ok(brush)
    }

    pub fn mount_island(
        &mut self,
        gpu: &Gpu,
        owner: &ContainerVisual,
        tint: &SpriteVisual,
        content: &SpriteVisual,
        contour: &CompositionRoundedRectangleGeometry,
        heights: (f32, f32),
    ) -> Result<()> {
        if self.heights != Some(heights) {
            let progress = self
                .edge
                .Compositor()?
                .CreateExpressionAnimationWithExpression(h!("Clamp((contour.Size.Y - closed) * inverse, 0.0, 1.0)"))?;
            progress.SetReferenceParameter(h!("contour"), contour)?;
            progress.SetScalarParameter(h!("closed"), heights.0)?;
            progress.SetScalarParameter(h!("inverse"), 1.0 / (heights.1 - heights.0).max(1.0))?;
            self.material.StartAnimation(h!("Open"), &progress)?;
            self.heights = Some(heights);
        }
        if self.island.is_some() {
            return Ok(());
        }
        let c = self.edge.Compositor()?;
        let atlas = Self::atlas(gpu, &c, &optical_maps()[5])?;
        let overlay = c.CreateContainerVisual()?;
        let cards = c.CreateContainerVisual()?;
        overlay.SetRelativeSizeAdjustment(Vector2 { X: 1.0, Y: 1.0 })?;
        cards.SetRelativeSizeAdjustment(Vector2 { X: 1.0, Y: 1.0 })?;
        for (name, formula) in [
            (h!("Opacity"), h!("layer.Opacity")),
            (h!("IsVisible"), h!("layer.IsVisible")),
            (h!("Scale"), h!("layer.Scale")),
            (h!("CenterPoint"), h!("layer.CenterPoint")),
            (h!("Offset"), h!("layer.Offset")),
        ] {
            let anim = c.CreateExpressionAnimationWithExpression(formula)?;
            anim.SetReferenceParameter(h!("layer"), content)?;
            cards.StartAnimation(name, &anim)?;
        }
        let visible = c.CreateExpressionAnimationWithExpression(h!("layer.IsVisible"))?;
        visible.SetReferenceParameter(h!("layer"), &self.parent)?;
        overlay.StartAnimation(h!("IsVisible"), &visible)?;
        overlay.Children()?.InsertAtTop(&cards)?;
        owner.Children()?.InsertAbove(&overlay, tint)?;
        self.root.Children()?.Remove(&self.edge)?;
        if let Err(error) = overlay.Children()?.InsertAtBottom(&self.edge) {
            let _ = self.root.Children()?.InsertAtTop(&self.edge);
            let _ = owner.Children()?.Remove(&overlay);
            return Err(error);
        }
        self.island = Some(IslandEdges {
            owner: owner.clone(),
            overlay,
            cards,
            atlas,
            pool: RefCell::default(),
            layout: RefCell::default(),
        });
        Ok(())
    }

    pub fn begin_card_layout(&self) -> Option<&RefCell<Vec<CardBounds>>> {
        let layout = &self.island.as_ref()?.layout;
        layout.borrow_mut().clear();
        Some(layout)
    }

    pub fn update_cards(&self, bounds: &[CardBounds]) -> Result<()> {
        let Some(island) = &self.island else { return Ok(()) };
        let mut pool = island.pool.borrow_mut();
        while pool.len() > bounds.len() {
            island.cards.Children()?.Remove(&pool.pop().unwrap().sprite)?;
        }
        for (i, &bounds) in bounds.iter().enumerate() {
            if i == pool.len() {
                let c = island.cards.Compositor()?;
                let source = c.CreateSurfaceBrushWithSurface(&self.surface)?;
                source.SetStretch(CompositionStretch::None)?;
                source.SetHorizontalAlignmentRatio(0.0)?;
                source.SetVerticalAlignmentRatio(0.0)?;
                let mask = c.CreateNineGridBrush()?;
                mask.SetSource(&island.atlas)?;
                mask.SetInsets(RADIUS + 1.0)?;
                mask.SetIsCenterHollow(true)?;
                let brush = c.CreateMaskBrush()?;
                brush.SetSource(&source)?;
                brush.SetMask(&mask)?;
                let sprite = c.CreateSpriteVisual()?;
                sprite.SetBrush(&brush)?;
                sprite.SetOpacity(0.84)?;
                island.cards.Children()?.InsertAtTop(&sprite)?;
                pool.push(Card { sprite, source, mask, bounds: None });
            }
            let card = &mut pool[i];
            if card.bounds == Some(bounds) {
                continue;
            }
            let (l, t, r, b) = bounds.rect;
            let (w, h) = (r - l, b - t);
            let travel = (bounds.radius * 1.25).min(22.0 * self.scale);
            let (sx, sy) = (w / (w - travel).max(w * 0.55), h / (h - travel).max(h * 0.55));
            card.sprite.SetOffset(Vector3 { X: l, Y: t, Z: 0.0 })?;
            card.sprite.SetSize(Vector2 { X: w, Y: h })?;
            card.source.SetScale(Vector2 { X: sx, Y: sy })?;
            card.source.SetOffset(Vector2 { X: -l * sx + (1.0 - sx) * w * 0.5, Y: -t * sy + (1.0 - sy) * h * 0.5 })?;
            card.mask.SetInsetScales(bounds.radius / RADIUS)?;
            card.bounds = Some(bounds);
        }
        Ok(())
    }

    pub fn resize(&self, size: (i32, i32)) -> Result<()> {
        let size = (size.0.max(1), size.1.max(1));
        if self.scene_size.get() != size {
            let pixels = Vector2 { X: size.0 as f32, Y: size.1 as f32 };
            self.sample.SetSize(pixels)?;
            self.surface.SetSourceSize(pixels)?;
            self.scene_size.set(size);
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn island_metrics(&self) -> Result<((i32, i32), usize)> {
        let size = self.surface.SourceSize()?;
        let offset = self.surface.SourceOffset()?;
        assert_eq!(offset, Vector2::default());
        assert!(self.detail.is_some(), "The Windows compositor must compile the optical material");
        Ok(((size.X as i32, size.Y as i32), self.island.as_ref().unwrap().pool.borrow().len()))
    }

    pub fn refresh(&self) -> Result<()> {
        let backdrop = self.parent.Compositor()?.CreateHostBackdropBrush()?;
        self.base.SetBrush(&backdrop)?;
        if let Some(brush) = &self.detail {
            brush.SetSourceParameter(h!("scene"), &backdrop)?;
        }
        Ok(())
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
        if let Some(island) = &self.island
            && let Ok(children) = island.owner.Children()
        {
            let _ = children.Remove(&island.overlay);
        }
        if let Ok(scene) = self.base.Brush() {
            let _ = self.parent.SetBrush(&scene);
        }
        if let Ok(children) = self.parent.Children() {
            let _ = children.Remove(&self.root);
        }
    }
}
