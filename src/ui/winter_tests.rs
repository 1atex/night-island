use super::super::winter_model::Ball;
use super::*;
use windows::Win32::Graphics::Dwm::DwmFlush;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::WinRT::*;

#[test]
#[ignore = "Opens an isolated native snow scene on the Windows desktop"]
fn native_winter_retains_shapes_and_releases_its_windows() -> Result<()> {
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
    let compositor = Compositor::new()?;
    let gpu = super::super::gfx::Gpu::new(&compositor)?;
    let backdrop = unsafe {
        CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST,
            w!("STATIC"),
            w!("Winter preview fixture"),
            WS_POPUP,
            80,
            120,
            800,
            500,
            None,
            None,
            Some(GetModuleHandleW(None)?.into()),
            None,
        )?
    };
    struct Background(HWND);
    impl Drop for Background {
        fn drop(&mut self) {
            unsafe {
                let _ = DestroyWindow(self.0);
            }
        }
    }
    let _background = Background(backdrop);
    let surface = super::super::popup::PopupSurface::new((800, 500))?;
    surface.paint(&gpu, |p| {
        use super::super::gfx::rgba;
        p.fill_rect((0.0, 0.0, 800.0, 500.0), rgba(0.045, 0.09, 0.15, 1.0))?;
        p.fill_round_rect((270.0, 40.0, 530.0, 78.0), 19.0, rgba(0.01, 0.02, 0.03, 1.0))?;
        p.fill_round_rect((100.0, 180.0, 540.0, 380.0), 12.0, rgba(0.11, 0.15, 0.2, 1.0))?;
        p.fill_round_rect((180.0, 415.0, 620.0, 477.0), 22.0, rgba(0.13, 0.18, 0.25, 1.0))?;
        Ok(())
    })?;
    surface.present(backdrop, Some(POINT { x: 80, y: 120 }), 255)?;
    unsafe {
        let _ = ShowWindow(backdrop, SW_SHOWNOACTIVATE);
    }
    let bounds = RECT { left: 80, top: 120, right: 880, bottom: 620 };
    let settings = WinterSettings { enabled: true, dense: true, ..Default::default() };
    let mut scene =
        Scene::new(&compositor, Monitor { device: "test".into(), bounds, work: bounds, primary: true }, settings)?;
    let s = scene.scale;
    scene.snow.ledges(vec![
        Ledge { id: -1, x: 0.0, y: 500.0 / s, width: 800.0 / s },
        Ledge { id: -2, x: 280.0 / s, y: 40.0 / s, width: 240.0 / s },
        Ledge { id: 42, x: 108.0 / s, y: 180.0 / s, width: 424.0 / s },
        Ledge { id: -3, x: 192.0 / s, y: 415.0 / s, width: 416.0 / s },
    ]);
    for (index, drift) in scene.snow.drifts.iter_mut().enumerate() {
        for (bin, depth) in drift.depths.iter_mut().enumerate() {
            *depth = 1.5 + ((bin * 7 + index * 13) % 9) as f32 * 0.7;
        }
    }
    scene.snow.balls = vec![
        Ball { x: 650.0 / s, y: 469.0 / s, radius: 30.0 / s, velocity: 0.0, support: Some(-1) },
        Ball { x: 650.0 / s, y: 422.0 / s, radius: 21.0 / s, velocity: 0.0, support: Some(-1) },
        Ball { x: 650.0 / s, y: 390.0 / s, radius: 15.0 / s, velocity: 0.0, support: Some(-1) },
    ];
    scene.update_mask(vec![
        RECT { left: 180, top: 301, right: 620, bottom: 500 },
        RECT { left: 400, top: 340, right: 660, bottom: 480 },
    ])?;
    scene.render(0.033, settings, true)?;
    let shape = scene.flakes[0].shape.clone();
    let first_y = shape.Offset()?.Y;
    let pile = scene.pile_dots[0].shape.clone();
    let until = Instant::now() + Duration::from_millis(800);
    while Instant::now() < until {
        scene.render(0.033, settings, true)?;
        unsafe {
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            DwmFlush()?;
        }
        std::thread::sleep(Duration::from_millis(33));
    }
    assert_eq!(shape, scene.flakes[0].shape);
    assert!((shape.Offset()?.Y - first_y).abs() > 5.0, "Snowflakes must keep moving across rendered frames");
    assert_eq!(pile, scene.pile_dots[0].shape);
    assert_eq!(scene.ball_dots.len(), 3);
    assert!(scene.ball_dots[2][1]._geometry.Radius()?.X > 0.0);
    assert_ne!(unsafe { GetWindowLongPtrW(scene.hwnd, GWL_EXSTYLE) } & WS_EX_TRANSPARENT.0 as isize, 0);
    assert_ne!(
        unsafe { WindowFromPoint(POINT { x: 400, y: 350 }) },
        scene.hwnd,
        "The snow layer must pass hit testing to the application underneath"
    );
    if let Some(path) = std::env::var_os("NI_WINTER_PREVIEW") {
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: 800,
                biHeight: -500,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pixels = vec![0u8; 800 * 500 * 4];
        unsafe {
            let screen = GetDC(None);
            let mem = CreateCompatibleDC(Some(screen));
            let mut bits = std::ptr::null_mut();
            let bitmap = CreateDIBSection(Some(mem), &info, DIB_RGB_COLORS, &mut bits, None, 0)?;
            let previous = SelectObject(mem, bitmap.into());
            BitBlt(mem, 0, 0, 800, 500, Some(screen), 80, 120, SRCCOPY | CAPTUREBLT)?;
            SelectObject(mem, previous);
            GetDIBits(mem, bitmap, 0, 500, Some(pixels.as_mut_ptr().cast()), &mut info, DIB_RGB_COLORS);
            let _ = DeleteObject(bitmap.into());
            let _ = DeleteDC(mem);
            ReleaseDC(None, screen);
        }
        for y in 190..250 {
            for x in 120..400 {
                let pixel = &pixels[(y * 800 + x) * 4..][..3];
                assert!(
                    pixel.iter().all(|v| *v < 180),
                    "No snow may appear inside the working area of a window at {x},{y}: {pixel:?}"
                );
            }
        }
        let snow_pixels = pixels.chunks_exact(4).filter(|p| p[0] > 180 && p[1] > 180 && p[2] > 180).count();
        assert!(snow_pixels > 1000, "Composition must draw the snow and snowman: {snow_pixels}");
        for px in pixels.chunks_exact_mut(4) {
            px.swap(0, 2);
            px[3] = 255;
        }
        let mut png = png::Encoder::new(std::fs::File::create(path).unwrap(), 800, 500);
        png.set_color(png::ColorType::Rgba);
        png.set_depth(png::BitDepth::Eight);
        png.write_header().unwrap().write_image_data(&pixels).unwrap();
    }
    let hwnd = scene.hwnd;
    drop(scene);
    assert!(!unsafe { IsWindow(Some(hwnd)) }.as_bool());
    Ok(())
}
