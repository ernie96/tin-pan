//! A Suno playlist, as Tin Pan 1 reads and says it.

use serde_json::Value;

use crate::i18n::{plural, t};
use crate::json::as_int;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub description: String,
    pub is_public: bool,
    pub song_count: Option<i64>,
}

impl Playlist {
    pub fn read(p: &Value) -> Self {
        let text = |name: &str| p.get(name).and_then(Value::as_str).unwrap_or("").to_string();
        let id = match p.get("id") {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Null) | None => String::new(),
            Some(other) => other.to_string(),
        };
        let song_count = p.get("num_total_results").and_then(as_int).or_else(|| p.get("playlist_clips").and_then(Value::as_array).map(|c| c.len() as i64));
        Self { id, name: text("name"), description: text("description"), is_public: p.get("is_public").and_then(Value::as_bool) == Some(true), song_count }
    }

    pub fn display_name(&self) -> String {
        if self.name.trim().is_empty() { t("Untitled playlist") } else { self.name.trim().to_string() }
    }

    /// "Name, 12 songs", as the list says it.
    pub fn spoken_text(&self) -> String {
        match self.song_count {
            Some(n) => format!("{}, {}", self.display_name(), plural(n, "{0} song", "{0} songs")),
            None => self.display_name(),
        }
    }
}

/// Whether a playlist change failed: Suno lists failures and no successes.
pub fn change_failed(json: &str) -> bool {
    let Ok(root) = serde_json::from_str::<Value>(if json.trim().is_empty() { "{}" } else { json }) else { return false };
    let count = |name: &str| match root.get(name) {
        Some(Value::Array(a)) => a.len() as i64,
        Some(v) => as_int(v).unwrap_or(0),
        None => 0,
    };
    count("failures") > 0 && count("successes") == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_playlist_and_says_its_songs() {
        let p = Playlist::read(&serde_json::json!({"id": "p1", "name": "Mix", "num_total_results": "12", "is_public": true}));
        assert_eq!((p.id.as_str(), p.spoken_text(), p.is_public), ("p1", "Mix, 12 songs".to_string(), true));
        assert_eq!(Playlist::read(&serde_json::json!({"id": 7, "playlist_clips": [{}, {}]})).spoken_text(), "Untitled playlist, 2 songs");
    }

    #[test]
    fn a_change_failed_only_when_nothing_succeeded() {
        assert!(change_failed(r#"{"failures": ["x"], "successes": []}"#));
        assert!(!change_failed(r#"{"failures": ["x"], "successes": ["y"]}"#));
        assert!(!change_failed(""));
    }
}
