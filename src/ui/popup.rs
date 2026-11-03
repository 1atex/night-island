use windows::Win32::Foundation::{COLORREF, HWND, POINT, SIZE};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::WindowsAndMessaging::{ULW_ALPHA, UpdateLayeredWindow};
use windows::core::{Error, Result};

use super::gfx::{Gpu, Painter};

pub const POPUP_MARGIN: f32 = 10.0;

pub fn hit_panel(point: (f32, f32), size: (f32, f32), scale: f32) -> bool {
    let margin = POPUP_MARGIN * scale;
    if size.0 <= 2.0 * margin || size.1 <= 2.0 * margin {
        return false;
    }
    let radius = (28.0 * scale).min((size.0 - 2.0 * margin) / 2.0).min((size.1 - 2.0 * margin) / 2.0);
    let cx = point.0.clamp(margin + radius, size.0 - margin - radius);
    let cy = point.1.clamp(margin + radius, size.1 - margin - radius);
    (point.0 - cx).powi(2) + (point.1 - cy).powi(2) <= radius * radius
}

pub struct PopupSurface {
    dc: HDC,
    bitmap: HBITMAP,
    size: (i32, i32),
}

impl PopupSurface {
    pub fn new(size: (i32, i32)) -> Result<Self> {
        unsafe {
            let screen = GetDC(None);
            let dc = CreateCompatibleDC(Some(screen));
            ReleaseDC(None, screen);
            if dc.0.is_null() {
                return Err(Error::from_thread());
            }
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: size.0,
                    biHeight: -size.1,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut bits = std::ptr::null_mut();
            let bitmap = match CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0) {
                Ok(bitmap) => bitmap,
                Err(error) => {
                    let _ = DeleteDC(dc);
                    return Err(error);
                }
            };
            SelectObject(dc, bitmap.into());
            Ok(Self { dc, bitmap, size })
        }
    }

    pub fn paint(&self, gpu: &Gpu, draw: impl FnOnce(&Painter) -> Result<()>) -> Result<()> {
        let mut result = Ok(());
        gpu.paint_dc(self.dc, self.size, |p| result = draw(p))?;
        result
    }

    pub fn present(&self, hwnd: HWND, at: Option<POINT>, alpha: u8) -> Result<()> {
        unsafe {
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: alpha,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            UpdateLayeredWindow(
                hwnd,
                None,
                at.as_ref().map(|p| p as *const _),
                Some(&SIZE { cx: self.size.0, cy: self.size.1 }),
                Some(self.dc),
                Some(&POINT::default()),
                COLORREF(0),
                Some(&blend),
                ULW_ALPHA,
            )
        }
    }

    #[cfg(test)]
    pub fn pixels(&self) -> Vec<u8> {
        let mut bitmap = BITMAP::default();
        unsafe {
            assert_ne!(GetObjectW(self.bitmap.into(), size_of::<BITMAP>() as i32, Some((&raw mut bitmap).cast())), 0);
            std::slice::from_raw_parts(bitmap.bmBits.cast(), (self.size.0 * self.size.1 * 4) as usize).to_vec()
        }
    }
}

impl Drop for PopupSurface {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteDC(self.dc);
            let _ = DeleteObject(self.bitmap.into());
        }
    }
}
