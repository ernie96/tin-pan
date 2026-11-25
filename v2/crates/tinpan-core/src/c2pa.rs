//! Suno C2PA watermark detection and lossless metadata stripping.
//! Detection queries Suno's official validator: POST https://studio-api-prod.suno.com/api/c2pa/detect
//! Stripping removes the C2PA JUMBF / GEOB metadata without re-encoding audio.

use std::fs::File;
use std::io::Write;
use std::path::Path;

use reqwest::blocking::Client;
use serde::Deserialize;

use crate::error::{Error, Result};
use crate::i18n::t;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WatermarkVerdict {
    VerifiedSuno,
    NoSunoProvenance,
    Inconclusive,
}

#[derive(Deserialize)]
struct DetectResponse {
    verdict: String,
}

/// Checks whether an audio file carries Suno's C2PA content credentials via Suno's official endpoint.
pub fn detect_watermark(client: &Client, file_path: &Path) -> Result<WatermarkVerdict> {
    let file_name = file_path.file_name().and_then(|f| f.to_str()).unwrap_or("audio.mp3");
    let file_bytes = std::fs::read(file_path).map_err(|e| Error::api(e.to_string()))?;
    if file_bytes.is_empty() {
        return Err(Error::api(t("That file is empty.")));
    }

    let part = reqwest::blocking::multipart::Part::bytes(file_bytes)
        .file_name(file_name.to_string())
        .mime_str("application/octet-stream")
        .map_err(|e| Error::api(e.to_string()))?;

    let form = reqwest::blocking::multipart::Form::new().part("file", part);

    let res = client
        .post("https://studio-api-prod.suno.com/api/c2pa/detect")
        .multipart(form)
        .send()
        .map_err(|e| Error::api(format!("{}: {e}", t("Can't reach Suno. Check your internet connection."))))?;

    if res.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err(Error::api(t("Suno rate-limited this request. Try again in a moment.")));
    }
    if !res.status().is_success() {
        return Err(Error::api(t("Suno couldn't check the watermark for that file.")));
    }

    let body: DetectResponse = res.json().map_err(|_| Error::api(t("Suno sent a reply this app doesn't understand.")))?;

    match body.verdict.as_str() {
        "verified_suno" => Ok(WatermarkVerdict::VerifiedSuno),
        "no_suno_provenance" => Ok(WatermarkVerdict::NoSunoProvenance),
        _ => Ok(WatermarkVerdict::Inconclusive),
    }
}

/// Removes Suno C2PA provenance credentials from an audio file without re-encoding.
/// Returns true if a watermark was found and stripped, or false if none was found.
pub fn strip_watermark(input: &Path, output: &Path) -> Result<bool> {
    let data = std::fs::read(input).map_err(|e| Error::api(e.to_string()))?;
    let ext = input.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();

    let (stripped_data, found) = match ext.as_str() {
        "mp3" | "flac" => strip_id3_jumbf(&data),
        "wav" => strip_wav_c2pa(&data),
        "m4a" | "mp4" => strip_m4a_c2pa(&data),
        _ => {
            // Try ID3 first, then WAV/M4A heuristics
            if data.starts_with(b"ID3") {
                strip_id3_jumbf(&data)
            } else if data.starts_with(b"RIFF") {
                strip_wav_c2pa(&data)
            } else if data.len() > 8 && &data[4..8] == b"ftyp" {
                strip_m4a_c2pa(&data)
            } else {
                (data.clone(), false)
            }
        }
    };

    if found {
        let mut f = File::create(output).map_err(|e| Error::api(e.to_string()))?;
        f.write_all(&stripped_data).map_err(|e| Error::api(e.to_string()))?;
    }

    Ok(found)
}

fn syncsafe_to_int(bytes: &[u8]) -> usize {
    ((bytes[0] as usize & 0x7f) << 21)
        | ((bytes[1] as usize & 0x7f) << 14)
        | ((bytes[2] as usize & 0x7f) << 7)
        | (bytes[3] as usize & 0x7f)
}

fn int_to_syncsafe(val: usize, out: &mut [u8]) {
    out[0] = ((val >> 21) & 0x7f) as u8;
    out[1] = ((val >> 14) & 0x7f) as u8;
    out[2] = ((val >> 7) & 0x7f) as u8;
    out[3] = (val & 0x7f) as u8;
}

fn read_u32_be(bytes: &[u8]) -> usize {
    ((bytes[0] as usize) << 24)
        | ((bytes[1] as usize) << 16)
        | ((bytes[2] as usize) << 8)
        | (bytes[3] as usize)
}

/// Strips GEOB frames containing "application/jumbf" from ID3v2 tags.
fn strip_id3_jumbf(data: &[u8]) -> (Vec<u8>, bool) {
    if data.len() < 10 || !data.starts_with(b"ID3") {
        return (data.to_vec(), false);
    }

    let version = data[3];
    if !(3..=4).contains(&version) {
        return (data.to_vec(), false);
    }
    let is_v4 = version == 4;

    let tag_size = syncsafe_to_int(&data[6..10]);
    let total_tag_len = 10 + tag_size;
    if total_tag_len > data.len() {
        return (data.to_vec(), false);
    }

    let mut pos = 10;
    let mut frames_out = Vec::with_capacity(tag_size);
    let mut stripped_any = false;

    while pos + 10 <= total_tag_len {
        let frame_id = &data[pos..pos + 4];
        if frame_id[0] == 0 {
            // Padding reached
            break;
        }

        let frame_size = if is_v4 {
            syncsafe_to_int(&data[pos + 4..pos + 8])
        } else {
            read_u32_be(&data[pos + 4..pos + 8])
        };

        let frame_total_len = 10 + frame_size;
        if pos + frame_total_len > total_tag_len {
            break;
        }

        let frame_data = &data[pos..pos + frame_total_len];
        if frame_id == b"GEOB" {
            // Check if GEOB contains "application/jumbf"
            let content = &data[pos + 10..pos + frame_total_len];
            let is_jumbf = content.windows(17).any(|w| w.eq_ignore_ascii_case(b"application/jumbf"));
            if is_jumbf {
                stripped_any = true;
                pos += frame_total_len;
                continue;
            }
        }

        frames_out.extend_from_slice(frame_data);
        pos += frame_total_len;
    }

    if !stripped_any {
        return (data.to_vec(), false);
    }

    // Build new ID3 tag
    let new_tag_size = frames_out.len();
    let mut new_data = Vec::with_capacity(10 + new_tag_size + (data.len() - total_tag_len));
    new_data.extend_from_slice(&data[0..6]);
    let mut size_bytes = [0u8; 4];
    int_to_syncsafe(new_tag_size, &mut size_bytes);
    new_data.extend_from_slice(&size_bytes);
    new_data.extend_from_slice(&frames_out);
    new_data.extend_from_slice(&data[total_tag_len..]);

    (new_data, true)
}

/// Strips C2PA / JUMBF RIFF chunks from a WAV file.
fn strip_wav_c2pa(data: &[u8]) -> (Vec<u8>, bool) {
    if data.len() < 12 || !data.starts_with(b"RIFF") || &data[8..12] != b"WAVE" {
        return (data.to_vec(), false);
    }

    let mut pos = 12;
    let mut out = Vec::with_capacity(data.len());
    out.extend_from_slice(&data[0..12]);
    let mut stripped_any = false;

    while pos + 8 <= data.len() {
        let chunk_id = &data[pos..pos + 4];
        let chunk_size = (data[pos + 4] as usize)
            | ((data[pos + 5] as usize) << 8)
            | ((data[pos + 6] as usize) << 16)
            | ((data[pos + 7] as usize) << 24);

        let padded_size = chunk_size + (chunk_size % 2);
        let total_chunk_len = 8 + padded_size;
        if pos + total_chunk_len > data.len() {
            out.extend_from_slice(&data[pos..]);
            break;
        }

        let is_c2pa = chunk_id == b"c2pa" || chunk_id == b"jumb" || chunk_id == b"C2PA";
        if is_c2pa {
            stripped_any = true;
        } else {
            out.extend_from_slice(&data[pos..pos + total_chunk_len]);
        }
        pos += total_chunk_len;
    }

    if stripped_any && out.len() >= 8 {
        let riff_size = (out.len() - 8) as u32;
        out[4..8].copy_from_slice(&riff_size.to_le_bytes());
    }

    (out, stripped_any)
}

/// Strips top-level c2pa / jumb boxes from MP4/M4A containers.
fn strip_m4a_c2pa(data: &[u8]) -> (Vec<u8>, bool) {
    if data.len() < 8 {
        return (data.to_vec(), false);
    }

    // Keep box lengths and positions intact so media chunk offsets remain valid.
    const MANIFEST_UUID: &[u8] = &[0xd8,0xfe,0xc3,0xd6,0x1b,0x0e,0x48,0x3c,0x92,0x97,0x58,0x28,0x87,0x7e,0xc4,0x81];
    let mut out = data.to_vec();
    let mut pos = 0;
    let mut found = false;
    while let Some(header) = data.get(pos..).and_then(|b| b.get(..8)) {
        let mut size = read_u32_be(header);
        let mut header_len = 8;
        if size == 0 {
            size = data.len() - pos;
        } else if size == 1 {
            let Some(bytes) = data[pos..].get(8..16) else { break };
            let Ok(large) = usize::try_from(u64::from_be_bytes(bytes.try_into().unwrap())) else { break };
            size = large;
            header_len = 16;
        }
        if size < header_len || size > data.len() - pos { break; }
        let kind = &header[4..8];
        let payload = &data[pos + header_len..pos + size];
        if kind == b"c2pa" || kind == b"jumb" || (kind == b"uuid" && payload.starts_with(MANIFEST_UUID)) {
            out[pos + 4..pos + 8].copy_from_slice(b"free");
            out[pos + header_len..pos + size].fill(0);
            found = true;
        }
        pos += size;
    }
    (out, found)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_uuid_is_erased_without_changing_media_offsets() {
        let mut data = vec![0, 0, 0, 1];
        data.extend_from_slice(b"uuid");
        data.extend_from_slice(&35u64.to_be_bytes());
        data.extend_from_slice(&[0xd8, 0xfe, 0xc3, 0xd6, 0x1b, 0x0e, 0x48, 0x3c, 0x92, 0x97, 0x58, 0x28, 0x87, 0x7e, 0xc4, 0x81]);
        data.extend_from_slice(b"abc");
        data.extend_from_slice(b"\0\0\0\0mdatmedia");
        let (clean, found) = strip_m4a_c2pa(&data);
        assert!(found);
        assert_eq!(&clean[4..8], b"free");
        assert!(clean[16..35].iter().all(|byte| *byte == 0));
        assert_eq!(&clean[35..], &data[35..]);
        data[16] ^= 1;
        assert_eq!(strip_m4a_c2pa(&data), (data, false));
    }

    #[test]
    fn strips_id3_jumbf_frame() {
        let mut fake_mp3 = Vec::new();
        // ID3 header
        fake_mp3.extend_from_slice(b"ID3\x04\x00\x00");
        // Frame 1: TIT2 (Title)
        let tit2 = b"TIT2\x00\x00\x00\x05\x00\x00\x00Test";
        // Frame 2: GEOB with application/jumbf
        let geob_content = b"\x00application/jumbf\x00\x00desc\x00payload";
        let mut geob = Vec::new();
        geob.extend_from_slice(b"GEOB");
        let mut geob_size = [0u8; 4];
        int_to_syncsafe(geob_content.len(), &mut geob_size);
        geob.extend_from_slice(&geob_size);
        geob.extend_from_slice(b"\x00\x00");
        geob.extend_from_slice(geob_content);

        let total_frames_len = tit2.len() + geob.len();
        let mut tag_size = [0u8; 4];
        int_to_syncsafe(total_frames_len, &mut tag_size);
        fake_mp3.extend_from_slice(&tag_size);

        fake_mp3.extend_from_slice(tit2);
        fake_mp3.extend_from_slice(&geob);
        // Audio payload
        fake_mp3.extend_from_slice(b"\xFF\xFB\x90\x64audio_data");

        let (stripped, found) = strip_id3_jumbf(&fake_mp3);
        assert!(found);
        assert!(!stripped.windows(17).any(|w| w == b"application/jumbf"));
        assert!(stripped.windows(4).any(|w| w == b"TIT2"));
    }
}
