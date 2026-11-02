//! Suno's song files. Since September 2026 audio_url is a decoy (/api/forbidden); the real file is the
//! progressive M4A, encrypted with AES-CTR. POST /api/mango/rights gives its key and counter, each
//! wrapped in AES-GCM under SHA-256 of the account's token (or the reply's glt). Unwrapping them and
//! decrypting the file uses none of the account's downloads.

use aes::cipher::{KeyIvInit, StreamCipher};
use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::Engine;
use sha2::{Digest, Sha256};

use crate::clip::SunoClip;
use crate::error::{Error, Result};
use crate::i18n::t;

pub const DEFAULT_CDN_TEMPLATE: &str = "https://d2lwuy8qc234o3.cloudfront.net/1/clip/{0}.m4a";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SunoRights {
    pub key: String,
    pub iv: String,
    pub glt: String,
    pub aad: String,
    pub media_url: Option<String>,
}

pub fn default_media_url(clip_id: &str) -> String {
    DEFAULT_CDN_TEMPLATE.replace("{0}", &escape(clip_id))
}

fn escape(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

pub fn is_forbidden_decoy(url: Option<&str>) -> bool {
    url.is_none_or(|u| u.trim().is_empty() || u.to_ascii_lowercase().contains("forbidden"))
}

/// The best media address for a clip and whether it is encrypted: an unencrypted progressive file
/// first (when `prefer_unencrypted`), then the encrypted one, then the CDN's usual address.
pub fn pick_media_url(clip: &SunoClip, prefer_unencrypted: bool) -> (String, bool) {
    let media = clip.media_urls.as_deref().unwrap_or_default();
    if prefer_unencrypted
        && let Some(url) = media.iter().find(|m| !m.is_encrypted() && !m.is_video() && !is_forbidden_decoy(m.url.as_deref()))
    {
        return (url.url.clone().unwrap_or_default(), false);
    }
    if let Some(m) = media.iter().find(|m| !m.is_video() && !is_forbidden_decoy(m.url.as_deref())) {
        return (m.url.clone().unwrap_or_default(), m.is_encrypted());
    }
    if !is_forbidden_decoy(clip.audio_url.as_deref()) {
        return (clip.audio_url.clone().unwrap_or_default(), false);
    }
    (default_media_url(&clip.id), true)
}

/// Key material in hexadecimal, base64 or base64url. Empty if it is none of those.
pub fn decode_key_material(text: &str) -> Vec<u8> {
    let clean = text.trim();
    if clean.is_empty() {
        return Vec::new();
    }
    if clean.len().is_multiple_of(2)
        && clean.bytes().all(|b| b.is_ascii_hexdigit())
        && let Ok(bytes) = hex::decode(clean)
    {
        return bytes;
    }
    let standard = clean.replace('-', "+").replace('_', "/");
    let unpadded = standard.trim_end_matches('=');
    base64::engine::general_purpose::STANDARD_NO_PAD.decode(unpadded).unwrap_or_default()
}

/// Opens a 12-byte nonce + ciphertext + 16-byte tag envelope under AES-256-GCM. 16 or 32 raw bytes, or
/// anything too short to be an envelope, come back as they are.
pub fn unwrap_envelope(raw: &[u8], user_key: &[u8; 32], aad: &[u8]) -> Result<Vec<u8>> {
    if matches!(raw.len(), 16 | 32) || raw.len() < 28 {
        return Ok(raw.to_vec());
    }
    let cipher = Aes256Gcm::new(user_key.into());
    cipher
        .decrypt(Nonce::from_slice(&raw[..12]), Payload { msg: &raw[12..], aad })
        .map_err(|_| Error::api("The song's key didn't unlock."))
}

/// Reads the reply of POST /api/mango/rights: the fields at the top or under "data", in any of the names
/// Suno has used. None when it has no key and counter (an error page, say).
pub fn parse_rights(json: &str, clip_id: &str) -> Option<SunoRights> {
    let root: serde_json::Value = serde_json::from_str(json).ok()?;
    let root = root.as_object()?;
    let data = root.get("data").and_then(|d| d.as_object());
    let first = |names: &[&str]| {
        [data, Some(root)].into_iter().flatten().find_map(|obj| {
            names.iter().find_map(|n| obj.get(*n).and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(str::to_string))
        })
    };
    let key = first(&["key", "content_key"])?;
    let iv = first(&["iv", "content_iv"])?;
    let media_url = first(&["media_url", "mediaUrl", "url", "audio_url", "src"]).filter(|u| !is_forbidden_decoy(Some(u)));
    Some(SunoRights {
        key,
        iv,
        glt: first(&["glt", "glt_token"]).unwrap_or_default(),
        aad: first(&["aad"]).unwrap_or_else(|| clip_id.to_string()),
        media_url,
    })
}

/// The content key and counter from a rights reply. Tries each seed for the wrapping key (the bearer
/// token, then the glt) until the GCM tag checks out.
pub fn unwrap_rights(rights: &SunoRights, clip_id: &str, bearer_token: Option<&str>) -> Result<(Vec<u8>, [u8; 16])> {
    let raw_key = decode_key_material(&rights.key);
    let raw_iv = decode_key_material(&rights.iv);
    if raw_key.is_empty() || raw_iv.is_empty() {
        return Err(Error::api(t("Suno didn't send the key for this song's free file.")));
    }
    let aad = if rights.aad.is_empty() { clip_id } else { &rights.aad };

    let mut seeds: Vec<&str> = Vec::new();
    if let Some(token) = bearer_token.filter(|t| !t.is_empty()) {
        seeds.push(token);
    }
    if !rights.glt.is_empty() && !seeds.contains(&rights.glt.as_str()) {
        seeds.push(&rights.glt);
    }
    if seeds.is_empty() {
        seeds.push("");
    }

    for seed in seeds {
        let user_key: [u8; 32] = Sha256::digest(seed.as_bytes()).into();
        let (Ok(key), Ok(iv)) = (unwrap_envelope(&raw_key, &user_key, aad.as_bytes()), unwrap_envelope(&raw_iv, &user_key, aad.as_bytes()))
        else {
            continue;
        };
        if matches!(key.len(), 16 | 24 | 32)
            && let Ok(iv) = <[u8; 16]>::try_from(iv.as_slice())
        {
            return Ok((key, iv));
        }
    }
    Err(Error::api(t("Couldn't unlock this song's free file with your account.")))
}

/// "m4a", "mp3" or "unknown", from a file's first bytes.
pub fn detect_container(header: &[u8]) -> &'static str {
    if header.len() >= 8 && &header[4..8] == b"ftyp" {
        "m4a"
    } else if header.starts_with(b"ID3") || (header.len() >= 2 && header[0] == 0xFF && header[1] & 0xE0 == 0xE0) {
        "mp3"
    } else {
        "unknown"
    }
}

/// Decrypts (or encrypts: it's the same) a buffer in place with AES-CTR, a 128-bit big-endian counter.
pub fn apply_aes_ctr(data: &mut [u8], key: &[u8], iv: &[u8; 16]) -> Result<()> {
    type Ctr<C> = ctr::Ctr128BE<C>;
    let bad = |_| Error::api("Bad content key");
    match key.len() {
        16 => Ctr::<aes::Aes128>::new_from_slices(key, iv).map_err(bad)?.apply_keystream(data),
        24 => Ctr::<aes::Aes192>::new_from_slices(key, iv).map_err(bad)?.apply_keystream(data),
        32 => Ctr::<aes::Aes256>::new_from_slices(key, iv).map_err(bad)?.apply_keystream(data),
        n => return Err(Error::api(format!("Content key must be 16, 24 or 32 bytes, not {n}"))),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clip::MediaUrl;
    use rand::RngCore;

    fn media(url: &str, encoding: Option<&str>) -> MediaUrl {
        MediaUrl { url: Some(url.into()), encoding: encoding.map(|e| serde_json::Value::String(e.into())) }
    }

    fn clip(id: &str, media_urls: Vec<MediaUrl>) -> SunoClip {
        SunoClip {
            id: id.into(),
            audio_url: Some("https://studio-api.prod.suno.com/api/forbidden".into()),
            media_urls: Some(media_urls),
            ..SunoClip::default()
        }
    }

    #[test]
    fn picks_the_unencrypted_file_then_the_encrypted_one_then_the_cdn() {
        let both = clip("c1", vec![media("https://cdn.example.com/enc.m4a", Some("m4a-opus")), media("https://cdn.example.com/plain.m4a", None)]);
        assert_eq!(pick_media_url(&both, true), ("https://cdn.example.com/plain.m4a".into(), false));
        let encrypted = clip("c2", vec![media("https://cdn.example.com/enc.m4a", Some("m4a-opus"))]);
        assert_eq!(pick_media_url(&encrypted, true), ("https://cdn.example.com/enc.m4a".into(), true));
        let none = clip("test-clip-3", vec![]);
        assert_eq!(pick_media_url(&none, true), ("https://d2lwuy8qc234o3.cloudfront.net/1/clip/test-clip-3.m4a".into(), true));
    }

    #[test]
    fn decodes_hex_base64_and_base64url() {
        assert_eq!(decode_key_material("01020304"), vec![1, 2, 3, 4]);
        assert_eq!(decode_key_material("SGVsbG8="), b"Hello");
        assert_eq!(decode_key_material("--__"), vec![251, 239, 255]);
    }

    fn envelope(secret: &[u8], seed: &str, aad: &[u8]) -> String {
        let key: [u8; 32] = Sha256::digest(seed.as_bytes()).into();
        let mut nonce = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce);
        let sealed = Aes256Gcm::new((&key).into()).encrypt(Nonce::from_slice(&nonce), Payload { msg: secret, aad }).unwrap();
        base64::engine::general_purpose::STANDARD.encode([nonce.as_slice(), &sealed].concat())
    }

    #[test]
    fn unwraps_rights_with_the_token_or_falls_back_to_the_glt() {
        let mut key = [0u8; 32];
        let mut iv = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut key);
        rand::thread_rng().fill_bytes(&mut iv);
        for (wrapped_with, bearer) in [("test-bearer-token", "test-bearer-token"), ("valid-glt-token", "wrong-bearer-token")] {
            let rights = SunoRights {
                key: envelope(&key, wrapped_with, b"clip-123"),
                iv: envelope(&iv, wrapped_with, b"clip-123"),
                aad: "clip-123".into(),
                glt: "valid-glt-token".into(),
                media_url: None,
            };
            assert_eq!(unwrap_rights(&rights, "clip-123", Some(bearer)).unwrap(), (key.to_vec(), iv));
        }
    }

    #[test]
    fn parse_rights_reads_the_data_envelope_and_drops_the_decoy() {
        let json = r#"{"data":{"content_key":"0102030405060708090a0b0c0d0e0f10","content_iv":"1112131415161718191a1b1c1d1e1f20",
            "glt_token":"glt-abc","aad":"custom-aad","media_url":"https://studio-api.prod.suno.com/api/forbidden"}}"#;
        let rights = parse_rights(json, "clip-999").unwrap();
        assert_eq!((rights.key.as_str(), rights.glt.as_str(), rights.aad.as_str()), ("0102030405060708090a0b0c0d0e0f10", "glt-abc", "custom-aad"));
        assert_eq!(rights.media_url, None);
        assert_eq!(parse_rights("<html>", "c"), None);
    }

    #[test]
    fn aes_ctr_round_trips_and_carries_the_counter_across_bytes() {
        let mut key = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut key);
        // A counter about to carry out of its low bytes, so a 32-bit counter would go wrong.
        let mut iv = [0u8; 16];
        iv[12..].copy_from_slice(&[0xFF; 4]);
        let mut plain = vec![0u8; 150 * 1024 + 7];
        rand::thread_rng().fill_bytes(&mut plain);
        let mut data = plain.clone();
        apply_aes_ctr(&mut data, &key, &iv).unwrap();
        assert_ne!(data, plain);
        apply_aes_ctr(&mut data, &key, &iv).unwrap();
        assert_eq!(data, plain);
    }

    #[test]
    fn detects_m4a_and_mp3() {
        assert_eq!(detect_container(&[0, 0, 0, 0x20, b'f', b't', b'y', b'p', b'M', b'4', b'A', b' ']), "m4a");
        assert_eq!(detect_container(b"ID3\x04\0\0"), "mp3");
        assert_eq!(detect_container(&[0xFF, 0xFB, 0x90, 0x64]), "mp3");
        assert_eq!(detect_container(&[1, 2, 3, 4]), "unknown");
    }
}
