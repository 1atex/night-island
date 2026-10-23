use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
use windows::core::PWSTR;

pub fn foreground_app() -> Option<String> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_invalid() {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return None;
        }
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len).is_ok();
        let _ = CloseHandle(process);
        ok.then(|| program_name(&String::from_utf16_lossy(&buf[..len as usize])))
    }
}

pub fn program_name(path: &str) -> String {
    let file = path.rsplit(['\\', '/']).next().unwrap_or(path);
    file.rsplit_once('.')
        .map_or(file, |(stem, ext)| if ext.eq_ignore_ascii_case("exe") { stem } else { file })
        .to_string()
}

pub fn idle_secs() -> u64 {
    let mut info = LASTINPUTINFO { cbSize: size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
    unsafe {
        if !GetLastInputInfo(&mut info).as_bool() {
            return 0;
        }
        u64::from(GetTickCount().wrapping_sub(info.dwTime) / 1000)
    }
}

pub fn today() -> i64 {
    use chrono::{Local, NaiveDate};
    NaiveDate::from_ymd_opt(1970, 1, 1)
        .map_or(0, |epoch| Local::now().date_naive().signed_duration_since(epoch).num_days())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn program_names_come_from_the_path() {
        assert_eq!(program_name(r"C:\Program Files\Google\Chrome\chrome.exe"), "chrome");
        assert_eq!(program_name(r"C:\x\Tool.EXE"), "Tool");
        assert_eq!(program_name("plain"), "plain");
        assert_eq!(program_name(r"C:\x\archive.tar"), "archive.tar");
    }

    #[test]
    fn today_is_a_plausible_day() {
        assert!(today() > 20_000);
    }
}
