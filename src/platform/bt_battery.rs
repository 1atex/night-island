use std::sync::Mutex;

use windows::Win32::Devices::DeviceAndDriverInstallation::{
    DIGCF_ALLCLASSES, DIGCF_PRESENT, HDEVINFO, SP_DEVINFO_DATA, SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInfo,
    SetupDiGetClassDevsW, SetupDiGetDevicePropertyW,
};
use windows::Win32::Devices::Properties::{DEVPROP_TYPE_BOOLEAN, DEVPROP_TYPE_BYTE, DEVPROP_TYPE_STRING, DEVPROPTYPE};
use windows::Win32::Foundation::DEVPROPKEY;
use windows::core::{GUID, PCWSTR, w};

const BATTERY: DEVPROPKEY = DEVPROPKEY { fmtid: GUID::from_u128(0x104ea319_6ee2_4701_bd47_8ddbf425bbe5), pid: 2 };
const CONNECTED: DEVPROPKEY = DEVPROPKEY { fmtid: GUID::from_u128(0x83da6326_97a6_4088_9453_a1923f573b29), pid: 15 };
const FRIENDLY_NAME: DEVPROPKEY =
    DEVPROPKEY { fmtid: GUID::from_u128(0xa45c254e_df1c_4efd_8020_67d146a850e0), pid: 14 };
const DESCRIPTION: DEVPROPKEY = DEVPROPKEY { fmtid: GUID::from_u128(0xa45c254e_df1c_4efd_8020_67d146a850e0), pid: 2 };

const MAX_DEVICES: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Device {
    pub name: String,
    pub percent: u8,
}

static LATEST: Mutex<Vec<Device>> = Mutex::new(Vec::new());

pub fn latest() -> Vec<Device> {
    LATEST.lock().map(|d| d.clone()).unwrap_or_default()
}

pub fn summary(devices: &[Device]) -> Option<String> {
    if devices.is_empty() {
        return None;
    }
    let shown: Vec<String> = devices.iter().take(3).map(|d| format!("{}%", d.percent)).collect();
    Some(shown.join(", "))
}

pub fn refresh() {
    let found = read();
    if let Ok(mut slot) = LATEST.lock() {
        *slot = found;
    }
}

fn property(set: HDEVINFO, data: &SP_DEVINFO_DATA, key: &DEVPROPKEY, expected: u32) -> Option<Vec<u8>> {
    let mut kind = DEVPROPTYPE(0);
    let mut size = 0u32;
    let mut buffer = vec![0u8; 512];
    unsafe {
        SetupDiGetDevicePropertyW(set, data, key, &mut kind, Some(&mut buffer), Some(&mut size), 0).ok()?;
    }
    if kind.0 != expected {
        return None;
    }
    buffer.truncate(size as usize);
    Some(buffer)
}

fn text(set: HDEVINFO, data: &SP_DEVINFO_DATA, key: &DEVPROPKEY) -> Option<String> {
    let bytes = property(set, data, key, DEVPROP_TYPE_STRING.0)?;
    let units: Vec<u16> =
        bytes.chunks_exact(2).map(|b| u16::from_le_bytes([b[0], b[1]])).take_while(|c| *c != 0).collect();
    let name = String::from_utf16_lossy(&units);
    (!name.trim().is_empty()).then_some(name)
}

pub fn parse_percent(bytes: &[u8]) -> Option<u8> {
    bytes.first().copied().filter(|p| *p <= 100)
}

pub fn read() -> Vec<Device> {
    let mut out: Vec<Device> = Vec::new();
    for enumerator in [w!("BTHENUM"), w!("BTHLE")] {
        unsafe {
            let Ok(set) =
                SetupDiGetClassDevsW(None, PCWSTR(enumerator.as_ptr()), None, DIGCF_ALLCLASSES | DIGCF_PRESENT)
            else {
                continue;
            };
            for index in 0..512u32 {
                let mut data = SP_DEVINFO_DATA { cbSize: size_of::<SP_DEVINFO_DATA>() as u32, ..Default::default() };
                if SetupDiEnumDeviceInfo(set, index, &mut data).is_err() {
                    break;
                }
                let Some(percent) =
                    property(set, &data, &BATTERY, DEVPROP_TYPE_BYTE.0).as_deref().and_then(parse_percent)
                else {
                    continue;
                };
                let connected = property(set, &data, &CONNECTED, DEVPROP_TYPE_BOOLEAN.0)
                    .is_none_or(|b| b.first().is_some_and(|v| *v != 0));
                if !connected {
                    continue;
                }
                let name =
                    text(set, &data, &FRIENDLY_NAME).or_else(|| text(set, &data, &DESCRIPTION)).unwrap_or_default();
                if out.iter().all(|d| d.name != name || d.percent != percent) && out.len() < MAX_DEVICES {
                    out.push(Device { name, percent });
                }
            }
            let _ = SetupDiDestroyDeviceInfoList(set);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tile_text_lists_up_to_three_devices() {
        let d = |percent| Device { name: "x".into(), percent };
        assert_eq!(summary(&[]), None);
        assert_eq!(summary(&[d(85)]).as_deref(), Some("85%"));
        assert_eq!(summary(&[d(85), d(62), d(10), d(5)]).as_deref(), Some("85%, 62%, 10%"));
    }

    #[test]
    fn a_battery_byte_is_a_percent_only_in_range() {
        assert_eq!(parse_percent(&[85]), Some(85));
        assert_eq!(parse_percent(&[0]), Some(0));
        assert_eq!(parse_percent(&[100]), Some(100));
        assert_eq!(parse_percent(&[101]), None);
        assert_eq!(parse_percent(&[]), None);
    }

    #[test]
    #[ignore = "reads the Bluetooth devices of this computer"]
    fn lists_connected_devices_with_a_battery() {
        let devices = read();
        for d in &devices {
            eprintln!("BT|{}|{}%", d.name, d.percent);
        }
        eprintln!("BT count {}", devices.len());
    }
}
