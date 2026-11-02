use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use ed25519_dalek::{Signature, VerifyingKey};
use prost::Message;
use sha2::{Digest, Sha256};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};

use crate::services::proto::{Release, SignedRelease};
use crate::services::{http, sync};

pub const WM_UPDATE: u32 = WM_APP + 22;

pub const CURRENT: &str = env!("CARGO_PKG_VERSION");
pub const OS: &str = "windows-x64";
pub const APP: &str = "night-island";
const HOST: &str = sync::HOST;
const DOMAIN: &[u8] = b"night-island/release/v1\0";
const RELEASE_KEY_HEX: &str = include_str!("../../proto/release_public_key.txt");
const USER_AGENT: &str = "NightIsland/0.1";
const MAX_MANIFEST_BYTES: usize = 16 * 1024;
const MAX_INSTALLER_BYTES: usize = 64 * 1024 * 1024;
const MIN_INSTALLER_BYTES: u64 = 100 * 1024;
const FIRST_CHECK_DELAY: std::time::Duration = std::time::Duration::from_secs(40);
const CHECK_EVERY: std::time::Duration = std::time::Duration::from_secs(12 * 3600);

#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    pub version: String,
    pub beta: bool,
    pub notes: String,
    pub url: String,
    pub sha256: [u8; 32],
    pub size: u64,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub enum Status {
    #[default]
    Unknown,
    NotConfigured,
    Checking,
    UpToDate,
    Available(Found),
    Downloading(Found),
    Ready(Found, PathBuf),
    Failed(String),
}

static STATE: Mutex<Status> = Mutex::new(Status::Unknown);
static ENABLED: AtomicBool = AtomicBool::new(true);
static BETA: AtomicBool = AtomicBool::new(false);
static STARTED: AtomicBool = AtomicBool::new(false);

pub fn state() -> Status {
    STATE.lock().map(|g| g.clone()).unwrap_or_default()
}

fn set_state(hwnd: isize, status: Status) {
    if let Ok(mut g) = STATE.lock() {
        *g = status;
    }
    unsafe {
        let _ = PostMessageW(Some(HWND(hwnd as *mut std::ffi::c_void)), WM_UPDATE, WPARAM(0), LPARAM(0));
    }
}

pub fn release_key() -> Option<VerifyingKey> {
    let bytes = sync::from_hex32(RELEASE_KEY_HEX)?;
    VerifyingKey::from_bytes(&bytes).ok()
}

pub fn configured() -> bool {
    release_key().is_some()
}

fn channel(beta: bool) -> &'static str {
    if beta { "beta" } else { "stable" }
}

fn parse_version(v: &str) -> Option<(u64, u64, u64, Option<&str>)> {
    let (core, pre) = match v.split_once('-') {
        Some((core, pre)) => (core, Some(pre)),
        None => (v, None),
    };
    let mut parts = core.split('.');
    let num = |p: Option<&str>| p?.parse::<u64>().ok();
    let version = (num(parts.next())?, num(parts.next())?, num(parts.next())?);
    if parts.next().is_some() || pre.is_some_and(|p| p.is_empty() || p.len() > 32) {
        return None;
    }
    Some((version.0, version.1, version.2, pre))
}

pub fn is_newer(candidate: &str, current: &str) -> bool {
    let (Some(c), Some(o)) = (parse_version(candidate), parse_version(current)) else { return false };
    match (c.0, c.1, c.2).cmp(&(o.0, o.1, o.2)) {
        std::cmp::Ordering::Greater => true,
        std::cmp::Ordering::Less => false,
        std::cmp::Ordering::Equal => match (c.3, o.3) {
            (None, Some(_)) => true,
            (Some(a), Some(b)) => a > b,
            _ => false,
        },
    }
}

fn url_ok(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else { return false };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    authority == HOST && url.len() <= 300 && url.bytes().all(|b| b.is_ascii_graphic())
}

pub fn verify(signed: &SignedRelease, key: &VerifyingKey, want_channel: &str) -> Result<Found, &'static str> {
    let signature: [u8; 64] = signed.signature.as_slice().try_into().map_err(|_| "Bad signature")?;
    let mut message = DOMAIN.to_vec();
    message.extend_from_slice(&signed.release);
    key.verify_strict(&message, &Signature::from_bytes(&signature)).map_err(|_| "Bad signature")?;
    let release = Release::decode(signed.release.as_slice()).map_err(|_| "Bad release")?;
    if release.app != APP {
        return Err("Release is for another program");
    }
    if release.channel != want_channel || release.os != OS {
        return Err("Release is for another channel or system");
    }
    if !url_ok(&release.url) {
        return Err("Release address is not allowed");
    }
    let sha256: [u8; 32] = release.sha256.as_slice().try_into().map_err(|_| "Bad release hash")?;
    if parse_version(&release.version).is_none() {
        return Err("Bad release version");
    }
    if release.size < MIN_INSTALLER_BYTES || release.size > MAX_INSTALLER_BYTES as u64 {
        return Err("Bad release size");
    }
    Ok(Found {
        version: release.version,
        beta: release.beta,
        notes: release.notes.chars().filter(|c| *c == '\n' || !c.is_control()).take(600).collect(),
        url: release.url,
        sha256,
        size: release.size,
    })
}

fn check_now(hwnd: isize, beta: bool) {
    let Some(key) = release_key() else { return set_state(hwnd, Status::NotConfigured) };
    set_state(hwnd, Status::Checking);
    let url = format!("{}/release?channel={}&os={OS}", sync::BASE, channel(beta));
    let Some(reply) = http::get_bytes(&url, MAX_MANIFEST_BYTES, Some(USER_AGENT)) else {
        return set_state(hwnd, Status::Failed("No connection to the update server".into()));
    };
    if reply.status != 200 {
        crate::log_debug!("update server answered {}", reply.status);
        return set_state(hwnd, Status::Failed("The update server has no release".into()));
    }
    let status = match SignedRelease::decode(reply.body.as_slice()) {
        Err(_) => Status::Failed("The update information was not understood".into()),
        Ok(signed) => match verify(&signed, &key, channel(beta)) {
            Err(why) => {
                crate::log_warn!("update information refused: {why}");
                Status::Failed(why.into())
            }
            Ok(found) if is_newer(&found.version, CURRENT) => Status::Available(found),
            Ok(_) => Status::UpToDate,
        },
    };
    set_state(hwnd, status);
}

pub fn check(hwnd: HWND) {
    let (raw, beta) = (hwnd.0 as isize, BETA.load(Ordering::Relaxed));
    std::thread::spawn(move || check_now(raw, beta));
}

pub fn configure(enabled: bool, beta: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
    BETA.store(beta, Ordering::Relaxed);
}

pub fn start_background(hwnd: HWND) {
    if STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    let raw = hwnd.0 as isize;
    std::thread::spawn(move || {
        std::thread::sleep(FIRST_CHECK_DELAY);
        loop {
            if ENABLED.load(Ordering::Relaxed) && configured() {
                check_now(raw, BETA.load(Ordering::Relaxed));
            }
            std::thread::sleep(CHECK_EVERY);
        }
    });
}

fn updates_dir() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join("NightIsland").join("updates"))
}

pub fn download(hwnd: HWND, found: Found) {
    let raw = hwnd.0 as isize;
    std::thread::spawn(move || {
        set_state(raw, Status::Downloading(found.clone()));
        match fetch_and_check(&found) {
            Ok(path) => set_state(raw, Status::Ready(found, path)),
            Err(why) => set_state(raw, Status::Failed(why.into())),
        }
    });
}

fn fetch_and_check(found: &Found) -> Result<PathBuf, &'static str> {
    if !url_ok(&found.url) {
        return Err("Release address is not allowed");
    }
    let reply = http::get_bytes(&found.url, MAX_INSTALLER_BYTES, Some(USER_AGENT)).ok_or("The download failed")?;
    if reply.status != 200 {
        return Err("The download failed");
    }
    check_file(&reply.body, found)?;
    let dir = updates_dir().ok_or("No place for the download")?;
    fs::create_dir_all(&dir).map_err(|_| "No place for the download")?;
    let path = dir.join(format!("night-island-{}.exe", found.version));
    fs::write(&path, &reply.body).map_err(|_| "Could not save the download")?;
    Ok(path)
}

fn check_file(bytes: &[u8], found: &Found) -> Result<(), &'static str> {
    if bytes.len() as u64 != found.size {
        return Err("The download has the wrong size");
    }
    if Sha256::digest(bytes).as_slice() != found.sha256 {
        return Err("The download does not match its signature");
    }
    if !bytes.starts_with(b"MZ") {
        return Err("The download is not a program");
    }
    Ok(())
}

pub fn install(downloaded: &Path) -> std::io::Result<()> {
    let current = std::env::current_exe()?;
    let old = current.with_extension("exe.old");
    let _ = fs::remove_file(&old);
    fs::rename(&current, &old)?;
    if let Err(e) = fs::copy(downloaded, &current) {
        let _ = fs::rename(&old, &current);
        return Err(e);
    }
    if let Err(e) = std::process::Command::new(&current).arg("--updated").spawn() {
        let _ = fs::remove_file(&current);
        let _ = fs::rename(&old, &current);
        return Err(e);
    }
    Ok(())
}

pub fn clean_up_after_update() {
    if let Ok(current) = std::env::current_exe() {
        let _ = fs::remove_file(current.with_extension("exe.old"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn key() -> SigningKey {
        SigningKey::from_bytes(&[5; 32])
    }

    fn release() -> Release {
        Release {
            version: "0.2.0".into(),
            beta: false,
            url: format!("https://{HOST}/api/v1/installers/resource/application/Island.exe"),
            sha256: vec![1; 32],
            size: 3_000_000,
            notes: "Faster start.\nFewer bugs.".into(),
            signed_at: 1_760_000_000,
            channel: "stable".into(),
            os: OS.into(),
            app: APP.into(),
        }
    }

    fn sign(release: &Release, key: &SigningKey) -> SignedRelease {
        let bytes = release.encode_to_vec();
        let mut message = DOMAIN.to_vec();
        message.extend_from_slice(&bytes);
        SignedRelease { signature: key.sign(&message).to_bytes().to_vec(), release: bytes }
    }

    #[test]
    #[ignore = "needs a file made by ni-release"]
    fn a_release_made_by_the_tool_verifies() {
        let bin = fs::read(std::env::var("NI_TEST_RELEASE_BIN").unwrap()).unwrap();
        let pub_hex = std::env::var("NI_TEST_RELEASE_PUB").unwrap();
        let key = VerifyingKey::from_bytes(&sync::from_hex32(&pub_hex).unwrap()).unwrap();
        let found = verify(&SignedRelease::decode(bin.as_slice()).unwrap(), &key, "stable").unwrap();
        println!("verified {} ({} bytes)", found.version, found.size);
    }

    #[test]
    fn versions_compare_like_semver() {
        assert!(is_newer("0.2.0", "0.1.0") && is_newer("1.0.0", "0.9.9") && is_newer("0.1.10", "0.1.9"));
        assert!(!is_newer("0.1.0", "0.1.0") && !is_newer("0.0.9", "0.1.0"));
        assert!(is_newer("0.2.0", "0.2.0-beta.1"), "the release beats its pre-release");
        assert!(!is_newer("0.2.0-beta.1", "0.2.0"));
        assert!(is_newer("0.2.0-beta.2", "0.2.0-beta.1"));
        assert!(!is_newer("garbage", "0.1.0") && !is_newer("0.2", "0.1.0") && !is_newer("0.2.0.1", "0.1.0"));
    }

    #[test]
    fn a_good_signed_release_is_accepted() {
        let found = verify(&sign(&release(), &key()), &key().verifying_key(), "stable").unwrap();
        assert_eq!(found.version, "0.2.0");
        assert_eq!(found.sha256, [1; 32]);
        assert!(found.notes.contains("Fewer bugs"));
    }

    #[test]
    fn forged_or_altered_releases_are_refused() {
        let good = sign(&release(), &key());
        let stranger = SigningKey::from_bytes(&[6; 32]);
        assert_eq!(verify(&sign(&release(), &stranger), &key().verifying_key(), "stable"), Err("Bad signature"));
        let mut altered = good.clone();
        let last = altered.release.len() - 1;
        altered.release[last] ^= 1;
        assert_eq!(verify(&altered, &key().verifying_key(), "stable"), Err("Bad signature"));
        let mut cut = good;
        cut.signature.truncate(63);
        assert_eq!(verify(&cut, &key().verifying_key(), "stable"), Err("Bad signature"));
    }

    #[test]
    fn what_the_signature_covers_is_checked_too() {
        let ok = |r: Release| verify(&sign(&r, &key()), &key().verifying_key(), "stable");
        assert!(ok(Release { channel: "beta".into(), ..release() }).is_err(), "a beta cannot pose as stable");
        assert!(ok(Release { os: "linux".into(), ..release() }).is_err());
        assert_eq!(ok(Release { app: "night-shell".into(), ..release() }), Err("Release is for another program"));
        assert_eq!(ok(Release { app: String::new(), ..release() }), Err("Release is for another program"));
        for bad_url in [
            format!("http://{HOST}/x.exe"),
            "https://evil.example/x.exe".to_string(),
            format!("https://{HOST}@evil.example/x.exe"),
            format!("https://{HOST}.evil.example/x.exe"),
            format!("https://{HOST}:8443/x.exe"),
            format!("https://{HOST}/a b.exe"),
        ] {
            assert!(ok(Release { url: bad_url.clone(), ..release() }).is_err(), "{bad_url} must be refused");
        }
        assert!(ok(Release { sha256: vec![1; 31], ..release() }).is_err());
        assert!(ok(Release { version: "latest".into(), ..release() }).is_err());
        assert!(ok(Release { size: 10, ..release() }).is_err());
        assert!(ok(Release { size: 10_000_000_000, ..release() }).is_err());
    }

    #[test]
    fn a_download_must_match_what_was_signed() {
        let bytes: Vec<u8> = b"MZ".iter().copied().chain(std::iter::repeat_n(7u8, 200_000)).collect();
        let found = Found {
            version: "0.2.0".into(),
            beta: false,
            notes: String::new(),
            url: String::new(),
            sha256: Sha256::digest(&bytes).into(),
            size: bytes.len() as u64,
        };
        assert_eq!(check_file(&bytes, &found), Ok(()));
        let mut changed = bytes.clone();
        changed[500] ^= 1;
        assert_eq!(check_file(&changed, &found), Err("The download does not match its signature"));
        assert_eq!(check_file(&bytes[..1000], &found), Err("The download has the wrong size"));
        let not_exe: Vec<u8> = vec![7; bytes.len()];
        let found_not_exe = Found { sha256: Sha256::digest(&not_exe).into(), ..found };
        assert_eq!(check_file(&not_exe, &found_not_exe), Err("The download is not a program"));
    }
}
