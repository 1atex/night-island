use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ, RegCloseKey,
    RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
};
use windows::core::HSTRING;

const KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

fn command() -> Option<String> {
    Some(format!("\"{}\"", std::env::current_exe().ok()?.display()))
}

fn open(access: windows::Win32::System::Registry::REG_SAM_FLAGS) -> Option<HKEY> {
    let mut key = HKEY::default();
    unsafe {
        if access == KEY_SET_VALUE {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                &HSTRING::from(KEY),
                None,
                None,
                REG_OPTION_NON_VOLATILE,
                access,
                None,
                &mut key,
                None,
            )
            .ok()
            .ok()?;
        } else {
            RegOpenKeyExW(HKEY_CURRENT_USER, &HSTRING::from(KEY), None, access, &mut key).ok().ok()?;
        }
    }
    Some(key)
}

pub fn enabled() -> bool {
    enabled_as("NightIsland")
}

fn enabled_as(name: &str) -> bool {
    let (Some(key), Some(want)) = (open(KEY_QUERY_VALUE), command()) else { return false };
    let mut buf = [0u16; 1024];
    let mut size = (buf.len() * 2) as u32;
    let status = unsafe {
        RegQueryValueExW(key, &HSTRING::from(name), None, None, Some(buf.as_mut_ptr().cast()), Some(&mut size))
    };
    unsafe {
        let _ = RegCloseKey(key);
    }
    if status != ERROR_SUCCESS {
        return false;
    }
    let len = (size as usize / 2).min(buf.len());
    let have = String::from_utf16_lossy(&buf[..len]);
    have.trim_end_matches('\0').eq_ignore_ascii_case(&want)
}

pub fn set(on: bool) {
    set_as("NightIsland", on);
}

fn set_as(name: &str, on: bool) {
    let Some(key) = open(KEY_SET_VALUE) else { return };
    unsafe {
        if on {
            if let Some(cmd) = command() {
                let wide: Vec<u16> = cmd.encode_utf16().chain(std::iter::once(0)).collect();
                let bytes = std::slice::from_raw_parts(wide.as_ptr().cast::<u8>(), wide.len() * 2);
                let _ = RegSetValueExW(key, &HSTRING::from(name), None, REG_SZ, Some(bytes));
            }
        } else {
            let _ = RegDeleteValueW(key, &HSTRING::from(name));
        }
        let _ = RegCloseKey(key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_run_entry_can_be_switched_on_and_off() {
        let name = "NightIslandTest";
        set_as(name, false);
        assert!(!enabled_as(name));
        set_as(name, true);
        assert!(enabled_as(name));
        set_as(name, false);
        assert!(!enabled_as(name));
    }
}
