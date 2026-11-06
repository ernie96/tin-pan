//! Suno links someone shared: the song a pasted link points to.

/// What a pasted link says about a song.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SongLink {
    /// suno.com/song/{id}, or the bare id.
    Id(String),
    /// suno.com/s/{code}, the short link Suno's Share button makes; Suno sends it on to the song's page.
    Short(String),
}

const HOSTS: [&str; 5] = ["suno.com", "www.suno.com", "app.suno.ai", "suno.ai", "www.suno.ai"];

fn is_song_id(text: &str) -> bool {
    let parts: Vec<&str> = text.split('-').collect();
    parts.iter().map(|p| p.len()).eq([8, 4, 4, 4, 12]) && parts.iter().all(|p| p.chars().all(|c| c.is_ascii_hexdigit()))
}

fn is_short_code(text: &str) -> bool {
    !text.is_empty() && text.len() <= 64 && text.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// The song in one word: an address with or without https://, a path Suno redirects to, or an id.
fn read_word(word: &str) -> Option<SongLink> {
    let word = word.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '/');
    if is_song_id(word) {
        return Some(SongLink::Id(word.to_ascii_lowercase()));
    }
    let word = word.split(['?', '#']).next().unwrap_or("");
    let path = match word.strip_prefix('/') {
        Some(path) => path,
        None => {
            let address = match word.split_once("://") {
                Some((scheme, rest)) if scheme.eq_ignore_ascii_case("https") || scheme.eq_ignore_ascii_case("http") => rest,
                Some(_) => return None,
                None => word,
            };
            let (host, path) = address.split_once('/')?;
            if !HOSTS.contains(&host.to_ascii_lowercase().as_str()) {
                return None;
            }
            path
        }
    };
    let mut parts = path.split('/').filter(|p| !p.is_empty());
    match (parts.next()?, parts.next()?) {
        ("song" | "embed", id) if is_song_id(id) => Some(SongLink::Id(id.to_ascii_lowercase())),
        ("s", code) if is_short_code(code) => Some(SongLink::Short(code.to_string())),
        _ => None,
    }
}

/// The first Suno song link in pasted text, which may be a whole message with the link in it.
pub fn read_song_link(text: &str) -> Option<SongLink> {
    text.split_whitespace().find_map(read_word)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "7df4c545-4b20-4170-96c2-e6c6c6340f36";

    #[test]
    fn full_links_and_ids_give_the_song() {
        let song = Some(SongLink::Id(ID.to_string()));
        for text in [
            format!("https://suno.com/song/{ID}"),
            format!("https://suno.com/song/{ID}?sh=abc"),
            format!("http://www.suno.com/song/{ID}/"),
            format!("suno.com/song/{ID}"),
            format!("https://app.suno.ai/song/{ID}"),
            format!("https://suno.com/embed/{ID}"),
            format!(" {ID} "),
            format!("/song/{ID}?sh=abc"),
            format!("HTTPS://SUNO.COM/song/{}", ID.to_ascii_uppercase()),
        ] {
            assert_eq!(read_song_link(&text), song, "{text}");
        }
    }

    #[test]
    fn short_links_keep_their_code() {
        assert_eq!(read_song_link("https://suno.com/s/1YkOYOQleTyk"), Some(SongLink::Short("1YkOYOQleTyk".to_string())));
        assert_eq!(read_song_link("suno.com/s/ab-c_1?x=1"), Some(SongLink::Short("ab-c_1".to_string())));
    }

    #[test]
    fn the_link_is_found_inside_a_message() {
        assert_eq!(read_song_link("Listen to this: <https://suno.com/s/abc123>, it's great!"), Some(SongLink::Short("abc123".to_string())));
        assert_eq!(read_song_link(&format!("(https://suno.com/song/{ID}).")), Some(SongLink::Id(ID.to_string())));
    }

    #[test]
    fn anything_else_is_not_a_song() {
        for text in [
            "",
            "hello there",
            "https://suno.com/",
            "/",
            "https://suno.com/@ana",
            "https://suno.com/playlist/7df4c545-4b20-4170-96c2-e6c6c6340f36",
            "https://suno.com/song/not-an-id",
            "https://evil.example/s/abc",
            "https://suno.com.evil.example/s/abc",
            "ftp://suno.com/s/abc",
            "https://suno.com/s/",
            "https://suno.com/s/a.b",
        ] {
            assert_eq!(read_song_link(text), None, "{text}");
        }
    }
}
