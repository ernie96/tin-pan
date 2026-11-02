//! Personas (yours, loved and followed), custom models trained on your songs, and lyrics mashup, as Tin Pan 1
//! has them in the Library menu.

use std::thread;
use std::time::Duration;

use reqwest::Method;
use serde_json::{Value, json};

use crate::api::{SunoApi, escape};
use crate::clip::SunoClip;
use crate::create::{LyricsResult, PersonaInfo};
use crate::error::{Error, Result};
use crate::i18n::{t, tf};
use crate::json as sj;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersonaList {
    Mine,
    Loved,
    Followed,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PersonaDetails {
    pub id: String,
    pub name: String,
    pub description: String,
    pub is_public: bool,
    pub is_loved: bool,
    pub is_owned: bool,
    pub is_trashed: bool,
    pub is_voice: bool,
    pub root_clip_id: Option<String>,
    pub creator: Option<String>,
}

fn text(v: &Value, name: &str) -> Option<String> {
    v.get(name).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string)
}

impl PersonaDetails {
    pub fn read(p: &Value) -> Self {
        let info = PersonaInfo::read(p);
        let creator = text(p, "user_display_name").or_else(|| text(p, "user_handle").map(|h| format!("@{h}")));
        Self {
            id: text(p, "id").unwrap_or_default(),
            name: text(p, "name").unwrap_or_default(),
            description: text(p, "description").unwrap_or_default(),
            is_public: p.get("is_public").and_then(Value::as_bool) == Some(true),
            is_loved: p.get("is_loved").and_then(Value::as_bool) == Some(true),
            is_owned: p.get("is_owned").and_then(Value::as_bool) != Some(false),
            is_trashed: info.is_trashed,
            is_voice: info.is_voice,
            root_clip_id: info.root_clip_id,
            creator,
        }
    }

    pub fn display_name(&self) -> String {
        if self.name.trim().is_empty() { t("Unnamed persona") } else { self.name.trim().to_string() }
    }

    /// "Ana's voice, voice, public, loved, the description", as Tin Pan 1 lists them.
    pub fn spoken_text(&self) -> String {
        let mut parts = vec![self.display_name()];
        if self.is_voice {
            parts.push(t("voice"));
        }
        if let Some(creator) = self.creator.as_ref().filter(|_| !self.is_owned) {
            parts.push(tf("by {0}", &[creator]));
        }
        if self.is_owned {
            parts.push(if self.is_public { t("public") } else { t("private") });
        }
        if self.is_loved {
            parts.push(t("loved"));
        }
        if self.is_trashed {
            parts.push(t("in the trash"));
        }
        if !self.description.trim().is_empty() {
            parts.push(self.description.trim().to_string());
        }
        parts.join(", ")
    }
}

/// A model a custom one can be trained from.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelBase {
    pub key: String,
    pub name: String,
    pub is_default: bool,
}

/// A custom model Suno is still training.
#[derive(Debug, Clone, PartialEq)]
pub struct PendingModel {
    pub id: String,
    pub name: String,
}

impl SunoApi {
    /// Every page of one persona list (20 at most).
    pub fn persona_list(&self, list: PersonaList) -> Result<Vec<PersonaDetails>> {
        let path = match list {
            PersonaList::Loved => "/api/persona/get-loved-personas/",
            PersonaList::Followed => "/api/persona/get-followed-personas/",
            PersonaList::Mine => "/api/persona/get-personas/",
        };
        let mut result: Vec<PersonaDetails> = Vec::new();
        let mut token: Option<String> = None;
        for page in 1..=20 {
            let query = match &token {
                Some(token) => format!("?page={page}&continuation_token={}", escape(token)),
                None => format!("?page={page}"),
            };
            let root = sj::parse(&self.send(Method::GET, &format!("{path}{query}"), None)?)?;
            let Some(personas) = root.get("personas").and_then(Value::as_array) else { break };
            let before = result.len();
            for p in personas.iter().filter(|p| p.is_object()).map(PersonaDetails::read) {
                if !p.id.is_empty() && !result.iter().any(|x| x.id == p.id) {
                    result.push(p);
                }
            }
            token = text(&root, "continuation_token");
            let total = root.get("total_results").and_then(sj::as_int);
            if result.len() == before || total.is_some_and(|t| result.len() as i64 >= t) || (token.is_none() && total.is_none()) {
                break;
            }
        }
        Ok(result)
    }

    pub fn edit_persona(&self, id: &str, name: &str, description: &str, public: bool) -> Result<()> {
        let body = json!({ "persona_id": id, "name": name, "description": description, "is_public": public });
        self.send(Method::PUT, &format!("/api/persona/edit-persona/{}/", escape(id)), Some(&body)).map(drop)
    }

    /// Loves or un-loves a persona; whether it's loved now.
    pub fn toggle_persona_love(&self, id: &str) -> Result<bool> {
        let reply = self.send(Method::POST, &format!("/api/persona/{}/toggle_love/", escape(id)), None)?;
        Ok(sj::parse(&reply)?.get("loved").and_then(Value::as_bool) == Some(true))
    }

    /// Moves a persona to the trash, or back out with `undo`.
    pub fn trash_persona(&self, id: &str, undo: bool) -> Result<()> {
        self.send(Method::PUT, &format!("/api/persona/trash-persona/{}/?undo={undo}&hide=false", escape(id)), None).map(drop)
    }

    /// Songs made with a persona, the first 5 pages.
    pub fn persona_songs(&self, id: &str) -> Result<Vec<SunoClip>> {
        let mut result: Vec<SunoClip> = Vec::new();
        for page in 1..=5 {
            let clips = sj::find_clips(&self.send(Method::GET, &format!("/api/persona/get-persona-paginated/{}/?page={page}", escape(id)), None)?)?;
            let added: Vec<SunoClip> = clips.into_iter().filter(|c| !result.iter().any(|x| x.id == c.id)).collect();
            if added.is_empty() {
                break;
            }
            result.extend(added);
        }
        Ok(result)
    }

    pub fn custom_model_bases(&self) -> Result<Vec<ModelBase>> {
        let root = sj::parse(&self.send(Method::GET, "/api/custom-model/bases/", None)?)?;
        Ok(root
            .get("options")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|b| {
                let key = text(b, "base_model")?;
                Some(ModelBase { name: text(b, "name").unwrap_or_else(|| key.clone()), key, is_default: b.get("is_default").and_then(Value::as_bool) == Some(true) })
            })
            .collect())
    }

    pub fn pending_models(&self) -> Result<Vec<PendingModel>> {
        let root = sj::parse(&self.send(Method::GET, "/api/custom-model/pending/", None)?)?;
        Ok(root
            .get("pending_models")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|m| Some(PendingModel { id: text(m, "id")?, name: text(m, "name").unwrap_or_default() }))
            .collect())
    }

    /// Starts training a custom model on your songs. Names are up to 16 characters.
    pub fn create_custom_model(&self, clip_ids: &[String], name: &str, base: Option<&str>) -> Result<()> {
        let mut body = json!({ "clip_ids": clip_ids, "name": name });
        if let Some(base) = base {
            body["base_model"] = json!(base);
        }
        self.send(Method::POST, "/api/custom-model/create/", Some(&body)).map(drop)
    }

    pub fn rename_custom_model(&self, id: &str, name: &str) -> Result<()> {
        self.send(Method::PUT, "/api/custom-model/edit/", Some(&json!({ "id": id, "name": name }))).map(drop)
    }

    pub fn archive_custom_model(&self, id: &str) -> Result<()> {
        self.send(Method::POST, "/api/custom-model/archive/", Some(&json!({ "id": id }))).map(drop)
    }

    /// Suno writes one set of lyrics from two, then the lyrics are asked for every 2 seconds until done.
    pub fn mashup_lyrics(&self, first: &str, second: &str) -> Result<String> {
        let body = json!({ "lyrics_a": first, "lyrics_b": second, "create_session_token": uuid::Uuid::new_v4().to_string(), "source": "create_ui" });
        let id = sj::try_read_string(&self.send(Method::POST, "/api/generate/lyrics-mashup", Some(&body))?, "mashup_id")
            .filter(|i| !i.is_empty())
            .ok_or_else(|| Error::api(t("Suno didn't start the mashup.")))?;
        for _ in 0..30 {
            thread::sleep(Duration::from_secs(2));
            let result: LyricsResult = serde_json::from_str(&self.send(Method::GET, &format!("/api/generate/lyrics/{}", escape(&id)), None)?).unwrap_or_default();
            match result.status.as_deref() {
                Some("complete") if result.text.as_deref().is_some_and(|t| !t.trim().is_empty()) => return Ok(result.text.unwrap_or_default()),
                Some("error" | "failed") => return Err(Error::api(t("Suno couldn't mash those lyrics up."))),
                _ => {}
            }
        }
        Err(Error::api(t("Suno took too long writing the lyrics. Try again.")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn personas_say_what_they_are() {
        let p = PersonaDetails::read(&json!({ "id": "p1", "name": "Ana", "is_public": true, "is_loved": true, "persona_type": "vox", "description": "warm" }));
        assert_eq!(p.spoken_text(), "Ana, voice, public, loved, warm");
        let theirs = PersonaDetails::read(&json!({ "id": "p2", "name": "", "is_owned": false, "user_handle": "bo" }));
        assert_eq!(theirs.spoken_text(), "Unnamed persona, by @bo");
    }
}
