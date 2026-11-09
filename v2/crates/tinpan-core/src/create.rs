//! Making songs: the models the plan allows, the body POST /api/generate/v2-web/ takes, personas, and the
//! length checks Suno's web form makes, all as Tin Pan 1 does them.

use serde::Deserialize;
use serde_json::{Value, json};

use crate::error::{Error, Result};
use crate::i18n::{t, tf};

/// Limits in UTF-16 units, as the Suno web form counts them. 0 means no limit given.
#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct MaxLengths {
    pub title: usize,
    pub prompt: usize,
    pub tags: usize,
    pub negative_tags: usize,
    #[serde(rename = "gpt_description_prompt")]
    pub description: usize,
}

impl MaxLengths {
    pub const FALLBACK: Self = Self { title: 100, prompt: 5000, tags: 1000, negative_tags: 1000, description: 3000 };
}

/// A model from GET /api/billing/info/.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct SunoModel {
    /// Sent as "mv", for example chirp-fenix.
    pub external_key: String,
    pub name: String,
    #[serde(rename = "is_default_model")]
    pub is_default: bool,
    #[serde(default = "yes")]
    pub can_use: bool,
    pub max_lengths: Option<MaxLengths>,
    pub capabilities: Option<Vec<String>>,
    pub features: Option<Vec<String>>,
    pub badges: Option<Vec<String>>,
    /// A custom model's own id (rename and archive use it); Suno's models have none. A number or text.
    #[serde(deserialize_with = "opt_id")]
    pub id: Option<String>,
    /// Which sources can be combined, for example [["cover"], ["persona", "cover"]].
    pub allowed_condition_combinations: Option<Vec<Vec<String>>>,
    /// Someone else's custom model shared with you (GET /api/custom-model/shared), not one from your plan.
    #[serde(skip)]
    pub shared: bool,
}

fn opt_id<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Option<String>, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::String(s) if !s.is_empty() => Some(s),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    })
}

fn yes() -> bool {
    true
}

impl SunoModel {
    pub fn display_name(&self) -> String {
        let name = if self.name.trim().is_empty() { self.external_key.clone() } else { self.name.trim().to_string() };
        if self.shared { tf("{0}, shared with you", &[&name]) } else { name }
    }

    /// Whether it can make songs with a voice persona: "all" or "vox" among its capabilities. None when Suno lists none.
    pub fn takes_voice_personas(&self) -> Option<bool> {
        self.capabilities.as_ref().filter(|c| !c.is_empty()).map(|c| c.iter().any(|x| x == "all" || x == "vox"))
    }

    pub fn limits(&self) -> MaxLengths {
        self.max_lengths.unwrap_or(MaxLengths::FALLBACK)
    }

    /// Whether the model does a task ("cover", "extend") with exactly these sources, as Suno's web helpers check
    /// it. A model that lists nothing is given the benefit of the doubt, with Suno's older fixed table.
    pub fn supports(&self, task: &str, conditions: &[&str]) -> bool {
        if let Some(c) = self.capabilities.as_ref().filter(|c| !c.is_empty())
            && !c.iter().any(|x| x == "all" || x == task)
        {
            return false;
        }
        if let Some(allowed) = self.allowed_condition_combinations.as_ref().filter(|a| !a.is_empty()) {
            return allowed.iter().any(|a| a.len() == conditions.len() && conditions.iter().all(|c| a.iter().any(|x| x == c)));
        }
        legacy_supports(&self.external_key, conditions)
    }

    /// One of your own custom models: a chirp-custom key, an id, and a custom or training badge.
    pub fn is_custom(&self) -> bool {
        self.external_key.starts_with("chirp-custom")
            && self.id.as_deref().is_some_and(|i| !i.is_empty())
            && self.badges.as_ref().is_some_and(|b| b.iter().any(|x| x == "custom" || x == "training"))
    }

    /// Whether the form may send weirdness and influence numbers; None when Suno lists no features.
    pub fn takes_sliders(&self) -> Option<bool> {
        if self.badges.as_ref().is_some_and(|b| b.iter().any(|x| x == "custom")) {
            return Some(true);
        }
        self.features.as_ref().filter(|f| !f.is_empty()).map(|f| f.iter().any(|x| x == "create_control_sliders"))
    }

    /// The key a cover is sent with: Suno's web client covers with v3.5 and v4 through their "tau" models.
    pub fn cover_key(&self) -> String {
        match self.external_key.as_str() {
            "chirp-v3-0" | "chirp-v3-5" => "chirp-v3-5-tau".into(),
            "chirp-v4" => "chirp-v4-tau".into(),
            key => key.into(),
        }
    }
}

/// Suno's older fixed table, for replies without allowed_condition_combinations.
fn legacy_supports(key: &str, conditions: &[&str]) -> bool {
    const V35: [&str; 12] = ["v3-5", "v4", "auk", "bluejay", "crow", "dodo", "eagle", "fenix", "goose", "hawk", "ibis", "chirp-custom"];
    let is = |models: &[&str]| models.iter().any(|m| key.contains(m));
    let has = |needed: &[&str]| needed.iter().all(|n| conditions.contains(n));
    if has(&["persona", "cover"]) && !is(&V35[2..]) {
        return false;
    }
    if (has(&["cover"]) || has(&["persona"])) && !is(&V35) {
        return false;
    }
    if (has(&["playlist"]) || has(&["underpaint"]) || has(&["overpaint"])) && !is(&V35[3..]) {
        return false;
    }
    true
}

/// The models a task can use, and the one to start on; empty when none can.
pub fn models_for(usable: &[SunoModel], task: &str, conditions: &[&str]) -> Vec<SunoModel> {
    usable.iter().filter(|m| m.supports(task, conditions)).cloned().collect()
}

/// Older remaster models; Suno may have retired some, and says so if one is picked.
const OLDER_REMASTER_MODELS: [(&str, &str); 3] = [("chirp-flounder", "v5.5 (older)"), ("chirp-carp", "v5 (older)"), ("chirp-bass", "v4.5+ (older)")];

/// The plan's remaster models, then the older ones it doesn't mention. can_use is false on these even for
/// subscribers; being listed is what counts. Empty when the plan lists none.
pub fn remaster_models(listed: &[SunoModel]) -> Vec<SunoModel> {
    let mut models: Vec<SunoModel> = listed.iter().filter(|m| !m.external_key.is_empty()).cloned().collect();
    if models.is_empty() {
        return models;
    }
    for (key, name) in OLDER_REMASTER_MODELS {
        if !models.iter().any(|m| m.external_key == key) {
            models.push(SunoModel { external_key: key.into(), name: name.into(), can_use: true, ..SunoModel::default() });
        }
    }
    models
}

/// Remaster's body, as Suno's web Remaster builds it: defaults are left out, the numbers are 0 to 100 here.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RemasterRequest {
    pub clip_id: String,
    pub model: String,
    pub style: String,
    /// subtle, normal or high.
    pub strength: String,
    /// natural, boost or clarity.
    pub profile: String,
    pub freedom: Option<f64>,
    pub tone: Option<f64>,
    pub clarity: Option<f64>,
    pub fine_strength: Option<f64>,
    pub stereo_width: Option<f64>,
}

impl RemasterRequest {
    pub fn body(&self) -> Value {
        let mut body = json!({ "clip_id": self.clip_id });
        if !self.model.is_empty() {
            body["model_name"] = json!(self.model);
        }
        if !self.style.trim().is_empty() {
            body["tags"] = json!(self.style.trim());
        }
        if !self.strength.is_empty() {
            body["variation_category"] = json!(self.strength);
        }
        if !self.profile.is_empty() {
            body["style_profile"] = json!(self.profile);
        }
        for (name, value, fallback) in [("freedom", self.freedom, 0.0), ("tone", self.tone, 0.5), ("clarity", self.clarity, 0.5), ("strength", self.fine_strength, 0.5), ("stereo_width", self.stereo_width, 0.5)] {
            if let Some(v) = value {
                let v = (v / 100.0).clamp(0.0, 1.0);
                if (v - fallback).abs() > 1e-9 {
                    body[name] = json!(v);
                }
            }
        }
        body
    }
}

/// The models a billing reply lists, skipping any Suno sends in a shape not understood.
pub fn read_models(value: &Value) -> Vec<SunoModel> {
    value.as_array().into_iter().flatten().filter_map(|m| serde_json::from_value::<SunoModel>(m.clone()).ok()).collect()
}

/// GET /api/custom-model/shared: the models shared with you, and what Suno's current generation of models
/// can do (`generation_config`: its capabilities and which sources may be combined).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SharedModels {
    pub models: Vec<SunoModel>,
    pub capabilities: Vec<String>,
    pub combinations: Vec<Vec<String>>,
}

pub fn read_shared_models(root: &Value) -> SharedModels {
    let config = root.get("generation_config");
    let strings = |v: &Value| v.as_array().map(|l| l.iter().filter_map(Value::as_str).map(str::to_string).collect::<Vec<_>>()).unwrap_or_default();
    SharedModels {
        models: root.get("items").map(read_models).unwrap_or_default(),
        capabilities: config.and_then(|c| c.get("capabilities")).map(strings).unwrap_or_default(),
        combinations: config.and_then(|c| c.get("allowed_condition_combinations")).and_then(Value::as_array).map(|l| l.iter().map(strings).filter(|c| !c.is_empty()).collect()).unwrap_or_default(),
    }
}

/// Adds the shared models to the plan's: one the plan already lists only fills in what the plan left out;
/// a custom model (one with its own id) it doesn't is added, marked as shared; Suno's own models the plan
/// leaves out stay out, since the plan may not use them. A shared model that says nothing of what it can do takes the
/// current generation's capabilities and combinations.
pub fn add_shared_models(models: &mut Vec<SunoModel>, shared: SharedModels) {
    for mut model in shared.models.into_iter().filter(|m| !m.external_key.is_empty()) {
        match models.iter_mut().find(|m| m.external_key == model.external_key) {
            Some(known) => {
                known.capabilities = known.capabilities.take().or(model.capabilities);
                known.allowed_condition_combinations = known.allowed_condition_combinations.take().or(model.allowed_condition_combinations);
                known.max_lengths = known.max_lengths.or(model.max_lengths);
            }
            None if model.id.is_some() => {
                if model.capabilities.as_ref().is_none_or(Vec::is_empty) && !shared.capabilities.is_empty() {
                    model.capabilities = Some(shared.capabilities.clone());
                }
                if model.allowed_condition_combinations.as_ref().is_none_or(Vec::is_empty) && !shared.combinations.is_empty() {
                    model.allowed_condition_combinations = Some(shared.combinations.clone());
                }
                model.shared = true;
                models.push(model);
            }
            None => {}
        }
    }
}

/// The models this plan may use, and the one to start on: the last one used, else Suno's default.
pub fn usable(models: &[SunoModel]) -> Vec<SunoModel> {
    models.iter().filter(|m| m.can_use && !m.external_key.is_empty()).cloned().collect()
}

pub fn starting_model(usable: &[SunoModel], last: &str) -> usize {
    usable.iter().position(|m| m.external_key == last).or_else(|| usable.iter().position(|m| m.is_default)).unwrap_or(0)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Persona {
    pub id: String,
    pub name: String,
}

/// GET /api/persona/get-personas/: {personas: [{id, name}]}.
pub fn read_personas(root: &Value) -> Vec<Persona> {
    root.get("personas")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|p| {
            let id = match p.get("id")? {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            let name = p.get("name").and_then(Value::as_str).unwrap_or("").trim().to_string();
            (!id.is_empty()).then(|| Persona { id, name: if name.is_empty() { t("Unnamed persona") } else { name } })
        })
        .collect()
}

/// What a persona needs from get-persona: its source song and whether it's a voice persona.
#[derive(Debug, Clone, PartialEq)]
pub struct PersonaInfo {
    pub root_clip_id: Option<String>,
    pub is_voice: bool,
    pub is_trashed: bool,
    pub is_hidden: bool,
    pub root_duration: Option<f64>,
}

const EMPTY_CLIP_ID: &str = "00000000-0000-0000-0000-000000000000";

impl PersonaInfo {
    /// An empty or all-zero root is no source song; "vox" in any case, or is_vox_persona, is a voice persona; the
    /// root's length comes from the persona's own clip when it is the root.
    pub fn read(root: &Value) -> Self {
        let text = |name: &str| root.get(name).and_then(Value::as_str).map(str::trim).unwrap_or("").to_string();
        let flag = |name: &str| root.get(name).and_then(Value::as_bool) == Some(true);
        let clip_root = Some(text("root_clip_id")).filter(|r| !r.is_empty() && r != EMPTY_CLIP_ID);
        let duration = clip_root.as_ref().and_then(|r| {
            let clip = root.get("clip")?;
            (clip.get("id")?.as_str()? == r).then(|| clip.get("metadata")?.get("duration")?.as_f64()).flatten().filter(|d| d.is_finite() && *d >= 0.0)
        });
        Self {
            root_clip_id: clip_root,
            is_voice: text("persona_type").eq_ignore_ascii_case("vox") || flag("is_vox_persona"),
            is_trashed: flag("is_trashed"),
            is_hidden: flag("is_hidden"),
            root_duration: duration,
        }
    }
}

/// How the song is described: by a description Suno writes from, or by your own lyrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Inspiration,
    Custom,
}

/// What the Create window sends. `token` and `token_provider` come from the captcha when Suno asks for one.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GenerateRequest {
    pub model: String,
    pub inspiration: bool,
    /// Lyrics in custom mode, the description in inspiration mode.
    pub prompt: String,
    pub title: String,
    pub tags: String,
    pub negative_tags: String,
    pub instrumental: bool,
    pub max_mode: bool,
    /// 0 to 100; None lets Suno choose.
    pub weirdness: Option<f64>,
    pub style_influence: Option<f64>,
    /// How closely a cover follows the original, 0 to 100.
    pub audio_influence: Option<f64>,
    pub persona_id: Option<String>,
    pub task: Option<String>,
    /// Covers: the song covered. Extensions: the song continued and the second it's continued from.
    pub cover_clip_id: Option<String>,
    pub continue_clip_id: Option<String>,
    pub continue_at: Option<f64>,
    /// Made from another song (cover, extend), as Suno's web client sends it.
    pub is_remix: bool,
    /// Use as inspiration: task playlist_condition with playlist_id "inspiration" and the songs it takes after.
    pub playlist_id: Option<String>,
    pub playlist_clip_ids: Option<Vec<String>>,
    pub artist_clip_id: Option<String>,
    pub artist_start: Option<f64>,
    pub artist_end: Option<f64>,
    pub token: Option<String>,
    pub token_provider: Option<i32>,
}

impl GenerateRequest {
    pub fn new(model: &str, mode: Mode) -> Self {
        Self { model: model.to_string(), inspiration: mode == Mode::Inspiration, ..Self::default() }
    }

    /// Whether weirdness or influence numbers are set, for models that don't take them.
    pub fn has_sliders(&self) -> bool {
        self.weirdness.is_some() || self.style_influence.is_some() || self.audio_influence.is_some()
    }

    /// The body, with every field present, even as null: the server wants them all.
    pub fn body(&self) -> Value {
        let mut metadata = json!({
            "web_client_pathname": "/create",
            "is_max_mode": self.max_mode,
            "is_mumble": false,
            "create_mode": if self.inspiration { "inspiration" } else if self.cover_clip_id.is_some() { "cover" } else { "custom" },
            "user_tier": "",
            "create_session_token": uuid::Uuid::new_v4().to_string(),
            "disable_volume_normalization": false,
        });
        if self.has_sliders() {
            let mut sliders = serde_json::Map::new();
            for (name, value) in [("weirdness_constraint", self.weirdness), ("style_weight", self.style_influence), ("audio_weight", self.audio_influence)] {
                if let Some(v) = value {
                    sliders.insert(name.into(), json!(v / 100.0));
                }
            }
            metadata["control_sliders"] = Value::Object(sliders);
        }
        if self.is_remix {
            metadata["is_remix"] = json!(true);
        }
        let mut body = json!({
            "token": self.token,
            "generation_type": "TEXT",
            "title": self.title,
            "tags": self.tags,
            "negative_tags": self.negative_tags,
            "mv": self.model,
            "prompt": self.prompt,
            "make_instrumental": self.instrumental,
            "user_uploaded_images_b64": null,
            "metadata": metadata,
            "override_fields": [],
            "cover_clip_id": self.cover_clip_id,
            "cover_start_s": null,
            "cover_end_s": null,
            "persona_id": self.persona_id,
            "artist_clip_id": self.artist_clip_id,
            "artist_start_s": self.artist_start,
            "artist_end_s": self.artist_end,
            "continue_clip_id": self.continue_clip_id,
            "continued_aligned_prompt": null,
            "continue_at": self.continue_at,
            "transaction_uuid": uuid::Uuid::new_v4().to_string(),
        });
        if let Some(task) = &self.task {
            body["task"] = json!(task);
        }
        if let Some(provider) = self.token_provider {
            body["token_provider"] = json!(provider);
        }
        if let Some(playlist) = &self.playlist_id {
            body["playlist_id"] = json!(playlist);
        }
        if let Some(ids) = &self.playlist_clip_ids {
            body["playlist_clip_ids"] = json!(ids);
        }
        body
    }

    /// The same limits the Suno web form applies, counted the same way (UTF-16 units).
    pub fn check_lengths(&self, limits: MaxLengths) -> Option<String> {
        let (prompt_name, prompt_limit) = if self.inspiration { (t("Description"), limits.description) } else { (t("Lyrics"), limits.prompt) };
        let checks = [(t("Title"), &self.title, limits.title), (t("Style"), &self.tags, limits.tags), (t("Exclude styles"), &self.negative_tags, limits.negative_tags), (prompt_name, &self.prompt, prompt_limit)];
        checks.into_iter().find_map(|(name, value, limit)| {
            let length = value.encode_utf16().count();
            (limit > 0 && length > limit).then(|| tf("{0} is {1} characters. This model allows {2}.", &[&name, &length, &limit]))
        })
    }

    /// A persona only takes effect with its task and its source song, as Suno's Create page sends them: task
    /// "artist_consistency" ("vox" for a voice persona) with artist_clip_id and the song's range, 0 to its length.
    pub fn apply_persona(&mut self, persona: &PersonaInfo, model_takes_voice: Option<bool>) -> Result<()> {
        if persona.is_trashed || persona.is_hidden {
            return Err(Error::api(t("That persona is in the trash or hidden, so Suno can't use it. Choose another persona.")));
        }
        let mut voice = persona.is_voice;
        if voice && model_takes_voice == Some(false) {
            // An older model can't do voice personas; one with a source song still works as an ordinary persona.
            if persona.root_clip_id.is_none() {
                return Err(Error::api(t("The model you chose can't use voice personas. Choose a newer model.")));
            }
            voice = false;
        }
        if !voice && persona.root_clip_id.is_none() {
            return Err(Error::api(t("That persona has no source song, so Suno can't use it. Choose another persona.")));
        }
        self.artist_clip_id = persona.root_clip_id.clone();
        self.artist_start = Some(0.0);
        self.artist_end = persona.root_clip_id.as_ref().and(persona.root_duration);
        self.task = Some(
            match self.task.as_deref() {
                Some("cover") => if voice { "vox_cover" } else { "artist_cover" },
                Some("extend") => if voice { "vox_extend" } else { "artist_extend" },
                None | Some("") => if voice { "vox" } else { "artist_consistency" },
                Some(other) => other,
            }
            .to_string(),
        );
        Ok(())
    }
}

/// "0 to 100" from a text box: None when empty, Err with what to say when it isn't such a number.
pub fn percent(text: &str, name: &str) -> std::result::Result<Option<f64>, String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    match text.replace(',', ".").parse::<f64>() {
        Ok(v) if (0.0..=100.0).contains(&v) => Ok(Some(v)),
        _ => Err(tf("{0} must be a number from 0 to 100, or empty.", &[&name])),
    }
}

/// The voice choice as the style words Suno takes.
pub fn voice_tag(choice: usize) -> &'static str {
    match choice {
        1 => "male vocals",
        2 => "female vocals",
        _ => "",
    }
}

/// The written lyrics from POST /api/generate/lyrics/ and its polling.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct LyricsResult {
    pub id: Option<String>,
    pub status: Option<String>,
    pub title: Option<String>,
    pub text: Option<String>,
    pub error_message: Option<String>,
}

/// GET /api/prompts/suggestions: style ideas, as strings or objects with the text in a field; each once.
pub fn read_style_ideas(root: &Value) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let mut add = |list: &Vec<Value>| {
        for item in list {
            let text = match item {
                Value::String(s) => Some(s.trim().to_string()),
                Value::Object(_) => ["prompt", "text", "tags", "tag", "name", "label", "value"].iter().find_map(|n| item.get(*n).and_then(Value::as_str)).map(|s| s.trim().to_string()),
                _ => None,
            };
            if let Some(text) = text.filter(|t| !t.is_empty())
                && !found.iter().any(|f| f.eq_ignore_ascii_case(&text))
            {
                found.push(text);
            }
        }
    };
    match root {
        Value::Array(list) => add(list),
        _ => {
            for name in ["prompts", "suggestions", "tags", "styles"] {
                if let Some(list) = root.get(name).and_then(Value::as_array) {
                    add(list);
                }
            }
        }
    }
    found
}

/// A lyrics draft from Suno's lyrics editor.
#[derive(Debug, Clone, PartialEq)]
pub struct LyricsDraft {
    pub id: String,
    pub title: String,
    pub updated: Option<chrono::DateTime<chrono::Utc>>,
}

impl LyricsDraft {
    pub fn name(&self) -> String {
        if self.title.trim().is_empty() { t("Untitled") } else { self.title.trim().to_string() }
    }

    pub fn spoken_text(&self) -> String {
        match self.updated {
            Some(when) => {
                let local = when.with_timezone(&chrono::Local);
                let at = format!("{} {}", crate::i18n::long_date(local.date_naive()), local.format("%H:%M"));
                format!("{}, {}", self.name(), tf("changed {0}", &[&at]))
            }
            None => self.name(),
        }
    }
}

/// GET /api/lyrics-projects: {projects: [{id, title, updated_at}], next_cursor}.
pub fn read_lyrics_drafts(root: &Value) -> (Vec<LyricsDraft>, Option<String>) {
    let list = ["projects", "results", "items"].iter().find_map(|n| root.get(*n).and_then(Value::as_array));
    let drafts = list
        .into_iter()
        .flatten()
        .filter_map(|p| {
            let id = ["id", "project_id"].iter().find_map(|n| p.get(*n).and_then(Value::as_str)).filter(|s| !s.is_empty())?.to_string();
            let title = ["title", "name"].iter().find_map(|n| p.get(*n).and_then(Value::as_str)).unwrap_or("").to_string();
            let updated = ["updated_at", "created_at"].iter().find_map(|n| p.get(*n).and_then(Value::as_str)).and_then(|d| chrono::DateTime::parse_from_rfc3339(d).ok()).map(|d| d.with_timezone(&chrono::Utc));
            Some(LyricsDraft { id, title, updated })
        })
        .collect();
    (drafts, root.get("next_cursor").and_then(Value::as_str).filter(|c| !c.is_empty()).map(str::to_string))
}

/// The words of one draft: the project's own "lyrics" field; failing that, the longest lyrics, content, text,
/// body or prompt at any depth.
pub fn read_lyrics_draft_text(root: &Value) -> Option<String> {
    if let Some(own) = root.get("lyrics").and_then(Value::as_str).filter(|s| !s.trim().is_empty()) {
        return Some(own.trim().to_string());
    }
    fn look<'a>(value: &'a Value, depth: u8, best: &mut Option<&'a str>) {
        if depth > 6 {
            return;
        }
        match value {
            Value::Object(obj) => {
                for (name, v) in obj {
                    match v {
                        Value::String(s) if matches!(name.as_str(), "lyrics" | "content" | "text" | "body" | "prompt") => {
                            if !s.trim().is_empty() && s.len() > best.map_or(0, str::len) {
                                *best = Some(s);
                            }
                        }
                        _ => look(v, depth + 1, best),
                    }
                }
            }
            Value::Array(list) => list.iter().for_each(|v| look(v, depth + 1, best)),
            _ => {}
        }
    }
    let mut best = None;
    look(root, 0, &mut best);
    best.map(|s| s.trim().to_string())
}

/// A draft's title as Suno's lyrics editor sends it: trimmed and cut to 200 characters.
pub fn draft_title(title: &str) -> String {
    title.trim().chars().take(200).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn models_say_what_they_support_and_remasters_leave_defaults_out() {
        let mut m = SunoModel { external_key: "chirp-v3-0".into(), ..SunoModel::default() };
        assert!(!m.supports("cover", &["persona", "cover"]) && m.cover_key() == "chirp-v3-5-tau");
        m.allowed_condition_combinations = Some(vec![vec!["cover".into()]]);
        assert!(m.supports("cover", &["cover"]) && !m.supports("cover", &["persona", "cover"]));
        m.capabilities = Some(vec!["extend".into()]);
        assert!(!m.supports("cover", &["cover"]));
        let r = RemasterRequest { clip_id: "c".into(), tone: Some(50.0), clarity: Some(80.0), ..RemasterRequest::default() };
        let body = r.body();
        assert!(body.get("tone").is_none() && (body["clarity"].as_f64().unwrap() - 0.8).abs() < 1e-9);
        assert_eq!(remaster_models(&[SunoModel { external_key: "x".into(), ..SunoModel::default() }]).len(), 4);
        assert!(remaster_models(&[]).is_empty());
    }

    #[test]
    fn style_ideas_and_drafts_read() {
        let ideas = read_style_ideas(&serde_json::json!({ "prompts": ["Lo-fi", { "text": "lo-fi" }, { "tag": "Jazz" }] }));
        assert_eq!(ideas, vec!["Lo-fi", "Jazz"]);
        let (drafts, next) = read_lyrics_drafts(&serde_json::json!({ "projects": [{ "id": "d1", "title": "" }, { "title": "no id" }], "next_cursor": "c" }));
        assert_eq!((drafts.len(), drafts[0].name(), next.as_deref()), (1, "Untitled".to_string(), Some("c")));
        let text = read_lyrics_draft_text(&serde_json::json!({ "doc": { "blocks": [{ "text": "short" }, { "content": "the longer words" }] } }));
        assert_eq!(text.as_deref(), Some("the longer words"));
    }

    #[test]
    fn the_body_has_every_field_and_sliders_only_when_set() {
        let mut r = GenerateRequest::new("chirp-fenix", Mode::Custom);
        r.prompt = "La la".into();
        let body = r.body();
        assert_eq!((body["mv"].as_str(), body["metadata"]["create_mode"].as_str()), (Some("chirp-fenix"), Some("custom")));
        assert!(body.get("cover_clip_id").is_some() && body.get("task").is_none() && body["metadata"].get("control_sliders").is_none());
        r.weirdness = Some(40.0);
        r.token = Some("t".into());
        r.token_provider = Some(2);
        let body = r.body();
        assert_eq!((body["metadata"]["control_sliders"]["weirdness_constraint"].as_f64(), body["token_provider"].as_i64()), (Some(0.4), Some(2)));
    }

    #[test]
    fn lengths_count_utf16_and_percent_reads_numbers() {
        let mut r = GenerateRequest::new("m", Mode::Inspiration);
        r.prompt = "a".repeat(11);
        let limits = MaxLengths { description: 10, ..MaxLengths::default() };
        assert_eq!(r.check_lengths(limits).as_deref(), Some("Description is 11 characters. This model allows 10."));
        assert_eq!(percent(" 42,5 ", "Weirdness"), Ok(Some(42.5)));
        assert!(percent("200", "Weirdness").is_err());
        assert_eq!(percent("", "Weirdness"), Ok(None));
    }

    #[test]
    fn personas_set_their_task_and_source_song() {
        let info = PersonaInfo::read(&serde_json::json!({ "root_clip_id": "r", "persona_type": "VOX", "clip": { "id": "r", "metadata": { "duration": 90.5 } } }));
        assert_eq!((info.is_voice, info.root_duration), (true, Some(90.5)));
        let mut r = GenerateRequest::new("m", Mode::Custom);
        r.apply_persona(&info, Some(false)).unwrap();
        assert_eq!((r.task.as_deref(), r.artist_clip_id.as_deref(), r.artist_end), (Some("artist_consistency"), Some("r"), Some(90.5)));
        let none = PersonaInfo::read(&serde_json::json!({ "root_clip_id": EMPTY_CLIP_ID }));
        assert!(GenerateRequest::new("m", Mode::Custom).apply_persona(&none, None).is_err());
    }

    #[test]
    fn shared_models_join_the_plan_without_replacing_it() {
        let mut models = read_models(&json!([{ "external_key": "chirp-v5", "name": "v5", "is_default_model": true }]));
        let shared = read_shared_models(&json!({
            "generation_config": { "capabilities": ["generate", "cover"], "allowed_condition_combinations": [["cover"], [], ["persona", "cover"]] },
            "items": [ { "external_key": "chirp-v5", "name": "Renamed", "capabilities": ["generate", "vox"] }, { "external_key": "ana-model", "id": 7, "name": "Ana's model" }, { "external_key": "chirp-v9", "name": "Not in the plan" }, { "name": "no key" } ]
        }));
        assert_eq!(shared.combinations, vec![vec!["cover".to_string()], vec!["persona".to_string(), "cover".to_string()]]);
        add_shared_models(&mut models, shared);
        assert_eq!(models.len(), 2);
        assert_eq!((models[0].name.as_str(), models[0].takes_voice_personas()), ("v5", Some(true)));
        assert!(models[1].shared && models[1].can_use);
        assert_eq!(models[1].capabilities.as_deref(), Some(&["generate".to_string(), "cover".to_string()][..]));
        assert_eq!(models[1].display_name(), "Ana's model, shared with you");
    }

    #[test]
    fn models_skip_odd_entries_and_start_on_the_last_one_used() {
        let models = read_models(&serde_json::json!([{ "external_key": "a", "name": "A" }, { "external_key": "b", "is_default_model": true }, 5, { "external_key": "c", "can_use": false }]));
        let usable = usable(&models);
        assert_eq!(usable.len(), 2);
        assert_eq!((starting_model(&usable, "a"), starting_model(&usable, "zzz")), (0, 1));
    }
}
