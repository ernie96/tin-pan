//! Downloads, as Tin Pan 1's: Download (Ctrl+D) in the format from Settings, Download MP3, WAV and video.
//! The free file comes first (the song's own file, decrypted when Suno keeps it encrypted); only when
//! that's refused is Suno's own download offered, saying whether it counts against the plan.

use std::path::{Path, PathBuf};

use tinpan_core::cleanup;
use tinpan_core::clip::SunoClip;
use tinpan_core::config::CleanupConfig;
use tinpan_core::drm;
use tinpan_core::files;
use tinpan_core::i18n::{plural, t, tf};

use crate::net::{Command, Extras};
use crate::{App, dialogs, speech};

/// The download folder: the one in Settings, or "Suno" in the Music folder, made if it isn't there.
pub fn folder(chosen: &str) -> Result<PathBuf, String> {
    let dir = if chosen.trim().is_empty() {
        dirs::audio_dir().or_else(|| dirs::home_dir().map(|h| h.join("Music"))).unwrap_or_default().join("Suno")
    } else {
        PathBuf::from(chosen.trim())
    };
    std::fs::create_dir_all(&dir).map_err(|_| t("Can't use the download folder. Check it in Settings."))?;
    Ok(dir)
}

/// Where a song is saved in this format: the free file's own type for "mp3" (an M4A, usually), keeping
/// a file that's already there under the other type, as Tin Pan 1 does.
fn target(clip: &SunoClip, format: &str, dir: &Path) -> PathBuf {
    let in_dir = |extension: &str| dir.join(files::for_song(clip.title.as_deref(), &clip.id, extension));
    match format {
        "wav" | "mp4" => in_dir(format),
        _ => {
            let (url, _) = drm::pick_media_url(clip, true);
            let extension = files::extension_of(Some(&url), "m4a");
            let first = in_dir(&extension);
            let other = in_dir(if extension == "m4a" { "mp3" } else { "m4a" });
            if !first.exists() && other.exists() { other } else { first }
        }
    }
}

/// When to clean up a download with sunscreen.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Cleanup {
    Ask,
    FromSettings,
    Never,
}

impl App {
    /// Download (Ctrl+D): the format chosen in Settings, asking every time whether to clean it up; on the
    /// free plan a WAV choice gives an MP3, as in Tin Pan 1.
    pub(crate) fn download(&mut self) {
        let wav = self.config.current.audio.output_format == "wav";
        if self.shown().marked_count() > 0 {
            let format = if wav && self.is_paid() != Some(false) { "wav" } else { "mp3" };
            return self.download_marked(format);
        }
        if wav && self.is_paid() == Some(false) {
            self.start_download("mp3", Cleanup::Ask, &(t("WAV needs Pro or Premier, so this is an MP3.") + " "));
        } else {
            self.start_download(if wav { "wav" } else { "mp3" }, Cleanup::Ask, "");
        }
    }

    /// Download MP3, WAV (only on a paid plan, or one not known yet) or video; Settings decides the clean-up.
    pub(crate) fn download_as(&mut self, format: &str) {
        if format == "wav" && self.is_paid() == Some(false) {
            speech::say(&t("WAV downloads need Pro or Premier."));
            return;
        }
        if format != "mp4" && self.shown().marked_count() > 0 {
            return self.download_marked(format);
        }
        self.start_download(format, if format == "mp4" { Cleanup::Never } else { Cleanup::FromSettings }, "");
    }

    /// The question Download asks every time: what clean-up does and what it costs in space.
    fn ask_to_clean(&self, clip: &SunoClip) -> bool {
        let keep = if self.config.current.cleanup.keep_original { t("The download is kept too.") } else { t("It replaces the download (Settings can keep both).") };
        let question = tf(
            "Clean up {0} too? sunscreen removes the haze and buzz in Suno songs and saves the result as a 24-bit WAV file, often 10 to 20 times bigger than the download. {1} It takes about 10 seconds for a 5-minute song. The clean-up steps are in Settings.",
            &[&clip.display_title(), &keep],
        );
        dialogs::confirm(&self.frame, &question, &t("Clean up"))
    }

    fn clean_options(&self, clean: bool) -> Option<CleanupConfig> {
        clean.then(|| self.config.current.cleanup.clone())
    }

    /// Clean up download (Ctrl+Shift+H): a song already in the download folder.
    pub(crate) fn clean_up_download(&mut self) {
        self.with_song(|app, clip| {
            let dir = match folder(&app.config.current.audio.download_dir) {
                Ok(dir) => dir,
                Err(message) => return speech::say(&message),
            };
            let downloaded = ["m4a", "mp3", "wav"].iter().map(|e| dir.join(files::for_song(clip.title.as_deref(), &clip.id, e))).find(|p| p.exists());
            match downloaded {
                Some(path) => {
                    speech::say_quietly(&tf("Cleaning up {0}.", &[&clip.display_title()]));
                    app.net.send(Command::Clean { title: clip.display_title(), path, options: app.config.current.cleanup.clone() });
                }
                None => speech::say(&tf("{0} isn't downloaded yet. Download it first, or use Download, which offers to clean it up.", &[&clip.display_title()])),
            }
        });
    }

    pub(crate) fn cleaned(&mut self, title: &str, result: Result<PathBuf, tinpan_core::Error>) {
        match result {
            Ok(_) => speech::say_quietly(&tf("Cleaned up {0}.", &[&title])),
            Err(e) => speech::say(&e.to_string()),
        }
    }

    /// Download all (Alt+L): every finished song in the list, through the free route only; songs already in
    /// the folder are skipped. Settings decides the clean-up.
    pub(crate) fn download_all(&mut self) {
        let mut format = self.config.current.audio.output_format.clone();
        if format == "wav" && self.is_paid() == Some(false) {
            format = "mp3".into();
        }
        let dir = match folder(&self.config.current.audio.download_dir) {
            Ok(dir) => dir,
            Err(message) => return speech::say(&message),
        };
        let todo: Vec<SunoClip> = self.shown().songs.iter().filter(|c| c.is_complete()).cloned().collect();
        if todo.is_empty() {
            return speech::say(&t("No finished songs in this list."));
        }
        self.download_songs(&todo, format, dir, &t("Download all"), &t("Every song in this list is already downloaded."));
    }

    /// Download, Download MP3 or Download WAV with songs marked: the marked songs that are finished, the way
    /// Download all does it (the free direct download, skipping songs already in the folder).
    fn download_marked(&mut self, format: &str) {
        let dir = match folder(&self.config.current.audio.download_dir) {
            Ok(dir) => dir,
            Err(message) => return speech::say(&message),
        };
        let todo: Vec<SunoClip> = self.shown().marked_songs().into_iter().filter(SunoClip::is_complete).collect();
        if todo.is_empty() {
            return speech::say(&t("None of the marked songs are finished."));
        }
        let ids: Vec<String> = todo.iter().map(|c| c.id.clone()).collect();
        if self.download_songs(&todo, format.to_string(), dir, &t("Download"), &t("Every marked song is already downloaded.")) {
            self.unmark(&ids);
        }
    }

    /// Asks, then downloads the songs not in `dir` yet. False when nothing was started.
    fn download_songs(&mut self, todo: &[SunoClip], format: String, dir: PathBuf, button: &str, already: &str) -> bool {
        let missing: Vec<(SunoClip, PathBuf)> = todo.iter().map(|c| (c.clone(), target(c, &format, &dir))).filter(|(_, path)| !path.exists()).collect();
        if missing.is_empty() {
            speech::say(already);
            return false;
        }
        let question = tf(
            "Download {0} as {1} into {2}? {3} already there will be skipped.",
            &[&plural(missing.len() as i64, "{0} song", "{0} songs"), &format.to_uppercase(), &dir.display(), &(todo.len() - missing.len())],
        );
        if !dialogs::confirm(&self.frame, &question, button) {
            return false;
        }
        let clean = self.config.current.cleanup.after_download;
        let mut said = tf("Downloading {0}.", &[&plural(missing.len() as i64, "{0} song", "{0} songs")]);
        if clean {
            said += " ";
            said += &t("Each one is cleaned up after it downloads.");
        }
        speech::say(&said);
        let extras = self.extras();
        self.net.send(Command::DownloadAll { songs: missing, format, dir, extras, clean: self.clean_options(clean) });
        true
    }

    fn is_paid(&self) -> Option<bool> {
        self.billing.as_ref().and_then(tinpan_core::billing::BillingInfo::is_paid)
    }

    fn start_download(&mut self, format: &str, cleanup: Cleanup, note: &str) {
        let note = note.to_string();
        self.with_song(|app, clip| {
            if !clip.is_complete() {
                speech::say(&if clip.has_failed() { t("That song failed to generate.") } else { t("That song isn't finished yet.") });
                return;
            }
            let dir = match folder(&app.config.current.audio.download_dir) {
                Ok(dir) => dir,
                Err(message) => {
                    speech::say(&message);
                    return;
                }
            };
            let path = target(&clip, format, &dir);
            if path.exists() {
                // Download offers to clean up a song downloaded before but never cleaned up.
                if cleanup == Cleanup::Ask && format != "mp4" && !cleanup::cleaned_path(&path).exists() && app.ask_to_clean(&clip) {
                    speech::say_quietly(&tf("Cleaning up {0}.", &[&clip.display_title()]));
                    app.net.send(Command::Clean { title: clip.display_title(), path, options: app.config.current.cleanup.clone() });
                } else {
                    speech::say(&tf("{0} is already downloaded.", &[&clip.display_title()]));
                }
                return;
            }
            let clean = match cleanup {
                Cleanup::Ask => app.ask_to_clean(&clip),
                Cleanup::FromSettings => app.config.current.cleanup.after_download,
                Cleanup::Never => false,
            };
            let doing = if format == "wav" { tf("Asking Suno for a WAV of {0}.", &[&clip.display_title()]) } else { tf("Downloading {0}.", &[&clip.display_title()]) };
            speech::say(&(note.clone() + &doing));
            let extras = app.extras();
            let clean = app.clean_options(clean);
            app.net.send(Command::Download { clip: Box::new(clip), format: format.to_string(), path, extras, clean });
        });
    }

    fn extras(&self) -> Extras {
        let audio = &self.config.current.audio;
        Extras { lyrics: audio.save_lyrics, timed_lyrics: audio.save_timed_lyrics, details: audio.save_details, cover: audio.save_cover }
    }

    pub(crate) fn downloaded(&mut self, clip: &SunoClip, through_suno: bool) {
        if through_suno {
            // Downloading it again is free now.
            self.each_copy(&clip.id, |song| song.is_download_unlocked = Some(true));
            // One fewer download left.
            self.net.send(Command::Account { say: None });
        }
        speech::say_quietly(&tf("Saved {0}.", &[&clip.display_title()]));
    }

    /// The free file was refused: offers Suno's own download, saying what it costs, as Tin Pan 1 does.
    pub(crate) fn download_failed(&mut self, clip: SunoClip, format: String, path: PathBuf, error: &str, offer: bool, clean: Option<CleanupConfig>) {
        if !offer {
            speech::say(error);
            return;
        }
        let left = self.billing.as_ref().and_then(|b| b.download_usage.as_ref()).and_then(tinpan_core::billing::DownloadUsage::remaining);
        let cost = match (clip.is_download_unlocked == Some(true), left) {
            (true, _) => t("You've downloaded this song before, so it won't count against your downloads."),
            (false, Some(left)) => tf("Suno counts it as 1 of your {0} downloads left.", &[&left]),
            (false, None) => t("Suno counts it against your downloads."),
        };
        let problem = tf("The free file didn't download: {0}", &[&error]);
        let question = tf("{0} Use Suno's own download for \"{1}\" instead? {2}", &[&problem, &clip.display_title(), &cost]);
        if !dialogs::confirm(&self.frame, &question, &t("Download")) {
            return;
        }
        // Suno's own download is the format asked for (an MP3, say), whatever the free file was.
        let path = path.with_extension(format.as_str());
        if path.exists() {
            speech::say(&tf("{0} is already downloaded.", &[&clip.display_title()]));
            return;
        }
        speech::say(&tf("Downloading {0} through Suno.", &[&clip.display_title()]));
        let extras = self.extras();
        let studio = self.billing.as_ref().is_some_and(tinpan_core::billing::BillingInfo::has_studio);
        self.net.send(Command::DownloadThroughSuno { clip: Box::new(clip), format, path, extras, studio, clean });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_song_saves_under_its_free_files_type_and_keeps_an_old_download() {
        let dir = std::env::temp_dir().join(format!("tinpan-dl-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let clip = SunoClip::from_value(&serde_json::json!({
            "id": "1a2b3c4d-x", "title": "Song", "status": "complete",
            "media_urls": [{"url": "https://cdn.example.com/1/clip/1a2b3c4d.m4a", "encoding": "m4a-opus"}]
        }))
        .unwrap();
        assert_eq!(target(&clip, "mp3", &dir).file_name().unwrap(), "Song (1a2b3c4d).m4a");
        assert_eq!(target(&clip, "wav", &dir).file_name().unwrap(), "Song (1a2b3c4d).wav");
        let old = dir.join("Song (1a2b3c4d).mp3");
        std::fs::write(&old, b"x").unwrap();
        assert_eq!(target(&clip, "mp3", &dir), old);
        let _ = std::fs::remove_file(old);
        let _ = std::fs::remove_dir(dir);
    }
}
