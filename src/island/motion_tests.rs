use super::*;
use windows::Win32::Foundation::POINT;
use windows::Win32::Graphics::Dwm::DwmFlush;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::WinRT::RoUninitialize;

struct NativeWindow(HWND);

impl Drop for NativeWindow {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.0);
        }
    }
}

fn window(ex: WINDOW_EX_STYLE) -> Result<NativeWindow> {
    Ok(NativeWindow(unsafe {
        CreateWindowExW(
            ex | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            w!("STATIC"),
            w!("Island motion fixture"),
            WS_POPUP,
            0,
            0,
            1,
            1,
            None,
            None,
            Some(GetModuleHandleW(None)?.into()),
            None,
        )?
    }))
}

fn seek(app: &App, progress: f32) -> Result<()> {
    for property in [h!("Offset"), h!("Size"), h!("CornerRadius")] {
        let controller = app.geometry.TryGetAnimationController(property)?;
        controller.Pause()?;
        controller.SetProgress(progress)?;
    }
    for sprite in [&app.tint, &app.expanded.sprite, &app.collapsed.sprite, &app.pet.char_layer.sprite] {
        let controller = sprite.TryGetAnimationController(h!("Opacity"))?;
        controller.Pause()?;
        controller.SetProgress(progress)?;
    }
    for property in [h!("Scale"), h!("Offset")] {
        let controller = app.expanded.sprite.TryGetAnimationController(property)?;
        controller.Pause()?;
        controller.SetProgress(progress)?;
    }
    if app.ui.cover_in_flight {
        for property in [h!("Offset"), h!("Size")] {
            let controller = app.cover_layer.sprite.TryGetAnimationController(property)?;
            controller.Pause()?;
            controller.SetProgress(progress)?;
        }
    }
    unsafe {
        let mut message = MSG::default();
        while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        DwmFlush()?;
        std::thread::sleep(Duration::from_millis(25));
        DwmFlush()?;
    }
    Ok(())
}

fn capture(app: &App, frame: usize) -> Result<()> {
    let Some(folder) = std::env::var_os("NI_OPEN_MOTION_PREVIEW") else { return Ok(()) };
    let size = app.window_size(Win::Big);
    let (x, y) = app.window_origin(Win::Big);
    unsafe {
        let screen = GetDC(None);
        let dc = CreateCompatibleDC(Some(screen));
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
        let bitmap = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0)?;
        let previous = SelectObject(dc, bitmap.into());
        let copied =
            BitBlt(dc, 0, 0, size.0, size.1, Some(screen), x.round() as i32, y.round() as i32, SRCCOPY | CAPTUREBLT);
        let mut pixels = std::slice::from_raw_parts(bits.cast::<u8>(), (size.0 * size.1 * 4) as usize).to_vec();
        SelectObject(dc, previous);
        let _ = DeleteObject(bitmap.into());
        let _ = DeleteDC(dc);
        ReleaseDC(None, screen);
        copied?;
        for pixel in pixels.chunks_exact_mut(4) {
            pixel.swap(0, 2);
            pixel[3] = 255;
        }
        assert!(
            pixels.chunks_exact(4).any(|pixel| pixel[0] < 20 && pixel[1] < 20 && pixel[2] < 20),
            "The native island must be visible in captured frame {frame}"
        );
        let folder = std::path::PathBuf::from(folder);
        std::fs::create_dir_all(&folder).unwrap();
        let mut png = png::Encoder::new(
            std::fs::File::create(folder.join(format!("island-{frame:03}.png"))).unwrap(),
            size.0 as u32,
            size.1 as u32,
        );
        png.set_color(png::ColorType::Rgba);
        png.set_depth(png::BitDepth::Eight);
        png.write_header().unwrap().write_image_data(&pixels).unwrap();
    }
    Ok(())
}

#[test]
#[ignore = "Uses an isolated Windows compositor and synthetic island windows"]
fn native_island_open_close_retarget_dpi_and_content() -> Result<()> {
    unsafe {
        RoInitialize(RO_INIT_SINGLETHREADED)?;
    }
    struct Runtime;
    impl Drop for Runtime {
        fn drop(&mut self) {
            unsafe {
                RoUninitialize();
            }
        }
    }
    let _runtime = Runtime;
    let _queue = unsafe {
        CreateDispatcherQueueController(DispatcherQueueOptions {
            dwSize: size_of::<DispatcherQueueOptions>() as u32,
            threadType: DQTYPE_THREAD_CURRENT,
            apartmentType: DQTAT_COM_NONE,
        })?
    };
    for scale in [1.0, 1.25, 1.5, 2.0] {
        for edge in Edge::ALL {
            let owner = window(WS_EX_NOREDIRECTIONBITMAP)?;
            let settings = Settings {
                placement: Placement::Docked,
                dock_edge: edge,
                character: Character::None,
                music_cover: true,
                style: Style::Flat,
                ..Default::default()
            };
            let mut app = App::new_visuals(owner.0, scale, settings)?;
            app.ui.cfg = app.settings.clone();
            app.place(Win::Big);
            app.set_geometry_now(Win::Big, false, false)?;
            app.state = State::Expanded;
            let start = Instant::now();
            app.animate_to(true, false)?;
            assert!(start.elapsed() < Duration::from_millis(250));
            for progress in [0.0, 0.25, 0.6, 1.0] {
                seek(&app, progress)?;
            }
            app.state = State::Collapsing;
            app.animate_to(false, false)?;
            let before = app.open_motion.as_ref().unwrap().progress.value(Instant::now());
            app.state = State::Expanded;
            app.animate_to(true, false)?;
            let after = app.open_motion.as_ref().unwrap().progress.frame(0.0);
            assert!((before - after).abs() < 0.03);
            seek(&app, 1.0)?;
            app.kill_timer(Timer::Expand);
            app.set_timer(Timer::Expand, 100);
            app.set_timer(Timer::Expand, 5000);
            std::thread::sleep(Duration::from_millis(130));
            let mut message = MSG::default();
            assert!(unsafe { PeekMessageW(&mut message, Some(owner.0), WM_TIMER, WM_TIMER, PM_REMOVE).as_bool() });
            assert_eq!(message.wParam.0, Timer::Expand as usize);
            app.kill_timer(Timer::Expand);
            app.settings.island_animations = false;
            app.animate_to(false, false)?;
            assert_eq!(app.transition_ms(false), 1);
            assert_eq!(app.open_motion.as_ref().unwrap().progress.value(Instant::now()), 0.0);
            app.animate_to(true, false)?;
            assert_eq!(app.open_content_point(300, 100), Some((300, 100)));
            app.open_motion = None;
        }
    }
    let background = window(WS_EX_LAYERED | WS_EX_TOPMOST)?;
    let owner = window(WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOPMOST)?;
    let (screen_w, screen_h) = App::screen_size();
    let settings = Settings {
        placement: Placement::Floating,
        float_pos: Some([(screen_w * 0.5) as i32, (screen_h * 0.25) as i32]),
        style: Style::Flat,
        character: Character::None,
        music_cover: true,
        music_eq: false,
        edge_spikes: false,
        auto_hide: false,
        ..Default::default()
    };
    let mut app = App::new_visuals(owner.0, 1.0, settings)?;
    app.ui.cfg = app.settings.clone();
    app.ui.media = media::Media {
        title: "Midnight City".into(),
        artist: "M83".into(),
        playing: true,
        source: "Motion preview".into(),
        cover: media::debug_cover(16, &[72, 140, 230, 255].repeat(256)),
        ..Default::default()
    };
    app.cw = view::collapsed_width(&app.ui, COLLAPSED.0);
    app.collapsed.resize(app.window_size(Win::Small))?;
    app.place(Win::Big);
    app.set_geometry_now(Win::Big, false, false)?;
    app.state = State::Expanded;
    app.repaint();
    let size = app.window_size(Win::Big);
    let (x, y) = app.window_origin(Win::Big);
    let surface = crate::ui::popup::PopupSurface::new(size)?;
    surface.paint(&app.gpu, |p| {
        p.fill_rect((0.0, 0.0, size.0 as f32, size.1 as f32), crate::ui::gfx::rgba(0.14, 0.21, 0.29, 1.0))
    })?;
    surface.present(background.0, Some(POINT { x: x.round() as i32, y: y.round() as i32 }), 255)?;
    unsafe {
        let _ = ShowWindow(background.0, SW_SHOWNOACTIVATE);
        SetWindowPos(
            owner.0,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
        )?;
    }
    app.animate_to(true, false)?;
    app.start_cover_motion(true);
    for frame in 0..=30 {
        seek(&app, frame as f32 / 30.0)?;
        capture(&app, frame)?;
    }
    app.finish_cover_motion();
    app.collapse();
    for frame in 0..=24 {
        seek(&app, frame as f32 / 24.0)?;
        capture(&app, 31 + frame)?;
    }
    let before = app.pill_screen(false);
    app.shrink_window();
    assert_eq!(before, app.pill_screen(false));
    for property in [h!("Offset"), h!("Size"), h!("CornerRadius")] {
        assert!(app.geometry.TryGetAnimationController(property).is_err());
    }
    for frame in 0..10 {
        unsafe {
            let mut message = MSG::default();
            while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
            DwmFlush()?;
            std::thread::sleep(Duration::from_millis(25));
            DwmFlush()?;
        }
        capture(&app, 56 + frame)?;
        let dc = unsafe { GetDC(None) };
        for dx in [4.0, 12.0, 35.0] {
            let color = unsafe {
                GetPixel(dc, (before.0 + before.2 + dx).round() as i32, (before.1 + before.3 * 0.5).round() as i32)
            };
            assert!((color.0 & 255) > 25, "The island stretched right after collapse, frame {frame}: {color:?}");
        }
        unsafe {
            ReleaseDC(None, dc);
        }
    }
    app.settings.style = Style::Liquid;
    app.settings.edge_spikes = true;
    app.settings.spike_color = true;
    app.settings.pill_blur = false;
    app.apply_layout();
    let body = app.pill_rect(false);
    app.liquid.pull_ready = true;
    app.liquid.bump_amp = 24.0;
    app.liquid.pull = crate::ui::gfx::ElasticPull {
        point: (body.0 + body.2 * 0.6, body.1 + body.3),
        normal: (0.0, 1.0),
        spread: 44.0,
        depth: 24.0,
    };
    for (index, style) in [
        crate::data::settings::EqStyle::Spikes,
        crate::data::settings::EqStyle::Wave,
        crate::data::settings::EqStyle::Bars,
    ]
    .into_iter()
    .enumerate()
    {
        app.settings.eq_style = style;
        app.paint_edge_levels(&[0.25, 0.6, 0.4, 0.75, 0.65, 0.5, 0.75, 0.5, 0.2]);
        assert!(app.liquid.body_active.get());
        assert!(!app.liquid.bump.sprite.IsVisible()?);
        let _ = app.compositor.RequestCommitAsync();
        unsafe {
            let mut message = MSG::default();
            while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
            DwmFlush()?;
            std::thread::sleep(Duration::from_millis(25));
            DwmFlush()?;
        }
        capture(&app, 70 + index)?;
    }
    Ok(())
}
