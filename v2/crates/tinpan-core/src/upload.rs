//! Uploading a song to Suno, as Tin Pan 1 does it: a ticket for Suno's storage, the file sent to it (one more
//! try with a fresh ticket if the connection drops), upload-finish, Suno's check (every 4 seconds, up to 5
//! minutes), initialize-clip, then the title, lyrics, the cover Suno made and the accepted upload terms.

use std::collections::BTreeMap;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use reqwest::Method;
use reqwest::blocking::{Client, multipart};
use serde_json::{Value, json};

use crate::api::SunoApi;
use crate::error::{Error, Result};
use crate::i18n::{t, tf};
use crate::json as sj;

/// The audio types Suno's web uploader takes as they are.
pub const EXTENSIONS: [&str; 6] = ["mp3", "m4a", "wav", "flac", "ogg", "aac"];

/// The only storage host an upload may go to.
pub const STORAGE_HOST: &str = "suno-uploads.s3.amazonaws.com";

/// A bound on what Tin Pan will push through a chosen file (not a Suno limit).
pub const MAX_BYTES: u64 = 500 * 1024 * 1024;

const REQUIRED_FIELDS: [&str; 4] = ["AWSAccessKeyId", "key", "policy", "signature"];
const PROCESSING_LIMIT: Duration = Duration::from_secs(5 * 60);
const POLL_EVERY: Duration = Duration::from_secs(4);

/// Where and how to send an upload's bytes: a single-use signed storage POST.
#[derive(Debug, Clone, PartialEq)]
pub struct Ticket {
    pub id: String,
    pub url: String,
    pub fields: BTreeMap<String, String>,
}

/// What an upload is at, for saying it: sending, then Suno's check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Sending,
    Checking,
}

/// Only an https address on Suno's storage host, with no port, user or fragment.
pub fn is_storage(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else { return false };
    let host = rest.split(['/', '?']).next().unwrap_or("");
    host.eq_ignore_ascii_case(STORAGE_HOST) && !url.contains('#')
}

/// Checks the initializer's reply: an id, an allowed storage address, and every signed field.
pub fn read_ticket(json: &str) -> Result<Ticket> {
    let broken = || Error::api(t("Suno didn't start the upload properly. Try again."));
    let root: Value = serde_json::from_str(json).map_err(|_| Error::api(t("Suno sent a reply this app doesn't understand.")))?;
    let id = root.get("id").and_then(Value::as_str).filter(|s| !s.is_empty()).ok_or_else(broken)?.to_string();
    let url = root.get("url").and_then(Value::as_str).ok_or_else(broken)?.to_string();
    if !is_storage(&url) {
        return Err(Error::api(t("Suno gave an upload address this app won't use.")));
    }
    let mut fields = BTreeMap::new();
    for (name, value) in root.get("fields").and_then(Value::as_object).ok_or_else(broken)? {
        fields.insert(name.clone(), value.as_str().ok_or_else(broken)?.to_string());
    }
    if REQUIRED_FIELDS.iter().any(|f| fields.get(*f).is_none_or(String::is_empty)) {
        return Err(broken());
    }
    Ok(Ticket { id, url, fields })
}

/// A file Suno takes: its name without the extension, the extension, and the name to send.
fn checked_file(path: &Path) -> Result<(String, String, String)> {
    let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    if size == 0 {
        return Err(Error::api(t("That file is empty or missing.")));
    }
    if size > MAX_BYTES {
        return Err(Error::api(t("That file is over 500 MB, too big to upload.")));
    }
    let extension = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    if !EXTENSIONS.contains(&extension.as_str()) {
        return Err(Error::api(t("Choose an MP3, M4A, WAV, FLAC, OGG or AAC file.")));
    }
    let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let file_name = format!("{stem}.{extension}");
    Ok((stem, extension, file_name))
}

impl SunoApi {
    fn start_upload(&self, extension: &str, upload_type: &str) -> Result<Ticket> {
        match self.send(Method::POST, "/api/uploads/audio/", Some(&json!({ "extension": extension, "is_stem_mix": false, "upload_type": upload_type }))) {
            Ok(reply) => read_ticket(&reply),
            Err(e) if e.status() == Some(423) => Err(Error::api(t("Suno doesn't allow uploads from this account right now."))),
            Err(e) => Err(e),
        }
    }

    /// Sends a song file and makes a song of it; the new song's id. `stage` hears when Suno starts checking it.
    pub fn upload_song(&self, storage: &Client, path: &Path, title: &str, lyrics: &str, stage: &dyn Fn(Stage)) -> Result<String> {
        let (stem, extension, file_name) = checked_file(path)?;
        stage(Stage::Sending);
        let upload_id = self.send_with_retry(storage, path, &file_name, &extension, "file_upload")?;
        let finish = json!({ "agreed_to_vip_upload_terms": false, "upload_filename": file_name, "upload_type": "file_upload" });
        self.send(Method::POST, &format!("/api/uploads/audio/{}/upload-finish/", crate::api::escape(&upload_id)), Some(&finish))?;
        stage(Stage::Checking);
        let (processed_title, image) = self.wait_for_processing(&upload_id)?;
        let reply = self.send(Method::POST, &format!("/api/uploads/audio/{}/initialize-clip/", crate::api::escape(&upload_id)), Some(&json!({ "user_reviewed_tags": true })))?;
        let clip_id = sj::try_read_string(&reply, "clip_id")
            .filter(|c| !c.is_empty())
            .or_else(|| sj::find_clips_by_id(&reply).ok().and_then(|c| c.into_iter().next()).map(|c| c.id))
            .ok_or_else(|| Error::api(t("Suno processed the upload but didn't make a song from it. Refresh the library to check.")))?;
        let name = if title.trim().is_empty() { processed_title.unwrap_or(stem) } else { title.trim().to_string() };
        let mut body = json!({ "title": name, "is_audio_upload_tos_accepted": true });
        if !lyrics.trim().is_empty() {
            body["lyrics"] = json!(lyrics);
        }
        if let Some(image) = image.filter(|i| !i.trim().is_empty()) {
            body["image_url"] = json!(image);
        }
        self.send(Method::POST, &format!("/api/gen/{}/set_metadata/", crate::api::escape(&clip_id)), Some(&body))?;
        log::info!("Uploaded {file_name} as clip {clip_id}");
        Ok(clip_id)
    }

    /// A recording for Suno to work with rather than a song (`upload_type` "voice_recording" for a voice
    /// persona): sent, finished and checked, as Suno's own recorder does it; the upload's id.
    pub(crate) fn upload_recording(&self, storage: &Client, path: &Path, upload_type: &str) -> Result<String> {
        let (_, extension, file_name) = checked_file(path)?;
        let upload_id = self.send_with_retry(storage, path, &file_name, &extension, upload_type)?;
        let finish = json!({ "agreed_to_vip_upload_terms": false, "upload_filename": file_name, "upload_type": upload_type });
        self.send(Method::POST, &format!("/api/uploads/audio/{}/upload-finish/", crate::api::escape(&upload_id)), Some(&finish))?;
        self.wait_for_processing(&upload_id)?;
        Ok(upload_id)
    }

    /// A signed ticket can't be replayed, so a dropped connection or a storage error gets one more try with a fresh one.
    fn send_with_retry(&self, storage: &Client, path: &Path, file_name: &str, extension: &str, upload_type: &str) -> Result<String> {
        for attempt in 1..=2 {
            let ticket = self.start_upload(extension, upload_type)?;
            match post_to_storage(storage, &ticket, path, file_name) {
                Ok(()) => return Ok(ticket.id),
                Err(transient) => {
                    log::warn!("Upload storage POST failed, attempt {attempt}");
                    if !transient {
                        break;
                    }
                }
            }
        }
        Err(Error::api(t("The upload didn't go through. Check your internet connection and try again.")))
    }

    /// Suno's title and cover for the upload once it's checked, or why it failed.
    fn wait_for_processing(&self, upload_id: &str) -> Result<(Option<String>, Option<String>)> {
        let until = Instant::now() + PROCESSING_LIMIT;
        loop {
            let reply = self.send(Method::GET, &format!("/api/uploads/audio/{}/", crate::api::escape(upload_id)), None)?;
            match sj::try_read_string(&reply, "status").as_deref() {
                Some("complete") => return Ok((sj::try_read_string(&reply, "title"), sj::try_read_string(&reply, "image_url"))),
                Some("error") => {
                    return Err(Error::api(match sj::try_read_string(&reply, "error_message").filter(|m| !m.trim().is_empty()) {
                        Some(why) => tf("Suno couldn't process the upload: {0}", &[&why.trim()]),
                        None => t("Suno couldn't process the upload."),
                    }));
                }
                _ => {}
            }
            if Instant::now() >= until {
                return Err(Error::api(t("Suno took too long processing the upload. It may still appear in your library later.")));
            }
            thread::sleep(POLL_EVERY);
        }
    }
}

/// The picture types Suno takes for covers.
pub const PICTURE_EXTENSIONS: [&str; 4] = ["jpg", "jpeg", "png", "webp"];

impl SunoApi {
    /// Sends a picture for a cover; its upload's id once Suno approves it.
    pub fn upload_picture(&self, storage: &Client, path: &Path) -> Result<String> {
        let extension = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        if !PICTURE_EXTENSIONS.contains(&extension.as_str()) {
            return Err(Error::api(t("Choose a JPG, PNG or WebP picture.")));
        }
        if std::fs::metadata(path).map(|m| m.len()).unwrap_or(0) > 64 * 1024 * 1024 {
            return Err(Error::api(t("That picture is too big for Suno.")));
        }
        let extension = if extension == "jpeg" { "jpg".to_string() } else { extension };
        let ticket = read_ticket(&self.send(Method::POST, "/api/uploads/image/", Some(&json!({ "extension": extension })))?)?;
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        if post_to_storage(storage, &ticket, path, &name).is_err() {
            log::warn!("Picture storage POST failed");
            return Err(Error::api(t("The upload didn't go through. Check your internet connection and try again.")));
        }
        let reply = self.send(Method::POST, &format!("/api/uploads/image/{}/upload-finish/", crate::api::escape(&ticket.id)), Some(&json!({})))?;
        if sj::try_read_string(&reply, "moderation_status").is_some_and(|s| s != "approved") {
            return Err(Error::api(t("Suno didn't accept that picture.")));
        }
        Ok(ticket.id)
    }
}

/// The signed fields first, then the file, as the storage's POST policy expects. Err(true) is worth a retry.
fn post_to_storage(storage: &Client, ticket: &Ticket, path: &Path, file_name: &str) -> std::result::Result<(), bool> {
    if !is_storage(&ticket.url) {
        return Err(false);
    }
    let mut form = multipart::Form::new();
    for (name, value) in &ticket.fields {
        form = form.text(name.clone(), value.clone());
    }
    let mime = ticket.fields.get("Content-Type").cloned().unwrap_or_else(|| "application/octet-stream".into());
    let part = multipart::Part::file(path).map_err(|_| false)?.file_name(file_name.to_string()).mime_str(&mime).map_err(|_| false)?;
    form = form.part("file", part);
    match storage.post(&ticket.url).timeout(Duration::from_secs(30 * 60)).multipart(form).send() {
        Ok(response) if response.status().is_success() => Ok(()),
        Ok(response) => Err(response.status().is_server_error()),
        Err(e) => {
            log::warn!("Upload storage POST couldn't connect: {e}");
            Err(true)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tickets_need_suno_storage_and_every_signed_field() {
        let good = r#"{"id":"u1","url":"https://suno-uploads.s3.amazonaws.com/","fields":{"AWSAccessKeyId":"a","key":"k","policy":"p","signature":"s","Content-Type":"audio/mpeg"}}"#;
        let ticket = read_ticket(good).unwrap();
        assert_eq!((ticket.id.as_str(), ticket.fields.len()), ("u1", 5));
        assert!(read_ticket(&good.replace("suno-uploads.s3.amazonaws.com", "evil.example.com")).is_err());
        assert!(read_ticket(&good.replace(r#""policy":"p","#, "")).is_err());
        assert!(!is_storage("http://suno-uploads.s3.amazonaws.com/"));
        assert!(!is_storage("https://suno-uploads.s3.amazonaws.com.evil.com/"));
    }
}
