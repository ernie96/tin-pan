//! Exports library songs and playlists to standard spreadsheet CSV or JSON format.

use std::fs::File;
use std::io::Write;
use std::path::Path;

use crate::clip::SunoClip;
use crate::error::{Error, Result};

/// Escapes a CSV field according to RFC 4180: wraps in double quotes if it contains commas, quotes or newlines,
/// and doubles internal double quotes.
fn escape_csv(val: &str) -> String {
    if val.contains(',') || val.contains('"') || val.contains('\n') || val.contains('\r') {
        format!("\"{}\"", val.replace('"', "\"\""))
    } else {
        val.to_string()
    }
}

/// Exports songs to CSV format with a UTF-8 BOM so Excel and spreadsheet apps display international text cleanly.
pub fn export_csv(songs: &[SunoClip], output_path: &Path) -> Result<()> {
    let mut file = File::create(output_path).map_err(|e| Error::api(e.to_string()))?;

    // UTF-8 BOM for Microsoft Excel / spreadsheet compatibility
    file.write_all(b"\xEF\xBB\xBF").map_err(|e| Error::api(e.to_string()))?;

    // Headers
    writeln!(
        file,
        "ID,Title,Status,Model,Duration (sec),Style,Lyrics,Plays,Likes,Public,Trashed,Created,Audio URL"
    )
    .map_err(|e| Error::api(e.to_string()))?;

    for song in songs {
        let id = escape_csv(&song.id);
        let title = escape_csv(&song.display_title());
        let status = escape_csv(song.status.as_deref().unwrap_or(""));
        let model = escape_csv(song.model_name.as_deref().unwrap_or(""));
        let dur = song.duration().map(|d| format!("{d:.1}")).unwrap_or_default();
        let style = escape_csv(song.metadata.as_ref().and_then(|m| m.tags.as_deref()).unwrap_or(""));
        let lyrics = escape_csv(song.lyrics().unwrap_or(""));
        let plays = song.play_count.to_string();
        let likes = song.upvote_count.to_string();
        let public = if song.is_public { "true" } else { "false" };
        let trashed = if song.is_trashed == Some(true) { "true" } else { "false" };
        let created = escape_csv(
            &song
                .created_at
                .map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_default(),
        );
        let audio_url = escape_csv(song.audio_url.as_deref().unwrap_or(""));

        writeln!(
            file,
            "{id},{title},{status},{model},{dur},{style},{lyrics},{plays},{likes},{public},{trashed},{created},{audio_url}"
        )
        .map_err(|e| Error::api(e.to_string()))?;
    }

    Ok(())
}

/// Exports songs as a formatted, human-readable JSON array.
pub fn export_json(songs: &[SunoClip], output_path: &Path) -> Result<()> {
    let text = serde_json::to_string_pretty(songs).map_err(|e| Error::api(e.to_string()))?;
    let mut file = File::create(output_path).map_err(|e| Error::api(e.to_string()))?;
    file.write_all(text.as_bytes()).map_err(|e| Error::api(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_csv_fields_properly() {
        assert_eq!(escape_csv("Hello, World"), "\"Hello, World\"");
        assert_eq!(escape_csv("He said \"Hello\""), "\"He said \"\"Hello\"\"\"");
        assert_eq!(escape_csv("Normal"), "Normal");
    }
}
