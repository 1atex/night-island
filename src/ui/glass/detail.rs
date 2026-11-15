use std::sync::Mutex;

use windows::Foundation::{IPropertyValue, PropertyValue};
use windows::Graphics::Effects::{
    IGraphicsEffect, IGraphicsEffect_Impl, IGraphicsEffectSource, IGraphicsEffectSource_Impl,
};
use windows::UI::Composition::{
    CompositionBrush, CompositionEffectBrush, CompositionEffectFactory, CompositionEffectSourceParameter, Compositor,
};
use windows::Win32::Foundation::{E_BOUNDS, E_INVALIDARG, E_POINTER};
use windows::Win32::Graphics::Direct2D::Common::D2D1_BLEND_MODE_LINEAR_LIGHT;
use windows::Win32::Graphics::Direct2D::{
    CLSID_D2D1ArithmeticComposite, CLSID_D2D1Blend, CLSID_D2D1GaussianBlur, CLSID_D2D1Invert,
};
use windows::Win32::System::WinRT::Graphics::Direct2D::{
    GRAPHICS_EFFECT_PROPERTY_MAPPING, GRAPHICS_EFFECT_PROPERTY_MAPPING_DIRECT, IGraphicsEffectD2D1Interop,
    IGraphicsEffectD2D1Interop_Impl,
};
use windows::core::{Error, GUID, HSTRING, Interface, PCWSTR, Result, h, implement};

#[implement(IGraphicsEffect, IGraphicsEffectSource, IGraphicsEffectD2D1Interop)]
struct Effect {
    class: GUID,
    name: Mutex<HSTRING>,
    properties: Vec<IPropertyValue>,
    sources: Vec<IGraphicsEffectSource>,
}

impl IGraphicsEffectSource_Impl for Effect_Impl {}

impl IGraphicsEffect_Impl for Effect_Impl {
    fn Name(&self) -> Result<HSTRING> {
        Ok(self.name.lock().unwrap_or_else(|e| e.into_inner()).clone())
    }

    fn SetName(&self, name: &HSTRING) -> Result<()> {
        *self.name.lock().unwrap_or_else(|e| e.into_inner()) = name.clone();
        Ok(())
    }
}

impl IGraphicsEffectD2D1Interop_Impl for Effect_Impl {
    fn GetEffectId(&self) -> Result<GUID> {
        Ok(self.class)
    }

    fn GetNamedPropertyMapping(
        &self,
        name: &PCWSTR,
        index: *mut u32,
        mapping: *mut GRAPHICS_EFFECT_PROPERTY_MAPPING,
    ) -> Result<()> {
        if name.is_null() || index.is_null() || mapping.is_null() {
            return Err(Error::from_hresult(E_POINTER));
        }
        if self.class != CLSID_D2D1GaussianBlur || unsafe { name.to_string()? } != "BlurAmount" {
            return Err(Error::from_hresult(E_INVALIDARG));
        }
        unsafe {
            *index = 0;
            *mapping = GRAPHICS_EFFECT_PROPERTY_MAPPING_DIRECT;
        }
        Ok(())
    }

    fn GetPropertyCount(&self) -> Result<u32> {
        Ok(self.properties.len() as u32)
    }

    fn GetProperty(&self, index: u32) -> Result<IPropertyValue> {
        self.properties.get(index as usize).cloned().ok_or_else(|| Error::from_hresult(E_BOUNDS))
    }

    fn GetSourceCount(&self) -> Result<u32> {
        Ok(self.sources.len() as u32)
    }

    fn GetSource(&self, index: u32) -> Result<IGraphicsEffectSource> {
        self.sources.get(index as usize).cloned().ok_or_else(|| Error::from_hresult(E_BOUNDS))
    }
}

pub(super) fn sampling_brush(
    c: &Compositor,
    backdrop: &CompositionBrush,
    scale: f32,
) -> Result<CompositionEffectBrush> {
    let scene = CompositionEffectSourceParameter::Create(h!("scene"))?.cast::<IGraphicsEffectSource>()?;
    let frost: IGraphicsEffect = Effect {
        class: CLSID_D2D1GaussianBlur,
        name: Default::default(),
        properties: vec![
            PropertyValue::CreateSingle(1.2 * scale)?.cast()?,
            PropertyValue::CreateUInt32(0)?.cast()?,
            PropertyValue::CreateUInt32(1)?.cast()?,
        ],
        sources: vec![scene],
    }
    .into();
    let brush = c.CreateEffectFactory(&frost)?.CreateBrush()?;
    brush.SetSourceParameter(h!("scene"), backdrop)?;
    Ok(brush)
}

pub(super) fn refraction_factory(c: &Compositor, scale: f32) -> Result<CompositionEffectFactory> {
    let body = CompositionEffectSourceParameter::Create(h!("body"))?.cast::<IGraphicsEffectSource>()?;
    let shifted = CompositionEffectSourceParameter::Create(h!("shifted"))?.cast::<IGraphicsEffectSource>()?;
    let band: IGraphicsEffectSource = Effect {
        class: CLSID_D2D1ArithmeticComposite,
        name: Default::default(),
        properties: vec![
            PropertyValue::CreateSingleArray(&[0.0, 1.0, -1.0, 0.0])?.cast()?,
            PropertyValue::CreateBoolean(true)?.cast()?,
        ],
        sources: vec![body, shifted],
    }
    .into();
    let feather: IGraphicsEffectSource = Effect {
        class: CLSID_D2D1GaussianBlur,
        name: Default::default(),
        properties: vec![
            PropertyValue::CreateSingle(0.85 * scale)?.cast()?,
            PropertyValue::CreateUInt32(0)?.cast()?,
            PropertyValue::CreateUInt32(0)?.cast()?,
        ],
        sources: vec![band],
    }
    .into();
    let scene = CompositionEffectSourceParameter::Create(h!("refracted"))?.cast::<IGraphicsEffectSource>()?;
    let sampled = CompositionEffectSourceParameter::Create(h!("sampled"))?.cast::<IGraphicsEffectSource>()?;
    let inverse: IGraphicsEffectSource =
        Effect { class: CLSID_D2D1Invert, name: Default::default(), properties: vec![], sources: vec![sampled] }.into();
    let displacement: IGraphicsEffectSource = Effect {
        class: CLSID_D2D1ArithmeticComposite,
        name: Default::default(),
        properties: vec![
            PropertyValue::CreateSingleArray(&[0.0, 0.5, 0.5, 0.0])?.cast()?,
            PropertyValue::CreateBoolean(true)?.cast()?,
        ],
        sources: vec![scene, inverse],
    }
    .into();
    let refraction: IGraphicsEffect = Effect {
        class: CLSID_D2D1ArithmeticComposite,
        name: Default::default(),
        properties: vec![
            PropertyValue::CreateSingleArray(&[1.0, 0.0, 0.0, 0.0])?.cast()?,
            PropertyValue::CreateBoolean(true)?.cast()?,
        ],
        sources: vec![displacement, feather],
    }
    .into();
    c.CreateEffectFactory(&refraction)
}

pub(super) fn light_matched_edges(c: &Compositor) -> Result<CompositionEffectBrush> {
    let live = CompositionEffectSourceParameter::Create(h!("live"))?.cast::<IGraphicsEffectSource>()?;
    let edge = CompositionEffectSourceParameter::Create(h!("edge"))?.cast::<IGraphicsEffectSource>()?;
    let refraction: IGraphicsEffect = Effect {
        class: CLSID_D2D1Blend,
        name: Default::default(),
        properties: vec![PropertyValue::CreateUInt32(D2D1_BLEND_MODE_LINEAR_LIGHT.0 as u32)?.cast()?],
        sources: vec![live, edge],
    }
    .into();
    c.CreateEffectFactory(&refraction)?.CreateBrush()
}
