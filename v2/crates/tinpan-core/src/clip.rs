//! A Suno song ("clip") as its API sends it, and how Tin Pan says it.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer};

use crate::i18n::{t, tf};

/// Suno's replies sometimes carry null, or a number as text, where a count or a yes/no usually is
/// ("play_count": null). Read strictly, one such field would drop the whole song from the list.
mod lenient {
    use serde::{Deserialize, Deserializer};
    use serde_json::Value;

    pub fn int<'de, D: Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
        Ok(match Value::deserialize(d)? {
            Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f.round() as i64)).unwrap_or(0),
            Value::String(s) => s.trim().parse().unwrap_or(0),
            Value::Bool(b) => i64::from(b),
            _ => 0,
        })
    }

    pub fn flag<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
        Ok(opt_flag(d)?.unwrap_or(false))
    }

    pub fn opt_flag<'de, D: Deserializer<'de>>(d: D) -> Result<Option<bool>, D::Error> {
        Ok(match Value::deserialize(d)? {
            Value::Bool(b) => Some(b),
            Value::String(s) => s.trim().parse().ok(),
            Value::Number(n) => n.as_i64().map(|n| n != 0),
            _ => None,
        })
    }

    pub fn opt_number<'de, D: Deserializer<'de>>(d: D) -> Result<Option<f64>, D::Error> {
        Ok(match Value::deserialize(d)? {
            Value::Number(n) => n.as_f64(),
            Value::String(s) => s.trim().parse().ok(),
            _ => None,
        }
        .filter(|f: &f64| f.is_finite()))
    }

    pub fn text<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
        Ok(opt_text(d)?.unwrap_or_default())
    }

    pub fn opt_text<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
        Ok(match Value::deserialize(d)? {
            Value::String(s) => Some(s),
            Value::Number(n) => Some(n.to_string()),
            _ => None,
        })
    }
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct SunoClip {
    #[serde(deserialize_with = "lenient::text")]
    pub id: String,
    #[serde(deserialize_with = "lenient::opt_text")]
    pub title: Option<String>,
    #[serde(deserialize_with = "lenient::opt_text")]
    pub audio_url: Option<String>,
    #[serde(deserialize_with = "lenient::opt_text")]
    pub video_url: Option<String>,
    #[serde(deserialize_with = "lenient::opt_text")]
    pub image_url: Option<String>,
    #[serde(deserialize_with = "lenient::opt_text")]
    pub image_large_url: Option<String>,
    #[serde(deserialize_with = "lenient::opt_flag")]
    pub is_trashed: Option<bool>,
    /// submitted, queued, streaming, complete or error.
    #[serde(deserialize_with = "lenient::opt_text")]
    pub status: Option<String>,
    #[serde(deserialize_with = "lenient::opt_text")]
    pub model_name: Option<String>,
    #[serde(deserialize_with = "date")]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(deserialize_with = "lenient::flag")]
    pub is_public: bool,
    #[serde(deserialize_with = "lenient::int")]
    pub play_count: i64,
    #[serde(deserialize_with = "lenient::int")]
    pub upvote_count: i64,
    #[serde(deserialize_with = "lenient::opt_text")]
    pub handle: Option<String>,
    #[serde(rename = "display_name", deserialize_with = "lenient::opt_text")]
    pub creator_name: Option<String>,
    #[serde(deserialize_with = "lenient::flag")]
    pub is_liked: bool,
    /// {"reaction_type": "D"} or a plain "D"; "D" is disliked.
    pub reaction: Option<serde_json::Value>,
    #[serde(deserialize_with = "lenient::opt_flag")]
    pub is_download_unlocked: Option<bool>,
    pub media_urls: Option<Vec<MediaUrl>>,
    pub metadata: Option<ClipMetadata>,
}

fn date<'de, D: Deserializer<'de>>(d: D) -> Result<Option<DateTime<Utc>>, D::Error> {
    Ok(Option::<String>::deserialize(d).ok().flatten().and_then(|s| DateTime::parse_from_rfc3339(&s).ok()).map(|d| d.to_utc()))
}

impl SunoClip {
    /// Reads a clip from JSON; None when it has no id.
    pub fn from_value(value: &serde_json::Value) -> Option<Self> {
        let clip: Self = serde_json::from_value(value.clone()).ok()?;
        (!clip.id.is_empty()).then_some(clip)
    }

    pub fn display_title(&self) -> String {
        match self.title.as_deref().map(str::trim) {
            Some(title) if !title.is_empty() => title.to_string(),
            _ => t("Untitled song"),
        }
    }

    pub fn duration(&self) -> Option<f64> {
        self.metadata.as_ref().and_then(|m| m.duration)
    }

    pub fn is_ready(&self) -> bool {
        matches!(self.status.as_deref(), Some("complete" | "streaming"))
    }

    pub fn is_complete(&self) -> bool {
        self.status.as_deref() == Some("complete")
    }

    pub fn has_failed(&self) -> bool {
        self.status.as_deref() == Some("error")
    }

    pub fn can_play(&self) -> bool {
        self.is_ready()
    }

    pub fn is_disliked(&self) -> bool {
        let letter = match &self.reaction {
            Some(serde_json::Value::String(s)) => Some(s.as_str()),
            Some(serde_json::Value::Object(o)) => o.get("reaction_type").and_then(|v| v.as_str()),
            _ => None,
        };
        letter == Some("D")
    }

    pub fn web_link(&self) -> String {
        format!("https://suno.com/song/{}", self.id)
    }

    /// What a screen reader says for this song in a list: title, length, then anything unusual.
    pub fn spoken_text(&self) -> String {
        let mut text = self.display_title();
        if let Some(d) = self.duration().filter(|d| *d > 0.0) {
            text += &format!(", {}", format_time(d));
        }
        if self.is_liked {
            text += &format!(", {}", t("liked"));
        } else if self.is_disliked() {
            text += &format!(", {}", t("disliked"));
        }
        if self.has_failed() {
            text += &format!(", {}", t("failed"));
        } else if !self.is_ready() && self.status.as_deref().is_some_and(|s| !s.is_empty()) {
            text += &format!(", {}", t("still being made"));
        }
        let meta = self.metadata.as_ref();
        if meta.and_then(|m| m.kind.as_deref()) == Some("concat") {
            text += &format!(", {}", t("whole song"));
        } else if meta.is_some_and(|m| m.has_stem == Some(true) || m.stem_from_id.is_some()) {
            text += &format!(", {}", t("stem"));
        }
        text
    }

    /// The spoken text, then who made it, for lists of other people's songs.
    pub fn spoken_with_creator(&self) -> String {
        let name = self.creator_name.as_deref().filter(|n| !n.trim().is_empty());
        let handle = self.handle.as_deref().filter(|h| !h.trim().is_empty());
        match (name, handle) {
            (Some(name), _) => format!("{}, {}", self.spoken_text(), tf("by {0}", &[&name])),
            (None, Some(handle)) => format!("{}, {}", self.spoken_text(), tf("by {0}", &[&format!("@{handle}")])),
            _ => self.spoken_text(),
        }
    }

    /// Takes a reaction Suno answered with ("L", "D", or none once cleared).
    pub fn set_reaction(&mut self, state: Option<&str>) {
        self.is_liked = state == Some("L");
        self.reaction = state.map(|s| serde_json::Value::String(s.to_string()));
    }

    /// Why Suno says the song failed, in a sentence; None when it gave no reason.
    pub fn failure_reason(&self) -> Option<String> {
        let meta = self.metadata.as_ref()?;
        if let Some(message) = meta.error_message.as_deref().map(str::trim).filter(|m| !m.is_empty()) {
            return Some(message.to_string());
        }
        match meta.error_type.as_deref().map(str::trim).filter(|t| !t.is_empty())? {
            "moderation_error" => Some(t("Suno's content rules turned it down.")),
            other => Some(other.replace('_', " ")),
        }
    }

    /// Suno's status word, in the chosen language.
    fn status_name(&self) -> String {
        match self.status.as_deref() {
            Some("complete") => t("complete"),
            Some("streaming") => t("streaming"),
            Some("submitted") => t("submitted"),
            Some("queued") => t("queued"),
            Some("error") => t("error"),
            None | Some("") => t("unknown"),
            Some(other) => other.to_string(),
        }
    }

    /// One fact per item, to step through with the arrow keys (Alt+Enter), as in Tin Pan 1.
    pub fn info_lines(&self) -> Vec<String> {
        let meta = self.metadata.as_ref();
        let text = |f: fn(&ClipMetadata) -> &Option<String>| meta.and_then(|m| f(m).as_deref()).map(str::trim).filter(|s| !s.is_empty());
        let mut lines = vec![self.display_title()];
        if let Some(d) = self.duration().filter(|d| *d > 0.0) {
            lines.push(tf("Length: {0}", &[&format_time(d)]));
        }
        lines.push(tf("Status: {0}", &[&self.status_name()]));
        if let Some(reason) = self.failure_reason() {
            lines.push(tf("Error: {0}", &[&reason]));
        }
        if let Some(style) = text(|m| &m.tags) {
            lines.push(tf("Style: {0}", &[&style]));
        }
        if meta.and_then(|m| m.make_instrumental) == Some(true) {
            lines.push(t("Instrumental"));
        }
        if let Some(description) = text(|m| &m.gpt_description_prompt) {
            lines.push(tf("Description: {0}", &[&description]));
        }
        if let Some(bpm) = meta.and_then(|m| m.avg_bpm).filter(|b| *b > 0.0) {
            lines.push(tf("Tempo: about {0:0} beats per minute", &[&bpm.round()]));
        }
        if let Some(model) = self.model_name.as_deref().filter(|m| !m.is_empty()) {
            lines.push(tf("Model: {0}", &[&model]));
        }
        if text(|m| &m.persona_id).is_some() {
            lines.push(t("Made with a persona"));
        }
        if let Some(handle) = self.handle.as_deref().filter(|h| !h.trim().is_empty()) {
            let who = match self.creator_name.as_deref().filter(|n| !n.trim().is_empty()) {
                Some(name) => format!("{name} (@{handle})"),
                None => format!("@{handle}"),
            };
            lines.push(tf("By {0}", &[&who]));
        }
        if let Some(made) = self.created_at {
            lines.push(tf("Made: {0}", &[&made.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M")]));
        }
        lines.push(if self.is_public { t("Public") } else { t("Private") });
        lines.push(tf("Plays: {0}. Likes: {1}.", &[&self.play_count, &self.upvote_count]));
        if self.is_download_unlocked == Some(true) {
            lines.push(t("Downloaded before, so downloading again is free."));
        }
        lines.push(tf("Link: {0}", &[&self.web_link()]));
        lines
    }

    /// Everything worth knowing, then the lyrics, for the details window (Ctrl+I).
    pub fn details_text(&self) -> String {
        let mut lines = self.info_lines();
        if let Some(lyrics) = self.lyrics() {
            lines.push(String::new());
            lines.push(t("Lyrics:"));
            lines.push(lyrics.to_string());
        }
        lines.join("\n")
    }

}

/// Lyrics from timed words, as Tin Pan 1 joins them: a space after each word unless it already ends in
/// one or a line break.
pub fn join_words(words: &[crate::lyrics::AlignedWord]) -> String {
    let mut text = String::new();
    for word in words.iter().map(|w| &w.word) {
        text.push_str(word);
        if word.chars().last().is_some_and(|c| !c.is_whitespace()) {
            text.push(' ');
        }
    }
    text.trim().to_string()
}

impl SunoClip {
    /// The song's stored lyrics (custom mode), if it has any.
    pub fn lyrics(&self) -> Option<&str> {
        self.metadata.as_ref().and_then(|m| m.prompt.as_deref()).map(str::trim).filter(|l| !l.is_empty())
    }
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct MediaUrl {
    pub url: Option<String>,
    /// Present (for example "m4a-opus") only on encrypted files.
    pub encoding: Option<serde_json::Value>,
}

impl MediaUrl {
    pub fn is_encrypted(&self) -> bool {
        self.encoding.as_ref().is_some_and(|e| !e.is_null())
    }

    pub fn is_video(&self) -> bool {
        self.url.as_deref().is_some_and(|u| u.split('?').next().unwrap_or("").to_ascii_lowercase().ends_with(".mp4"))
    }
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct ClipMetadata {
    #[serde(deserialize_with = "lenient::opt_flag")]
    pub can_remix: Option<bool>,
    /// Whether the song's page lists its remixes.
    #[serde(deserialize_with = "lenient::opt_flag")]
    pub show_remix: Option<bool>,
    #[serde(deserialize_with = "lenient::opt_number")]
    pub duration: Option<f64>,
    #[serde(deserialize_with = "lenient::opt_text")]
    pub tags: Option<String>,
    /// The lyrics in custom mode.
    #[serde(deserialize_with = "lenient::opt_text")]
    pub prompt: Option<String>,
    #[serde(deserialize_with = "lenient::opt_text")]
    pub gpt_description_prompt: Option<String>,
    #[serde(deserialize_with = "lenient::opt_flag")]
    pub make_instrumental: Option<bool>,
    #[serde(deserialize_with = "lenient::opt_number")]
    pub avg_bpm: Option<f64>,
    /// "gen", "concat", "upsample" and so on.
    #[serde(rename = "type", deserialize_with = "lenient::opt_text")]
    pub kind: Option<String>,
    #[serde(deserialize_with = "lenient::opt_flag")]
    pub has_stem: Option<bool>,
    #[serde(deserialize_with = "lenient::opt_text")]
    pub stem_from_id: Option<String>,
    #[serde(deserialize_with = "lenient::opt_text")]
    pub error_message: Option<String>,
    #[serde(deserialize_with = "lenient::opt_text")]
    pub error_type: Option<String>,
    #[serde(deserialize_with = "lenient::opt_text")]
    pub task: Option<String>,
    #[serde(deserialize_with = "lenient::opt_text")]
    pub persona_id: Option<String>,
    #[serde(deserialize_with = "lenient::opt_text")]
    pub cover_clip_id: Option<String>,
    /// For an extension, the songs it continues, oldest first; read loosely, so an odd shape never loses the song.
    pub history: Option<serde_json::Value>,
    /// For a whole song, the parts it joins.
    pub concat_history: Option<serde_json::Value>,
}

impl ClipMetadata {
    /// The song this one was made from, when its own details say: a cover's song, or the first song an
    /// extension or whole song is made of.
    pub fn original_id(&self) -> Option<String> {
        fn first_id(list: Option<&serde_json::Value>) -> Option<String> {
            list?.as_array()?.iter().find_map(|item| match item {
                serde_json::Value::String(s) if !s.is_empty() => Some(s.clone()),
                serde_json::Value::Object(o) => o.get("id").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(str::to_string),
                _ => None,
            })
        }
        self.cover_clip_id.clone().filter(|c| !c.is_empty()).or_else(|| first_id(self.history.as_ref())).or_else(|| first_id(self.concat_history.as_ref()))
    }
}

/// "3:05", or "1:02:03" from an hour on.
pub fn format_time(seconds: f64) -> String {
    let total = seconds.max(0.0).floor() as u64;
    let (h, m, s) = (total / 3600, total / 60 % 60, total % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_clip_even_with_loose_fields() {
        let value = serde_json::json!({
            "id": "c1", "title": "Song", "status": "complete", "play_count": null, "upvote_count": "12",
            "is_public": "true", "created_at": "2026-10-21T10:00:00.000Z",
            "metadata": {"duration": 125.4, "tags": "pop", "type": "concat"}
        });
        let clip = SunoClip::from_value(&value).unwrap();
        assert_eq!((clip.id.as_str(), clip.play_count, clip.upvote_count, clip.is_public), ("c1", 0, 12, true));
        assert!(clip.created_at.is_some());
        assert_eq!(clip.spoken_text(), "Song, 2:05, whole song");
    }

    #[test]
    fn a_clip_without_an_id_is_skipped() {
        assert!(SunoClip::from_value(&serde_json::json!({"title": "x"})).is_none());
    }

    #[test]
    fn says_failed_and_unfinished_songs_and_their_creator() {
        let mut clip = SunoClip { id: "c".into(), status: Some("error".into()), handle: Some("ana".into()), ..SunoClip::default() };
        assert_eq!(clip.spoken_with_creator(), "Untitled song, failed, by @ana");
        clip.status = Some("queued".into());
        assert_eq!(clip.spoken_text(), "Untitled song, still being made");
        assert!(!clip.can_play());
    }

    #[test]
    fn information_has_one_fact_per_line_and_details_add_the_lyrics() {
        let value = serde_json::json!({
            "id": "c1", "title": "Song", "status": "error", "is_public": true, "play_count": 3, "upvote_count": 1, "handle": "ana",
            "metadata": {"duration": 65, "tags": "pop", "prompt": "la la", "error_type": "moderation_error", "avg_bpm": 119.6}
        });
        let mut clip = SunoClip::from_value(&value).unwrap();
        let lines = clip.info_lines();
        assert_eq!(&lines[..4], ["Song", "Length: 1:05", "Status: error", "Error: Suno's content rules turned it down."]);
        assert!(lines.contains(&"Tempo: about 120 beats per minute".to_string()));
        assert!(lines.contains(&"By @ana".to_string()));
        assert_eq!(lines.last().unwrap(), "Link: https://suno.com/song/c1");
        assert!(clip.details_text().ends_with("Lyrics:\nla la"));
        clip.set_reaction(Some("D"));
        assert!(clip.is_disliked() && !clip.is_liked);
        clip.set_reaction(Some("L"));
        assert!(clip.is_liked && !clip.is_disliked());
    }

    #[test]
    fn formats_times() {
        assert_eq!((format_time(65.9), format_time(3723.0)), ("1:05".into(), "1:02:03".into()));
    }
}
