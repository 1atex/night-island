#[path = "../services/proto.rs"]
#[allow(dead_code)]
mod proto;

use std::{env, fs, process::ExitCode};

use ed25519_dalek::{Signer, SigningKey};
use prost::Message;
use sha2::{Digest, Sha256};
use windows::Win32::Security::Cryptography::{BCRYPT_USE_SYSTEM_PREFERRED_RNG, BCryptGenRandom};

const DOMAIN: &[u8] = b"night-island/release/v1\0";
const OS: &str = "windows-x64";

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex32(text: &str) -> Option<[u8; 32]> {
    let text = text.trim();
    if text.len() != 64 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

fn keygen(path: &str) -> Result<(), String> {
    if fs::metadata(path).is_ok() {
        return Err(format!("{path} already exists: refusing to overwrite a key"));
    }
    let mut seed = [0u8; 32];
    unsafe { BCryptGenRandom(None, &mut seed, BCRYPT_USE_SYSTEM_PREFERRED_RNG) }
        .ok()
        .map_err(|e| format!("the system random generator failed: {e}"))?;
    fs::write(path, format!("{}\n", hex(&seed))).map_err(|e| format!("cannot write {path}: {e}"))?;
    let public = SigningKey::from_bytes(&seed).verifying_key();
    println!("Private key written to {path}. Keep it offline and never commit it.");
    println!("Public key (put it into proto/release_public_key.txt and rebuild the program):");
    println!("{}", hex(public.as_bytes()));
    Ok(())
}

fn sign(args: &[String]) -> Result<(), String> {
    let need = |name: &str| arg(args, name).ok_or_else(|| format!("missing {name}"));
    let seed_text = fs::read_to_string(need("--key")?).map_err(|e| format!("cannot read the key: {e}"))?;
    let seed = unhex32(&seed_text).ok_or("the key file is not a 64-digit hex key")?;
    let exe = fs::read(need("--exe")?).map_err(|e| format!("cannot read the program: {e}"))?;
    if !exe.starts_with(b"MZ") {
        return Err("the file is not a Windows program".into());
    }
    let channel = arg(args, "--channel").unwrap_or_else(|| "stable".into());
    if !matches!(channel.as_str(), "stable" | "beta") {
        return Err("--channel is stable or beta".into());
    }
    let release = proto::Release {
        version: need("--version")?,
        beta: channel == "beta",
        url: need("--url")?,
        sha256: Sha256::digest(&exe).to_vec(),
        size: exe.len() as u64,
        notes: arg(args, "--notes").unwrap_or_default(),
        signed_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64),
        channel,
        os: OS.into(),
        app: arg(args, "--app").unwrap_or_else(|| "night-island".into()),
    };
    let bytes = release.encode_to_vec();
    let mut message = DOMAIN.to_vec();
    message.extend_from_slice(&bytes);
    let signed = proto::SignedRelease {
        signature: SigningKey::from_bytes(&seed).sign(&message).to_bytes().to_vec(),
        release: bytes,
    };
    let out = need("--out")?;
    fs::write(&out, signed.encode_to_vec()).map_err(|e| format!("cannot write {out}: {e}"))?;
    println!("Signed {} ({} bytes, sha256 {}) -> {out}", release.version, release.size, hex(&release.sha256));
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("keygen") => args.get(1).ok_or_else(|| "usage: ni-release keygen <private-key-file>".to_string()).and_then(|p| keygen(p)),
        Some("sign") => sign(&args),
        _ => Err("usage: ni-release keygen <file> | ni-release sign --key <file> --exe <path> --version <x.y.z> --url <https url> [--channel stable|beta] [--notes <text>] --out <file>".into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("{why}");
            ExitCode::FAILURE
        }
    }
}
