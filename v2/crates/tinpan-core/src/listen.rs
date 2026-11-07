//! Listen for Credits: Suno pays credits for rating songs. Each task is two or more songs and one or more
//! questions ("Which follows the prompt better?"), each answered with a song or "neither", after listening
//! to every song for a while. Read from Suno's own Listen for Credits page (2026-11-07): GET
//! /api/preferences/summary and /tasks, PUT /profile, POST /submit (with a Turnstile token when Suno says
//! one is due), /skip-task/{id} and /tasks/release-buffer. Only the plain audio comparison is offered here.

use std::collections::BTreeMap;

use reqwest::Method;
use serde_json::{Value, json};

use crate::api::{SunoApi, escape};
use crate::error::{Error, Result};
use crate::i18n::t;
use crate::json as sj;

/// The kind of task asked for: rating ready-made songs.
const QUERY: &str = "project_category=audio&task_mode=ready_made";

fn text(v: &Value, name: &str) -> String {
    v.get(name).and_then(Value::as_str).unwrap_or("").trim().to_string()
}

fn int(v: &Value, name: &str) -> i64 {
    v.get(name).and_then(sj::as_int).unwrap_or(0)
}

/// Where you stand: tasks waiting, today's limit, and whether Suno wants the rater profile or a check first.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Summary {
    pub tasks_available: i64,
    pub credits_waiting: i64,
    pub credits_processed: i64,
    pub daily_limit_reached: bool,
    pub next_available_in_seconds: i64,
    pub profile_needed: bool,
    /// Seconds until Suno wants a Turnstile check with a rating; 0 or less means now.
    pub check_due_in_seconds: Option<f64>,
}

impl Summary {
    pub fn read(v: &Value) -> Self {
        Self {
            tasks_available: int(v, "tasks_available"),
            credits_waiting: int(v, "credits_waiting"),
            credits_processed: int(v, "credits_processed"),
            daily_limit_reached: v.get("daily_submit_limit_reached").and_then(Value::as_bool).unwrap_or(false),
            next_available_in_seconds: int(v, "next_available_in_seconds"),
            profile_needed: v.get("rater_profile_required").and_then(Value::as_bool).unwrap_or(false)
                && !v.get("rater_profile_completed").and_then(Value::as_bool).unwrap_or(false),
            check_due_in_seconds: v.get("turnstile_due_in_seconds").and_then(Value::as_f64),
        }
    }

    pub fn check_due(&self) -> bool {
        self.check_due_in_seconds.is_some_and(|s| s <= 0.0)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Song {
    pub key: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Question {
    pub key: String,
    pub name: String,
    pub description: String,
    pub allow_neither: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Task {
    pub id: String,
    pub question: String,
    pub instructions: String,
    pub songs: Vec<Song>,
    pub questions: Vec<Question>,
    pub min_listen_seconds: u32,
    pub credits: Option<f64>,
}

impl Task {
    /// A task Tin Pan can show: songs to compare and questions about them, nothing else on the page.
    pub fn read(v: &Value) -> Option<Self> {
        let id = text(v, "task_id");
        let project = v.get("project")?;
        let audio = project.get("media_type").and_then(Value::as_str).is_none_or(|m| m == "audio");
        let plain = project.get("layout").is_none_or(Value::is_null);
        if id.is_empty() || !audio || !plain {
            return None;
        }
        let songs: Vec<Song> = v.get("review_items")?.as_array()?.iter().filter_map(|i| Some(Song { key: text(i, "key"), url: i.get("url")?.as_str()?.to_string() })).filter(|s| !s.key.is_empty()).collect();
        let questions: Vec<Question> = project
            .get("preference_dimensions")?
            .as_array()?
            .iter()
            .map(|d| Question { key: text(d, "key"), name: text(d, "name"), description: text(d, "description"), allow_neither: d.get("allow_neither").and_then(Value::as_bool).unwrap_or(true) })
            .filter(|q| !q.key.is_empty())
            .collect();
        if songs.len() < 2 || questions.is_empty() {
            return None;
        }
        Some(Self {
            id,
            question: text(project, "question"),
            instructions: text(project, "instructions"),
            songs,
            questions,
            min_listen_seconds: project.get("min_review_time_seconds").and_then(sj::as_int).filter(|s| *s > 0).unwrap_or(30) as u32,
            credits: project.get("credits_per_annotation").and_then(Value::as_f64),
        })
    }
}

/// What Suno asks before the first rating: the same choices as its own form, by Suno's names.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Profile {
    pub creator_type: String,
    pub musical_background: String,
    pub genres: Vec<String>,
    pub other_genres: String,
    pub languages: Vec<String>,
    pub other_languages: String,
    pub listening_setup: String,
}

pub const CREATOR_TYPES: [(&str, &str); 5] = [
    ("casual_sharer", "I make songs to share with friends"),
    ("ai_hobbyist", "I make songs with AI as a hobby"),
    ("aspiring_artist", "An aspiring artist"),
    ("established_artist", "An established artist"),
    ("other_creator", "Another kind of creator"),
];
pub const BACKGROUNDS: [(&str, &str); 4] =
    [("listener", "A listener"), ("hobbyist", "A hobby musician"), ("trained", "Musically trained"), ("professional", "A professional musician")];
pub const GENRES: [(&str, &str); 23] = [
    ("pop", "Pop"),
    ("rock", "Rock"),
    ("alternative_indie", "Alternative and indie"),
    ("hip_hop", "Hip hop"),
    ("electronic", "Electronic"),
    ("dance_edm", "Dance and EDM"),
    ("rnb", "R&B"),
    ("soul_funk", "Soul and funk"),
    ("country", "Country"),
    ("folk", "Folk"),
    ("jazz", "Jazz"),
    ("blues", "Blues"),
    ("classical", "Classical"),
    ("musical_theater", "Musical theater"),
    ("reggaeton", "Reggaeton"),
    ("reggae_dancehall", "Reggae and dancehall"),
    ("afrobeats", "Afrobeats"),
    ("bollywood", "Bollywood"),
    ("indian_classical", "Indian classical"),
    ("k_pop", "K-pop"),
    ("j_pop", "J-pop"),
    ("metal", "Metal"),
    ("ambient", "Ambient"),
];
pub const LANGUAGES: [(&str, &str); 15] = [
    ("english", "English"),
    ("spanish", "Spanish"),
    ("mandarin_chinese", "Mandarin Chinese"),
    ("hindi", "Hindi"),
    ("arabic", "Arabic"),
    ("french", "French"),
    ("portuguese", "Portuguese"),
    ("bengali", "Bengali"),
    ("russian", "Russian"),
    ("japanese", "Japanese"),
    ("korean", "Korean"),
    ("german", "German"),
    ("indonesian_malay", "Indonesian and Malay"),
    ("italian", "Italian"),
    ("turkish", "Turkish"),
];
pub const SETUPS: [(&str, &str); 5] = [
    ("over_ear_headphones", "Over-ear headphones"),
    ("in_ear_earbuds", "Earbuds"),
    ("studio_monitors", "Studio monitors"),
    ("home_speakers", "Home speakers"),
    ("laptop_phone_speakers", "Laptop or phone speakers"),
];

impl Profile {
    pub fn read(v: &Value) -> Self {
        let list = |name: &str| v.get(name).and_then(Value::as_array).map(|l| l.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
        Self {
            creator_type: text(v, "creator_type"),
            musical_background: text(v, "musical_background"),
            genres: list("preferred_genres"),
            other_genres: text(v, "other_genres"),
            languages: list("language_fluencies"),
            other_languages: text(v, "other_languages"),
            listening_setup: text(v, "listening_setup"),
        }
    }

    /// Suno's form needs a choice in each of these, and at least one genre and one language (or typed ones).
    pub fn is_complete(&self) -> bool {
        !self.creator_type.is_empty()
            && !self.musical_background.is_empty()
            && !self.listening_setup.is_empty()
            && (!self.genres.is_empty() || !self.other_genres.trim().is_empty())
            && (!self.languages.is_empty() || !self.other_languages.trim().is_empty())
    }

    fn body(&self) -> Value {
        json!({
            "creator_type": self.creator_type, "musical_background": self.musical_background,
            "preferred_genres": self.genres, "other_genres": self.other_genres.trim(),
            "language_fluencies": self.languages, "other_languages": self.other_languages.trim(),
            "listening_setup": self.listening_setup,
        })
    }
}

/// What came of a rating.
#[derive(Debug, Clone, PartialEq)]
pub enum Sent {
    /// Taken; the next task when Suno handed one over (one Tin Pan can't show is left out).
    Taken { next: Option<Task> },
    /// Suno wants the Turnstile check passed first.
    CheckNeeded,
    /// The task went to someone else, or ran out.
    Gone,
}

fn time_zone() -> String {
    iana_time_zone::get_timezone().unwrap_or_else(|_| "UTC".to_string())
}

impl SunoApi {
    pub fn listen_summary(&self) -> Result<Summary> {
        Ok(Summary::read(&sj::parse(&self.send(Method::GET, &format!("/api/preferences/summary?{QUERY}"), None)?)?))
    }

    pub fn listen_profile(&self) -> Result<Option<Profile>> {
        let root = sj::parse(&self.send(Method::GET, "/api/preferences/profile", None)?)?;
        Ok(root.get("profile").filter(|p| p.is_object()).map(Profile::read))
    }

    pub fn save_listen_profile(&self, profile: &Profile) -> Result<()> {
        self.send(Method::PUT, "/api/preferences/profile", Some(&profile.body())).map(|_| ())
    }

    /// The next task, or None when there's none Tin Pan can show. One it can't show is handed back.
    pub fn listen_task(&self) -> Result<Option<Task>> {
        let root = sj::parse(&self.send(Method::GET, &format!("/api/preferences/tasks?{QUERY}"), None)?)?;
        if text(&root, "task_id").is_empty() {
            return Ok(None);
        }
        let task = Task::read(&root);
        if task.is_none() {
            log::info!("Listen for Credits: a task of a kind Tin Pan doesn't show; handed back");
            self.release_listen_tasks();
        }
        Ok(task)
    }

    /// A task's song file. Only Suno's own storage is asked.
    pub fn listen_song(&self, url: &str) -> Result<Vec<u8>> {
        let host = url.strip_prefix("https://").and_then(|r| r.split('/').next()).unwrap_or("");
        let suno = host.ends_with(".amazonaws.com") && host.starts_with("suno-") || host.ends_with(".suno.ai") || host.ends_with(".suno.com") || host == "cdn1.suno.ai";
        if !suno {
            return Err(Error::api(t("Suno gave a song address this app won't use.")));
        }
        self.fetch_file(url)
    }

    /// Sends the answers: question key to song key (or "neither"), and seconds listened per song.
    pub fn send_listen_rating(&self, task_id: &str, answers: &BTreeMap<String, String>, listened: &BTreeMap<String, u32>, check: Option<&str>) -> Result<Sent> {
        let mut body = json!({ "task_id": task_id, "preference": answers, "feedback": "", "listen_time_seconds": listened });
        if let Some(token) = check {
            body["turnstile_token"] = json!(token);
        }
        let path = format!("/api/preferences/submit?{QUERY}&tz={}", escape(&time_zone()));
        match self.send(Method::POST, &path, Some(&body)) {
            Ok(reply) => {
                let root = sj::parse(&reply)?;
                Ok(Sent::Taken { next: root.get("next_task").filter(|n| n.is_object()).and_then(Task::read) })
            }
            Err(Error::Api { status: Some(403), .. }) if check.is_none() => Ok(Sent::CheckNeeded),
            Err(Error::Api { status: Some(404), .. }) => Ok(Sent::Gone),
            Err(e) => Err(e),
        }
    }

    pub fn skip_listen_task(&self, task_id: &str, listened: &BTreeMap<String, u32>) -> Result<()> {
        match self.send(Method::POST, &format!("/api/preferences/skip-task/{}?{QUERY}", escape(task_id)), Some(&json!({ "listen_time_seconds": listened }))) {
            Ok(_) | Err(Error::Api { status: Some(404), .. }) => Ok(()),
            Err(e) => Err(e),
        }
    }

    /// Hands back tasks claimed and not answered, as Suno's page does when you leave it.
    pub fn release_listen_tasks(&self) {
        if let Err(e) = self.send(Method::POST, "/api/preferences/tasks/release-buffer", None) {
            log::warn!("Listen for Credits: releasing tasks failed: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task_json(layout: Value) -> Value {
        json!({ "task_id": "t1", "review_items": [ { "key": "a", "url": "https://suno-data-uploads.s3.amazonaws.com/x/a.mp3" }, { "key": "b", "url": "https://suno-data-uploads.s3.amazonaws.com/x/b.mp3" } ],
            "project": { "media_type": "audio", "layout": layout, "question": "Which fits the prompt?", "instructions": "Listen to both.", "min_review_time_seconds": 20, "credits_per_annotation": 1,
                "preference_dimensions": [ { "key": "fit", "name": "Prompt fit", "description": "How well it follows the prompt" }, { "key": "vocals", "name": "Vocals", "allow_neither": false } ] } })
    }

    #[test]
    fn a_plain_audio_task_reads() {
        let task = Task::read(&task_json(Value::Null)).unwrap();
        assert_eq!((task.id.as_str(), task.songs.len(), task.min_listen_seconds), ("t1", 2, 20));
        assert_eq!(task.questions[0].name, "Prompt fit");
        assert!(task.questions[0].allow_neither && !task.questions[1].allow_neither);
    }

    #[test]
    fn other_kinds_of_task_are_left_out() {
        assert!(Task::read(&task_json(json!({ "blocks": [] }))).is_none());
        let mut video = task_json(Value::Null);
        video["project"]["media_type"] = json!("video");
        assert!(Task::read(&video).is_none());
        let mut one_song = task_json(Value::Null);
        one_song["review_items"].as_array_mut().unwrap().pop();
        assert!(Task::read(&one_song).is_none());
    }

    #[test]
    fn the_summary_says_when_the_profile_or_a_check_is_needed() {
        let summary = Summary::read(&json!({ "tasks_available": 3, "rater_profile_required": true, "rater_profile_completed": false, "turnstile_due_in_seconds": 0 }));
        assert!(summary.profile_needed && summary.check_due());
        let done = Summary::read(&json!({ "rater_profile_required": true, "rater_profile_completed": true, "turnstile_due_in_seconds": 120 }));
        assert!(!done.profile_needed && !done.check_due());
    }

    #[test]
    fn a_profile_needs_every_answer() {
        let mut profile = Profile { creator_type: "ai_hobbyist".into(), musical_background: "listener".into(), listening_setup: "in_ear_earbuds".into(), ..Profile::default() };
        assert!(!profile.is_complete());
        profile.genres.push("pop".into());
        profile.other_languages = "Romanian".into();
        assert!(profile.is_complete());
        assert_eq!(profile.body()["language_fluencies"], json!([]));
    }
}
