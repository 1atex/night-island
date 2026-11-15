use super::*;
use std::os::windows::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::LRESULT;
use windows::Win32::Graphics::Dwm::DwmFlush;
use windows::Win32::Graphics::Gdi::{CreateSolidBrush, DeleteObject, GetDC, GetPixel, ReleaseDC};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;

#[test]
fn capture_policy_excludes_games_shell_and_unknown_programs() {
    assert!(supported(r"C:\Users\Noon\AppData\Roaming\Telegram Desktop\Telegram.exe"));
    assert!(supported(r"C:\Program Files\Zen Browser\zen.exe"));
    assert!(!supported(r"C:\Windows\explorer.exe"));
    assert!(!supported(r"C:\Games\Discord.exe"));
    assert!(!supported(r"D:\Steam\steamapps\common\Game\browser.exe"));
    assert!(!supported(r"C:\tools\unknown.exe"));
    let settings: crate::data::desktop_dock::DockSettings = serde_json::from_str("{}").unwrap();
    assert!(!settings.experimental_capture);
}

#[test]
#[ignore = "Only launched as a child process by the capture integration test"]
fn capture_fixture() -> windows::core::Result<()> {
    let Some(ready) = std::env::var_os("NI_CAPTURE_FIXTURE_READY") else {
        return Ok(());
    };
    unsafe extern "system" fn proc(hwnd: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
        if message == WM_APP + 120 {
            unsafe {
                let _ = SetWindowDisplayAffinity(hwnd, WDA_MONITOR);
            }
            return LRESULT(0);
        }
        if message == WM_DESTROY {
            unsafe {
                PostQuitMessage(0);
            }
        }
        unsafe { DefWindowProcW(hwnd, message, w, l) }
    }
    unsafe {
        let instance = GetModuleHandleW(None)?;
        let brush = CreateSolidBrush(windows::Win32::Foundation::COLORREF(0x00ff00ff));
        RegisterClassW(&WNDCLASSW {
            lpfnWndProc: Some(proc),
            hInstance: instance.into(),
            hbrBackground: brush,
            lpszClassName: w!("NightIsland.CaptureFixture"),
            ..Default::default()
        });
        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST,
            w!("NightIsland.CaptureFixture"),
            w!("Night Island capture test"),
            WS_POPUP,
            80,
            140,
            100,
            100,
            None,
            None,
            Some(instance.into()),
            None,
        )?;
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        std::fs::write(ready, (hwnd.0 as usize).to_string()).unwrap();
        let id = hwnd.0 as usize;
        std::thread::spawn(move || {
            use std::io::Read;
            let _ = std::io::stdin().read(&mut [0u8; 1]);
            let _ = PostMessageW(Some(HWND(id as _)), WM_CLOSE, WPARAM(0), LPARAM(0));
        });
        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            DispatchMessageW(&message);
        }
        let _ = DeleteObject(brush.into());
    }
    Ok(())
}

struct Fixture {
    child: Child,
    ready: PathBuf,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.child.stdin.take();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(&self.ready);
    }
}

fn screen_color() -> u32 {
    unsafe {
        let _ = DwmFlush();
        let dc = GetDC(None);
        let color = GetPixel(dc, 120, 180).0;
        ReleaseDC(None, dc);
        color
    }
}

#[test]
#[ignore = "Loads the capture integration into a disposable test process and checks actual desktop capture"]
fn native_capture_foreign_window_roundtrip_and_screenshot() -> Result<(), Box<dyn std::error::Error>> {
    let ready = std::env::temp_dir().join(format!("night-island-capture-fixture-{}.txt", std::process::id()));
    let executable = std::env::current_exe()?;
    let child = Command::new(&executable)
        .args(["--ignored", "--exact", "platform::app_capture::tests::capture_fixture", "--nocapture"])
        .env("NI_CAPTURE_FIXTURE_READY", &ready)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(0x08000000)
        .spawn()?;
    let _fixture = Fixture { child, ready: ready.clone() };
    let deadline = Instant::now() + Duration::from_secs(5);
    let handle = loop {
        if let Ok(text) = std::fs::read_to_string(&ready)
            && let Ok(handle) = text.parse::<isize>()
        {
            break handle;
        }
        assert!(Instant::now() < deadline, "Capture fixture did not start");
        std::thread::sleep(Duration::from_millis(20));
    };
    let window = Window {
        handle,
        path: executable.to_string_lossy().into_owned(),
        title: "Fixture".into(),
        active: false,
        minimized: false,
        island_settings: false,
    };
    let path = helper_file().map_err(|e| format!("{e:?}"))?;
    let module = Module(unsafe {
        LoadLibraryExW(
            &HSTRING::from(path.as_os_str()),
            None,
            LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
        )?
    });
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(screen_color() & 0xffffff, 0xff00ff);
    for _ in 0..2 {
        apply(&window, true, &module).map_err(|e| format!("Enable: {e:?}"))?;
        assert!(marked(&window));
        let mut affinity = 0;
        unsafe {
            GetWindowDisplayAffinity(HWND(handle as _), &mut affinity)?;
        }
        assert_eq!(affinity, WDA_EXCLUDEFROMCAPTURE.0);
        std::thread::sleep(Duration::from_millis(100));
        assert_ne!(screen_color() & 0xffffff, 0xff00ff, "The protected window must disappear from desktop capture");
        apply(&window, false, &module).map_err(|e| format!("Restore: {e:?}"))?;
        assert!(!marked(&window));
        unsafe {
            GetWindowDisplayAffinity(HWND(handle as _), &mut affinity)?;
        }
        assert_eq!(affinity, WDA_NONE.0);
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(screen_color() & 0xffffff, 0xff00ff);
    }
    let mut stale = window.clone();
    stale.path = "different.exe".into();
    assert_eq!(validate(&stale), Err(Failure::Changed));
    unsafe {
        SendMessageW(HWND(handle as _), WM_APP + 120, None, None);
    }
    apply(&window, true, &module).map_err(|e| format!("Preserve original: {e:?}"))?;
    apply(&window, false, &module).map_err(|e| format!("Restore original: {e:?}"))?;
    let mut original = 0;
    unsafe {
        GetWindowDisplayAffinity(HWND(handle as _), &mut original)?;
    }
    assert_eq!(original, WDA_MONITOR.0);
    Ok(())
}
