//! Suno replies that come in more than one shape: bare arrays, {"clips": [...]}, nested sections.

use std::collections::HashSet;

use serde_json::Value;

use crate::clip::SunoClip;
use crate::error::{Error, Result};
use crate::i18n::t;

/// A page of the library or a feed.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FeedPage {
    pub clips: Vec<SunoClip>,
    pub next_cursor: Option<String>,
    pub has_more: Option<bool>,
    pub num_total: Option<i64>,
}

pub fn parse(json: &str) -> Result<Value> {
    serde_json::from_str(json).map_err(|_| Error::api(t("Suno sent a reply this app doesn't understand.")))
}

/// A whole number, also from text ("12"); None for anything else.
pub fn as_int(value: &Value) -> Option<i64> {
    match value {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

fn clips(list: &[Value]) -> Vec<SunoClip> {
    list.iter().filter_map(SunoClip::from_value).collect()
}

pub fn read_feed_page(json: &str) -> Result<FeedPage> {
    Ok(match parse(json)? {
        Value::Array(list) => FeedPage { clips: clips(&list), ..FeedPage::default() },
        Value::Object(root) => FeedPage {
            clips: root.get("clips").and_then(Value::as_array).map(|l| clips(l)).unwrap_or_default(),
            next_cursor: root.get("next_cursor").and_then(Value::as_str).map(str::to_string),
            has_more: root.get("has_more").and_then(Value::as_bool),
            num_total: root.get("num_total").and_then(as_int),
        },
        _ => FeedPage::default(),
    })
}

/// A reply that is a clip, a list of clips, or {"clips": [...]}.
pub fn find_clips_by_id(json: &str) -> Result<Vec<SunoClip>> {
    let root = parse(json)?;
    Ok(match &root {
        Value::Array(list) => clips(list),
        Value::Object(obj) => match obj.get("clips").and_then(Value::as_array) {
            Some(list) => clips(list),
            None if obj.get("id").is_some_and(Value::is_string) => SunoClip::from_value(&root).into_iter().collect(),
            None => Vec::new(),
        },
        _ => Vec::new(),
    })
}

/// Every object that looks like a clip (an id and an audio_url), at any depth, each once.
pub fn find_clips(json: &str) -> Result<Vec<SunoClip>> {
    fn walk(value: &Value, found: &mut Vec<SunoClip>, seen: &mut HashSet<String>) {
        match value {
            Value::Object(obj) => {
                if obj.get("id").is_some_and(Value::is_string)
                    && obj.contains_key("audio_url")
                    && let Some(clip) = SunoClip::from_value(value)
                {
                    if seen.insert(clip.id.clone()) {
                        found.push(clip);
                    }
                    return;
                }
                obj.values().for_each(|v| walk(v, found, seen));
            }
            Value::Array(list) => list.iter().for_each(|v| walk(v, found, seen)),
            _ => {}
        }
    }
    let root = parse(json)?;
    let (mut found, mut seen) = (Vec::new(), HashSet::new());
    walk(&root, &mut found, &mut seen);
    Ok(found)
}

/// A top-level text field, or None (also for replies that aren't JSON, such as error pages).
pub fn try_read_string(json: &str, name: &str) -> Option<String> {
    serde_json::from_str::<Value>(json).ok()?.get(name)?.as_str().map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLIP: &str = r#"{"id":"c1","title":"Song","audio_url":"https://cdn1.suno.ai/c1.mp3","status":"complete","metadata":{"duration":125.4,"tags":"pop"}}"#;

    #[test]
    fn reads_wrapped_and_bare_pages() {
        let page = read_feed_page(&format!(r#"{{"clips":[{CLIP}],"num_total":41,"next_cursor":"n","has_more":true}}"#)).unwrap();
        assert_eq!((page.clips.len(), page.num_total, page.next_cursor.as_deref(), page.has_more), (1, Some(41), Some("n"), Some(true)));
        assert_eq!(read_feed_page(&format!("[{CLIP}]")).unwrap().clips[0].title.as_deref(), Some("Song"));
    }

    #[test]
    fn finds_nested_clips_once() {
        let json = format!(r#"{{"sections":[{{"items":[{CLIP},{CLIP}]}}],"more":{{"clip":{CLIP}}}}}"#);
        assert_eq!(find_clips(&json).unwrap().len(), 1);
    }

    #[test]
    fn a_single_clip_reply_is_found_by_id() {
        assert_eq!(find_clips_by_id(CLIP).unwrap()[0].id, "c1");
        assert!(find_clips_by_id("{}").unwrap().is_empty());
    }

    #[test]
    fn error_pages_are_refused_or_read_as_nothing() {
        assert!(read_feed_page("<html>").is_err());
        assert_eq!(try_read_string("<html>", "x"), None);
        assert_eq!(try_read_string(r#"{"x":"y"}"#, "x").as_deref(), Some("y"));
    }
}
