use std::fs;
use std::io::Read;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use windows::Win32::Foundation::{HLOCAL, LocalFree};
use windows::Win32::Security::Cryptography::{
    BCRYPT_USE_SYSTEM_PREFERRED_RNG, BCryptGenRandom, CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData,
    CryptUnprotectData,
};

const MAX_FILE_BYTES: u64 = 16 * 1024;

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
#[serde(default)]
pub struct Config {
    pub spotify_client_id: String,
    pub discord_client_id: String,
    pub discord_client_secret: String,
}

pub fn dir() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("APPDATA")?).join("NightIsland"))
}

pub fn config_path() -> Option<PathBuf> {
    Some(dir()?.join("integrations.json"))
}

pub fn load() -> Config {
    config_path()
        .and_then(|p| fs::File::open(p).ok())
        .and_then(|f| {
            let mut text = String::new();
            f.take(MAX_FILE_BYTES).read_to_string(&mut text).ok()?;
            serde_json::from_str(&text).ok()
        })
        .unwrap_or_default()
}

pub fn save(c: &Config) {
    let Some(path) = config_path() else { return };
    let Some(dir) = path.parent() else { return };
    let _ = fs::create_dir_all(dir);
    let Ok(json) = serde_json::to_string_pretty(c) else { return };
    let tmp = path.with_extension("json.tmp");
    if fs::write(&tmp, json).is_ok() {
        let _ = fs::rename(&tmp, &path);
    }
}

pub fn ensure_template() -> Option<PathBuf> {
    let path = config_path()?;
    if !path.exists() {
        fs::create_dir_all(path.parent()?).ok()?;
        fs::write(&path, serde_json::to_string_pretty(&Config::default()).ok()?).ok()?;
    }
    Some(path)
}

pub fn random_bytes(len: usize) -> Vec<u8> {
    let mut buf = vec![0u8; len];
    unsafe {
        let _ = BCryptGenRandom(None, &mut buf, BCRYPT_USE_SYSTEM_PREFERRED_RNG);
    }
    buf
}

fn blob(data: &[u8]) -> CRYPT_INTEGER_BLOB {
    CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 }
}

fn take(out: CRYPT_INTEGER_BLOB) -> Vec<u8> {
    unsafe {
        let v = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        let _ = LocalFree(Some(HLOCAL(out.pbData.cast())));
        v
    }
}

pub fn protect(data: &[u8]) -> Option<Vec<u8>> {
    let mut out = CRYPT_INTEGER_BLOB::default();
    unsafe { CryptProtectData(&blob(data), None, None, None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut out).ok()? };
    Some(take(out))
}

pub fn unprotect(data: &[u8]) -> Option<Vec<u8>> {
    let mut out = CRYPT_INTEGER_BLOB::default();
    unsafe { CryptUnprotectData(&blob(data), None, None, None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut out).ok()? };
    Some(take(out))
}

pub fn save_secret(name: &str, text: &str) {
    let (Some(dir), Some(sealed)) = (dir(), protect(text.as_bytes())) else { return };
    let _ = fs::create_dir_all(&dir);
    let _ = fs::write(dir.join(format!("{name}.bin")), sealed);
}

pub fn load_secret(name: &str) -> Option<String> {
    let mut sealed = Vec::new();
    fs::File::open(dir()?.join(format!("{name}.bin"))).ok()?.take(MAX_FILE_BYTES).read_to_end(&mut sealed).ok()?;
    String::from_utf8(unprotect(&sealed)?).ok()
}

pub fn delete_secret(name: &str) {
    if let Some(dir) = dir() {
        let _ = fs::remove_file(dir.join(format!("{name}.bin")));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_round_trip_and_are_not_plain_text() {
        let sealed = protect(b"refresh-token-123").expect("dpapi");
        assert!(!sealed.windows(7).any(|w| w == b"refresh"));
        assert_eq!(unprotect(&sealed).as_deref(), Some(&b"refresh-token-123"[..]));
        assert!(unprotect(b"garbage").is_none());
    }

    #[test]
    fn random_bytes_differ() {
        assert_ne!(random_bytes(32), random_bytes(32));
    }

    #[test]
    fn partial_config_loads() {
        let c: Config = serde_json::from_str(r#"{"spotify_client_id":"abc"}"#).unwrap();
        assert_eq!(c.spotify_client_id, "abc");
        assert!(c.discord_client_id.is_empty());
    }
}
