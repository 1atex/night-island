use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use ed25519_dalek::{Signer, SigningKey};
use prost::Message;
use sha2::{Digest, Sha256};

use crate::services::proto::{Lyrics, LyricsPut, LyricsReply};
use crate::services::{http, integrations};

pub const BASE: &str = "https://backend.night-watchers.com/api/v2/night_island";
const USER_AGENT: &str = "NightIsland/0.1";
const ID_DOMAIN: &[u8] = b"night-island/lyrics/v1\0";
const PUT_DOMAIN: &[u8] = b"night-island/lyrics-put/v1\0";
const KEY_SECRET: &str = "install_key";
const MAX_REPLY_BYTES: usize = 400_000;
const MAX_TEXT_BYTES: usize = 200_000;

pub fn norm(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

pub fn lyrics_id(artist: &str, title: &str) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(ID_DOMAIN);
    hash.update(norm(artist).as_bytes());
    hash.update([0]);
    hash.update(norm(title).as_bytes());
    hash.finalize().into()
}

pub fn id_path(id: &[u8; 32]) -> String {
    URL_SAFE_NO_PAD.encode(id)
}

fn put_message(id: &[u8], lyrics: &Lyrics) -> Vec<u8> {
    let mut out = PUT_DOMAIN.to_vec();
    out.extend_from_slice(id);
    out.extend_from_slice(lyrics.lrc.as_bytes());
    out.push(0);
    out.extend_from_slice(lyrics.plain.as_bytes());
    out
}

pub fn make_put(key: &SigningKey, id: [u8; 32], lyrics: Lyrics) -> LyricsPut {
    let signature = key.sign(&put_message(&id, &lyrics)).to_bytes().to_vec();
    LyricsPut { id: id.to_vec(), lyrics: Some(lyrics), client_key: key.verifying_key().to_bytes().to_vec(), signature }
}

#[cfg(test)]
fn put_is_valid(put: &LyricsPut) -> bool {
    let (Some(lyrics), Ok(key), Ok(sig)) = (
        put.lyrics.as_ref(),
        <[u8; 32]>::try_from(put.client_key.as_slice()),
        <[u8; 64]>::try_from(put.signature.as_slice()),
    ) else {
        return false;
    };
    ed25519_dalek::VerifyingKey::from_bytes(&key).is_ok_and(|k| {
        k.verify_strict(&put_message(&put.id, lyrics), &ed25519_dalek::Signature::from_bytes(&sig)).is_ok()
    })
}

pub fn install_key() -> Option<SigningKey> {
    if let Some(hex) = integrations::load_secret(KEY_SECRET)
        && let Some(seed) = from_hex32(&hex)
    {
        return Some(SigningKey::from_bytes(&seed));
    }
    let seed: [u8; 32] = integrations::random_bytes(32).try_into().ok()?;
    if seed == [0; 32] {
        return None;
    }
    integrations::save_secret(KEY_SECRET, &to_hex(&seed));
    Some(SigningKey::from_bytes(&seed))
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub(crate) fn from_hex32(text: &str) -> Option<[u8; 32]> {
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

pub fn fetch_lrc(id: &[u8; 32]) -> Option<String> {
    let url = format!("{BASE}/lyrics/{}", id_path(id));
    let reply = http::get_bytes(&url, MAX_REPLY_BYTES, Some(USER_AGENT))?;
    if reply.status != 200 {
        crate::log_debug!("shared lyrics cache answered {}", reply.status);
        return None;
    }
    lrc_from_reply(&reply.body)
}

fn lrc_from_reply(body: &[u8]) -> Option<String> {
    let reply = LyricsReply::decode(body).ok()?;
    let lrc = reply.lyrics.filter(|_| reply.found)?.lrc;
    (!lrc.is_empty() && lrc.len() <= MAX_TEXT_BYTES).then_some(lrc)
}

pub fn upload(id: [u8; 32], lrc: String, plain: String) {
    if lrc.len() > MAX_TEXT_BYTES || plain.len() > MAX_TEXT_BYTES || (lrc.is_empty() && plain.is_empty()) {
        return;
    }
    let Some(key) = install_key() else { return };
    let put = make_put(&key, id, Lyrics { lrc, plain });
    let url = format!("{BASE}/lyrics/{}", id_path(&id));
    match http::send_bytes(
        "PUT",
        &url,
        &put.encode_to_vec(),
        "application/x-protobuf",
        MAX_REPLY_BYTES,
        Some(USER_AGENT),
    ) {
        Some(reply) => crate::log_debug!("shared lyrics upload answered {}", reply.status),
        None => crate::log_debug!("shared lyrics upload could not be made"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        to_hex(bytes)
    }

    #[test]
    fn track_ids_have_fixed_known_values() {
        assert_eq!(
            hex(&lyrics_id("Daft Punk", "Get Lucky")),
            "3748f8cbf41316e66c9d9fe5f60396c9e974f8739580250a092b8107765da68b"
        );
        assert_eq!(
            hex(&lyrics_id("Сплин", "Выхода нет")),
            "61241f9c17e72580dfdf0110449d33cf9a0ca0dd14cf00874cd224dc4b638409"
        );
    }

    #[test]
    fn ids_ignore_case_and_spacing_but_not_words() {
        assert_eq!(lyrics_id("  DAFT   punk ", "get\tLUCKY"), lyrics_id("Daft Punk", "Get Lucky"));
        assert_ne!(lyrics_id("Daft Punk", "Get Lucky"), lyrics_id("Daft Punk", "Get Luck"));
        assert_ne!(lyrics_id("ab", "c"), lyrics_id("a", "bc"));
        assert_eq!(id_path(&[0xfb; 32]).len(), 43);
        assert!(!id_path(&[0xfb; 32]).contains(['+', '/', '=']));
    }

    #[test]
    fn uploads_verify_and_cannot_be_altered() {
        let key = SigningKey::from_bytes(&[9; 32]);
        let id = lyrics_id("a", "b");
        let lyrics = Lyrics { lrc: "[00:01.00]x\n[00:02.00]y".into(), plain: "x y".into() };
        let put = make_put(&key, id, lyrics);
        assert!(put_is_valid(&put));
        let mut other_text = put.clone();
        other_text.lyrics.as_mut().unwrap().lrc.push('!');
        assert!(!put_is_valid(&other_text));
        let mut other_id = put.clone();
        other_id.id[0] ^= 1;
        assert!(!put_is_valid(&other_id));
        let mut other_key = put.clone();
        other_key.client_key = SigningKey::from_bytes(&[8; 32]).verifying_key().to_bytes().to_vec();
        assert!(!put_is_valid(&other_key));
        let mut short = put;
        short.signature.truncate(10);
        assert!(!put_is_valid(&short));
    }

    #[test]
    fn replies_are_read_defensively() {
        let found =
            LyricsReply { found: true, lyrics: Some(Lyrics { lrc: "[00:01.00]a".into(), plain: String::new() }) };
        assert_eq!(lrc_from_reply(&found.encode_to_vec()).as_deref(), Some("[00:01.00]a"));
        assert_eq!(lrc_from_reply(&[]), None, "an empty body is a miss");
        assert_eq!(lrc_from_reply(b"\xff\xff"), None);
        let no_lrc =
            LyricsReply { found: true, lyrics: Some(Lyrics { lrc: String::new(), plain: "only words".into() }) };
        assert_eq!(lrc_from_reply(&no_lrc.encode_to_vec()), None);
        let lying =
            LyricsReply { found: false, lyrics: Some(Lyrics { lrc: "[00:01.00]a".into(), plain: String::new() }) };
        assert_eq!(lrc_from_reply(&lying.encode_to_vec()), None);
        let huge = LyricsReply {
            found: true,
            lyrics: Some(Lyrics { lrc: "x".repeat(MAX_TEXT_BYTES + 1), plain: String::new() }),
        };
        assert_eq!(lrc_from_reply(&huge.encode_to_vec()), None);
    }

    #[test]
    fn hex_keys_round_trip() {
        let seed = [0xab; 32];
        assert_eq!(from_hex32(&to_hex(&seed)), Some(seed));
        assert_eq!(from_hex32("zz"), None);
        assert_eq!(from_hex32(&"0".repeat(63)), None);
    }
}
