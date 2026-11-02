//! Library menu, Upload a song (Alt+U on the Library page), as in Tin Pan 1: choose a file, check its length
//! against the plan, name it and confirm the rights, then it's sent and appears at the top of your songs.

use std::path::{Path, PathBuf};

use tinpan_core::clip::format_time;
use tinpan_core::i18n::{t, tf};
use wxdragon::prelude::*;

use crate::form::{self, Field, Value};
use crate::net::Command;
use crate::{App, speech};

/// A song file's length in seconds, read with Tin Pan's own decoders; None when it can't be read as audio.
fn length_of(path: &Path) -> Option<Option<f64>> {
    let data = std::fs::read(path).ok()?;
    let source = crate::song_source::SongSource::new(data).ok()?;
    Some(rodio::Source::total_duration(&source).map(|d| d.as_secs_f64()))
}

impl App {
    pub(crate) fn upload_song(&mut self) {
        if !self.signed_in {
            speech::say(&t("Sign in from Settings first."));
            return;
        }
        let wildcard = format!("{} (*.mp3;*.wav;*.m4a;*.flac;*.ogg;*.aac;*.wma)|*.mp3;*.wav;*.m4a;*.flac;*.ogg;*.aac;*.wma", t("Audio files"));
        let picker = FileDialog::builder(&self.frame).with_message(&t("Choose a song to upload to Suno")).with_wildcard(&wildcard).with_style(FileDialogStyle::Open | FileDialogStyle::FileMustExist).build();
        if picker.show_modal() != ID_OK {
            return;
        }
        let Some(path) = picker.get_path().map(PathBuf::from) else { return };
        // Files Suno doesn't take as they are are converted first, on the way; their length is checked then.
        let as_is = path.extension().is_some_and(|e| tinpan_core::upload::EXTENSIONS.contains(&e.to_string_lossy().to_lowercase().as_str()));
        let length = if as_is {
            let Some(length) = length_of(&path) else {
                speech::say_error(&t("Tin Pan can't read that file as audio."));
                return;
            };
            length
        } else {
            None
        };
        // The plan's limits, when Suno says them and the file's length is known.
        if let (Some(seconds), Some(limits)) = (length, self.billing.as_ref().and_then(|b| b.audio_upload_limits)) {
            if let Some(min) = limits.min_seconds().filter(|m| seconds < *m) {
                speech::say_error(&tf("That song is {0}. Suno takes uploads of at least {1}.", &[&format_time(seconds), &format_time(min)]));
                return;
            }
            if let Some(max) = limits.max_seconds().filter(|m| seconds > *m) {
                speech::say_error(&tf("That song is {0}. Suno takes uploads of up to {1} on this account.", &[&format_time(seconds), &format_time(max)]));
                return;
            }
        }
        let name = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let file = path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let mut fields = [
            Field { label: "_Title", value: Value::Text { value: name.clone(), multiline: false, max_len: 100 } },
            Field { label: "_Lyrics (optional)", value: Value::Text { value: String::new(), multiline: true, max_len: 0 } },
            Field::check("I _own this audio or have the rights to upload it, and I accept Suno's terms for uploads (suno.com/terms)", false),
        ];
        let intro = tf(
            "Uploads {0} ({1}) to your Suno account, where it becomes a song you can play, extend, cover and remaster. MP3, M4A, WAV, FLAC, OGG and AAC go as they are; other files are converted to M4A first. Suno checks each upload and may refuse music it finds is copyrighted.",
            &[&file, &length.map(format_time).unwrap_or_else(|| "?".into())],
        );
        if !form::show(&self.frame, &t("Upload a song"), &intro, &t("Upload"), &mut fields) {
            return;
        }
        if !fields[2].checked() {
            speech::say_error(&t("Nothing was uploaded. Suno needs you to confirm the rights first."));
            return;
        }
        let title = Some(fields[0].text()).filter(|t| !t.is_empty()).unwrap_or(name);
        speech::say(&tf("Uploading {0}.", &[&title]));
        let limits = self.billing.as_ref().and_then(|b| b.audio_upload_limits);
        self.net.send(Command::UploadSong { path, title, lyrics: fields[1].text(), limits });
    }

    /// The new song goes to the top of your songs and is followed until Suno has it ready.
    pub(crate) fn song_uploaded(&mut self, clip: tinpan_core::clip::SunoClip, title: &str) {
        let clips = [clip];
        self.upsert_songs(&clips);
        self.watch(&clips);
        speech::say_quietly(&tf("Uploaded {0}. It's at the top of your library.", &[&title]));
    }
}
