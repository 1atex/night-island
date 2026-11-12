use super::*;
use crate::platform::dnd;
use std::sync::atomic::{AtomicBool, Ordering};
use windows::Win32::Foundation::{RECT, S_OK};
use windows::Win32::System::Ole::{DROPEFFECT, DROPEFFECT_COPY, DoDragDrop, IDropSource, IDropSource_Impl};
use windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS;
use windows::Win32::System::WinRT::*;
use windows::core::{BOOL, HRESULT, implement};

#[implement(IDropSource)]
struct TestDrag {
    target: isize,
    release: Arc<AtomicBool>,
    deadline: Instant,
}

impl IDropSource_Impl for TestDrag_Impl {
    fn QueryContinueDrag(&self, escape: BOOL, _: MODIFIERKEYS_FLAGS) -> HRESULT {
        if escape.as_bool() || Instant::now() >= self.deadline {
            return HRESULT(0x40101);
        }
        if self.release.load(Ordering::Acquire) {
            let mut pt = POINT::default();
            let target = unsafe { GetCursorPos(&mut pt).is_ok() && WindowFromPoint(pt).0 as isize == self.target };
            return HRESULT(if target { 0x40100 } else { 0x40101 });
        }
        S_OK
    }
    fn GiveFeedback(&self, _: DROPEFFECT) -> HRESULT {
        HRESULT(0x40102)
    }
}

struct FixtureWindow(HWND);
impl Drop for FixtureWindow {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.0);
        }
    }
}
unsafe extern "system" fn fixture_wndproc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    unsafe { DefWindowProcW(hwnd, msg, w, l) }
}
fn fixture(x: i32, y: i32) -> Result<FixtureWindow> {
    let instance = unsafe { GetModuleHandleW(None)? };
    static REGISTER: std::sync::Once = std::sync::Once::new();
    REGISTER.call_once(|| unsafe {
        assert_ne!(
            RegisterClassW(&WNDCLASSW {
                lpfnWndProc: Some(fixture_wndproc),
                hInstance: instance.into(),
                lpszClassName: w!("NightIsland.FileDragFixture"),
                ..Default::default()
            }),
            0
        );
    });
    Ok(FixtureWindow(unsafe {
        CreateWindowExW(
            WS_EX_APPWINDOW,
            w!("NightIsland.FileDragFixture"),
            w!("Dock file fixture"),
            WS_POPUP | WS_VISIBLE,
            x,
            y,
            240,
            180,
            None,
            None,
            Some(GetModuleHandleW(None)?.into()),
            None,
        )?
    }))
}

#[test]
#[ignore = "Moves the pointer across two isolated Windows OLE drop targets"]
fn native_file_drag_reveals_selected_window_and_delivers_original_file() -> Result<()> {
    unsafe {
        RoInitialize(RO_INIT_SINGLETHREADED)?;
    }
    struct Restore(POINT, HWND);
    impl Drop for Restore {
        fn drop(&mut self) {
            unsafe {
                let _ = SetCursorPos(self.0.x, self.0.y);
                let _ = SetForegroundWindow(self.1);
                RoUninitialize();
            }
        }
    }
    let mut old = POINT::default();
    unsafe {
        GetCursorPos(&mut old)?;
    }
    let _restore = Restore(old, unsafe { GetForegroundWindow() });
    let _queue = unsafe {
        CreateDispatcherQueueController(DispatcherQueueOptions {
            dwSize: size_of::<DispatcherQueueOptions>() as u32,
            threadType: DQTYPE_THREAD_CURRENT,
            apartmentType: DQTAT_COM_NONE,
        })?
    };
    let comp = Compositor::new()?;
    let gpu = Gpu::new(&comp)?;
    let work = desktop::monitors().into_iter().find(|m| m.primary).unwrap().work;
    let target = fixture(work.left + 90, work.top + 120)?;
    let source = fixture(work.left + 85, work.top + 115)?;
    let path = std::env::current_exe().unwrap().to_string_lossy().into_owned();
    let pin = Pin::from_path(&path);
    let cfg = DockSettings {
        pins: vec![pin],
        enabled: true,
        show_running: false,
        hide_fullscreen: false,
        auto_hide: false,
        animations: false,
        alt_tab: false,
        replace_taskbar: false,
        ..Default::default()
    };
    let mut dock = Dock::create(source.0, &comp, &gpu, &cfg, true)?;
    assert!(dock.state.file_registration.is_some());
    let hwnd = dock.state.hwnd;
    unsafe {
        let _ = KillTimer(Some(hwnd), PULSE);
    }
    dock.state.entries[0].windows = [source.0, target.0]
        .into_iter()
        .enumerate()
        .map(|(i, h)| desktop::Window {
            handle: h.0 as isize,
            path: path.clone(),
            title: format!("File window {i}"),
            active: i == 0,
            minimized: false,
            island_settings: false,
        })
        .collect();
    dock.state.render();
    let (_, r) = dock.state.rects[0];
    let icon = POINT {
        x: dock.state.origin.0 + ((r.0 + r.2) * 0.5) as i32,
        y: dock.state.origin.1 + ((r.1 + r.3) * 0.5) as i32,
    };
    let dest = POINT { x: work.left + 160, y: work.top + 180 };
    dnd::register(target.0);
    let file = std::env::temp_dir().join(format!("ni-file-drag-{}.txt", std::process::id()));
    std::fs::write(&file, b"drag fixture").unwrap();
    let data = dnd::file_data(&file).unwrap();
    let release = Arc::new(AtomicBool::new(false));
    let drag: IDropSource = TestDrag {
        target: target.0.0 as isize,
        release: release.clone(),
        deadline: Instant::now() + Duration::from_secs(7),
    }
    .into();
    let target_id = target.0.0 as isize;
    let dock_id = hwnd.0 as isize;
    unsafe {
        let _ = SetForegroundWindow(source.0);
        SetCursorPos(dest.x, dest.y)?;
    }
    let driver = std::thread::spawn(move || unsafe {
        std::thread::sleep(Duration::from_millis(130));
        SetCursorPos(icon.x, icon.y).unwrap();
        let deadline = Instant::now() + Duration::from_secs(4);
        let popup = loop {
            if let Ok(popup) = FindWindowW(w!("NightIsland.WindowSwitcher"), w!("Night Island"))
                && GetWindow(popup, GW_OWNER).is_ok_and(|o| o.0 as isize == dock_id)
            {
                break popup;
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(30));
        };
        let mut rect = RECT::default();
        GetWindowRect(popup, &mut rect).unwrap();
        SetCursorPos(rect.left + (rect.right - rect.left) * 3 / 4, rect.top + (rect.bottom - rect.top) * 2 / 3)
            .unwrap();
        while GetForegroundWindow().0 as isize != target_id && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        if GetForegroundWindow().0 as isize != target_id {
            return false;
        }
        SetCursorPos(dest.x, dest.y).unwrap();
        while WindowFromPoint(dest).0 as isize != target_id && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(
            WindowFromPoint(dest).0 as isize,
            target_id,
            "Selected fixture must accept the file under the pointer"
        );
        std::thread::sleep(Duration::from_millis(140));
        release.store(true, Ordering::Release);
        SetCursorPos(dest.x + 1, dest.y).unwrap();
        true
    });
    let mut effect = DROPEFFECT::default();
    let result = unsafe { DoDragDrop(&data, &drag, DROPEFFECT_COPY, &mut effect) };
    let reached = driver.join().unwrap();
    dnd::unregister(target.0);
    let received = dnd::take_dropped();
    let _ = std::fs::remove_file(&file);
    assert!(reached, "The grouped dock target must reveal the selected window");
    assert_eq!(result, HRESULT(0x40100));
    assert_eq!(effect, DROPEFFECT_COPY);
    assert_eq!(received, vec![file]);
    assert_eq!(dock.config().pins.len(), 1);
    Ok(())
}
