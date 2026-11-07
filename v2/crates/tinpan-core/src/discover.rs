//! Creator pages, Explore, contests and comments, read as Tin Pan 1 reads them. Most of these replies were
//! never captured whole, so each reader takes the fields it knows and skips the rest.

use serde_json::Value;

use crate::clip::SunoClip;
use crate::i18n::{group_digits, long_date, plural, t, tf};
use crate::json::as_int;

fn first_text(obj: &Value, names: &[&str]) -> Option<String> {
    names.iter().find_map(|n| obj.get(*n).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string))
}

fn first_array<'a>(obj: &'a Value, names: &[&str]) -> Option<&'a Vec<Value>> {
    names.iter().find_map(|n| obj.get(*n).and_then(Value::as_array))
}

fn number(obj: &Value, names: &[&str]) -> i64 {
    names.iter().find_map(|n| obj.get(*n).and_then(as_int)).unwrap_or(0)
}

fn date(obj: &Value, names: &[&str]) -> Option<chrono::DateTime<chrono::Utc>> {
    names.iter().find_map(|n| {
        let text = obj.get(*n)?.as_str()?;
        chrono::DateTime::parse_from_rfc3339(text)
            .map(|d| d.with_timezone(&chrono::Utc))
            .ok()
            .or_else(|| chrono::NaiveDateTime::parse_from_str(text, "%Y-%m-%dT%H:%M:%S%.f").ok().map(|d| d.and_utc()))
    })
}

/// Every object that looks like a clip (an id and an audio_url), at any depth, each once.
pub fn clips_in(value: &Value) -> Vec<SunoClip> {
    fn walk(value: &Value, found: &mut Vec<SunoClip>) {
        match value {
            Value::Object(obj) => {
                if obj.get("id").is_some_and(Value::is_string)
                    && obj.contains_key("audio_url")
                    && let Some(clip) = SunoClip::from_value(value)
                {
                    if !found.iter().any(|c| c.id == clip.id) {
                        found.push(clip);
                    }
                    return;
                }
                obj.values().for_each(|v| walk(v, found));
            }
            Value::Array(list) => list.iter().for_each(|v| walk(v, found)),
            _ => {}
        }
    }
    let mut found = Vec::new();
    walk(value, &mut found);
    found
}

/// POST /api/unified/search/omnisearch: the songs section's songs. The reply is a feed whose items are
/// sections (omnisearch_songs, _users, _playlists and more), each a feed of its own, as Suno's search page reads it.
pub fn read_omnisearch(root: &Value) -> Vec<SunoClip> {
    let items = root.get("feed").and_then(|f| f.get("items")).and_then(Value::as_array);
    let section = items.into_iter().flatten().find(|item| {
        first_text(item, &["content_id", "contentId"]).is_some_and(|id| id == "omnisearch_songs" || id.starts_with("omnisearch_songs:"))
    });
    section.map(clips_in).unwrap_or_default()
}

/// "https://suno.com/@ana?x=1", "@ana" and "ana" are all "ana".
pub fn creator_handle(text: &str) -> String {
    let text = text.trim();
    let text = text.rsplit_once('@').map_or(text, |(_, rest)| rest);
    text.split(['?', '#', '/']).next().unwrap_or("").trim().to_string()
}

/// GET /api/profiles/{handle}: who it is, their numbers and a page of their songs.
#[derive(Debug, Clone, Default)]
pub struct CreatorPage {
    pub handle: String,
    pub display_name: String,
    pub description: String,
    pub total_songs: i64,
    pub followers: i64,
    pub following: i64,
    pub plays: i64,
    pub songs: Vec<SunoClip>,
    /// Their public playlists, when the page lists them.
    pub playlists: Vec<crate::playlist::Playlist>,
}

impl CreatorPage {
    pub fn read(root: &Value, handle: &str) -> Self {
        if !root.is_object() {
            return Self { handle: handle.to_string(), ..Self::default() };
        }
        let stats = root.get("stats").filter(|s| s.is_object()).unwrap_or(root);
        let songs = match first_array(root, &["clips"]) {
            Some(list) => list.iter().filter_map(SunoClip::from_value).collect(),
            None => clips_in(root),
        };
        Self {
            handle: first_text(root, &["handle"]).unwrap_or_else(|| handle.to_string()),
            display_name: first_text(root, &["display_name"]).unwrap_or_default(),
            description: first_text(root, &["profile_description", "description", "bio"]).unwrap_or_default(),
            total_songs: number(root, &["num_total_clips", "clip_count"]),
            followers: number(stats, &["followers_count", "num_followers"]),
            following: number(stats, &["following_count", "num_following"]),
            plays: number(stats, &["play_count__sum"]),
            songs,
            playlists: first_array(root, &["playlists", "user_playlists"])
                .into_iter()
                .flatten()
                .filter(|l| l.is_object())
                .map(crate::playlist::Playlist::read)
                .filter(|l| !l.id.is_empty())
                .collect(),
        }
    }

    pub fn name(&self) -> String {
        if self.display_name.trim().is_empty() { format!("@{}", self.handle) } else { format!("{} (@{})", self.display_name, self.handle) }
    }

    /// The list's title: who it is and the numbers that matter.
    pub fn summary(&self) -> String {
        tf("{0}: {1} songs, {2} followers", &[&self.name(), &self.total_songs, &self.followers])
    }

    pub fn info_lines(&self) -> Vec<String> {
        let mut lines = vec![self.name()];
        if !self.description.trim().is_empty() {
            lines.push(self.description.trim().to_string());
        }
        lines.push(tf("Songs: {0}", &[&self.total_songs]));
        lines.push(tf("Followers: {0}. Following: {1}.", &[&self.followers, &self.following]));
        if self.plays > 0 {
            lines.push(tf("Plays of all their songs: {0}", &[&group_digits(self.plays)]));
        }
        lines.push(tf("Link: {0}", &[&format!("https://suno.com/@{}", self.handle)]));
        lines
    }
}

/// One of Explore's shelves of songs.
#[derive(Debug, Clone)]
pub struct ExploreShelf {
    pub title: String,
    pub songs: Vec<SunoClip>,
}

/// POST /api/unified/explore: feeds[] with feed_title and items. An item's content_item is a song, or itself a
/// feed (content_type "generic_feed") whose items are the songs: that one becomes a shelf of its own, so its
/// title ("Staff Picks") isn't lost. A shelf with no songs of its own isn't listed.
pub fn read_explore(root: &Value) -> (Vec<ExploreShelf>, Option<String>) {
    fn is_feed(item: &Value, content: &Value) -> bool {
        let generic = |v: &Value| v.get("content_type").and_then(Value::as_str) == Some("generic_feed");
        generic(item) || generic(content) || (content.get("items").is_some_and(Value::is_array) && first_text(content, &["feed_id", "feed_title"]).is_some())
    }
    fn add(feed: &Value, shelves: &mut Vec<ExploreShelf>, depth: u8) {
        let title = first_text(feed, &["feed_title", "title", "name", "label", "display_name"])
            .or_else(|| first_text(feed, &["feed_id", "id", "identifier"]))
            .unwrap_or_else(|| "Explore".to_string());
        let mut songs: Vec<SunoClip> = Vec::new();
        let mut nested = Vec::new();
        match feed.get("items").and_then(Value::as_array) {
            Some(items) => {
                for item in items.iter().filter(|i| i.is_object()) {
                    let content = item.get("content_item").filter(|c| c.is_object()).unwrap_or(item);
                    if depth < 2 && is_feed(item, content) {
                        nested.push(content);
                    } else {
                        for song in clips_in(content) {
                            if !songs.iter().any(|s| s.id == song.id) {
                                songs.push(song);
                            }
                        }
                    }
                }
            }
            None => songs = clips_in(feed),
        }
        if !songs.is_empty() {
            shelves.push(ExploreShelf { title, songs });
        }
        for inner in nested {
            add(inner, shelves, depth + 1);
        }
    }
    let mut shelves = Vec::new();
    if let Some(feeds) = first_array(root, &["feeds", "sections"]) {
        for feed in feeds.iter().filter(|f| f.is_object()) {
            add(feed, &mut shelves, 0);
        }
    }
    (shelves, first_text(root, &["next_cursor"]))
}

/// A Suno contest (GET /api/contests/).
#[derive(Debug, Clone)]
pub struct Contest {
    pub title: String,
    pub description: String,
    pub ends: Option<chrono::DateTime<chrono::Utc>>,
    pub entries: i64,
    pub base_clip_id: Option<String>,
}

impl Contest {
    pub fn spoken_text(&self) -> String {
        let mut text = self.title.clone();
        if let Some(ends) = self.ends {
            let day = long_date(ends.with_timezone(&chrono::Local).date_naive());
            text += ", ";
            text += &if ends < chrono::Utc::now() { tf("ended {0}", &[&day]) } else { tf("ends {0}", &[&day]) };
        }
        if self.entries > 0 {
            text += ", ";
            text += &plural(self.entries, "{0} entry", "{0} entries");
        }
        if !self.description.trim().is_empty() {
            text += ". ";
            text += self.description.trim();
        }
        text
    }
}

/// GET /api/contests/: {contests: [{title, description, end_time or ends_at, submission_count, base_clip_id}]}.
pub fn read_contests(root: &Value) -> Vec<Contest> {
    let list = match root {
        Value::Array(list) => Some(list),
        _ => first_array(root, &["contests", "results", "items"]),
    };
    list.into_iter()
        .flatten()
        .filter(|c| c.is_object())
        .map(|c| Contest {
            title: first_text(c, &["title", "name", "display_name"]).unwrap_or_default(),
            description: first_text(c, &["description", "subtitle"]).unwrap_or_default(),
            ends: date(c, &["end_time", "ends_at", "end_date"]),
            entries: number(c, &["submission_count", "num_submissions", "entry_count"]),
            base_clip_id: first_text(c, &["base_clip_id", "clip_id"]),
        })
        .filter(|c| !c.title.is_empty())
        .collect()
}

/// A comment on a song.
#[derive(Debug, Clone, PartialEq)]
pub struct Comment {
    pub author: String,
    pub text: String,
    pub likes: i64,
}

impl Comment {
    /// "Ana: Great song (3 likes)".
    pub fn spoken_text(&self) -> String {
        let head = if self.author.is_empty() { t("Someone") } else { self.author.clone() };
        let likes = if self.likes > 0 { format!(" ({})", plural(self.likes, "{0} like", "{0} likes")) } else { String::new() };
        format!("{head}: {}{likes}", self.text)
    }
}

/// GET /api/gen/{id}/comments: {results: [...], next_cursor}. Field names vary, so several are tried.
pub fn read_comments(root: &Value) -> (Vec<Comment>, Option<String>) {
    let list = match root {
        Value::Array(list) => Some(list),
        _ => first_array(root, &["results", "comments", "items"]),
    };
    let comments = list
        .into_iter()
        .flatten()
        .filter(|c| c.is_object())
        .filter_map(|c| {
            let text = first_text(c, &["content", "text", "body"])?;
            let user = c.get("user").filter(|u| u.is_object()).unwrap_or(c);
            Some(Comment {
                author: first_text(user, &["display_name", "user_display_name", "handle", "user_handle"]).unwrap_or_default(),
                text,
                likes: number(c, &["num_likes", "like_count", "upvote_count"]),
            })
        })
        .collect();
    (comments, first_text(root, &["next_cursor", "nextCursor", "cursor"]))
}

/// A song this one credits (a cover's original, the parts of an extension): its title, who made it and how.
#[derive(Debug, Clone, PartialEq)]
pub struct CreditedSong {
    pub clip_id: Option<String>,
    pub title: String,
    pub creator: Option<String>,
    pub relationship: Option<String>,
    pub deleted: bool,
}

impl CreditedSong {
    pub fn spoken_text(&self) -> String {
        let mut parts = vec![self.title.clone()];
        if let Some(creator) = &self.creator {
            parts.push(tf("by {0}", &[creator]));
        }
        if let Some(how) = self.relationship.as_deref().filter(|r| !r.is_empty()) {
            parts.push(how.replace('_', " "));
        }
        if self.deleted {
            parts.push(t("deleted"));
        }
        parts.join(", ")
    }
}

/// GET /api/clips/{id}/attribution: source_clips with clip_id, title, user, relationship and is_deleted.
pub fn read_credits(root: &Value) -> Vec<CreditedSong> {
    first_array(root, &["source_clips"])
        .into_iter()
        .flatten()
        .filter(|s| s.is_object())
        .map(|s| {
            let user = s.get("user").filter(|u| u.is_object());
            CreditedSong {
                clip_id: first_text(s, &["clip_id"]),
                title: first_text(s, &["title"]).unwrap_or_else(|| t("Untitled")),
                creator: user.and_then(|u| first_text(u, &["user_display_name"]).or_else(|| first_text(u, &["user_handle"]).map(|h| format!("@{h}")))),
                relationship: first_text(s, &["relationship"]),
                deleted: s.get("is_deleted").and_then(Value::as_bool) == Some(true),
            }
        })
        .collect()
}

/// A notification: who did what. Unread ones say "New" first.
#[derive(Debug, Clone, PartialEq)]
pub struct Notification {
    pub title: String,
    pub message: String,
    pub is_read: bool,
}

impl Notification {
    pub fn spoken_text(&self) -> String {
        let text = match (self.title.is_empty(), self.message.is_empty()) {
            (true, _) => self.message.clone(),
            (false, true) => self.title.clone(),
            (false, false) => format!("{}: {}", self.title, self.message),
        };
        if self.is_read { text } else { tf("New: {0}", &[&text]) }
    }
}

/// notification/v3 gives the sentence in text[] pieces ("Ana", " liked ", "Rain"), with no title; v2 has a
/// title and a message.
pub fn read_notifications(root: &Value) -> Vec<Notification> {
    let list = match root {
        Value::Array(list) => Some(list),
        _ => first_array(root, &["notifications", "results", "items", "data"]),
    };
    list.into_iter()
        .flatten()
        .filter(|e| e.is_object())
        .map(|e| {
            let is_read = e.get("is_read").and_then(Value::as_bool) != Some(false);
            match e.get("text").and_then(Value::as_array) {
                Some(parts) => {
                    let joined: String = parts.iter().filter_map(|p| p.as_str().or_else(|| p.get("text").and_then(Value::as_str))).collect();
                    Notification { title: String::new(), message: joined.split_whitespace().collect::<Vec<_>>().join(" "), is_read }
                }
                None => Notification {
                    title: first_text(e, &["title", "heading", "type"]).unwrap_or_default(),
                    message: first_text(e, &["message", "body", "content", "text"]).unwrap_or_default(),
                    is_read,
                },
            }
        })
        .filter(|n| !n.title.is_empty() || !n.message.is_empty())
        .collect()
}

/// The id of the song another came from, from GET /api/clips/parent (shape not captured).
pub fn read_parent_id(root: &Value, child_id: &str) -> Option<String> {
    if let Some(clip) = clips_in(root).into_iter().find(|c| c.id != child_id) {
        return Some(clip.id);
    }
    if !root.is_object() {
        return None;
    }
    let id = first_text(root, &["parent_id", "parent_clip_id", "clip_id", "id"]).or_else(|| root.get("parent").and_then(|p| first_text(p, &["id", "clip_id"])));
    id.filter(|id| id != child_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn song(id: &str) -> Value {
        json!({ "id": id, "audio_url": format!("https://cdn1.suno.ai/{id}.mp3"), "title": id })
    }

    #[test]
    fn notifications_read_both_shapes() {
        let list = read_notifications(&json!({ "notifications": [ { "text": ["Ana", " liked  ", { "text": "Rain" }], "is_read": false }, { "title": "Hi", "message": "there" }, {} ] }));
        assert_eq!(list.iter().map(Notification::spoken_text).collect::<Vec<_>>(), vec!["New: Ana liked Rain", "Hi: there"]);
    }

    #[test]
    fn credits_say_who_and_how() {
        let credits = read_credits(&json!({ "source_clips": [ { "clip_id": "c", "title": "Rain", "user": { "user_handle": "ana" }, "relationship": "cover_of" }, { "is_deleted": true } ] }));
        assert_eq!(credits.iter().map(CreditedSong::spoken_text).collect::<Vec<_>>(), vec!["Rain, by @ana, cover of", "Untitled, deleted"]);
    }

    #[test]
    fn search_keeps_only_the_songs_section() {
        let root = json!({ "feed": { "items": [
            { "content_id": "omnisearch_top_result", "content_item": { "items": [ { "content_type": "clip", "content_item": song("top") } ] } },
            { "content_id": "omnisearch_songs", "content_item": { "feed_title": "Songs", "items": [ { "content_type": "clip", "content_item": song("a") }, { "content_type": "clip", "content_item": song("b") } ] } },
            { "content_id": "omnisearch_users", "content_item": { "items": [ { "content_item": { "handle": "ana", "clips": [song("u")] } } ] } }
        ] } });
        let ids: Vec<String> = read_omnisearch(&root).into_iter().map(|s| s.id).collect();
        assert_eq!(ids, vec!["a", "b"]);
        assert!(read_omnisearch(&json!({ "feed": { "items": [] } })).is_empty());
    }

    #[test]
    fn handles_come_from_links_and_names() {
        assert_eq!(creator_handle("https://suno.com/@ana?x=1"), "ana");
        assert_eq!(creator_handle("@ana"), "ana");
        assert_eq!(creator_handle(" ana "), "ana");
    }

    #[test]
    fn explore_keeps_nested_shelves_and_skips_empty_ones() {
        let root = json!({ "feeds": [
            { "feed_title": "Hot", "items": [ { "content_item": song("a") }, { "content_item": { "content_type": "generic_feed", "feed_title": "Staff Picks", "items": [ { "content_item": song("b") } ] } } ] },
            { "feed_title": "Shortcuts", "items": [ { "content_item": { "link": "x" } } ] }
        ], "next_cursor": "n" });
        let (shelves, next) = read_explore(&root);
        let titles: Vec<&str> = shelves.iter().map(|s| s.title.as_str()).collect();
        assert_eq!((titles, next.as_deref()), (vec!["Hot", "Staff Picks"], Some("n")));
        assert_eq!(shelves[1].songs[0].id, "b");
    }

    #[test]
    fn creators_contests_comments_and_parents_read() {
        let page = CreatorPage::read(&json!({ "handle": "ana", "display_name": "Ana", "num_total_clips": 2, "stats": { "followers_count": 5 }, "clips": [song("a"), song("b")] }), "x");
        assert_eq!((page.summary(), page.songs.len()), ("Ana (@ana): 2 songs, 5 followers".to_string(), 2));
        let contests = read_contests(&json!({ "contests": [ { "title": "Remix it", "submission_count": 3, "base_clip_id": "c" }, { "title": "" } ] }));
        assert_eq!((contests.len(), contests[0].spoken_text()), (1, "Remix it, 3 entries".to_string()));
        let (comments, _) = read_comments(&json!({ "results": [ { "content": "Great", "user": { "display_name": "Bo" }, "num_likes": 1 }, { "content": "" } ] }));
        assert_eq!(comments.iter().map(Comment::spoken_text).collect::<Vec<_>>(), vec!["Bo: Great (1 like)"]);
        assert_eq!(read_parent_id(&json!({ "parent_id": "p" }), "c").as_deref(), Some("p"));
        assert_eq!(read_parent_id(&json!({ "clip_id": "c" }), "c"), None);
    }
}
