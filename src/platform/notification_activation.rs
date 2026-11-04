use windows::Win32::Foundation::PROPERTYKEY;
use windows::Win32::System::Com::StructuredStorage::PropVariantToStringAlloc;
use windows::Win32::System::Com::{CLSCTX_LOCAL_SERVER, CoCreateInstance, CoTaskMemFree};
use windows::Win32::UI::Shell::PropertiesSystem::{GPS_DEFAULT, IPropertyStore, SHGetPropertyStoreFromParsingName};
use windows::core::{GUID, HSTRING, PCWSTR};

use super::notifications::Record;

#[windows_core::interface("53e31837-6600-4a81-9395-75cffe746f94")]
unsafe trait INotificationActivationCallback: windows_core::IUnknown {
    unsafe fn activate(
        &self,
        app_id: PCWSTR,
        arguments: PCWSTR,
        data: *const std::ffi::c_void,
        count: u32,
    ) -> windows::core::HRESULT;
}

fn property(store: &IPropertyStore, id: u32) -> Option<String> {
    unsafe {
        let value = store
            .GetValue(&PROPERTYKEY { fmtid: GUID::from_u128(0x9f4c2855_9f79_4b39_a8d0_e1d42de1d5f3), pid: id })
            .ok()?;
        let raw = PropVariantToStringAlloc(&value).ok()?;
        let value = raw.to_string().ok();
        CoTaskMemFree(Some(raw.0.cast()));
        value
    }
}

fn shortcut(app_id: &str) -> Option<(String, Option<GUID>)> {
    for path in super::apps::notification_shortcuts() {
        let Ok(store) = (unsafe {
            SHGetPropertyStoreFromParsingName::<_, _, IPropertyStore>(&HSTRING::from(&path), None, GPS_DEFAULT)
        }) else {
            continue;
        };
        if property(&store, 5).as_deref() == Some(app_id) {
            let activator = property(&store, 26).and_then(|id| GUID::try_from(id.trim_matches(['{', '}'])).ok());
            return Some((path, activator));
        }
    }
    None
}

fn protocol_target(app_id: &str, kind: &str, launch: &str) -> Option<String> {
    if kind != "protocol" || launch.len() > 4096 || launch.chars().any(char::is_control) {
        return None;
    }
    let scheme = launch.split_once(':')?.0.to_ascii_lowercase();
    let app = app_id.to_ascii_lowercase();
    let allowed = (app.starts_with("com.squirrel.discord.") && scheme == "discord")
        || ((app.starts_with("telegramdesktop.") || app.starts_with("telegrammessengerllp.telegramdesktop_"))
            && scheme == "tg");
    allowed.then(|| launch.to_string())
}

fn shell_open(target: &str) -> bool {
    unsafe {
        windows::Win32::UI::Shell::ShellExecuteW(
            None,
            windows::core::w!("open"),
            &HSTRING::from(target),
            None,
            None,
            windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
        )
        .0 as isize
            > 32
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum OpenMethod {
    Protocol,
    Callback,
    Application,
}

pub(super) fn open(record: &Record) -> Option<OpenMethod> {
    let xml = super::notification_payload::open()
        .and_then(|db| super::notification_payload::read(&db, record.id, &record.app_id, &record.title, &record.body));
    let attributes = xml.as_ref().and_then(|xml| xml.DocumentElement().ok()).map(|root| {
        let attr = |name| root.GetAttribute(&HSTRING::from(name)).unwrap_or_default().to_string();
        (attr("activationType"), attr("launch"))
    });
    if let Some((kind, launch)) = &attributes
        && let Some(target) = protocol_target(&record.app_id, kind, launch)
        && shell_open(&target)
    {
        return Some(OpenMethod::Protocol);
    }
    if let Some((path, activator)) = shortcut(&record.app_id) {
        if let (Some(clsid), Some((kind, launch))) = (activator, &attributes)
            && matches!(kind.as_str(), "" | "foreground")
            && launch.len() <= 16384
            && !launch.contains('\0')
        {
            unsafe {
                if let Ok(callback) =
                    CoCreateInstance::<_, INotificationActivationCallback>(&clsid, None, CLSCTX_LOCAL_SERVER)
                    && callback
                        .activate(
                            PCWSTR(HSTRING::from(&record.app_id).as_ptr()),
                            PCWSTR(HSTRING::from(launch).as_ptr()),
                            std::ptr::null(),
                            0,
                        )
                        .is_ok()
                {
                    return Some(OpenMethod::Callback);
                }
            }
        }
        if shell_open(&path) {
            return Some(OpenMethod::Application);
        }
    }
    let target = super::notifications::sender_target(&record.app_id)?;
    let item = unsafe {
        windows::Win32::UI::Shell::SHCreateItemFromParsingName::<_, _, windows::Win32::UI::Shell::IShellItem>(
            &HSTRING::from(&target),
            None,
        )
    };
    (item.is_ok() && shell_open(&target)).then_some(OpenMethod::Application)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_sender_owned_protocols_are_replayed() {
        assert!(protocol_target("com.squirrel.Discord.Discord", "protocol", "discord://-/channels/1/2").is_some());
        assert!(protocol_target("TelegramDesktop.test", "protocol", "tg://resolve?domain=test").is_some());
        for (app, kind, value) in [
            ("OtherApp", "protocol", "discord://-/channels/1/2"),
            ("com.squirrel.Discord.Discord", "foreground", "discord://-/channels/1/2"),
            ("com.squirrel.Discord.Discord", "protocol", "file:///C:/bad.exe"),
            ("com.squirrel.Discord.Discord", "protocol", "powershell:command"),
            ("com.squirrel.Discord.Discord", "protocol", "discord://a\0b"),
        ] {
            assert!(protocol_target(app, kind, value).is_none());
        }
    }

    #[test]
    #[ignore]
    fn installed_discord_activation_registration() {
        unsafe {
            windows::Win32::System::WinRT::RoInitialize(windows::Win32::System::WinRT::RO_INIT_MULTITHREADED).unwrap();
        }
        let found = shortcut("com.squirrel.Discord.Discord").expect("Discord shortcut has its real AppUserModelID");
        assert!(found.1.is_some());
        println!("Discord shortcut and notification activator found");
        unsafe {
            windows::Win32::System::WinRT::RoUninitialize();
        }
    }
}
