use std::collections::HashMap;
use std::ffi::c_void;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use windows::Win32::Foundation::SIZE;
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, DeleteObject, GetDC, GetDIBits, GetObjectW, HGDIOBJ,
    ReleaseDC,
};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoTaskMemFree};
use windows::Win32::UI::Shell::{
    BHID_EnumItems, IEnumShellItems, IShellItem, IShellItemImageFactory, SHCreateItemFromParsingName,
    SIGDN_DESKTOPABSOLUTEPARSING, SIGDN_NORMALDISPLAY, SIIGBF_BIGGERSIZEOK, SIIGBF_ICONONLY,
};
use windows::core::{HSTRING, Interface, PWSTR, w};

const MAX_ENTRIES: usize = 4000;
const MAX_DEPTH: usize = 5;

pub struct AppEntry {
    pub name: String,
    lower: String,
    pub path: PathBuf,
    pub launch: String,
}

pub struct Icon {
    pub side: u32,
    pub bgra: Vec<u8>,
}

static ICONS: Mutex<Option<HashMap<String, Option<Arc<Icon>>>>> = Mutex::new(None);

pub fn icon(entry: &AppEntry) -> Option<Arc<Icon>> {
    let mut guard = ICONS.lock().ok()?;
    let cache = guard.get_or_insert_with(HashMap::new);
    if let Some(hit) = cache.get(&entry.launch) {
        return hit.clone();
    }
    let loaded = load_icon(&entry.launch).map(Arc::new);
    cache.insert(entry.launch.clone(), loaded.clone());
    loaded
}

pub fn icon_for_path(path: &str) -> Option<Arc<Icon>> {
    let mut guard = ICONS.lock().ok()?;
    let cache = guard.get_or_insert_with(HashMap::new);
    if let Some(hit) = cache.get(path) {
        return hit.clone();
    }
    if cache.len() > 500 {
        cache.clear();
    }
    let loaded = load_icon(path).map(Arc::new);
    cache.insert(path.to_string(), loaded.clone());
    loaded
}

fn load_icon(target: &str) -> Option<Icon> {
    unsafe {
        let item: IShellItem = SHCreateItemFromParsingName(&HSTRING::from(target), None).ok()?;
        let factory: IShellItemImageFactory = item.cast().ok()?;
        let bitmap = factory.GetImage(SIZE { cx: 48, cy: 48 }, SIIGBF_ICONONLY | SIIGBF_BIGGERSIZEOK).ok()?;
        let mut info = BITMAP::default();
        GetObjectW(HGDIOBJ(bitmap.0), size_of::<BITMAP>() as i32, Some((&raw mut info).cast::<c_void>()));
        let (w, h) = (info.bmWidth, info.bmHeight);
        if w <= 0 || h <= 0 || w > 512 || h > 512 {
            let _ = DeleteObject(bitmap.into());
            return None;
        }
        let mut header = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut buf = vec![0u8; (w * h * 4) as usize];
        let dc = GetDC(None);
        let rows = GetDIBits(dc, bitmap, 0, h as u32, Some(buf.as_mut_ptr().cast()), &mut header, DIB_RGB_COLORS);
        ReleaseDC(None, dc);
        let _ = DeleteObject(bitmap.into());
        if rows == 0 {
            return None;
        }
        if buf.chunks_exact(4).all(|p| p[3] == 0) {
            for p in buf.chunks_exact_mut(4) {
                p[3] = 255;
            }
        }
        let side = w.min(h) as usize;
        let (ox, oy) = ((w as usize - side) / 2, (h as usize - side) / 2);
        let mut square = Vec::with_capacity(side * side * 4);
        for y in 0..side {
            let start = ((oy + y) * w as usize + ox) * 4;
            square.extend_from_slice(&buf[start..start + side * 4]);
        }
        Some(Icon { side: side as u32, bgra: square })
    }
}

fn scan_apps_folder() -> Vec<(String, String)> {
    let mut out = Vec::new();
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let Ok(folder) =
            SHCreateItemFromParsingName::<_, _, IShellItem>(w!("shell:::{4234D49B-0245-4DF3-B780-3893943456E1}"), None)
        else {
            return out;
        };
        let Ok(items) = folder.BindToHandler::<_, IEnumShellItems>(None, &BHID_EnumItems) else { return out };
        let text = |item: &IShellItem, kind| -> Option<String> {
            let raw: PWSTR = item.GetDisplayName(kind).ok()?;
            let s = raw.to_string().ok();
            CoTaskMemFree(Some(raw.0 as *const c_void));
            s
        };
        loop {
            let mut slot = [None];
            let mut fetched = 0u32;
            if items.Next(&mut slot, Some(&mut fetched)).is_err() || fetched == 0 {
                break;
            }
            let Some(item) = slot[0].take() else { break };
            let (Some(name), Some(parse)) =
                (text(&item, SIGDN_NORMALDISPLAY), text(&item, SIGDN_DESKTOPABSOLUTEPARSING))
            else {
                continue;
            };
            let launch = if parse.starts_with("shell:") { parse } else { format!("shell:AppsFolder\\{parse}") };
            out.push((name, launch));
            if out.len() >= MAX_ENTRIES {
                break;
            }
        }
    }
    out
}

static INDEX: OnceLock<Vec<AppEntry>> = OnceLock::new();

pub fn start_indexing() {
    std::thread::spawn(|| {
        let _ = INDEX.set(scan());
    });
}

fn roots() -> Vec<PathBuf> {
    let programs = |base: Option<std::ffi::OsString>| {
        base.map(|b| PathBuf::from(b).join(r"Microsoft\Windows\Start Menu\Programs"))
    };
    [programs(std::env::var_os("ProgramData")), programs(std::env::var_os("APPDATA"))].into_iter().flatten().collect()
}

fn scan() -> Vec<AppEntry> {
    let mut out: Vec<AppEntry> = Vec::new();
    let mut stack: Vec<(PathBuf, usize)> = roots().into_iter().map(|p| (p, 0)).collect();
    while let Some((dir, depth)) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            if out.len() >= MAX_ENTRIES {
                break;
            }
            let Ok(kind) = entry.file_type() else { continue };
            if kind.is_symlink() {
                continue;
            }
            let path = entry.path();
            if kind.is_dir() {
                if depth < MAX_DEPTH {
                    stack.push((path, depth + 1));
                }
            } else if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("lnk")) {
                let Some(name) = path.file_stem().map(|s| s.to_string_lossy().into_owned()) else {
                    continue;
                };
                let lower = name.to_lowercase();
                if ["uninstall", "удалить", "деинсталл"].iter().any(|w| lower.contains(w))
                    || out.iter().any(|a| a.lower == lower)
                {
                    continue;
                }
                let launch = path.to_string_lossy().into_owned();
                out.push(AppEntry { name, lower, path, launch });
            }
        }
    }
    for (name, launch) in scan_apps_folder() {
        let lower = name.to_lowercase();
        let junk = ["uninstall", "удалить", "деинсталл", "удаление"].iter().any(|w| lower.contains(w));
        if junk || out.iter().any(|a| a.lower == lower) || out.len() >= MAX_ENTRIES {
            continue;
        }
        let tail = launch.strip_prefix("shell:AppsFolder\\").unwrap_or("");
        if tail.as_bytes().get(1) == Some(&b':') {
            out.push(AppEntry { name, lower, path: PathBuf::from(tail), launch: tail.to_string() });
        } else {
            out.push(AppEntry { name, lower, path: PathBuf::new(), launch });
        }
    }
    out
}

pub fn find(query: &str, limit: usize) -> Vec<&'static AppEntry> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    let Some(index) = INDEX.get() else { return Vec::new() };
    let mut scored: Vec<(u8, usize, &AppEntry)> = index
        .iter()
        .filter_map(|a| {
            let score = if a.lower == q {
                0
            } else if a.lower.starts_with(&q) {
                1
            } else if a.lower.split([' ', '-', '_', '.']).any(|w| w.starts_with(&q)) {
                2
            } else if a.lower.contains(&q) {
                3
            } else {
                return None;
            };
            Some((score, a.name.chars().count(), a))
        })
        .collect();
    scored.sort_by_key(|&(score, len, _)| (score, len));
    scored.into_iter().take(limit).map(|(_, _, a)| a).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn apps_folder_lists_apps() {
        let apps = scan_apps_folder();
        for (n, l) in apps.iter().take(15) {
            println!("{n} -> {l}");
        }
        assert!(!apps.is_empty());
    }
}
