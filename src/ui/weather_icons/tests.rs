use super::*;

use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct2D::Common::{D2D_SIZE_U, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_PIXEL_FORMAT};
use windows::Win32::Graphics::Direct2D::{
    D2D1_BITMAP_OPTIONS_CANNOT_DRAW, D2D1_BITMAP_OPTIONS_CPU_READ, D2D1_BITMAP_OPTIONS_TARGET, D2D1_BITMAP_PROPERTIES1,
    D2D1_DEVICE_CONTEXT_OPTIONS_NONE, D2D1_MAP_OPTIONS_READ, D2D1CreateDevice, ID2D1Bitmap1,
};
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_WARP;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice, ID3D11Device,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Dxgi::IDXGIDevice;

#[test]
fn animation_tracks_are_complete_and_finite() {
    assert_eq!(tracks().len(), assets::ASSETS.len());
    for (name, _) in assets::ASSETS {
        let tracks = &tracks()[*name];
        assert!(!tracks.is_empty(), "{name}");
        for track in tracks {
            assert!(track.duration > 0.0);
            assert!(track.frames.len() >= 2);
            assert_eq!(track.frames.first().unwrap().at, 0.0, "{name}: {}", track.id);
            assert_eq!(track.frames.last().unwrap().at, 1.0, "{name}: {}", track.id);
            assert!(track.frames.windows(2).all(|pair| pair[0].at < pair[1].at));
            let size = track.frames[0].value.len();
            assert!(size > 0);
            assert!(
                track.frames.iter().all(|frame| frame.value.len() == size && frame.value.iter().all(|v| v.is_finite()))
            );
            let mut buffer = vec![0.0; size];
            for time in [0.0, 0.1, 0.5, 1.0, 10.0, 1000.0, 1_000_000.0] {
                track.sample(time, &mut buffer);
                assert!(buffer.iter().all(|value| value.is_finite()), "{name}");
            }
        }
    }
}

#[test]
fn clouds_morph_without_moving_their_baseline_or_jumping_at_the_seam() {
    let track = tracks()["rain"]
        .iter()
        .find(|track| matches!(track.kind, Kind::Path) && track.frames[0].value.len() > 8)
        .unwrap();
    let mut first = vec![0.0; track.frames[0].value.len()];
    let mut next = first.clone();
    track.sample(track.delay + track.duration, &mut first);
    track.sample(track.delay + 2.0 * track.duration - 0.00001, &mut next);
    assert!(first.iter().zip(&next).all(|(a, b)| (a - b).abs() < 0.001));
    track.sample(track.duration * 1.34 + track.delay, &mut next);
    assert!(first.iter().zip(&next).any(|(a, b)| (a - b).abs() > 0.1));
    for phase in [1.0, 1.17, 1.34, 1.51, 1.68, 1.84] {
        track.sample(track.delay + phase * track.duration, &mut next);
        assert_eq!(next[1], 14.5);
        assert_eq!(next[37], 14.5);
    }
}

fn context() -> Result<ID2D1DeviceContext5> {
    unsafe {
        let mut d3d: Option<ID3D11Device> = None;
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_WARP,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut d3d),
            None,
            None,
        )?;
        let dxgi: IDXGIDevice = d3d.ok_or_else(Error::empty)?.cast()?;
        D2D1CreateDevice(&dxgi, None)?.CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)?.cast()
    }
}

fn bitmap(dc: &ID2D1DeviceContext5, read: bool) -> Result<ID2D1Bitmap1> {
    let props = D2D1_BITMAP_PROPERTIES1 {
        pixelFormat: D2D1_PIXEL_FORMAT { format: DXGI_FORMAT_B8G8R8A8_UNORM, alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED },
        dpiX: 96.0,
        dpiY: 96.0,
        bitmapOptions: if read {
            D2D1_BITMAP_OPTIONS_CPU_READ | D2D1_BITMAP_OPTIONS_CANNOT_DRAW
        } else {
            D2D1_BITMAP_OPTIONS_TARGET
        },
        ..Default::default()
    };
    unsafe { dc.CreateBitmap(D2D_SIZE_U { width: 128, height: 128 }, None, 0, &props) }
}

fn pixels(bitmap: &ID2D1Bitmap1, staging: &ID2D1Bitmap1) -> Result<Vec<u8>> {
    unsafe {
        staging.CopyFromBitmap(None, bitmap, None)?;
        let mapped = staging.Map(D2D1_MAP_OPTIONS_READ)?;
        let mut pixels = Vec::with_capacity(128 * 128 * 4);
        for y in 0..128 {
            let row = std::slice::from_raw_parts(mapped.bits.add(y * mapped.pitch as usize), 128 * 4);
            for pixel in row.chunks_exact(4) {
                let channel =
                    |c: u8| if pixel[3] == 0 { 0 } else { (u32::from(c) * 255 / u32::from(pixel[3])).min(255) as u8 };
                pixels.extend_from_slice(&[channel(pixel[2]), channel(pixel[1]), channel(pixel[0]), pixel[3]]);
            }
        }
        staging.Unmap()?;
        Ok(pixels)
    }
}

#[test]
#[ignore = "Needs a real Direct3D 11 / Direct2D device, which wine does not provide"]
fn every_native_icon_renders_and_updates_in_direct2d() -> Result<()> {
    let dc = context()?;
    let target = bitmap(&dc, false)?;
    let staging = bitmap(&dc, true)?;
    unsafe {
        dc.SetTarget(&target);
        dc.SetTransform(&Matrix3x2 { M11: 128.0 / 24.0, M22: 128.0 / 24.0, ..Matrix3x2::identity() });
    }
    for (name, _) in assets::ASSETS {
        let mut icon = Icon::new(&dc, name).unwrap_or_else(|error| panic!("{name}: {error}"));
        let mut snapshots = Vec::new();
        for time in [0.0, 0.37, 1.9, 4.8, 9.7] {
            icon.update(time).unwrap_or_else(|error| panic!("{name} at {time}: {error}"));
            unsafe {
                dc.BeginDraw();
                dc.Clear(None);
                dc.DrawSvgDocument(&icon.doc);
                dc.EndDraw(None, None)?;
            }
            let rgba = pixels(&target, &staging)?;
            assert!(rgba.chunks_exact(4).filter(|pixel| pixel[3] > 20).count() > 12, "empty icon: {name} at {time}");
            snapshots.push(rgba);
        }
        assert!(snapshots[1..].iter().any(|frame| frame != &snapshots[0]), "static animation: {name}");
        if let Ok(dir) = std::env::var("NI_WEATHER_PREVIEW_DIR") {
            let dir = std::path::Path::new(&dir);
            std::fs::create_dir_all(dir).unwrap();
            let file = std::fs::File::create(dir.join(format!("{name}.png"))).unwrap();
            let mut encoder = png::Encoder::new(file, 128, 128);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.write_header().unwrap().write_image_data(&snapshots[0]).unwrap();
        }
    }
    Ok(())
}

#[test]
#[ignore = "Needs a real Direct3D 11 / Direct2D device, which wine does not provide"]
fn icons_render_during_offscreen_tab_transitions() -> Result<()> {
    use windows::Win32::Graphics::Direct2D::D2D1_COMPATIBLE_RENDER_TARGET_OPTIONS_NONE;
    let dc = context()?;
    let target = bitmap(&dc, false)?;
    unsafe { dc.SetTarget(&target) };
    let offscreen = unsafe {
        dc.CreateCompatibleRenderTarget(
            Some(&D2D_SIZE_F { width: 128.0, height: 128.0 }),
            None,
            None,
            D2D1_COMPATIBLE_RENDER_TARGET_OPTIONS_NONE,
        )?
    };
    let render_target: ID2D1RenderTarget = offscreen.cast()?;
    let mut icons = WeatherIcons::default();
    unsafe { render_target.BeginDraw() };
    icons.draw(&render_target, "rain", (0.0, 0.0, 128.0, 128.0), false)?;
    unsafe { render_target.EndDraw(None, None) }
}
