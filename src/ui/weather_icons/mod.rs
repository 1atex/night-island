mod assets;

#[cfg(test)]
mod tests;

use std::cell::Cell;
use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use serde::Deserialize;
use windows::Win32::Foundation::E_INVALIDARG;
use windows::Win32::Graphics::Direct2D::Common::D2D_SIZE_F;
use windows::Win32::Graphics::Direct2D::{
    D2D1_SVG_ATTRIBUTE_POD_TYPE_COLOR, D2D1_SVG_ATTRIBUTE_POD_TYPE_FLOAT, D2D1_SVG_ATTRIBUTE_POD_TYPE_LENGTH,
    D2D1_SVG_ATTRIBUTE_POD_TYPE_MATRIX, D2D1_SVG_LENGTH, D2D1_SVG_LENGTH_UNITS_NUMBER, ID2D1DeviceContext5,
    ID2D1RenderTarget, ID2D1SvgDocument, ID2D1SvgElement, ID2D1SvgPathData,
};
use windows::Win32::UI::Shell::SHCreateMemStream;
use windows::Win32::UI::WindowsAndMessaging::{
    SPI_GETCLIENTAREAANIMATION, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
};
use windows::core::{BOOL, Error, HSTRING, Interface, Result, w};
use windows_numerics::Matrix3x2;

type Tracks = HashMap<String, Vec<Track>>;

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Kind {
    Path,
    Opacity,
    Transform,
    DashOffset,
    Color,
}

#[derive(Deserialize)]
struct Keyframe {
    at: f64,
    value: Vec<f32>,
    easing: [f32; 4],
}

#[derive(Deserialize)]
struct Track {
    id: String,
    kind: Kind,
    duration: f64,
    delay: f64,
    origin: [f32; 2],
    frames: Vec<Keyframe>,
}

fn tracks() -> &'static Tracks {
    static TRACKS: OnceLock<Tracks> = OnceLock::new();
    TRACKS.get_or_init(|| {
        serde_json::from_str(include_str!("../../../assets/weather/animation.json")).expect("weather animation data")
    })
}

fn ease([x1, y1, x2, y2]: [f32; 4], t: f32) -> f32 {
    if x1 == y1 && x2 == y2 {
        return t;
    }
    let at = |a: f32, b: f32, s: f32| 3.0 * a * (1.0 - s).powi(2) * s + 3.0 * b * (1.0 - s) * s * s + s * s * s;
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..16 {
        let mid = (lo + hi) * 0.5;
        if at(x1, x2, mid) < t {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    at(y1, y2, (lo + hi) * 0.5)
}

impl Track {
    fn sample(&self, elapsed: f64, buffer: &mut [f32]) {
        let progress = ((elapsed - self.delay).max(0.0) / self.duration).fract();
        let upper = self.frames.partition_point(|frame| frame.at <= progress).clamp(1, self.frames.len() - 1);
        let (a, b) = (&self.frames[upper - 1], &self.frames[upper]);
        let t = ((progress - a.at) / (b.at - a.at)).clamp(0.0, 1.0) as f32;
        let t = ease(a.easing, t);
        for ((out, a), b) in buffer.iter_mut().zip(&a.value).zip(&b.value) {
            *out = a + (b - a) * t;
        }
    }
}

struct Target {
    element: ID2D1SvgElement,
    path: Option<ID2D1SvgPathData>,
    track: &'static Track,
    buffer: Vec<f32>,
}

impl Target {
    fn new(doc: &ID2D1SvgDocument, track: &'static Track) -> Result<Self> {
        let id = HSTRING::from(&track.id);
        let element = unsafe { doc.FindElementById(&id)? };
        let path =
            if matches!(track.kind, Kind::Path) { Some(unsafe { element.GetAttributeValue(w!("d"))? }) } else { None };
        let buffer = vec![0.0; track.frames[0].value.len()];
        Ok(Self { element, path, track, buffer })
    }

    fn apply(&mut self, elapsed: f64) -> Result<()> {
        self.track.sample(elapsed, &mut self.buffer);
        unsafe {
            match self.track.kind {
                Kind::Path => self
                    .path
                    .as_ref()
                    .ok_or_else(|| Error::from_hresult(E_INVALIDARG))?
                    .UpdateSegmentData(&self.buffer, 0),
                Kind::Opacity => self.element.SetAttributeValue2(
                    w!("opacity"),
                    D2D1_SVG_ATTRIBUTE_POD_TYPE_FLOAT,
                    self.buffer.as_ptr().cast(),
                    size_of::<f32>() as u32,
                ),
                Kind::DashOffset => {
                    let length = D2D1_SVG_LENGTH { value: self.buffer[0], units: D2D1_SVG_LENGTH_UNITS_NUMBER };
                    self.element.SetAttributeValue2(
                        w!("stroke-dashoffset"),
                        D2D1_SVG_ATTRIBUTE_POD_TYPE_LENGTH,
                        (&raw const length).cast(),
                        size_of::<D2D1_SVG_LENGTH>() as u32,
                    )
                }
                Kind::Color => self.element.SetAttributeValue2(
                    w!("stop-color"),
                    D2D1_SVG_ATTRIBUTE_POD_TYPE_COLOR,
                    self.buffer.as_ptr().cast(),
                    (4 * size_of::<f32>()) as u32,
                ),
                Kind::Transform => {
                    let [tx, ty, angle] = self.buffer[..] else { return Err(Error::from_hresult(E_INVALIDARG)) };
                    let (sin, cos) = angle.to_radians().sin_cos();
                    let [ox, oy] = self.track.origin;
                    let matrix = Matrix3x2 {
                        M11: cos,
                        M12: sin,
                        M21: -sin,
                        M22: cos,
                        M31: tx + ox - ox * cos + oy * sin,
                        M32: ty + oy - ox * sin - oy * cos,
                    };
                    self.element.SetAttributeValue2(
                        w!("transform"),
                        D2D1_SVG_ATTRIBUTE_POD_TYPE_MATRIX,
                        (&raw const matrix).cast(),
                        size_of::<Matrix3x2>() as u32,
                    )
                }
            }
        }
    }
}

struct Icon {
    doc: ID2D1SvgDocument,
    targets: Vec<Target>,
    last_time: Option<f64>,
}

impl Icon {
    fn new(dc: &ID2D1DeviceContext5, name: &'static str) -> Result<Self> {
        let data = assets::ASSETS
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, data)| *data)
            .ok_or_else(|| Error::from_hresult(E_INVALIDARG))?;
        let stream = unsafe { SHCreateMemStream(Some(data)) }.ok_or_else(Error::from_thread)?;
        let doc = unsafe { dc.CreateSvgDocument(&stream, D2D_SIZE_F { width: 24.0, height: 24.0 })? };
        let targets = tracks()
            .get(name)
            .ok_or_else(|| Error::from_hresult(E_INVALIDARG))?
            .iter()
            .map(|track| Target::new(&doc, track))
            .collect::<Result<Vec<_>>>()?;
        Ok(Self { doc, targets, last_time: None })
    }

    fn update(&mut self, elapsed: f64) -> Result<()> {
        if self.last_time == Some(elapsed) {
            return Ok(());
        }
        for target in &mut self.targets {
            target.apply(elapsed)?;
        }
        self.last_time = Some(elapsed);
        Ok(())
    }
}

pub struct WeatherIcons {
    icons: HashMap<&'static str, Icon>,
    born: Instant,
}

impl Default for WeatherIcons {
    fn default() -> Self {
        Self { icons: HashMap::new(), born: Instant::now() }
    }
}

impl WeatherIcons {
    pub fn draw(
        &mut self,
        target: &ID2D1RenderTarget,
        name: &'static str,
        rect: (f32, f32, f32, f32),
        animated: bool,
    ) -> Result<()> {
        let dc: ID2D1DeviceContext5 = target.cast()?;
        if !self.icons.contains_key(name) {
            self.icons.insert(name, Icon::new(&dc, name)?);
        }
        let elapsed = if animated && animations_enabled() { self.born.elapsed().as_secs_f64() } else { 0.0 };
        let icon = self.icons.get_mut(name).ok_or_else(|| Error::from_hresult(E_INVALIDARG))?;
        icon.update(elapsed)?;
        let side = (rect.2 - rect.0).min(rect.3 - rect.1);
        let scale = side / 24.0;
        let dx = (rect.0 + rect.2 - side) * 0.5;
        let dy = (rect.1 + rect.3 - side) * 0.5;
        unsafe {
            let mut old = Matrix3x2::identity();
            dc.GetTransform(&mut old);
            let local = Matrix3x2 { M11: scale, M22: scale, M31: dx, M32: dy, ..Matrix3x2::identity() };
            dc.SetTransform(&(local * old));
            dc.DrawSvgDocument(&icon.doc);
            dc.SetTransform(&old);
        }
        Ok(())
    }
}

pub fn animations_enabled() -> bool {
    thread_local! {
        static ENABLED: Cell<Option<(Instant, bool)>> = const { Cell::new(None) };
    }
    ENABLED.with(|cached| {
        if let Some((at, enabled)) = cached.get()
            && at.elapsed() < Duration::from_secs(2)
        {
            return enabled;
        }
        let mut enabled = BOOL(1);
        let _ = unsafe {
            SystemParametersInfoW(
                SPI_GETCLIENTAREAANIMATION,
                0,
                Some((&raw mut enabled).cast()),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            )
        };
        let enabled = enabled.as_bool();
        cached.set(Some((Instant::now(), enabled)));
        enabled
    })
}
