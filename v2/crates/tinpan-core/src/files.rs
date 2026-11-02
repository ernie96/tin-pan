//! File names for downloads, as Tin Pan 1 makes them, so both versions see the same files.

const RESERVED: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6",
    "LPT7", "LPT8", "LPT9",
];
const KNOWN_EXTENSIONS: [&str; 10] = ["mp3", "m4a", "mp4", "wav", "ogg", "flac", "jpg", "jpeg", "png", "webp"];

/// "Title (1a2b3c4d).mp3". The short id keeps Suno's two same-titled takes from overwriting each other.
pub fn for_song(title: Option<&str>, id: &str, extension: &str) -> String {
    let short: String = id.chars().take(8).collect();
    format!("{} ({}).{}", clean(title.unwrap_or("")), clean(&short), extension.trim_start_matches('.'))
}

/// The file type in a download address ("m4a" for .../x.m4a?sig=...), or the fallback.
pub fn extension_of<'a>(url: Option<&'a str>, fallback: &'a str) -> String {
    let path = url.and_then(|u| u.split(['?', '#']).next()).unwrap_or("");
    let is_address = url.is_some_and(|u| u.contains("://"));
    let extension = path.rsplit('/').next().and_then(|name| name.rsplit_once('.')).map(|(_, e)| e.to_ascii_lowercase());
    match extension {
        Some(e) if is_address && KNOWN_EXTENSIONS.contains(&e.as_str()) => e,
        _ => fallback.to_string(),
    }
}

/// A name every system accepts: no characters Windows forbids, no trailing dots or spaces, no device names.
pub fn clean(text: &str) -> String {
    let replaced: String = text.trim().chars().map(|c| if r#"<>:"/\|?*"#.contains(c) || c.is_control() { '_' } else { c }).collect();
    let mut name = replaced.trim_end_matches(['.', ' ']).to_string();
    if name.chars().count() > 120 {
        name = name.chars().take(120).collect::<String>().trim_end_matches(['.', ' ']).to_string();
    }
    if name.is_empty() {
        name = "Untitled".into();
    }
    let stem = name.split('.').next().unwrap_or("");
    if RESERVED.iter().any(|r| r.eq_ignore_ascii_case(stem)) {
        name.insert(0, '_');
    }
    name
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_match_tin_pan_1s() {
        assert_eq!(for_song(Some("My: song?"), "1a2b3c4d-5e6f", "mp3"), "My_ song_ (1a2b3c4d).mp3");
        assert_eq!(for_song(None, "abc", ".m4a"), "Untitled (abc).m4a");
        assert_eq!(clean("con"), "_con");
        assert_eq!(clean("trailing. . "), "trailing");
    }

    #[test]
    fn extensions_come_from_the_address() {
        assert_eq!(extension_of(Some("https://cdn.x/1/clip/a.m4a?sig=1"), "mp3"), "m4a");
        assert_eq!(extension_of(Some("https://cdn.x/a.exe"), "mp3"), "mp3");
        assert_eq!(extension_of(None, "jpg"), "jpg");
    }
}
