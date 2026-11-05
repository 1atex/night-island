use super::*;
use windows::UI::Composition::Compositor;
use windows::Win32::Foundation::RECT;
use windows::Win32::System::WinRT::{
    CreateDispatcherQueueController, DQTAT_COM_NONE, DQTYPE_THREAD_CURRENT, DispatcherQueueOptions,
    RO_INIT_SINGLETHREADED, RoInitialize, RoUninitialize,
};
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetThreadDpiAwarenessContext,
};

struct Runtime(DPI_AWARENESS_CONTEXT);
impl Drop for Runtime {
    fn drop(&mut self) {
        unsafe {
            SetThreadDpiAwarenessContext(self.0);
            RoUninitialize();
        }
    }
}
struct Window(HWND);
impl Drop for Window {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.0);
        }
    }
}

fn save(wing: &Wing, name: &str) -> Result<()> {
    let Some(folder) = std::env::var_os("NI_TOP_PANEL_PREVIEW") else { return Ok(()) };
    let folder = std::path::PathBuf::from(folder);
    std::fs::create_dir_all(&folder).unwrap();
    let surface = PopupSurface::new(wing.state.size)?;
    surface.paint(&wing.state.gpu, |p| wing.state.draw(p))?;
    let mut pixels = surface.pixels();
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.swap(0, 2);
        let alpha = pixel[3] as u32;
        if alpha != 0 {
            for value in &mut pixel[..3] {
                *value = ((*value as u32 * 255) / alpha).min(255) as u8;
            }
        }
    }
    let mut encoder = png::Encoder::new(
        std::fs::File::create(folder.join(format!("{name}.png"))).unwrap(),
        wing.state.size.0 as u32,
        wing.state.size.1 as u32,
    );
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header().unwrap().write_image_data(&pixels).unwrap();
    Ok(())
}

fn pump() {
    unsafe {
        let mut message = MSG::default();
        while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

#[test]
fn island_panel_corridor_covers_diagonal_crossings_without_covering_the_desktop() {
    let island = (500.0, 8.0, 1100.0, 408.0);
    let right = (1120.0, -2.0, 1400.0, 52.0);
    let left = (200.0, -2.0, 480.0, 52.0);
    for point in [(1110.0, 30.0), (1110.0, 200.0), (1110.0, 400.0)] {
        assert!(across_gap(island, right, point, 12.0));
    }
    assert!(across_gap(island, left, (490.0, 250.0), 12.0));
    assert!(!across_gap(island, right, (1250.0, 250.0), 12.0));
    assert!(!across_gap(island, right, (1110.0, 450.0), 12.0));
}

fn click(wing: &Wing, target: Hit) {
    let button = wing.state.buttons.iter().find(|b| b.hit == target).unwrap();
    let x = (((button.rect.0 + button.rect.2) * 0.5) * wing.state.scale) as u16;
    let y = (((button.rect.1 + button.rect.3) * 0.5) * wing.state.scale) as u16;
    let point = LPARAM((u32::from(x) | (u32::from(y) << 16)) as isize);
    unsafe {
        SendMessageW(wing.state.hwnd, WM_LBUTTONDOWN, Some(WPARAM(0)), Some(point));
        SendMessageW(wing.state.hwnd, WM_LBUTTONUP, Some(WPARAM(0)), Some(point));
    }
}

#[test]
fn top_panel_blocks_can_all_be_removed_and_persist_independently() {
    let mut settings = crate::data::settings::Settings::default();
    for id in [
        crate::view::Sid::TopEnabled,
        crate::view::Sid::TopTray,
        crate::view::Sid::TopAutoHide,
        crate::view::Sid::TopApplication,
        crate::view::Sid::TopWifi,
        crate::view::Sid::TopBluetooth,
        crate::view::Sid::TopVolume,
        crate::view::Sid::TopBattery,
        crate::view::Sid::TopFullscreen,
        crate::view::Sid::TopReserve,
        crate::view::Sid::TopMicrophone,
        crate::view::Sid::TopKeyboard,
        crate::view::Sid::TopWorkspaces,
        crate::view::Sid::TopAnimations,
        crate::view::Sid::TopLiquid,
    ] {
        crate::view::toggle_set(id, &mut settings, false);
        assert!(!crate::view::toggle_get(id, &settings));
        crate::view::toggle_set(id, &mut settings, true);
        assert!(crate::view::toggle_get(id, &settings));
    }
    settings.top_panel.bluetooth = false;
    settings.top_panel.battery = false;
    let encoded = serde_json::to_string(&settings).unwrap();
    let restored: crate::data::settings::Settings = serde_json::from_str(&encoded).unwrap();
    assert_eq!(restored.top_panel, settings.top_panel);
    assert_eq!(blocks(&restored.top_panel), [Block::Tray, Block::Wifi, Block::Volume]);
    let old: crate::data::settings::Settings = serde_json::from_str("{}").unwrap();
    assert!(!old.top_panel.enabled);
    assert!(old.top_panel.auto_hide && old.top_panel.tray && old.desktop_dock.replace_taskbar);
    for (index, entrance) in PanelEntrance::ALL.into_iter().enumerate() {
        crate::view::cycle_pick(crate::view::Sid::TopEntrance, &mut settings, index);
        assert_eq!(settings.top_panel.entrance, entrance);
        let restored: crate::data::settings::Settings =
            serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
        assert_eq!(restored.top_panel.entrance, entrance);
    }
    let empty = TopPanelSettings {
        tray: false,
        application: false,
        wifi: false,
        bluetooth: false,
        volume: false,
        battery: false,
        ..Default::default()
    };
    assert!(!empty.has_blocks());
}

#[test]
#[ignore = "Shows isolated top-panel widgets; uses synthetic state without changing audio or radios"]
fn native_top_panel_layout_actions_privacy_and_cleanup() -> Result<()> {
    unsafe {
        RoInitialize(RO_INIT_SINGLETHREADED)?;
    }
    let _runtime = Runtime(unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) });
    let _queue = unsafe {
        CreateDispatcherQueueController(DispatcherQueueOptions {
            dwSize: size_of::<DispatcherQueueOptions>() as u32,
            threadType: DQTYPE_THREAD_CURRENT,
            apartmentType: DQTAT_COM_NONE,
        })?
    };
    let compositor = Compositor::new()?;
    let gpu = Gpu::new(&compositor)?;
    let parent = Window(unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            w!("STATIC"),
            w!("Top panel fixture"),
            WS_POPUP,
            20,
            200,
            200,
            150,
            None,
            None,
            Some(GetModuleHandleW(None)?.into()),
            None,
        )?
    });
    let settings = TopPanelSettings {
        enabled: true,
        auto_hide: false,
        reserve_space: false,
        hide_fullscreen: false,
        entrance: PanelEntrance::Sides,
        ..Default::default()
    };
    let mut panel = TopPanel::new(parent.0, &gpu, &settings)?;
    let foreground = unsafe { GetForegroundWindow() };
    let window = desktop::Window {
        handle: parent.0.0 as isize,
        path: std::env::current_exe().unwrap().to_string_lossy().into_owned(),
        title: "Документ — рабочая сессия".into(),
        active: true,
        minimized: false,
        island_settings: true,
    };
    let snapshot = Snapshot {
        active: Some(window.clone()),
        windows: vec![window],
        radios: Radios { wifi: Some(true), bluetooth: Some(false) },
        volume: Some(42),
        muted: false,
        device: "Динамики · USB Audio".into(),
        battery: Some((76, true)),
        microphone_muted: Some(false),
        keyboard_layout: "RU".into(),
    };
    for scale in [1.0, 1.5, 2.0] {
        panel.configure(&settings, scale);
        let center = (panel.monitor.work.left + panel.monitor.work.right) as f32 * 0.5;
        let island =
            (center - 100.0 * scale, panel.top() + 8.0 * scale, center + 100.0 * scale, panel.top() + 42.0 * scale);
        panel.update(snapshot.clone(), island, false);
        for wing in [&mut panel.left, &mut panel.right] {
            wing.state.travel.show(true, 0, Instant::now());
            wing.state.motion_frame();
            wing.state.render();
        }
        assert!(panel.left.state.visible && panel.right.state.visible);
        assert!(
            panel.right.state.origin.0 + panel.right.state.size.0 <= panel.monitor.work.right - (144.0 * scale) as i32
        );
        save(&panel.left, &format!("left-{scale:.1}"))?;
        save(&panel.right, &format!("right-{scale:.1}"))?;
        let mut frame_times = Vec::new();
        for _ in 0..40 {
            let started = Instant::now();
            panel.left.state.render();
            panel.right.state.render();
            frame_times.push(started.elapsed().as_secs_f64() * 1000.0);
        }
        frame_times.sort_by(f64::total_cmp);
        println!("Two panel frames at scale {scale}: median {:.2}ms, p95 {:.2}ms", frame_times[20], frame_times[38]);
        let right_edge = panel.right.state.origin.0 + panel.right.state.size.0;
        let buffer = panel.right.state.surface.borrow().as_ref().unwrap().storage_id();
        let wifi_x = {
            let button = panel.right.state.buttons.iter().find(|b| b.hit == Hit::Block(Block::Wifi)).unwrap();
            panel.right.state.origin.0 as f32 + button.rect.0 * scale
        };
        panel.right.state.tray_requested = true;
        panel.set_tray_icons(crate::platform::system_tray::inline::preview_entries());
        for elapsed in (0..=240).step_by(20) {
            panel.right.state.tray_started = Instant::now() - std::time::Duration::from_millis(elapsed);
            panel.right.state.tray_frame();
            panel.update(snapshot.clone(), island, false);
            assert!(panel.right.state.visible, "The expanding wing must remain visible at {elapsed} ms");
            assert_eq!(panel.right.state.origin.0 + panel.right.state.size.0, right_edge);
            assert!(panel.right.state.origin.0 as f32 >= island.2 + 8.0 * scale);
            assert_eq!(panel.right.state.surface.borrow().as_ref().unwrap().storage_id(), buffer);
            let button = panel.right.state.buttons.iter().find(|b| b.hit == Hit::Block(Block::Wifi)).unwrap();
            let x = panel.right.state.origin.0 as f32 + button.rect.0 * scale;
            assert!((x - wifi_x).abs() <= 1.0);
            let mut actual = RECT::default();
            unsafe {
                GetWindowRect(panel.right.state.hwnd, &mut actual)?;
            }
            assert_eq!((actual.left, actual.right), (panel.right.state.origin.0, right_edge));
            save(&panel.right, &format!("tray-frame-{scale:.1}-{elapsed}"))?;
        }
        panel.right.state.tray_started = Instant::now() - std::time::Duration::from_secs(1);
        panel.right.state.tray_frame();
        assert_eq!(panel.right.state.origin.0 + panel.right.state.size.0, right_edge);
        assert_eq!(
            panel.right.state.buttons.iter().filter(|b| matches!(b.hit, Hit::TrayIcon(_))).count(),
            panel.right.state.tray_capacity
        );
        save(&panel.right, &format!("inline-tray-{scale:.1}"))?;
        click(&panel.right, Hit::TrayIcon(0));
        assert!(matches!(panel.take_request(1), Some(Request::TrayAction(_, false, _))));
        click(&panel.right, Hit::TrayPage);
        panel.right.state.tray_started = Instant::now() - std::time::Duration::from_secs(1);
        panel.right.state.tray_frame();
        assert_eq!(panel.right.state.tray_page, panel.right.state.tray_capacity);
        panel.open(Block::Tray);
        panel.right.state.tray_started = Instant::now() - std::time::Duration::from_secs(1);
        panel.right.state.tray_frame();
        assert_eq!(panel.right.state.tray_width, 0.0);
        assert_eq!(panel.right.state.origin.0 + panel.right.state.size.0, right_edge);
        panel.open(Block::Tray);
        assert!(panel.right.state.tray_open);
        assert!(matches!(panel.take_request(1), Some(Request::Tray(_))));
        panel.open(Block::Tray);
        panel.set_tray_icons(crate::platform::system_tray::inline::preview_entries());
        assert!(!panel.right.state.tray_open && !panel.right.state.tray_requested);
        panel.right.state.tray_started = Instant::now() - std::time::Duration::from_secs(1);
        panel.right.state.tray_frame();
        for block in [Block::Application, Block::Wifi, Block::Bluetooth, Block::Volume, Block::Battery] {
            panel.open(block);
            assert!(panel.card.state.visible);
            assert_eq!(panel.card.state.size.0, (306.0 * scale) as i32);
            for button in &panel.card.state.buttons {
                let (width, height) = panel.card.state.dimensions();
                assert!(button.rect.0 >= 10.0 && button.rect.2 <= width - 10.0);
                assert!(button.rect.1 >= 10.0 && button.rect.3 <= height - 10.0);
            }
            save(&panel.card, &format!("card-{block:?}-{scale:.1}"))?;
            click(&panel.card, Hit::Close);
            assert!(!panel.card.state.visible);
        }
        panel.tray_help();
        assert!(panel.card.state.visible);
        assert_eq!(panel.card.state.size.0, (338.0 * scale) as i32);
        save(&panel.card, &format!("tray-help-{scale:.1}"))?;
        click(&panel.card, Hit::WindowsSettings);
        assert!(matches!(panel.take_request(2), Some(Request::WindowsSettings(Block::Tray))));
        panel.tray_help();
        click(&panel.card, Hit::TrayRetry);
        assert!(matches!(panel.take_request(2), Some(Request::Open(Block::Tray))));
        panel.open(Block::Volume);
        click(&panel.card, Hit::Mute);
        assert!(matches!(panel.take_request(2), Some(Request::Mute)));
        click(&panel.card, Hit::Volume);
        assert!(matches!(panel.take_request(2), Some(Request::Volume(_))));
        assert_ne!(unsafe { GetCapture() }, panel.card.state.hwnd);
        panel.set_ghost(0.2, true);
        for wing in [&panel.left, &panel.right, &panel.card] {
            assert!(wing.state.click_through);
            assert_eq!(wing.state.ghost_alpha, 51);
            assert_ne!(unsafe { GetWindowLongPtrW(wing.state.hwnd, GWL_EXSTYLE) } & WS_EX_TRANSPARENT.0 as isize, 0);
            assert_eq!(
                unsafe { SendMessageW(wing.state.hwnd, WM_NCHITTEST, None, Some(LPARAM(0))) }.0,
                HTTRANSPARENT as isize
            );
        }
        panel.set_ghost(1.0, false);
        for wing in [&panel.left, &panel.right, &panel.card] {
            assert_eq!(wing.state.ghost_alpha, 255);
            assert_eq!(unsafe { GetWindowLongPtrW(wing.state.hwnd, GWL_EXSTYLE) } & WS_EX_TRANSPARENT.0 as isize, 0);
        }
        panel.set_capture_hidden(true);
        for wing in [&panel.left, &panel.right, &panel.card] {
            let mut affinity = 0;
            unsafe {
                GetWindowDisplayAffinity(wing.state.hwnd, &mut affinity)?;
            }
            assert_eq!(affinity, WDA_EXCLUDEFROMCAPTURE.0);
        }
        panel.set_capture_hidden(false);
        panel.update(snapshot.clone(), island, true);
        assert!(!panel.left.state.visible && !panel.right.state.visible && !panel.card.state.visible);
        panel.update(
            snapshot.clone(),
            (panel.monitor.work.left as f32, panel.top(), panel.monitor.work.right as f32, panel.top() + 400.0),
            false,
        );
        assert!(!panel.left.state.visible && !panel.right.state.visible);
        assert_eq!(unsafe { GetForegroundWindow() }, foreground);
    }
    for entrance in PanelEntrance::ALL {
        for scale in [1.0, 1.5, 2.0] {
            let mut config = settings.clone();
            config.entrance = entrance;
            panel.left.state.hide();
            panel.right.state.hide();
            panel.configure(&config, scale);
            let center = (panel.monitor.work.left + panel.monitor.work.right) as f32 / 2.0;
            let island =
                (center - 100.0 * scale, panel.top() + 8.0 * scale, center + 100.0 * scale, panel.top() + 42.0 * scale);
            panel.update(snapshot.clone(), island, false);
            let mut previous = [0u8; 2];
            for elapsed in [0, 90, 180, 270, 400] {
                for (index, wing) in [&mut panel.left, &mut panel.right].into_iter().enumerate() {
                    wing.state.travel.seek(elapsed);
                    wing.state.motion_frame();
                    wing.state.render();
                    assert!(wing.state.alpha >= previous[index]);
                    previous[index] = wing.state.alpha;
                    if elapsed == 400 {
                        assert_eq!(wing.state.origin, wing.state.destination);
                        assert!(!wing.state.opening);
                    }
                }
            }
            if entrance == PanelEntrance::Island {
                let original = panel.right.state.destination;
                panel.right.place((original.0 + 40, original.1), scale, true);
                assert!((panel.right.state.origin.0 - original.0).abs() <= 1);
                assert!(panel.animating());
                panel.right.state.relocation.seek(400);
                panel.right.state.motion_frame();
                assert_eq!(panel.right.state.origin.0, original.0 + 40);
                panel.right.place(original, scale, true);
                panel.right.state.relocation.seek(400);
                panel.right.state.motion_frame();
                assert_eq!(panel.right.state.origin, original);
                let tray = &panel.right.state;
                let button_point = (tray.origin.0 as f32 + 25.0 * scale, tray.origin.1 as f32 + 25.0 * scale);
                assert!(panel.interacting_at(island, button_point));
                assert!(panel.interacting_at(island, (island.2 + 5.0 * scale, island.1 + 20.0 * scale)));
                panel.set_ghost(0.2, true);
                assert!(!panel.interacting_at(island, button_point));
                panel.set_ghost(1.0, false);
                assert!(
                    (panel.left.state.origin.0 as f32 + panel.left.state.size.0 as f32
                        - (island.0 - config.island_gap as f32 * scale))
                        .abs()
                        <= 1.0
                );
                assert!(
                    (panel.right.state.origin.0 as f32 - (island.2 + config.island_gap as f32 * scale)).abs() <= 1.0
                );
                let right_start = panel.right.state.origin.0;
                let wifi =
                    panel.right.state.buttons.iter().find(|button| button.hit == Hit::Block(Block::Wifi)).unwrap().rect;
                panel.right.state.tray_requested = true;
                panel.set_tray_icons(crate::platform::system_tray::inline::preview_entries());
                for elapsed in [0, 80, 160, 240] {
                    panel.right.state.tray_started = Instant::now() - std::time::Duration::from_millis(elapsed);
                    panel.right.state.tray_frame();
                    panel.update(snapshot.clone(), island, false);
                    assert_eq!(panel.right.state.origin.0, right_start);
                    assert_eq!(
                        panel
                            .right
                            .state
                            .buttons
                            .iter()
                            .find(|button| button.hit == Hit::Block(Block::Wifi))
                            .unwrap()
                            .rect,
                        wifi
                    );
                    assert!(
                        panel.right.state.origin.0 + panel.right.state.size.0
                            <= panel.monitor.work.right - (144.0 * scale) as i32
                    );
                }
                save(&panel.right, &format!("island-tray-{scale:.1}"))?;
            }
            save(&panel.left, &format!("{entrance:?}-left-{scale:.1}"))?;
            save(&panel.right, &format!("{entrance:?}-right-{scale:.1}"))?;
        }
    }
    panel.left.state.hide();
    panel.right.state.hide();
    let mut only_volume = settings.clone();
    only_volume.animations = false;
    struct Cursor(POINT);
    impl Drop for Cursor {
        fn drop(&mut self) {
            unsafe {
                let _ = SetCursorPos(self.0.x, self.0.y);
            }
        }
    }
    let mut point = POINT::default();
    unsafe {
        GetCursorPos(&mut point)?;
    }
    let _cursor = Cursor(point);
    let mut auto = settings.clone();
    auto.animations = false;
    auto.auto_hide = true;
    panel.configure(&auto, 1.0);
    let outside = (panel.monitor.bounds.left + 80, panel.monitor.bounds.top + 400);
    unsafe {
        SetCursorPos(outside.0, outside.1)?;
    }
    panel.update(snapshot.clone(), (-1000.0, -1000.0, -900.0, -900.0), false);
    assert!(!panel.left.state.visible && !panel.right.state.visible);
    unsafe {
        SetCursorPos(panel.monitor.bounds.left + 400, panel.monitor.bounds.top + 1)?;
    }
    panel.update(snapshot.clone(), (-1000.0, -1000.0, -900.0, -900.0), false);
    assert!(panel.left.state.visible && panel.right.state.visible);
    unsafe {
        SetCursorPos(panel.right.state.origin.0 + 40, panel.right.state.origin.1 + 25)?;
    }
    panel.last_inside = Instant::now() - std::time::Duration::from_secs(2);
    panel.update(snapshot.clone(), (-1000.0, -1000.0, -900.0, -900.0), false);
    assert!(panel.right.state.visible);
    panel.open(Block::Volume);
    unsafe {
        SetCursorPos(panel.card.state.origin.0 + 40, panel.card.state.origin.1 + 60)?;
    }
    panel.last_inside = Instant::now() - std::time::Duration::from_secs(2);
    panel.update(snapshot.clone(), (-1000.0, -1000.0, -900.0, -900.0), false);
    assert!(panel.card.state.visible && panel.right.state.visible);
    unsafe {
        SetCursorPos(outside.0, outside.1)?;
    }
    panel.last_inside = Instant::now() - std::time::Duration::from_secs(2);
    panel.update(snapshot.clone(), (-1000.0, -1000.0, -900.0, -900.0), false);
    assert!(!panel.card.state.visible && !panel.right.state.visible);
    only_volume.application = false;
    only_volume.tray = false;
    only_volume.wifi = false;
    only_volume.bluetooth = false;
    only_volume.battery = false;
    panel.configure(&only_volume, 1.0);
    panel.update(snapshot.clone(), (-1000.0, -1000.0, -900.0, -900.0), false);
    assert!(!panel.left.state.visible && panel.right.state.visible);
    assert_eq!(panel.right.state.buttons.len(), 1);
    let mut extra = only_volume.clone();
    extra.auto_hide = false;
    extra.volume = false;
    extra.microphone = true;
    extra.keyboard = true;
    extra.workspaces = true;
    panel.configure(&extra, 1.0);
    panel.update(snapshot.clone(), (-1000.0, -1000.0, -900.0, -900.0), false);
    assert!(panel.left.state.visible && !panel.right.state.visible);
    save(&panel.left, "extra-controls")?;
    click(&panel.left, Hit::Block(Block::Microphone));
    assert!(matches!(panel.take_request(0), Some(Request::Microphone)));
    click(&panel.left, Hit::Block(Block::Keyboard));
    assert!(matches!(panel.take_request(0), Some(Request::Keyboard)));
    click(&panel.left, Hit::Block(Block::Workspaces));
    assert!(matches!(panel.take_request(0), Some(Request::Workspaces)));
    let mut unavailable = snapshot;
    unavailable.microphone_muted = None;
    panel.update(unavailable, (-1000.0, -1000.0, -900.0, -900.0), false);
    click(&panel.left, Hit::Block(Block::Microphone));
    assert!(panel.take_request(0).is_none());
    let windows = [panel.left.state.hwnd, panel.right.state.hwnd, panel.card.state.hwnd];
    drop(panel);
    pump();
    for hwnd in windows {
        assert!(!unsafe { IsWindow(Some(hwnd)).as_bool() });
    }
    Ok(())
}

#[test]
#[ignore = "Briefly reserves and restores desktop work area using a native appbar"]
fn native_top_panel_space_is_reserved_once_and_restored() -> Result<()> {
    let before = desktop::monitors().into_iter().find(|m| m.primary).unwrap();
    let parent = Window(unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            w!("STATIC"),
            w!("Top panel space fixture"),
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
    });
    let mut reservation = super::super::top_panel_space::Reservation::new(parent.0, &before, 50)?;
    let after = desktop::monitors().into_iter().find(|m| m.primary).unwrap();
    assert!(
        after.work.top >= before.work.top + 50,
        "Work area must leave room for the panel: {:?} -> {:?}",
        before.work,
        after.work
    );
    let first = reservation.rect();
    for _ in 0..3 {
        reservation.refresh(&after, 50);
    }
    assert_eq!(reservation.rect(), first);
    drop(reservation);
    let deadline = Instant::now() + std::time::Duration::from_secs(2);
    loop {
        pump();
        let restored = desktop::monitors().into_iter().find(|m| m.primary).unwrap();
        if restored.work == before.work {
            break;
        }
        assert!(Instant::now() < deadline, "Work area must be restored: {:?} -> {:?}", before.work, restored.work);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(first.bottom - first.top == 50 && first != RECT::default());
    Ok(())
}
