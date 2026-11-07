#[cfg(test)]
use std::cell::Cell;
use std::cell::RefCell;
use std::collections::HashMap;

use windows::Win32::Graphics::DirectWrite::*;
use windows::core::{BOOL, Interface, PCWSTR, Result, w};

use crate::data::settings::InterfaceFont;

#[cfg(not(test))]
static SELECTED: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

thread_local! {
    #[cfg(test)]
    static CHOICE: Cell<InterfaceFont> = const { Cell::new(InterfaceFont::Segoe) };
    static FONTS: RefCell<Option<Fonts>> = const { RefCell::new(None) };
}

struct Fonts {
    factory: IDWriteFactory,
    loader: IDWriteInMemoryFontFileLoader,
    montserrat: IDWriteFontCollection,
    sf: Option<PCWSTR>,
    widths: HashMap<(InterfaceFont, String, u32, i32), f32>,
    native: Vec<windows::Win32::Foundation::HANDLE>,
}

impl Fonts {
    fn new() -> Result<Self> {
        unsafe {
            let factory: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            let modern = factory.cast::<IDWriteFactory5>()?;
            let loader = modern.CreateInMemoryFontFileLoader()?;
            factory.RegisterFontFileLoader(&loader)?;
            let result = (|| {
                let builder = modern.CreateFontSetBuilder()?;
                for data in [
                    include_bytes!("../../assets/fonts/Montserrat-Regular.ttf").as_slice(),
                    include_bytes!("../../assets/fonts/Montserrat-SemiBold.ttf").as_slice(),
                ] {
                    let file = loader.CreateInMemoryFontFileReference(
                        &factory,
                        data.as_ptr().cast(),
                        data.len() as u32,
                        None,
                    )?;
                    builder.AddFontFile(&file)?;
                }
                let montserrat = modern.CreateFontCollectionFromFontSet(&builder.CreateFontSet()?)?.cast()?;
                let mut system = None;
                factory.GetSystemFontCollection(&mut system, false)?;
                let sf = system.and_then(|system| {
                    [w!("SF Pro Text"), w!("SF Pro Display"), w!("SF Pro")].into_iter().find(|name| {
                        let (mut index, mut exists) = (0, BOOL(0));
                        system.FindFamilyName(*name, &mut index, &mut exists).is_ok() && exists.as_bool()
                    })
                });
                Ok((montserrat, sf))
            })();
            match result {
                Ok((montserrat, sf)) => {
                    Ok(Self { factory, loader, montserrat, sf, widths: HashMap::new(), native: Vec::new() })
                }
                Err(error) => {
                    let _ = factory.UnregisterFontFileLoader(&loader);
                    Err(error)
                }
            }
        }
    }

    fn format(&self, choice: InterfaceFont, size: f32, weight: DWRITE_FONT_WEIGHT) -> Result<IDWriteTextFormat> {
        let (name, collection) = match choice {
            InterfaceFont::Montserrat => (w!("Montserrat"), Some(&self.montserrat)),
            InterfaceFont::SanFrancisco => (self.sf.unwrap_or(w!("Segoe UI")), None),
            InterfaceFont::Segoe => (w!("Segoe UI"), None),
        };
        unsafe {
            self.factory.CreateTextFormat(
                name,
                collection,
                weight,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                size,
                w!("en-us"),
            )
        }
    }
}

impl Drop for Fonts {
    fn drop(&mut self) {
        unsafe {
            let _ = self.factory.UnregisterFontFileLoader(&self.loader);
            for handle in &self.native {
                let _ = windows::Win32::Graphics::Gdi::RemoveFontMemResourceEx(*handle);
            }
        }
    }
}

fn with_fonts<T>(action: impl FnOnce(&mut Fonts) -> Result<T>) -> Result<T> {
    FONTS.with_borrow_mut(|fonts| {
        if fonts.is_none() {
            *fonts = Some(Fonts::new()?);
        }
        action(fonts.as_mut().unwrap())
    })
}

pub fn set(choice: InterfaceFont) {
    #[cfg(not(test))]
    SELECTED.store(choice as u8, std::sync::atomic::Ordering::Relaxed);
    #[cfg(test)]
    CHOICE.set(choice);
}

pub fn selected() -> InterfaceFont {
    #[cfg(not(test))]
    {
        InterfaceFont::ALL[usize::from(SELECTED.load(std::sync::atomic::Ordering::Relaxed)).min(2)]
    }
    #[cfg(test)]
    {
        CHOICE.get()
    }
}

pub fn format(factory: &IDWriteFactory, size: f32, weight: DWRITE_FONT_WEIGHT) -> Result<IDWriteTextFormat> {
    if selected() != InterfaceFont::Segoe
        && let Ok(format) = with_fonts(|fonts| fonts.format(selected(), size, weight))
    {
        return Ok(format);
    }
    unsafe {
        factory.CreateTextFormat(
            w!("Segoe UI"),
            None,
            weight,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            size,
            w!("en-us"),
        )
    }
}

pub fn width(choice: InterfaceFont, text: &str, size: f32, weight: DWRITE_FONT_WEIGHT) -> f32 {
    with_fonts(|fonts| {
        let key = (choice, text.to_owned(), size.to_bits(), weight.0);
        if let Some(width) = fonts.widths.get(&key) {
            return Ok(*width);
        }
        let format = fonts.format(choice, size, weight)?;
        let wide: Vec<_> = text.encode_utf16().collect();
        let mut metrics = DWRITE_TEXT_METRICS::default();
        unsafe {
            fonts.factory.CreateTextLayout(&wide, &format, 100_000.0, 1000.0)?.GetMetrics(&mut metrics)?;
        }
        if fonts.widths.len() >= 256 {
            fonts.widths.clear();
        }
        fonts.widths.insert(key, metrics.widthIncludingTrailingWhitespace);
        Ok(metrics.widthIncludingTrailingWhitespace)
    })
    .unwrap_or_else(|_| text.chars().count() as f32 * size * 0.65)
}

pub fn sf_available() -> bool {
    with_fonts(|fonts| Ok(fonts.sf.is_some())).unwrap_or(false)
}

pub fn native_family() -> PCWSTR {
    with_fonts(|fonts| {
        if selected() == InterfaceFont::Montserrat && fonts.native.is_empty() {
            for data in [
                include_bytes!("../../assets/fonts/Montserrat-Regular.ttf").as_slice(),
                include_bytes!("../../assets/fonts/Montserrat-SemiBold.ttf").as_slice(),
            ] {
                let mut count = 0;
                let handle = unsafe {
                    windows::Win32::Graphics::Gdi::AddFontMemResourceEx(
                        data.as_ptr().cast(),
                        data.len() as u32,
                        None,
                        &raw mut count,
                    )
                };
                if !handle.is_invalid() {
                    fonts.native.push(handle);
                }
            }
        }
        Ok(match selected() {
            InterfaceFont::Montserrat if !fonts.native.is_empty() => w!("Montserrat"),
            InterfaceFont::SanFrancisco => fonts.sf.unwrap_or(w!("Segoe UI")),
            _ => w!("Segoe UI"),
        })
    })
    .unwrap_or(w!("Segoe UI"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_fonts_resolve_without_installing_and_keep_both_alphabets() -> Result<()> {
        let fonts = Fonts::new()?;
        for choice in InterfaceFont::ALL {
            for weight in [DWRITE_FONT_WEIGHT_NORMAL, DWRITE_FONT_WEIGHT_SEMI_BOLD] {
                let format = fonts.format(choice, 15.0, weight)?;
                let mut name = vec![0; unsafe { format.GetFontFamilyNameLength() } as usize + 1];
                unsafe {
                    format.GetFontFamilyName(&mut name)?;
                }
                if choice == InterfaceFont::Montserrat {
                    assert_eq!(String::from_utf16_lossy(&name[..name.len() - 1]), "Montserrat");
                }
                let width = width(choice, "Привет, Island! 21:51", 15.0, weight);
                assert!(width > 90.0 && width < 250.0, "{choice:?}: {width}");
            }
        }
        set(InterfaceFont::Montserrat);
        assert_eq!(selected(), InterfaceFont::Montserrat);
        let saved = serde_json::to_string(&crate::data::settings::Settings {
            interface_font: selected(),
            ..Default::default()
        })
        .unwrap();
        let loaded: crate::data::settings::Settings = serde_json::from_str(&saved).unwrap();
        assert_eq!(loaded.interface_font, InterfaceFont::Montserrat);
        assert_eq!(
            serde_json::from_str::<crate::data::settings::Settings>("{}").unwrap().interface_font,
            InterfaceFont::Segoe
        );
        set(InterfaceFont::Segoe);
        Ok(())
    }
}
