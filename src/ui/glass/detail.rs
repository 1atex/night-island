use std::sync::Mutex;

use windows::Foundation::{IPropertyValue, PropertyValue};
use windows::Graphics::Effects::{
    IGraphicsEffect, IGraphicsEffect_Impl, IGraphicsEffectSource, IGraphicsEffectSource_Impl,
};
use windows::UI::Composition::{
    CompositionBrush, CompositionEffectBrush, CompositionEffectFactory, CompositionEffectSourceParameter, Compositor,
};
use windows::Win32::Foundation::{E_BOUNDS, E_INVALIDARG, E_POINTER};
use windows::Win32::Graphics::Direct2D::{CLSID_D2D1ArithmeticComposite, CLSID_D2D1GaussianBlur};
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

pub(super) fn detail_brush(c: &Compositor, backdrop: &CompositionBrush, scale: f32) -> Result<CompositionEffectBrush> {
    let scene = CompositionEffectSourceParameter::Create(h!("scene"))?.cast::<IGraphicsEffectSource>()?;
    let low_pass: IGraphicsEffectSource = Effect {
        class: CLSID_D2D1GaussianBlur,
        name: Default::default(),
        properties: vec![
            PropertyValue::CreateSingle(27.0 * scale)?.cast()?,
            PropertyValue::CreateUInt32(0)?.cast()?,
            PropertyValue::CreateUInt32(1)?.cast()?,
        ],
        sources: vec![scene.clone()],
    }
    .into();
    let sharpen: IGraphicsEffectSource = Effect {
        class: CLSID_D2D1ArithmeticComposite,
        name: Default::default(),
        properties: vec![
            PropertyValue::CreateSingleArray(&[0.0, 5.5, -4.5, 0.0])?.cast()?,
            PropertyValue::CreateBoolean(true)?.cast()?,
        ],
        sources: vec![scene, low_pass],
    }
    .into();
    let frost: IGraphicsEffect = Effect {
        class: CLSID_D2D1GaussianBlur,
        name: Mutex::new(HSTRING::from("frost")),
        properties: vec![
            PropertyValue::CreateSingle(2.8 * scale)?.cast()?,
            PropertyValue::CreateUInt32(0)?.cast()?,
            PropertyValue::CreateUInt32(1)?.cast()?,
        ],
        sources: vec![sharpen],
    }
    .into();
    let properties: windows_collections::IIterable<HSTRING> = vec![HSTRING::from("frost.BlurAmount")].into();
    let brush = c.CreateEffectFactoryWithProperties(&frost, &properties)?.CreateBrush()?;
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
            PropertyValue::CreateSingle(1.8 * scale)?.cast()?,
            PropertyValue::CreateUInt32(0)?.cast()?,
            PropertyValue::CreateUInt32(0)?.cast()?,
        ],
        sources: vec![band],
    }
    .into();
    let scene = CompositionEffectSourceParameter::Create(h!("refracted"))?.cast::<IGraphicsEffectSource>()?;
    let refraction: IGraphicsEffect = Effect {
        class: CLSID_D2D1ArithmeticComposite,
        name: Default::default(),
        properties: vec![
            PropertyValue::CreateSingleArray(&[1.0, 0.0, 0.0, 0.0])?.cast()?,
            PropertyValue::CreateBoolean(true)?.cast()?,
        ],
        sources: vec![scene, feather],
    }
    .into();
    c.CreateEffectFactory(&refraction)
}
