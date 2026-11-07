//! A voice persona from your own singing, as Suno's Create, Voice page makes one (read from that page,
//! 2026-11-07): your singing is uploaded as a voice recording and its vocals taken out
//! (POST /api/processed_clip/voice-vox-stem); then Suno checks it's you, from a short recording of a phrase
//! it gives you (GET /api/voice-verification/phrase/, POST /api/voice-verification/, polled until it's
//! decided); then POST /api/persona/create/ makes the persona.

use std::path::Path;
use std::thread;
use std::time::Duration;

use reqwest::Method;
use reqwest::blocking::Client;
use serde_json::{Value, json};

use crate::api::{SunoApi, escape};
use crate::error::{Error, Result};
use crate::i18n::t;
use crate::json as sj;

/// Your singing, ready to be checked and made into a persona.
#[derive(Debug, Clone)]
pub struct Singing {
    pub processed_id: String,
    pub recording_id: String,
    pub seconds: f64,
}

/// The phrase to record, in the language asked for.
#[derive(Debug, Clone)]
pub struct Phrase {
    pub id: String,
    pub text: String,
}

/// How Suno's check of the phrase recording came out.
#[derive(Debug, Clone, PartialEq)]
pub enum Check {
    Approved(String),
    /// It didn't hear the phrase.
    PhraseMissing,
    /// The voice didn't match the singing.
    VoiceDiffers,
    /// Still undecided after a minute.
    TakingLong,
}

pub fn read_check(v: &Value) -> Option<Check> {
    match v.get("status").and_then(Value::as_str)? {
        "pending" => None,
        "approved" => Some(Check::Approved(v.get("id").and_then(Value::as_str).unwrap_or("").to_string())),
        _ if v.get("rejection_reason").and_then(Value::as_str) == Some("didnt_say_verification_phrase") => Some(Check::PhraseMissing),
        _ => Some(Check::VoiceDiffers),
    }
}

impl SunoApi {
    /// Uploads your singing (`seconds` long) and has Suno take its vocals out.
    pub fn upload_singing(&self, storage: &Client, path: &Path, seconds: f64) -> Result<Singing> {
        let upload_id = self.upload_recording(storage, path, "voice_recording")?;
        let body = json!({ "upload_id": upload_id, "vocal_start_s": 0, "vocal_end_s": (seconds * 100.0).round() / 100.0 });
        let reply = sj::parse(&self.send(Method::POST, "/api/processed_clip/voice-vox-stem", Some(&body))?)?;
        let read = |name: &str| reply.get(name).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string);
        match (read("id"), read("voice_recording_id")) {
            (Some(processed_id), Some(recording_id)) => Ok(Singing { processed_id, recording_id, seconds }),
            _ => Err(Error::api(t("Suno couldn't find a voice in that recording."))),
        }
    }

    /// The phrase to say for Suno's check, in `language` ("en", "es" and so on).
    pub fn voice_phrase(&self, language: &str) -> Result<Phrase> {
        let reply = sj::parse(&self.send(Method::GET, &format!("/api/voice-verification/phrase/?language={}", escape(language)), None)?)?;
        let read = |name: &str| reply.get(name).and_then(Value::as_str).unwrap_or("").to_string();
        let (id, text) = (read("phrase_id"), read("phrase_text"));
        if id.is_empty() || text.is_empty() {
            return Err(Error::api(t("Suno gave no phrase to record. Try again later.")));
        }
        Ok(Phrase { id, text })
    }

    /// Uploads the phrase recording and waits for Suno to compare it with the singing.
    pub fn check_voice(&self, storage: &Client, singing: &Singing, phrase: &Phrase, path: &Path) -> Result<Check> {
        let upload_id = self.upload_recording(storage, path, "voice_recording")?;
        let reply = sj::parse(&self.send(Method::POST, "/api/processed_clip/voice-vox-stem", Some(&json!({ "upload_id": upload_id, "recording_type": "verification" })))?)?;
        let recording = reply.get("voice_recording_id").and_then(Value::as_str).filter(|s| !s.is_empty()).ok_or_else(|| Error::api(t("Suno couldn't hear the phrase in that recording. It needs to be a few seconds long.")))?;
        let body = json!({ "voice_recording_id": singing.recording_id, "verification_recording_id": recording, "phrase_id": phrase.id });
        let started = sj::parse(&self.send(Method::POST, "/api/voice-verification/", Some(&body))?)?;
        if let Some(done) = read_check(&started) {
            return Ok(done);
        }
        let id = started.get("id").and_then(Value::as_str).unwrap_or("").to_string();
        for _ in 0..40 {
            thread::sleep(Duration::from_millis(1500));
            if let Some(done) = read_check(&sj::parse(&self.send(Method::GET, &format!("/api/voice-verification/{}", escape(&id)), None)?)?) {
                return Ok(done);
            }
        }
        Ok(Check::TakingLong)
    }

    /// Waits for the singing's vocals, then makes the persona; its id.
    pub fn create_voice(&self, singing: &Singing, verification_id: &str, name: &str, description: &str, styles: &str) -> Result<String> {
        for _ in 0..120 {
            let reply = sj::parse(&self.send(Method::GET, &format!("/api/processed_clip/{}", escape(&singing.processed_id)), None)?)?;
            match reply.get("status").and_then(Value::as_str) {
                Some("complete" | "completed") => break,
                Some("failed" | "error") => return Err(Error::api(t("Suno couldn't take the vocals out of your singing."))),
                _ => thread::sleep(Duration::from_secs(1)),
            }
        }
        let mut body = json!({
            "is_voice_recording": true, "voice_recording_id": singing.recording_id, "name": name.trim(), "description": description.trim(),
            "is_public": false, "persona_type": "vox", "source": "random_song", "verification_id": verification_id,
            "vox_audio_id": singing.processed_id, "vocal_start_s": 0, "vocal_end_s": (singing.seconds * 100.0).round() / 100.0,
        });
        if !styles.trim().is_empty() {
            body["user_input_styles"] = json!(styles.trim());
        }
        let reply = sj::parse(&self.send(Method::POST, "/api/persona/create/", Some(&body))?)?;
        Ok(reply.get("id").and_then(Value::as_str).unwrap_or("").to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_read_as_suno_decides_them() {
        assert_eq!(read_check(&json!({ "status": "pending" })), None);
        assert_eq!(read_check(&json!({ "id": "v1", "status": "approved" })), Some(Check::Approved("v1".into())));
        assert_eq!(read_check(&json!({ "status": "rejected", "rejection_reason": "didnt_say_verification_phrase" })), Some(Check::PhraseMissing));
        assert_eq!(read_check(&json!({ "status": "rejected", "rejection_reason": "vocal_range_mismatch" })), Some(Check::VoiceDiffers));
    }
}
