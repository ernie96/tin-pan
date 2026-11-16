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

/// Granular error classification when uploading an audio slice to Suno.
#[derive(Debug)]
pub enum PartUploadError {
    Rejected(String),
    RateLimited(Duration),
    Error(Error),
}

impl std::fmt::Display for PartUploadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rejected(msg) => write!(f, "Audio rejected: {msg}"),
            Self::RateLimited(dur) => write!(f, "Rate limited: retry in {}s", dur.as_secs()),
            Self::Error(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for PartUploadError {}

/// Checks whether an upload status, error type or error message indicates content rejection
/// (copyright, moderation, duplicate, existing recording, catalog match).
pub fn is_upload_rejected(status: Option<&str>, error_type: Option<&str>, error_message: Option<&str>) -> bool {
    let combined = format!(
        "{} {} {}",
        status.unwrap_or_default(),
        error_type.unwrap_or_default(),
        error_message.unwrap_or_default()
    ).to_lowercase();
    const REJECTED_WORDS: [&str; 6] = ["rejected", "moderation", "copyright", "duplicate", "existing recording", "catalog"];
    REJECTED_WORDS.iter().any(|word| combined.contains(word))
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

    pub fn upload_part(&self, storage: &Client, path: &Path, file_name: &str) -> Result<String> {
        self.upload_part_slice(storage, path, file_name).map_err(|e| match e {
            PartUploadError::Error(err) => err,
            PartUploadError::Rejected(reason) => Error::api(crate::i18n::tf("Suno turned down a part of the song: {0}", &[&reason])),
            PartUploadError::RateLimited(_) => Error::api(t("Suno says too many requests. Wait a minute and try again.")),
        })
    }

    /// Sends a single WAV slice of a track and initializes it as an upload clip, returning granular error classification.
    pub fn upload_part_slice(&self, storage: &Client, path: &Path, file_name: &str) -> std::result::Result<String, PartUploadError> {
        let ticket = match self.start_upload("wav", "studio_file_upload") {
            Ok(t) => t,
            Err(e) => {
                if e.status() == Some(429) || e.to_string().contains("too many requests") || e.to_string().contains("rate limit") {
                    return Err(PartUploadError::RateLimited(Duration::from_secs(15)));
                }
                return Err(PartUploadError::Error(e));
            }
        };

        if let Err(transient) = post_to_storage(storage, &ticket, path, file_name) {
            log::warn!("Storage POST failed (transient={transient}), trying one more time with fresh ticket");
            let ticket2 = self.start_upload("wav", "studio_file_upload").map_err(PartUploadError::Error)?;
            post_to_storage(storage, &ticket2, path, file_name).map_err(|_| PartUploadError::Error(Error::api(t("The upload didn't go through. Check your internet connection and try again."))))?;
        }

        let upload_id = ticket.id;
        let finish = json!({
            "agreed_to_vip_upload_terms": false,
            "upload_filename": file_name,
            "upload_type": "studio_file_upload"
        });
        if let Err(e) = self.send(Method::POST, &format!("/api/uploads/audio/{}/upload-finish/", crate::api::escape(&upload_id)), Some(&finish)) {
            if e.status() == Some(429) || e.to_string().contains("too many requests") {
                return Err(PartUploadError::RateLimited(Duration::from_secs(15)));
            }
            return Err(PartUploadError::Error(e));
        }

        let until = Instant::now() + Duration::from_secs(3 * 60);
        loop {
            let reply = match self.send(Method::GET, &format!("/api/uploads/audio/{}/", crate::api::escape(&upload_id)), None) {
                Ok(r) => r,
                Err(e) => {
                    // Asked to wait: waits, but never past the deadline below.
                    if e.status() == Some(429) && Instant::now() < until {
                        thread::sleep(Duration::from_secs(5));
                        continue;
                    }
                    return Err(PartUploadError::Error(e));
                }
            };

            let status = sj::try_read_string(&reply, "status");
            let err_type = sj::try_read_string(&reply, "error_type");
            let err_msg = sj::try_read_string(&reply, "error_message");

            if status.as_deref() == Some("complete") {
                break;
            }

            if status.as_deref() == Some("failed") || status.as_deref() == Some("rejected") || status.as_deref() == Some("error") {
                let reason = err_msg.clone().or_else(|| err_type.clone()).unwrap_or_else(|| status.clone().unwrap_or_default());
                if is_upload_rejected(status.as_deref(), err_type.as_deref(), err_msg.as_deref()) {
                    return Err(PartUploadError::Rejected(reason));
                } else {
                    return Err(PartUploadError::Error(Error::api(crate::i18n::tf("Suno couldn't process the upload: {0}", &[&reason]))));
                }
            }

            if Instant::now() >= until {
                return Err(PartUploadError::Error(Error::api(t("Suno took too long processing the upload. It may still appear in your library later."))));
            }
            thread::sleep(Duration::from_secs(3));
        }

        let init_reply = match self.send(Method::POST, &format!("/api/uploads/audio/{}/initialize-clip/", crate::api::escape(&upload_id)), Some(&json!({ "user_reviewed_tags": true }))) {
            Ok(r) => r,
            Err(e) => {
                if e.status() == Some(429) {
                    return Err(PartUploadError::RateLimited(Duration::from_secs(15)));
                }
                return Err(PartUploadError::Error(e));
            }
        };

        let clip_id = sj::try_read_string(&init_reply, "clip_id")
            .filter(|c| !c.is_empty())
            .or_else(|| sj::find_clips_by_id(&init_reply).ok().and_then(|c| c.into_iter().next()).map(|c| c.id))
            .ok_or_else(|| PartUploadError::Error(Error::api(t("Suno processed the upload but didn't make a song from it. Refresh the library to check."))))?;

        Ok(clip_id)
    }

    /// Assembles uploaded part clips in Suno Studio, exports the full song, and returns the rendered clip's ID.
    pub fn assemble_and_render_studio_song(
        &self,
        title: &str,
        _lyrics: &str,
        parts: &[(String, f64)],
        on_progress: &dyn Fn(&str),
    ) -> Result<String> {
        if parts.is_empty() {
            return Err(Error::api(t("Suno couldn't render the song in Studio.")));
        }
        let seed_clip_id = &parts[0].0;
        on_progress(&t("Arranging song in Suno Studio..."));

        // 1. Create or load the Studio project for the seed clip
        let proj_reply = self.send(
            Method::POST,
            &format!("/api/studio/create-or-load-project-for-clip/{}", crate::api::escape(seed_clip_id)),
            None,
        )?;
        let project_id = sj::try_read_string(&proj_reply, "id")
            .ok_or_else(|| Error::api(t("Suno didn't create a Studio project for this upload.")))?;

        // 2. Fetch project arrangement state
        let get_reply = self.send(
            Method::GET,
            &format!("/api/studio/project/{}", crate::api::escape(&project_id)),
            None,
        )?;
        let root: Value = serde_json::from_str(&get_reply)
            .map_err(|_| Error::api(t("Suno sent a reply this app doesn't understand.")))?;
        let mut state = root.get("state").and_then(Value::as_object)
            .ok_or_else(|| Error::api(t("Suno returned a Studio project without arrangement state.")))?
            .clone();

        let bps = state.get("timing").and_then(|t| t.get("bps")).and_then(Value::as_f64)
            .or_else(|| state.get("timing").and_then(|t| t.get("fallbackBPS")).and_then(Value::as_f64))
            .unwrap_or(2.0)
            .clamp(0.3, 5.0);

        // 3. Find or preserve the existing audio track
        let mut tracks = state.remove("tracks").and_then(|v| match v {
            Value::Array(arr) => Some(arr),
            _ => None,
        }).unwrap_or_default();

        let track_idx = tracks.iter().position(|t| {
            t.get("type").and_then(Value::as_str) == Some("audio") && t.get("clips").is_some()
        });

        let (mut audio_track, is_new_track) = match track_idx {
            Some(idx) => (tracks.remove(idx), false),
            None => {
                let tid = uuid::Uuid::new_v4().to_string();
                (json!({
                    "type": "audio",
                    "id": tid,
                    "input": null,
                    "color": "#02AF4A",
                    "name": "Audio Track",
                    "clips": [],
                    "clipCreationIntents": [],
                    "height": 86,
                    "solo": false,
                    "mute": false,
                    "arm": false,
                    "amplitude": 1.0,
                    "balance": 0.0,
                    "instrument": { "type": "song" },
                    "soloTakeLaneId": null,
                    "takeLanesExpanded": false,
                    "takeLanes": [],
                    "signalChain": [],
                    "routingMode": "linear"
                }), true)
            }
        };

        let track_id = audio_track.get("id").and_then(Value::as_str).unwrap_or("default-track").to_string();
        let track_color = audio_track.get("color").and_then(Value::as_str).unwrap_or("#02AF4A").to_string();

        // 4. Update timing
        let mut timing = state.remove("timing").and_then(|v| match v {
            Value::Object(map) => Some(map),
            _ => None,
        }).unwrap_or_default();

        if is_new_track {
            timing.insert("type".to_string(), json!("follow-track"));
            timing.insert("fallbackBPS".to_string(), json!(bps));
            timing.insert("trackId".to_string(), json!(track_id));
        } else if timing.get("type").and_then(Value::as_str) == Some("follow-track") {
            timing.insert("fallbackBPS".to_string(), json!(bps));
            timing.insert("trackId".to_string(), json!(track_id));
        } else {
            timing.insert("bps".to_string(), json!(bps));
            timing.insert("type".to_string(), json!("manual"));
            timing.insert("lockBPS".to_string(), json!(true));
            timing.entry("bpsAutomation".to_string()).or_insert_with(|| json!([]));
        }
        state.insert("timing".to_string(), Value::Object(timing));

        // 5. Place all parts seamlessly in order on the audio track
        let existing_clips = audio_track.get("clips").and_then(Value::as_array).cloned().unwrap_or_default();
        let mut existing_map: std::collections::HashMap<String, Value> = std::collections::HashMap::new();
        let mut other_clips = Vec::new();
        for clip in existing_clips {
            if let Some(asset_id) = clip.get("asset").and_then(|a| a.get("id")).and_then(Value::as_str) {
                existing_map.insert(asset_id.to_string(), clip);
            } else {
                other_clips.push(clip);
            }
        }

        let mut placed_clips = Vec::new();
        let mut v = 0.0;

        for (idx, (clip_id, duration_sec)) in parts.iter().enumerate() {
            let dur_beats = duration_sec * bps;
            let end_beats = v + dur_beats;
            let part_name = format!("Part {}", idx + 1);

            let existing = existing_map.get(clip_id);
            let clip_uuid = existing
                .and_then(|c| c.get("id").and_then(Value::as_str))
                .map(|s| s.to_string())
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

            let clip_obj = json!({
                "type": "audio",
                "id": clip_uuid,
                "asset": { "type": "clip", "id": clip_id },
                "name": part_name,
                "color": track_color,
                "startBeats": v,
                "endBeats": end_beats,
                "readStartBeats": 0.0,
                "loop": { "enabled": false, "startBeats": 0.0, "endBeats": dur_beats },
                "warp": { "enabled": false, "awaitingAnalysis": false, "markersHash": null },
                "streaming": false,
                "mute": false,
                "reversed": false,
                "fadeInBeats": 0.0,
                "fadeOutBeats": 0.0,
                "fadeInCurve": 1.0,
                "fadeOutCurve": 1.0,
                "transposition": 0.0,
                "formantCorrection": 0.0,
                "amplitude": 1.0
            });
            placed_clips.push(clip_obj);
            v = end_beats;
        }
        let total_beats = v;

        let mut all_clips = other_clips;
        all_clips.extend(placed_clips);
        if let Some(obj) = audio_track.as_object_mut() {
            obj.insert("clips".to_string(), Value::Array(all_clips));
        }

        tracks.push(audio_track);
        state.insert("tracks".to_string(), Value::Array(tracks));

        // 6. Save project arrangement
        let save_body = json!({
            "project_id": project_id,
            "state": state,
            "title": title
        });
        self.send(Method::POST, "/api/studio/save-project", Some(&save_body))?;

        // 7. Re-fetch project to get fresh arrangement state and latest_version_id
        thread::sleep(Duration::from_millis(1000));
        let fresh_reply = self.send(
            Method::GET,
            &format!("/api/studio/project/{}", crate::api::escape(&project_id)),
            None,
        )?;
        let fresh_root: Value = serde_json::from_str(&fresh_reply)
            .map_err(|_| Error::api(t("Suno sent a reply this app doesn't understand.")))?;
        let fresh_version_id = fresh_root.get("latest_version_id").and_then(Value::as_str).map(|s| s.to_string());
        let fresh_state = fresh_root.get("state").cloned().unwrap_or(Value::Object(state));

        // 8. Generate downbeats
        let mut downbeats = Vec::new();
        let max_beats = (total_beats.ceil() as usize).min(4 * 3600 * 5);
        for s in 0..max_beats {
            downbeats.push(json!([s as f64 / bps, (s % 4) + 1]));
        }

        // 9. Render full song
        on_progress(&t("Rendering full song in Suno Studio. This can take a minute or two."));
        let mut render_body = json!({
            "title": title,
            "lyrics": "",
            "tags": "",
            "negative_tags": "",
            "style_summary": "",
            "caption": "",
            "state": fresh_state,
            "start_beats": 0.0,
            "end_beats": total_beats,
            "project_id": "default",
            "studio_project_id": project_id,
            "web_client_pathname": "/studio",
            "downbeats": downbeats,
            "export_mode": "fullsong_export",
            "generate_midi_preview": false
        });
        if let Some(v_id) = fresh_version_id {
            render_body["studio_project_version_id"] = json!(v_id);
        }

        let render_reply = self.send(Method::POST, "/api/studio/render-state", Some(&render_body))?;
        let rendered_clip_id = sj::try_read_string(&render_reply, "id")
            .ok_or_else(|| Error::api(t("Suno couldn't render the song in Studio.")))?;

        Ok(rendered_clip_id)
    }

    /// Sets the final metadata (title, lyrics, terms acceptance) on a clip.
    pub fn set_metadata(&self, clip_id: &str, title: &str, lyrics: &str) -> Result<()> {
        let mut body = json!({ "title": title, "is_audio_upload_tos_accepted": true });
        if !lyrics.trim().is_empty() {
            body["lyrics"] = json!(lyrics);
        }
        self.send(Method::POST, &format!("/api/gen/{}/set_metadata/", crate::api::escape(clip_id)), Some(&body)).map(drop)
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

    #[test]
    fn recognizes_upload_rejection_keywords() {
        assert!(is_upload_rejected(Some("rejected"), None, None));
        assert!(is_upload_rejected(Some("error"), Some("moderation_blocked"), None));
        assert!(is_upload_rejected(Some("failed"), None, Some("Detected copyright infringement.")));
        assert!(is_upload_rejected(None, None, Some("duplicate audio upload")));
        assert!(is_upload_rejected(Some("error"), None, Some("Matches existing recording in catalog.")));
        assert!(!is_upload_rejected(Some("complete"), None, None));
        assert!(!is_upload_rejected(Some("error"), None, Some("network timeout")));
        assert!(!is_upload_rejected(Some("processing"), None, None));
    }
}
