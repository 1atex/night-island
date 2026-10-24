use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use windows::Graphics::Imaging::{
    BitmapAlphaMode, BitmapDecoder, BitmapPixelFormat, BitmapTransform, ColorManagementMode, ExifOrientationMode,
};
use windows::Storage::Streams::{Buffer, DataReader, DataWriter, InMemoryRandomAccessStream};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};

use super::steam::steam_path;

pub const WM_STEAM_ART: u32 = WM_APP + 34;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum ArtKey {
    Game(u32),
    Avatar(u64),
}

pub struct Art {
    pub side: u32,
    pub bgra: Vec<u8>,
}

const MAX_FILE: u64 = 2 * 1024 * 1024;
const SIDE: u32 = 48;
const KEEP: usize = 600;

#[derive(Default)]
struct Cache {
    done: HashMap<ArtKey, Option<Arc<Art>>>,
    pending: HashSet<ArtKey>,
}

static CACHE: Mutex<Option<Cache>> = Mutex::new(None);

pub fn get(key: ArtKey) -> Option<Arc<Art>> {
    let guard = CACHE.try_lock().ok()?;
    guard.as_ref()?.done.get(&key)?.clone()
}

pub fn load(hwnd: HWND, keys: &[ArtKey]) {
    let Ok(mut guard) = CACHE.lock() else { return };
    let cache = guard.get_or_insert_with(Cache::default);
    if cache.done.len() > KEEP {
        cache.done.clear();
    }
    let todo: Vec<ArtKey> =
        keys.iter().copied().filter(|k| !cache.done.contains_key(k) && cache.pending.insert(*k)).collect();
    drop(guard);
    if todo.is_empty() {
        return;
    }
    let raw = hwnd.0 as isize;
    std::thread::spawn(move || {
        unsafe {
            let _ = RoInitialize(RO_INIT_MULTITHREADED);
        }
        let Some(root) = steam_path() else { return };
        for key in todo {
            let art = path_of(&root, key).and_then(|p| decode_file(&p)).map(Arc::new);
            if let Ok(mut guard) = CACHE.lock() {
                let cache = guard.get_or_insert_with(Cache::default);
                cache.pending.remove(&key);
                cache.done.insert(key, art);
            }
        }
        unsafe {
            let _ = PostMessageW(Some(HWND(raw as *mut c_void)), WM_STEAM_ART, WPARAM(0), LPARAM(0));
        }
    });
}

fn path_of(root: &Path, key: ArtKey) -> Option<PathBuf> {
    match key {
        ArtKey::Avatar(id) => {
            let path = root.join("config").join("avatarcache").join(format!("{id}.png"));
            path.is_file().then_some(path)
        }
        ArtKey::Game(id) => game_picture(&root.join("appcache").join("librarycache").join(id.to_string())),
    }
}

fn is_icon_name(name: &str) -> bool {
    name.strip_suffix(".jpg").is_some_and(|stem| stem.len() == 40 && stem.chars().all(|c| c.is_ascii_hexdigit()))
}

fn game_picture(dir: &Path) -> Option<PathBuf> {
    let mut folders = vec![dir.to_path_buf()];
    if let Ok(entries) = std::fs::read_dir(dir) {
        folders.extend(entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()).take(8));
    }
    let find = |accept: &dyn Fn(&str) -> bool| -> Option<PathBuf> {
        folders.iter().find_map(|folder| {
            std::fs::read_dir(folder)
                .ok()?
                .flatten()
                .find(|e| e.file_name().to_str().is_some_and(accept) && e.path().is_file())
                .map(|e| e.path())
        })
    };
    find(&is_icon_name).or_else(|| find(&|n| n == "header.jpg"))
}

fn decode_file(path: &Path) -> Option<Art> {
    if std::fs::metadata(path).ok()?.len() > MAX_FILE {
        return None;
    }
    decode(&std::fs::read(path).ok()?)
}

fn decode(bytes: &[u8]) -> Option<Art> {
    let stream = InMemoryRandomAccessStream::new().ok()?;
    let writer = DataWriter::CreateDataWriter(&stream).ok()?;
    writer.WriteBytes(bytes).ok()?;
    writer.StoreAsync().ok()?.join().ok()?;
    let _ = writer.DetachStream();
    stream.Seek(0).ok()?;
    let decoder = BitmapDecoder::CreateAsync(&stream).ok()?.join().ok()?;
    let (w, h) = (decoder.PixelWidth().ok()?, decoder.PixelHeight().ok()?);
    if w == 0 || h == 0 || w > 8192 || h > 8192 {
        return None;
    }
    let side = w.min(h).min(SIDE);
    let (tw, th) = if w <= h {
        (side, (u64::from(h) * u64::from(side) / u64::from(w)).max(u64::from(side)) as u32)
    } else {
        ((u64::from(w) * u64::from(side) / u64::from(h)).max(u64::from(side)) as u32, side)
    };
    let transform = BitmapTransform::new().ok()?;
    transform.SetScaledWidth(tw).ok()?;
    transform.SetScaledHeight(th).ok()?;
    let bitmap = decoder
        .GetSoftwareBitmapTransformedAsync(
            BitmapPixelFormat::Bgra8,
            BitmapAlphaMode::Premultiplied,
            &transform,
            ExifOrientationMode::IgnoreExifOrientation,
            ColorManagementMode::DoNotColorManage,
        )
        .ok()?
        .join()
        .ok()?;
    let (bw, bh) = (bitmap.PixelWidth().ok()? as usize, bitmap.PixelHeight().ok()? as usize);
    let buffer = Buffer::Create((bw * bh * 4) as u32).ok()?;
    bitmap.CopyToBuffer(&buffer).ok()?;
    let mut pixels = vec![0u8; bw * bh * 4];
    DataReader::FromBuffer(&buffer).ok()?.ReadBytes(&mut pixels).ok()?;
    let side = bw.min(bh);
    let (ox, oy) = ((bw - side) / 2, (bh - side) / 2);
    let mut square = Vec::with_capacity(side * side * 4);
    for y in 0..side {
        let start = ((oy + y) * bw + ox) * 4;
        square.extend_from_slice(&pixels[start..start + side * 4]);
    }
    Some(Art { side: side as u32, bgra: square })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_steams_icon_names_are_icons() {
        assert!(is_icon_name("6b0312cda02f5f777efa2f3318c307ff9acafbb5.jpg"));
        assert!(!is_icon_name("header.jpg"));
        assert!(!is_icon_name("library_600x900.jpg"));
        assert!(!is_icon_name("6b0312cda02f5f777efa2f3318c307ff9acafbb5.png"));
    }

    #[test]
    #[ignore = "reads this computer"]
    fn decodes_this_steam() {
        unsafe {
            let _ = RoInitialize(RO_INIT_MULTITHREADED);
        }
        let root = steam_path().expect("steam");
        let avatars = std::fs::read_dir(root.join("config").join("avatarcache")).expect("avatars");
        let mut seen = 0;
        for entry in avatars.flatten().take(3) {
            let art = decode_file(&entry.path()).expect("an avatar");
            println!("avatar {}: {}x{}", entry.path().display(), art.side, art.side);
            assert_eq!(art.bgra.len(), (art.side * art.side * 4) as usize);
            seen += 1;
        }
        let icon = path_of(&root, ArtKey::Game(10)).expect("an icon");
        let art = decode_file(&icon).expect("decoded");
        println!("game 10: {} -> {}x{}", icon.display(), art.side, art.side);
        assert!(seen > 0);
    }
}
