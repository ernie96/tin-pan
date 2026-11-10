//! Listen for Credits: Suno pays credits for rating songs. Each task is two or more songs and one or more
//! questions ("Which follows the prompt better?"), each answered with a song or "neither", after listening
//! to every song for a while. A task's questions come in Suno's older shape or its newer layout of blocks. Read from Suno's own Listen for Credits page (2026-11-07): GET
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

/// One choice of a question: the value sent to Suno and the words said for it.
#[derive(Debug, Clone, PartialEq)]
pub struct Choice {
    pub value: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Answer {
    /// One choice; "neither" is one of them when the question allows it.
    One(Vec<Choice>),
    /// Any number of choices.
    Many(Vec<Choice>),
    /// Words typed.
    Text,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Question {
    pub key: String,
    pub label: String,
    pub description: String,
    pub answer: Answer,
    pub required: bool,
    /// An "Other" choice that takes typed words, sent under `{key}_other`.
    pub allow_other: bool,
    /// Asked only when the answer to another question (by its key) is one of these values.
    pub shown_when: Option<(String, Vec<String>)>,
}

/// The value Suno takes for "Other" in a question that allows it.
pub const OTHER: &str = "__other__";

impl Question {
    /// Whether it's asked, given the answers so far (question key to the values chosen).
    pub fn is_shown(&self, answers: &BTreeMap<String, Vec<String>>) -> bool {
        match &self.shown_when {
            Some((key, values)) => answers.get(key).is_some_and(|chosen| chosen.iter().any(|c| values.contains(c))),
            None => true,
        }
    }
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

/// The kinds of block Suno's page draws; anything else in a layout is left out, as it leaves it out.
const BLOCKS: [&str; 12] = ["markdown", "audio", "image", "video", "badge", "text", "lyrics", "copyright", "columns", "question", "grid", "generative_compare"];

/// A layout's blocks in reading order, columns opened up.
fn flat_blocks(blocks: &[Value], out: &mut Vec<Value>) {
    for block in blocks.iter().filter(|b| b.get("type").and_then(Value::as_str).is_some_and(|t| BLOCKS.contains(&t))) {
        if block.get("type").and_then(Value::as_str) == Some("columns") {
            for column in block.get("columns").and_then(Value::as_array).into_iter().flatten() {
                flat_blocks(column.as_array().map(Vec::as_slice).unwrap_or_default(), out);
            }
        } else {
            out.push(block.clone());
        }
    }
}

/// `{{a.name}}` and the like in a label, filled from the songs as Suno's page fills them.
fn fill(text: &str, songs: &[Song], names: &BTreeMap<String, String>) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(open) = rest.find("{{") {
        let Some(close) = rest[open..].find("}}") else { break };
        out.push_str(&rest[..open]);
        let field = rest[open + 2..open + close].trim();
        let (item, part) = field.split_once('.').unwrap_or((field, "name"));
        if let Some(i) = songs.iter().position(|s| s.key == item) {
            out.push_str(&match part {
                "name" => names.get(item).cloned().unwrap_or_else(|| letter(i)),
                "key" => item.to_string(),
                _ => String::new(),
            });
        }
        rest = &rest[open + close + 2..];
    }
    out.push_str(rest);
    out
}

pub fn letter(i: usize) -> String {
    char::from(b'A' + i.min(25) as u8).to_string()
}

/// Reads a question from a layout block, or from a preference dimension (`input` is then its `type`, and
/// a dimension without one picks a song). None for a kind Tin Pan can't ask (an upload, a grid).
fn read_question(q: &Value, input_field: &str, songs: &[Song], names: &BTreeMap<String, String>) -> Option<std::result::Result<Question, String>> {
    let key = text(q, "key");
    if key.is_empty() || q.get("grid").is_some_and(|g| !g.is_null() && g != &Value::Bool(false)) {
        return None;
    }
    let label = fill(q.get("label").or_else(|| q.get("name")).and_then(Value::as_str).unwrap_or(&key).trim(), songs, names);
    let input = q.get(input_field).and_then(Value::as_str).unwrap_or(if input_field == "type" { "item_select" } else { "single_select" });
    let options: Vec<Choice> = q
        .get("options")
        .and_then(Value::as_array)
        .map(|l| {
            l.iter()
                .filter_map(|o| match o {
                    Value::String(s) => Some(Choice { value: s.clone(), label: s.clone() }),
                    _ => {
                        let value = o.get("value").and_then(|v| v.as_str().map(str::to_string).or_else(|| v.as_i64().map(|n| n.to_string())))?;
                        Some(Choice { label: o.get("label").and_then(Value::as_str).map_or_else(|| value.clone(), str::to_string), value })
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    let allow_neither = q.get("allow_neither").and_then(Value::as_bool) != Some(false);
    let songs_to_pick = || {
        let mut choices: Vec<Choice> = songs.iter().enumerate().map(|(i, s)| Choice { value: s.key.clone(), label: crate::i18n::tf("Song {0}", &[&letter(i)]) }).collect();
        if allow_neither {
            choices.push(Choice { value: "neither".into(), label: crate::i18n::t("Neither") });
        }
        choices
    };
    let answer = match input {
        "multi_select" => Answer::Many(options),
        "free_text" => Answer::Text,
        "single_select" | "item_select" if options.is_empty() => Answer::One(songs_to_pick()),
        "single_select" | "item_select" => Answer::One(options),
        other => return Some(Err(other.to_string())),
    };
    let shown_when = q.get("visible_when").and_then(|w| {
        let key = w.get("key")?.as_str()?.to_string();
        let values = w.get("in")?.as_array()?.iter().map(|v| v.as_str().map_or_else(|| v.to_string(), str::to_string)).collect();
        Some((key, values))
    });
    let required = if input_field == "type" && q.get("type").is_none() { true } else { q.get("required").and_then(Value::as_bool).unwrap_or(false) };
    Some(Ok(Question { key, label, description: text(q, "description"), answer, required, allow_other: q.get("allow_other").and_then(Value::as_bool).unwrap_or(false), shown_when }))
}

impl Task {
    /// A task Tin Pan can show: songs to listen to and questions to answer, in Suno's older shape (its
    /// questions in `preference_dimensions`) or its newer one (a `layout` of blocks). Err names what it
    /// can't show, for the log.
    pub fn read(v: &Value) -> std::result::Result<Self, String> {
        let id = text(v, "task_id");
        let project = v.get("project").ok_or("no project")?;
        if let Some(media) = project.get("media_type").and_then(Value::as_str).filter(|m| *m != "audio") {
            return Err(format!("media type {media}"));
        }
        let items = v.get("review_items").and_then(Value::as_array).cloned().unwrap_or_default();
        let songs: Vec<Song> = items.iter().filter_map(|i| Some(Song { key: text(i, "key"), url: i.get("url")?.as_str()?.to_string() })).filter(|s| !s.key.is_empty()).collect();
        let names: BTreeMap<String, String> = items.iter().filter_map(|i| Some((text(i, "key"), i.get("name")?.as_str()?.to_string()))).collect();
        let mut blocks = Vec::new();
        flat_blocks(project.get("layout").and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default(), &mut blocks);
        let kind = |b: &Value| b.get("type").and_then(Value::as_str).unwrap_or("").to_string();
        if let Some(b) = blocks.iter().find(|b| matches!(kind(b).as_str(), "generative_compare" | "grid")) {
            return Err(format!("{} block", kind(b)));
        }
        let mut questions = Vec::new();
        let from_layout: Vec<&Value> = blocks.iter().filter(|b| kind(b) == "question").collect();
        let source: Vec<(Value, &str)> = if from_layout.is_empty() {
            project.get("preference_dimensions").and_then(Value::as_array).into_iter().flatten().map(|d| (d.clone(), "type")).collect()
        } else {
            from_layout.into_iter().map(|b| (b.clone(), "input")).collect()
        };
        for (q, field) in &source {
            match read_question(q, field, &songs, &names) {
                Some(Ok(question)) if !questions.iter().any(|x: &Question| x.key == question.key) => questions.push(question),
                Some(Err(kind)) => return Err(format!("question of kind {kind}")),
                _ => {}
            }
        }
        // What the layout's text blocks say goes with the instructions.
        let notes: Vec<String> = blocks
            .iter()
            .filter(|b| matches!(kind(b).as_str(), "markdown" | "text"))
            .filter_map(|b| ["content", "text", "markdown", "body", "value"].iter().find_map(|f| b.get(*f).and_then(Value::as_str)))
            .map(|s| fill(s.trim(), &songs, &names))
            .filter(|s| !s.is_empty())
            .collect();
        if id.is_empty() || songs.is_empty() || questions.is_empty() {
            return Err(format!("{} songs, {} questions", songs.len(), questions.len()));
        }
        let mut instructions = text(project, "instructions");
        for note in notes {
            if !instructions.is_empty() {
                instructions.push_str("\n\n");
            }
            instructions.push_str(&note);
        }
        Ok(Self {
            id,
            question: text(project, "question"),
            instructions,
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

/// A task's shape for the log: its project's keys and the kinds of block in its layout, never its texts.
fn shape(root: &Value) -> String {
    let project = root.get("project");
    let keys: Vec<&str> = project.and_then(Value::as_object).map(|o| o.keys().map(String::as_str).collect()).unwrap_or_default();
    let mut blocks = Vec::new();
    flat_blocks(project.and_then(|p| p.get("layout")).and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default(), &mut blocks);
    let kinds: Vec<String> = blocks.iter().map(|b| format!("{}{}", b.get("type").and_then(Value::as_str).unwrap_or("?"), b.get("input").and_then(Value::as_str).map(|i| format!(":{i}")).unwrap_or_default())).collect();
    format!("project keys {keys:?}; blocks {kinds:?}")
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
        match Task::read(&root) {
            Ok(task) => Ok(Some(task)),
            Err(why) => {
                log::info!("Listen for Credits: a task Tin Pan can't show ({why}); handed back. {}", shape(&root));
                self.release_listen_tasks();
                Ok(None)
            }
        }
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

    /// Sends the answers (question key to the value, or values, chosen or typed) and seconds listened per song.
    pub fn send_listen_rating(&self, task_id: &str, answers: &BTreeMap<String, Value>, listened: &BTreeMap<String, u32>, check: Option<&str>) -> Result<Sent> {
        let mut body = json!({ "task_id": task_id, "preference": answers, "feedback": "", "listen_time_seconds": listened });
        if let Some(token) = check {
            body["turnstile_token"] = json!(token);
        }
        let path = format!("/api/preferences/submit?{QUERY}&tz={}", escape(&time_zone()));
        match self.send(Method::POST, &path, Some(&body)) {
            Ok(reply) => {
                let root = sj::parse(&reply)?;
                Ok(Sent::Taken { next: root.get("next_task").filter(|n| n.is_object()).and_then(|n| Task::read(n).ok()) })
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
    fn a_task_in_the_older_shape_reads() {
        let task = Task::read(&task_json(Value::Null)).unwrap();
        assert_eq!((task.id.as_str(), task.songs.len(), task.min_listen_seconds), ("t1", 2, 20));
        assert_eq!(task.questions[0].label, "Prompt fit");
        assert!(task.questions[0].required);
        let Answer::One(choices) = &task.questions[0].answer else { panic!() };
        assert_eq!(choices.iter().map(|c| c.value.as_str()).collect::<Vec<_>>(), vec!["a", "b", "neither"]);
        let Answer::One(choices) = &task.questions[1].answer else { panic!() };
        assert_eq!(choices.len(), 2);
    }

    #[test]
    fn a_task_in_the_layout_shape_reads() {
        let layout = json!([
            { "type": "markdown", "content": "Compare {{a.name}} with {{b.name}}." },
            { "type": "columns", "columns": [[{ "type": "audio", "item": "a" }], [{ "type": "audio", "item": "b" }]] },
            { "type": "question", "key": "better", "label": "Which is better?", "required": true },
            { "type": "question", "key": "why", "label": "Why?", "input": "multi_select", "options": [{ "value": "mix", "label": "The mix" }, "Vocals"], "allow_other": true,
              "visible_when": { "key": "better", "in": ["a", "b"] } },
            { "type": "question", "key": "notes", "label": "Anything else?", "input": "free_text" },
            { "type": "badge", "text": "ignored" },
            { "type": "something new" }
        ]);
        let mut json = task_json(layout);
        json["review_items"][0]["name"] = json!("First");
        let task = Task::read(&json).unwrap();
        assert!(task.instructions.ends_with("Compare First with B."));
        assert_eq!(task.questions.iter().map(|q| q.key.as_str()).collect::<Vec<_>>(), vec!["better", "why", "notes"]);
        let Answer::Many(why) = &task.questions[1].answer else { panic!() };
        assert_eq!((why[0].label.as_str(), why[1].value.as_str()), ("The mix", "Vocals"));
        assert!(task.questions[1].allow_other && !task.questions[1].required);
        let mut answers = BTreeMap::new();
        assert!(!task.questions[1].is_shown(&answers));
        answers.insert("better".to_string(), vec!["b".to_string()]);
        assert!(task.questions[1].is_shown(&answers));
        assert_eq!(task.questions[2].answer, Answer::Text);
    }

    #[test]
    fn tasks_tin_pan_cant_show_say_why() {
        assert_eq!(Task::read(&task_json(json!([{ "type": "generative_compare" }]))).unwrap_err(), "generative_compare block");
        assert_eq!(Task::read(&task_json(json!([{ "type": "question", "key": "f", "input": "upload" }]))).unwrap_err(), "question of kind upload");
        let mut video = task_json(Value::Null);
        video["project"]["media_type"] = json!("video");
        assert!(Task::read(&video).is_err());
        assert!(shape(&task_json(json!([{ "type": "question", "key": "x", "input": "free_text" }]))).contains("question:free_text"));
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
