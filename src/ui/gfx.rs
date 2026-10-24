use windows::Foundation::Size;
use windows::Graphics::DirectX::{DirectXAlphaMode, DirectXPixelFormat};
use windows::Graphics::SizeInt32;
use windows::UI::Composition::{CompositionDrawingSurface, CompositionGraphicsDevice, Compositor, SpriteVisual};
use windows::Win32::Foundation::{HMODULE, POINT};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D1_BEZIER_SEGMENT, D2D1_COLOR_F, D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_END_CLOSED,
    D2D1_FILL_MODE_WINDING,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_ALIASED, D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_ELLIPSE, D2D1_FACTORY_TYPE_SINGLE_THREADED,
    D2D1_FEATURE_LEVEL_DEFAULT, D2D1_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_TYPE_DEFAULT,
    D2D1_RENDER_TARGET_USAGE_NONE, D2D1_ROUNDED_RECT, D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE, D2D1CreateDevice,
    D2D1CreateFactory, ID2D1Device, ID2D1DeviceContext, ID2D1Factory, ID2D1Geometry, ID2D1GeometrySink,
    ID2D1PathGeometry, ID2D1RenderTarget,
};
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice, ID3D11Device,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT,
    DWRITE_FONT_WEIGHT_NORMAL, DWRITE_MEASURING_MODE_NATURAL, DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT,
    DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TRIMMING, DWRITE_TRIMMING_GRANULARITY_CHARACTER, DWRITE_WORD_WRAPPING_NO_WRAP,
    DWRITE_WORD_WRAPPING_WRAP, DWriteCreateFactory, IDWriteFactory, IDWriteTextLayout,
};
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::System::WinRT::Composition::{ICompositionDrawingSurfaceInterop, ICompositorInterop};
use windows::core::{BOOL, Interface, PCWSTR, Result, w};
use windows_numerics::Matrix3x2;

pub const WHITE: D2D1_COLOR_F = D2D1_COLOR_F { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };

pub fn rgba(r: f32, g: f32, b: f32, a: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F { r, g, b, a }
}

pub struct Gpu {
    gfx: CompositionGraphicsDevice,
    factory: ID2D1Factory,
    dwrite: IDWriteFactory,
    icons: PCWSTR,
}

impl Gpu {
    pub fn new(compositor: &Compositor) -> Result<Self> {
        unsafe {
            let mut d3d: Option<ID3D11Device> = None;
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut d3d),
                None,
                None,
            )?;
            let dxgi: IDXGIDevice = d3d.ok_or_else(windows::core::Error::empty)?.cast()?;
            let d2d: ID2D1Device = D2D1CreateDevice(&dxgi, None)?;
            let gfx = compositor.cast::<ICompositorInterop>()?.CreateGraphicsDevice(&d2d)?;
            let dwrite: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            let icons = pick_icon_font(&dwrite);
            let factory: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            Ok(Self { gfx, factory, dwrite, icons })
        }
    }

    pub fn layer(&self, compositor: &Compositor, size: (i32, i32)) -> Result<Layer> {
        let surface = self.gfx.CreateDrawingSurface(
            Size { Width: size.0 as f32, Height: size.1 as f32 },
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            DirectXAlphaMode::Premultiplied,
        )?;
        let sprite = compositor.CreateSpriteVisual()?;
        sprite.SetBrush(&compositor.CreateSurfaceBrushWithSurface(&surface)?)?;
        sprite.SetSize(windows_numerics::Vector2 { X: size.0 as f32, Y: size.1 as f32 })?;
        Ok(Layer { surface, sprite, size })
    }

    pub fn dwrite(&self) -> &IDWriteFactory {
        &self.dwrite
    }

    pub fn icons(&self) -> PCWSTR {
        self.icons
    }
}

pub struct Layer {
    surface: CompositionDrawingSurface,
    pub sprite: SpriteVisual,
    size: (i32, i32),
}

impl Gpu {
    pub fn paint_dc(
        &self,
        hdc: windows::Win32::Graphics::Gdi::HDC,
        size: (i32, i32),
        f: impl FnOnce(&Painter),
    ) -> Result<()> {
        use windows::Win32::Graphics::Direct2D::Common::{D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_PIXEL_FORMAT};
        use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
        unsafe {
            let props = D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 96.0,
                dpiY: 96.0,
                usage: D2D1_RENDER_TARGET_USAGE_NONE,
                minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
            };
            let dc_target = self.factory.CreateDCRenderTarget(&props)?;
            let rect = windows::Win32::Foundation::RECT { left: 0, top: 0, right: size.0, bottom: size.1 };
            dc_target.BindDC(hdc, &rect)?;
            dc_target.BeginDraw();
            dc_target.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
            dc_target.Clear(Some(&rgba(0.0, 0.0, 0.0, 0.0)));
            let target: ID2D1RenderTarget = dc_target.cast()?;
            f(&Painter { dc: &target, dwrite: self.dwrite(), icons: self.icons() });
            dc_target.EndDraw(None, None)
        }
    }
}

impl Layer {
    pub fn size(&self) -> (i32, i32) {
        self.size
    }

    pub fn resize(&mut self, size: (i32, i32)) -> Result<()> {
        if size == self.size {
            return Ok(());
        }
        self.surface.Resize(SizeInt32 { Width: size.0, Height: size.1 })?;
        self.sprite.SetSize(windows_numerics::Vector2 { X: size.0 as f32, Y: size.1 as f32 })?;
        self.size = size;
        Ok(())
    }

    pub fn surface_brush(&self, compositor: &Compositor) -> Result<windows::UI::Composition::CompositionSurfaceBrush> {
        compositor.CreateSurfaceBrushWithSurface(&self.surface)
    }

    pub fn paint(&self, gpu: &Gpu, f: impl FnOnce(&Painter)) -> Result<()> {
        unsafe {
            let interop: ICompositionDrawingSurfaceInterop = self.surface.cast()?;
            let mut offset = POINT::default();
            let dc: ID2D1DeviceContext = interop.BeginDraw(None, &mut offset)?;
            dc.SetTransform(&Matrix3x2::translation(offset.x as f32, offset.y as f32));
            dc.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
            dc.Clear(Some(&rgba(0.0, 0.0, 0.0, 0.0)));
            let target: ID2D1RenderTarget = dc.cast()?;
            f(&Painter { dc: &target, dwrite: gpu.dwrite(), icons: gpu.icons() });
            interop.EndDraw()
        }
    }
}

pub struct Painter<'a> {
    dc: &'a ID2D1RenderTarget,
    dwrite: &'a IDWriteFactory,
    icons: PCWSTR,
}

impl Painter<'_> {
    #[allow(clippy::too_many_arguments)]
    pub fn text(
        &self,
        text: &str,
        rect: (f32, f32, f32, f32),
        size: f32,
        weight: DWRITE_FONT_WEIGHT,
        align: DWRITE_TEXT_ALIGNMENT,
        color: D2D1_COLOR_F,
    ) -> Result<()> {
        self.text_in(w!("Segoe UI"), &crate::ui::i18n::tr(text), rect, size, weight, align, color)
    }

    pub fn text_wrapped(
        &self,
        text: &str,
        rect: (f32, f32, f32, f32),
        size: f32,
        weight: DWRITE_FONT_WEIGHT,
        color: D2D1_COLOR_F,
    ) -> Result<()> {
        use windows::Win32::Graphics::DirectWrite::DWRITE_TEXT_METRICS;
        let text = crate::ui::i18n::tr(text);
        unsafe {
            let format = self.dwrite.CreateTextFormat(
                w!("Segoe UI"),
                None,
                weight,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                size,
                w!("en-us"),
            )?;
            format.SetWordWrapping(DWRITE_WORD_WRAPPING_WRAP)?;
            let wide: Vec<u16> = text.encode_utf16().collect();
            let layout =
                self.dwrite.CreateTextLayout(&wide, &format, (rect.2 - rect.0).max(1.0), (rect.3 - rect.1).max(1.0))?;
            let mut m = DWRITE_TEXT_METRICS::default();
            layout.GetMetrics(&mut m)?;
            let y = rect.1 + ((rect.3 - rect.1) - m.height).max(0.0) / 2.0;
            self.draw_layout(rect.0, y, &layout, color)
        }
    }

    pub fn md_layout(
        &self,
        doc: &crate::ui::md::Doc,
        width: f32,
        size: f32,
        italic: bool,
    ) -> Result<IDWriteTextLayout> {
        use crate::ui::md::Style;
        use windows::Win32::Graphics::DirectWrite::{
            DWRITE_FONT_STYLE_ITALIC, DWRITE_FONT_WEIGHT_BOLD, DWRITE_TEXT_RANGE,
        };
        let layout = make_layout(self.dwrite, &doc.text, width, size)?;
        unsafe {
            let all = doc.text.encode_utf16().count() as u32;
            if italic && all > 0 {
                layout.SetFontStyle(DWRITE_FONT_STYLE_ITALIC, DWRITE_TEXT_RANGE { startPosition: 0, length: all })?;
            }
            for span in &doc.spans {
                let range = DWRITE_TEXT_RANGE { startPosition: span.start as u32, length: span.len as u32 };
                match span.style {
                    Style::Bold => layout.SetFontWeight(DWRITE_FONT_WEIGHT_BOLD, range)?,
                    Style::Italic => layout.SetFontStyle(DWRITE_FONT_STYLE_ITALIC, range)?,
                    Style::Code | Style::Block => {
                        layout.SetFontFamilyName(w!("Consolas"), range)?;
                        layout.SetFontSize(size * 0.92, range)?;
                    }
                    Style::Heading(level) => {
                        layout.SetFontWeight(DWRITE_FONT_WEIGHT_BOLD, range)?;
                        layout.SetFontSize(size * (1.55 - 0.13 * f32::from(level)), range)?;
                    }
                    Style::Link => layout.SetUnderline(true, range)?,
                }
            }
        }
        Ok(layout)
    }

    pub fn layout_height(&self, layout: &IDWriteTextLayout) -> f32 {
        use windows::Win32::Graphics::DirectWrite::DWRITE_TEXT_METRICS;
        let mut m = DWRITE_TEXT_METRICS::default();
        unsafe {
            let _ = layout.GetMetrics(&mut m);
        }
        m.height
    }

    pub fn md_draw(
        &self,
        layout: &IDWriteTextLayout,
        doc: &crate::ui::md::Doc,
        x: f32,
        y: f32,
        color: D2D1_COLOR_F,
        code_bg: D2D1_COLOR_F,
    ) -> Result<()> {
        use crate::ui::md::Style;
        use windows::Win32::Graphics::DirectWrite::DWRITE_HIT_TEST_METRICS;
        for span in &doc.spans {
            if !matches!(span.style, Style::Code | Style::Block) {
                continue;
            }
            let mut boxes = [DWRITE_HIT_TEST_METRICS::default(); 24];
            let mut count = 0u32;
            let _ = unsafe {
                layout.HitTestTextRange(span.start as u32, span.len as u32, x, y, Some(&mut boxes), &mut count)
            };
            for b in boxes.iter().take((count as usize).min(boxes.len())) {
                let pad = if span.style == Style::Block { 4.0 } else { 2.0 };
                self.fill_round_rect((b.left - pad, b.top, b.left + b.width + pad, b.top + b.height), 4.0, code_bg)?;
            }
        }
        self.draw_layout(x, y, layout, color)
    }

    pub fn svg(&self, data: &[u8], bounds: [f32; 4], rect: (f32, f32, f32, f32)) -> Result<()> {
        use windows::Win32::Graphics::Direct2D::Common::D2D_SIZE_F;
        use windows::Win32::Graphics::Direct2D::ID2D1DeviceContext5;
        let Ok(dc5) = self.dc.cast::<ID2D1DeviceContext5>() else { return Ok(()) };
        unsafe {
            let Some(stream) = windows::Win32::UI::Shell::SHCreateMemStream(Some(data)) else { return Ok(()) };
            let doc = dc5.CreateSvgDocument(&stream, D2D_SIZE_F { width: 64.0, height: 64.0 })?;
            let (bw, bh) = ((bounds[2] - bounds[0]).max(1.0), (bounds[3] - bounds[1]).max(1.0));
            let (rw, rh) = (rect.2 - rect.0, rect.3 - rect.1);
            let k = (rw / bw).min(rh / bh);
            let dx = rect.0 + (rw - bw * k) / 2.0 - bounds[0] * k;
            let dy = rect.1 + (rh - bh * k) / 2.0 - bounds[1] * k;
            let mut old = Matrix3x2::default();
            dc5.GetTransform(&mut old);
            let local = Matrix3x2 { M11: k, M12: 0.0, M21: 0.0, M22: k, M31: dx, M32: dy };
            dc5.SetTransform(&(local * old));
            dc5.DrawSvgDocument(&doc);
            dc5.SetTransform(&old);
        }
        Ok(())
    }

    fn icon(
        &self,
        view_box: &str,
        path: &str,
        rect: (f32, f32, f32, f32),
        size: f32,
        color: D2D1_COLOR_F,
    ) -> Result<()> {
        let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        let svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{view_box}\" width=\"64\" height=\"64\">             <path fill=\"#{:02x}{:02x}{:02x}\" fill-opacity=\"{:.3}\" d=\"{path}\"/></svg>",
            byte(color.r),
            byte(color.g),
            byte(color.b),
            color.a.clamp(0.0, 1.0)
        );
        let (cx, cy) = ((rect.0 + rect.2) / 2.0, (rect.1 + rect.3) / 2.0);
        let half = size * 1.25 / 2.0;
        self.svg(svg.as_bytes(), [0.0, 0.0, 64.0, 64.0], (cx - half, cy - half, cx + half, cy + half))
    }

    pub fn glyph(&self, ch: char, rect: (f32, f32, f32, f32), size: f32, color: D2D1_COLOR_F) -> Result<()> {
        if let Some((view_box, path)) = crate::ui::icons::lookup(ch) {
            return self.icon(view_box, path, rect, size, color);
        }
        let mut buf = [0u8; 4];
        self.text_in(
            self.icons,
            ch.encode_utf8(&mut buf),
            rect,
            size,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_CENTER,
            color,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn fill_bump(
        &self,
        _width: f32,
        seam: f32,
        cx: f32,
        half: f32,
        amp: f32,
        color: D2D1_COLOR_F,
    ) -> Result<ID2D1PathGeometry> {
        unsafe {
            let factory: ID2D1Factory = self.dc.GetFactory()?;
            let geometry = factory.CreatePathGeometry()?;
            let sink = geometry.Open()?;
            let pt = |x: f32, y: f32| windows_numerics::Vector2 { X: x, Y: y };
            let (x0, x1) = (cx - half, cx + half);
            sink.BeginFigure(pt(x0, seam - 1.0), D2D1_FIGURE_BEGIN_FILLED);
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: pt(x0 + half * 0.55, seam - 1.0),
                point2: pt(cx - half * 0.28, seam + amp),
                point3: pt(cx, seam + amp),
            });
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: pt(cx + half * 0.28, seam + amp),
                point2: pt(x1 - half * 0.55, seam - 1.0),
                point3: pt(x1, seam - 1.0),
            });
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            sink.Close()?;

            let brush = self.dc.CreateSolidColorBrush(&color, None)?;
            self.dc.FillGeometry(&geometry, &brush, None);
            Ok(geometry)
        }
    }

    pub fn dwrite(&self) -> &IDWriteFactory {
        self.dwrite
    }

    pub fn draw_layout(&self, x: f32, y: f32, layout: &IDWriteTextLayout, color: D2D1_COLOR_F) -> Result<()> {
        unsafe {
            let brush = self.dc.CreateSolidColorBrush(&color, None)?;
            self.dc.DrawTextLayout(
                windows_numerics::Vector2 { X: x, Y: y },
                layout,
                &brush,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
            );
        }
        Ok(())
    }

    pub fn push_clip(&self, r: (f32, f32, f32, f32)) {
        unsafe {
            self.dc.PushAxisAlignedClip(
                &D2D_RECT_F { left: r.0, top: r.1, right: r.2, bottom: r.3 },
                D2D1_ANTIALIAS_MODE_ALIASED,
            );
        }
    }

    pub fn pop_clip(&self) {
        unsafe { self.dc.PopAxisAlignedClip() };
    }

    pub fn fill_rect(&self, r: (f32, f32, f32, f32), color: D2D1_COLOR_F) -> Result<()> {
        unsafe {
            let brush = self.dc.CreateSolidColorBrush(&color, None)?;
            self.dc.FillRectangle(&D2D_RECT_F { left: r.0, top: r.1, right: r.2, bottom: r.3 }, &brush);
        }
        Ok(())
    }

    pub fn fill_ear(&self, size: f32, left: bool, color: D2D1_COLOR_F) -> Result<()> {
        unsafe {
            let factory: ID2D1Factory = self.dc.GetFactory()?;
            let geometry = factory.CreatePathGeometry()?;
            let sink = geometry.Open()?;
            let r = size;
            let k = 0.5523 * r;
            let mx = |x: f32| if left { x } else { r - x };
            let pt = |x: f32, y: f32| windows_numerics::Vector2 { X: mx(x), Y: y };
            sink.BeginFigure(pt(0.0, 0.0), D2D1_FIGURE_BEGIN_FILLED);
            sink.AddLine(pt(r, 0.0));
            sink.AddLine(pt(r, r));
            sink.AddBezier(&D2D1_BEZIER_SEGMENT { point1: pt(r, r - k), point2: pt(k, 0.0), point3: pt(0.0, 0.0) });
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            sink.Close()?;
            let brush = self.dc.CreateSolidColorBrush(&color, None)?;
            self.dc.FillGeometry(&geometry, &brush, None);
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn fill_spikes(
        &self,
        width: f32,
        seam: f32,
        levels: &[f32],
        depth: f32,
        color: D2D1_COLOR_F,
        tip: D2D1_COLOR_F,
    ) -> Result<ID2D1PathGeometry> {
        use windows::Win32::Graphics::Direct2D::Common::D2D1_GRADIENT_STOP;
        use windows::Win32::Graphics::Direct2D::{
            D2D1_EXTEND_MODE_CLAMP, D2D1_GAMMA_2_2, D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES,
        };
        unsafe {
            let factory: ID2D1Factory = self.dc.GetFactory()?;
            let pt = |x: f32, y: f32| windows_numerics::Vector2 { X: x, Y: y };
            let top = seam - 1.0;
            let slot = width / levels.len().max(1) as f32;
            let half = slot * 0.5;
            let spike = |sink: &ID2D1GeometrySink, cx: f32, d: f32| {
                sink.AddLine(pt(cx - half, top));
                sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                    point1: pt(cx - half * 0.45, top),
                    point2: pt(cx - half * 0.18, seam + d),
                    point3: pt(cx, seam + d),
                });
                sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                    point1: pt(cx + half * 0.18, seam + d),
                    point2: pt(cx + half * 0.45, top),
                    point3: pt(cx + half, top),
                });
            };
            let spikes: Vec<(f32, f32)> = levels
                .iter()
                .enumerate()
                .map(|(i, l)| ((i as f32 + 0.5) * slot, l.clamp(0.0, 1.0) * depth))
                .filter(|&(_, d)| d >= 0.8)
                .collect();
            let geometry = factory.CreatePathGeometry()?;
            let sink = geometry.Open()?;
            sink.SetFillMode(D2D1_FILL_MODE_WINDING);
            sink.BeginFigure(pt(0.0, top), D2D1_FIGURE_BEGIN_FILLED);
            for &(cx, d) in &spikes {
                spike(&sink, cx, d);
            }
            sink.AddLine(pt(width, top));
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            sink.Close()?;
            self.dc.FillGeometry(&geometry, &self.dc.CreateSolidColorBrush(&color, None)?, None);
            if (tip.r, tip.g, tip.b) == (color.r, color.g, color.b) {
                return Ok(geometry);
            }
            let stops = [
                D2D1_GRADIENT_STOP { position: 0.0, color: D2D1_COLOR_F { a: 0.0, ..tip } },
                D2D1_GRADIENT_STOP { position: 0.55, color: D2D1_COLOR_F { a: 0.85, ..tip } },
                D2D1_GRADIENT_STOP { position: 1.0, color: tip },
            ];
            let collection = self.dc.CreateGradientStopCollection(&stops, D2D1_GAMMA_2_2, D2D1_EXTEND_MODE_CLAMP)?;
            for &(cx, d) in &spikes {
                let one = factory.CreatePathGeometry()?;
                let sink = one.Open()?;
                sink.BeginFigure(pt(cx - half, top), D2D1_FIGURE_BEGIN_FILLED);
                spike(&sink, cx, d);
                sink.EndFigure(D2D1_FIGURE_END_CLOSED);
                sink.Close()?;
                let props = D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES {
                    startPoint: pt(cx, seam + d * 0.15),
                    endPoint: pt(cx, seam + d),
                };
                let brush = self.dc.CreateLinearGradientBrush(&props, None, &collection)?;
                self.dc.FillGeometry(&one, &brush, None);
            }
            Ok(geometry)
        }
    }

    pub fn fill_ellipse(&self, cx: f32, cy: f32, rx: f32, ry: f32, color: D2D1_COLOR_F) -> Result<()> {
        unsafe {
            let brush = self.dc.CreateSolidColorBrush(&color, None)?;
            self.dc.FillEllipse(
                &D2D1_ELLIPSE { point: windows_numerics::Vector2 { X: cx, Y: cy }, radiusX: rx, radiusY: ry },
                &brush,
            );
        }
        Ok(())
    }

    pub fn line(&self, from: (f32, f32), to: (f32, f32), width: f32, color: D2D1_COLOR_F) -> Result<()> {
        unsafe {
            let brush = self.dc.CreateSolidColorBrush(&color, None)?;
            self.dc.DrawLine(
                windows_numerics::Vector2 { X: from.0, Y: from.1 },
                windows_numerics::Vector2 { X: to.0, Y: to.1 },
                &brush,
                width,
                None,
            );
        }
        Ok(())
    }

    pub fn fill_triangle(&self, a: (f32, f32), b: (f32, f32), c: (f32, f32), color: D2D1_COLOR_F) -> Result<()> {
        unsafe {
            let factory: ID2D1Factory = self.dc.GetFactory()?;
            let geometry = factory.CreatePathGeometry()?;
            let sink = geometry.Open()?;
            let pt = |p: (f32, f32)| windows_numerics::Vector2 { X: p.0, Y: p.1 };
            sink.BeginFigure(pt(a), D2D1_FIGURE_BEGIN_FILLED);
            sink.AddLine(pt(b));
            sink.AddLine(pt(c));
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            sink.Close()?;
            let brush = self.dc.CreateSolidColorBrush(&color, None)?;
            self.dc.FillGeometry(&geometry, &brush, None);
        }
        Ok(())
    }

    pub fn fill_round_rect(&self, rect: (f32, f32, f32, f32), radius: f32, color: D2D1_COLOR_F) -> Result<()> {
        unsafe {
            let brush = self.dc.CreateSolidColorBrush(&color, None)?;
            self.dc.FillRoundedRectangle(&rounded(rect, radius), &brush);
        }
        Ok(())
    }

    pub fn image(&self, rect: (f32, f32, f32, f32), radius: f32, side: u32, bgra: &[u8]) -> Result<()> {
        use windows::Win32::Graphics::Direct2D::Common::{
            D2D_SIZE_U, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_PIXEL_FORMAT,
        };
        use windows::Win32::Graphics::Direct2D::{D2D1_BITMAP_PROPERTIES, D2D1_BRUSH_PROPERTIES};
        use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
        if bgra.len() < (side * side * 4) as usize || side == 0 {
            return Ok(());
        }
        unsafe {
            let props = D2D1_BITMAP_PROPERTIES {
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 96.0,
                dpiY: 96.0,
            };
            let bitmap = self.dc.CreateBitmap(
                D2D_SIZE_U { width: side, height: side },
                Some(bgra.as_ptr().cast()),
                side * 4,
                &props,
            )?;
            let k = (rect.2 - rect.0) / side as f32;
            let brush_props = D2D1_BRUSH_PROPERTIES {
                opacity: 1.0,
                transform: Matrix3x2 { M11: k, M12: 0.0, M21: 0.0, M22: k, M31: rect.0, M32: rect.1 },
            };
            let brush = self.dc.CreateBitmapBrush(&bitmap, None, Some(&brush_props))?;
            self.dc.FillRoundedRectangle(&rounded(rect, radius), &brush);
        }
        Ok(())
    }

    pub fn stroke_round_rect(
        &self,
        rect: (f32, f32, f32, f32),
        radius: f32,
        width: f32,
        color: D2D1_COLOR_F,
    ) -> Result<()> {
        unsafe {
            let brush = self.dc.CreateSolidColorBrush(&color, None)?;
            self.dc.DrawRoundedRectangle(&rounded(rect, radius), &brush, width, None);
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn text_in(
        &self,
        family: PCWSTR,
        text: &str,
        rect: (f32, f32, f32, f32),
        size: f32,
        weight: DWRITE_FONT_WEIGHT,
        align: DWRITE_TEXT_ALIGNMENT,
        color: D2D1_COLOR_F,
    ) -> Result<()> {
        unsafe {
            let format = self.dwrite.CreateTextFormat(
                family,
                None,
                weight,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                size,
                w!("en-us"),
            )?;
            format.SetTextAlignment(align)?;
            format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
            format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
            let sign = self.dwrite.CreateEllipsisTrimmingSign(&format)?;
            format.SetTrimming(
                &DWRITE_TRIMMING {
                    granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER,
                    delimiter: 0,
                    delimiterCount: 0,
                },
                &sign,
            )?;
            let brush = self.dc.CreateSolidColorBrush(&color, None)?;
            let wide: Vec<u16> = text.encode_utf16().collect();
            self.dc.DrawText(
                &wide,
                &format,
                &D2D_RECT_F { left: rect.0, top: rect.1, right: rect.2, bottom: rect.3 },
                &brush,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
                DWRITE_MEASURING_MODE_NATURAL,
            );
        }
        Ok(())
    }
}

fn rounded(r: (f32, f32, f32, f32), radius: f32) -> D2D1_ROUNDED_RECT {
    D2D1_ROUNDED_RECT {
        rect: D2D_RECT_F { left: r.0, top: r.1, right: r.2, bottom: r.3 },
        radiusX: radius,
        radiusY: radius,
    }
}

fn pick_icon_font(dwrite: &IDWriteFactory) -> PCWSTR {
    let fluent = w!("Segoe Fluent Icons");
    unsafe {
        let mut collection = None;
        if dwrite.GetSystemFontCollection(&mut collection, false).is_ok()
            && let Some(c) = collection
        {
            let (mut index, mut exists) = (0u32, BOOL(0));
            if c.FindFamilyName(fluent, &mut index, &mut exists).is_ok() && exists.as_bool() {
                return fluent;
            }
        }
    }
    w!("Segoe MDL2 Assets")
}

pub fn make_layout(dwrite: &IDWriteFactory, text: &str, width: f32, size: f32) -> Result<IDWriteTextLayout> {
    unsafe {
        let format = dwrite.CreateTextFormat(
            w!("Segoe UI"),
            None,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            size,
            w!("en-us"),
        )?;
        format.SetWordWrapping(DWRITE_WORD_WRAPPING_WRAP)?;
        let wide: Vec<u16> = text.encode_utf16().collect();
        dwrite.CreateTextLayout(&wide, &format, width.max(1.0), 1_000_000.0)
    }
}

#[windows::core::implement(
    windows::Graphics::IGeometrySource2D,
    windows::Win32::System::WinRT::Graphics::Direct2D::IGeometrySource2DInterop
)]
struct GeometrySource(ID2D1Geometry);

impl windows::Graphics::IGeometrySource2D_Impl for GeometrySource_Impl {}

impl windows::Win32::System::WinRT::Graphics::Direct2D::IGeometrySource2DInterop_Impl for GeometrySource_Impl {
    fn GetGeometry(&self) -> Result<ID2D1Geometry> {
        Ok(self.0.clone())
    }

    fn TryGetGeometryUsingFactory(&self, _factory: windows::core::Ref<ID2D1Factory>) -> Result<ID2D1Geometry> {
        Err(windows::core::Error::from_hresult(windows::Win32::Foundation::E_NOTIMPL))
    }
}

pub fn composition_path(geometry: &ID2D1PathGeometry) -> Result<windows::UI::Composition::CompositionPath> {
    let source: windows::Graphics::IGeometrySource2D = GeometrySource(geometry.cast()?).into();
    windows::UI::Composition::CompositionPath::Create(&source)
}
