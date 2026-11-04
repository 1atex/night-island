use super::*;

use crate::data::desktop_dock::Pin;
use crate::platform::dock_actions::{AppAudio, DiscordVoice, MediaSession};
use windows::UI::Composition::Compositor;
use windows::Win32::Foundation::COLORREF;
use windows::Win32::Graphics::Dwm::DwmFlush;
use windows::Win32::Graphics::Gdi::*;
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

struct Sources(Vec<HWND>);

impl Drop for Sources {
    fn drop(&mut self) {
        for hwnd in &self.0 {
            unsafe {
                if IsWindow(Some(*hwnd)).as_bool() {
                    let _ = DestroyWindow(*hwnd);
                }
            }
        }
    }
}

unsafe extern "system" fn source_proc(hwnd: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    unsafe {
        if message == WM_PAINT {
            let mut ps = PAINTSTRUCT::default();
            let dc = BeginPaint(hwnd, &mut ps);
            let brush = CreateSolidBrush(COLORREF(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as u32));
            let mut rect = RECT::default();
            let _ = GetClientRect(hwnd, &mut rect);
            FillRect(dc, &rect, brush);
            let _ = DeleteObject(brush.into());
            let _ = EndPaint(hwnd, &ps);
            return LRESULT(0);
        }
        DefWindowProcW(hwnd, message, w, l)
    }
}

fn pump() {
    unsafe {
        let mut message = MSG::default();
        while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        let _ = DwmFlush();
    }
    std::thread::sleep(Duration::from_millis(20));
}

fn click(preview: &Preview, action: Action) {
    let button = preview.state.buttons.iter().find(|button| button.action == action && button.enabled).unwrap();
    let x = ((button.rect.0 + button.rect.2) * 0.5 * preview.state.scale) as u16;
    let y = ((button.rect.1 + button.rect.3) * 0.5 * preview.state.scale) as u16;
    let point = LPARAM((u32::from(x) | (u32::from(y) << 16)) as isize);
    unsafe {
        SendMessageW(preview.state.hwnd, WM_MOUSEMOVE, Some(WPARAM(0)), Some(point));
        SendMessageW(preview.state.hwnd, WM_LBUTTONDOWN, Some(WPARAM(0)), Some(point));
        SendMessageW(preview.state.hwnd, WM_LBUTTONUP, Some(WPARAM(0)), Some(point));
    }
    pump();
}

fn expect_live_color(preview: &Preview, color: u32) {
    let image = scaled(preview.state.layout.image, preview.state.scale);
    let x = preview.state.origin.0 + ((image.0 + image.2) * 0.5) as i32;
    let y = preview.state.origin.1 + ((image.1 + image.3) * 0.5) as i32;
    let start = Instant::now();
    let mut pixel = COLORREF(0);
    while start.elapsed() < Duration::from_secs(2) {
        pump();
        unsafe {
            let dc = GetDC(None);
            pixel = GetPixel(dc, x, y);
            ReleaseDC(None, dc);
        }
        if pixel.0 == color {
            return;
        }
    }
    panic!("DWM preview pixel mismatch: expected {color:#x}, got {:#x}", pixel.0);
}

fn screenshot(preview: &Preview, name: &str) -> Result<()> {
    let Some(folder) = std::env::var_os("NI_DOCK_HOVER_SCREENSHOTS") else { return Ok(()) };
    let folder = std::path::PathBuf::from(folder);
    std::fs::create_dir_all(&folder).unwrap();
    let size = (
        (preview.state.layout.width * preview.state.scale).ceil() as i32,
        (preview.state.layout.height * preview.state.scale).ceil() as i32,
    );
    let surface = crate::ui::popup::PopupSurface::new(size)?;
    surface.paint(&preview.state.gpu, |p| preview.state.paint(p))?;
    pump();
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
        let bitmap = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0).unwrap();
        let previous = SelectObject(dc, bitmap.into());
        let copied =
            BitBlt(dc, 0, 0, size.0, size.1, Some(screen), preview.state.origin.0, preview.state.origin.1, SRCCOPY);
        ReleaseDC(None, screen);
        let mut pixels = std::slice::from_raw_parts(bits.cast::<u8>(), (size.0 * size.1 * 4) as usize).to_vec();
        SelectObject(dc, previous);
        let _ = DeleteObject(bitmap.into());
        let _ = DeleteDC(dc);
        copied?;
        for pixel in pixels.chunks_exact_mut(4) {
            pixel.swap(0, 2);
            pixel[3] = 255;
        }
        let mut encoder = png::Encoder::new(
            std::fs::File::create(folder.join(format!("{name}.png"))).unwrap(),
            size.0 as u32,
            size.1 as u32,
        );
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header().unwrap().write_image_data(&pixels).unwrap();
    }
    Ok(())
}

#[test]
#[ignore = "Shows synthetic source windows and the live dock hover popup without activating user apps"]
fn native_dock_preview_live_paint_switch_close_sync_and_dpi() -> Result<()> {
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
    let foreground = unsafe { GetForegroundWindow() };
    let monitor = desktop::monitors().into_iter().find(|monitor| monitor.primary).unwrap();
    let work = monitor.work;
    let instance = unsafe { GetModuleHandleW(None)? };
    let class = w!("NightIsland.DockPreviewFixture");
    unsafe {
        RegisterClassW(&WNDCLASSW {
            lpfnWndProc: Some(source_proc),
            hInstance: instance.into(),
            lpszClassName: class,
            ..Default::default()
        });
    }
    let mut sources = Sources(Vec::new());
    for (index, color) in [0x0011bb33, 0x00bb3311].into_iter().enumerate() {
        let source = unsafe {
            CreateWindowExW(
                WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
                class,
                w!("Synthetic dock preview source"),
                WS_POPUP,
                work.left + 20 + index as i32 * 20,
                work.top + 20,
                240,
                160,
                None,
                None,
                Some(instance.into()),
                None,
            )?
        };
        sources.0.push(source);
        unsafe {
            SetWindowLongPtrW(source, GWLP_USERDATA, color);
            let _ = ShowWindow(source, SW_SHOWNOACTIVATE);
        }
    }
    let path = std::env::current_exe().unwrap().to_string_lossy().into_owned();
    let mut entry = Entry {
        pin: Pin { name: "Preview fixture".into(), target: path.clone() },
        pinned: true,
        windows: sources
            .0
            .iter()
            .enumerate()
            .map(|(index, hwnd)| desktop::Window {
                handle: hwnd.0 as isize,
                path: path.clone(),
                title: format!("Synthetic source {}", index + 1),
                active: false,
                minimized: false,
                island_settings: false,
            })
            .collect(),
        icon: None,
    };
    let center = (work.left + work.right) as f32 * 0.5;
    let anchor = (center - 24.0, work.bottom as f32 - 30.0, center + 24.0, work.bottom as f32);
    let mut preview = Preview::new(sources.0[0], &gpu, &entry, anchor, work, 1.0, false)?;
    for scale in [1.0, 1.5, 2.0] {
        preview.show(&entry, anchor, work, scale);
        assert!(preview.is_open());
        assert!(preview.state.thumbnail_visible);
        expect_live_color(&preview, 0x0011bb33);
        click(&preview, Action::NextWindow);
        assert_eq!(preview.state.window_index, 1);
        expect_live_color(&preview, 0x00bb3311);
        click(&preview, Action::PreviousWindow);
        expect_live_color(&preview, 0x0011bb33);
        preview.state.snapshot = Snapshot {
            target: path.clone(),
            ready: true,
            media: vec![MediaSession {
                id: 1,
                title: "Synthetic track title".into(),
                artist: "Fixture artist".into(),
                source: "Fixture".into(),
                playing: true,
                can_previous: true,
                can_toggle: true,
                can_next: true,
            }],
            audio: Some(AppAudio { volume: 0.64, muted: false }),
            discord: Some(DiscordVoice {
                connected: true,
                available: true,
                error: None,
                mute: false,
                deaf: false,
                channel: "Fixture voice".into(),
            }),
        };
        preview.state.arrange();
        expect_live_color(&preview, 0x0011bb33);
        for button in &preview.state.buttons {
            assert!(button.rect.0 >= 0.0 && button.rect.2 <= preview.state.layout.width);
            assert!(button.rect.1 >= 0.0 && button.rect.3 <= preview.state.layout.height);
        }
        screenshot(&preview, &format!("hover-controls-{scale:.1}"))?;
        click(&preview, Action::Dismiss);
        assert!(!preview.is_open());
        assert!(preview.dismissed());
        assert!(preview.state.thumbnail.is_none());
        assert!(!unsafe { IsWindowVisible(preview.state.hwnd).as_bool() });
        assert_eq!(unsafe { GetForegroundWindow() }, foreground);
    }
    preview.show(&entry, anchor, work, 1.0);
    click(&preview, Action::NextWindow);
    click(&preview, Action::CloseWindow);
    assert!(!unsafe { IsWindow(Some(sources.0[1])).as_bool() });
    entry.windows.retain(|window| unsafe { IsWindow(Some(HWND(window.handle as *mut _))).as_bool() });
    preview.sync_entry(&entry);
    assert_eq!(preview.state.entry.windows.len(), 1);
    assert_eq!(preview.state.window_index, 0);
    expect_live_color(&preview, 0x0011bb33);
    unsafe {
        SetWindowLongPtrW(sources.0[0], GWLP_USERDATA, 0x00cc4422);
        let _ = InvalidateRect(Some(sources.0[0]), None, false);
    }
    expect_live_color(&preview, 0x00cc4422);
    entry.windows.clear();
    preview.sync_entry(&entry);
    assert!(!preview.is_open());
    assert!(preview.state.thumbnail.is_none());
    assert_eq!(unsafe { GetForegroundWindow() }, foreground);
    Ok(())
}
