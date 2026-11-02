//! Reading a song's timed lyrics aloud, line by line, as they're sung (Shift+F3, off to start with), and the
//! playing-song cue in song lists (a click, "playing", both, or nothing), as in Tin Pan 1.

use tinpan_core::i18n::t;
use tinpan_core::lyrics::{AlignedWord, LyricsCursor, lines_from};

use crate::net::Command;
use crate::{App, speech, views};

/// How often the lyrics look at where the song is.
pub const LIVE_EVERY_MS: i32 = 100;

/// Tin Pan 1's cue modes, kept under its names in the settings.
pub const CUES: [(&str, &str); 4] = [
    ("Click", "A short click"),
    ("Say", "Say playing (also shown in braille)"),
    ("ClickAndSay", "Click and say playing"),
    ("Off", "Nothing"),
];

pub fn cue_clicks(mode: &str) -> bool {
    matches!(mode, "Click" | "ClickAndSay")
}

pub fn cue_says(mode: &str) -> bool {
    matches!(mode, "Say" | "ClickAndSay")
}

impl App {
    /// Shift+F3, and Settings' Live lyrics box.
    pub(crate) fn toggle_live_lyrics(&mut self) {
        let on = !self.config.current.audio.read_lyrics_live;
        self.set_live_lyrics(on);
        self.settings_show_live_lyrics();
        if on {
            speech::say_error(&t("Lyrics will be read as they're sung."));
        } else {
            speech::say_error(&t("Lyrics won't be read as they're sung."));
        }
    }

    pub(crate) fn set_live_lyrics(&mut self, on: bool) {
        self.config.current.audio.read_lyrics_live = on;
        self.save_settings();
        self.live_cursor = None;
        self.live_for = None;
        if on {
            self.load_live_lyrics(true);
        }
    }

    /// The playing song's timed lyrics, asked for when a song starts with live lyrics on. `say_none` says when
    /// the song has none (only when asked for with Shift+F3, as in Tin Pan 1).
    pub(crate) fn load_live_lyrics(&mut self, say_none: bool) {
        if !self.config.current.audio.read_lyrics_live {
            return;
        }
        let Some(clip) = &self.playing else { return };
        if self.live_for.as_deref() == Some(clip.id.as_str()) {
            return;
        }
        self.live_for = Some(clip.id.clone());
        self.live_cursor = None;
        self.live_say_none = say_none;
        self.net.send(Command::TimedLyrics { clip_id: clip.id.clone() });
    }

    pub(crate) fn live_lyrics_loaded(&mut self, clip_id: &str, words: &[AlignedWord]) {
        // A song that's no longer playing, or live lyrics switched off meanwhile.
        if self.live_for.as_deref() != Some(clip_id) || !self.config.current.audio.read_lyrics_live {
            return;
        }
        let cursor = LyricsCursor::new(lines_from(words));
        if cursor.is_empty() && self.live_say_none {
            speech::say_quietly(&t("This song has no timed lyrics."));
        }
        self.live_cursor = Some(cursor);
    }

    /// Every tenth of a second: the line being sung, said once it starts.
    pub(crate) fn live_tick(&mut self) {
        let (Some(cursor), Some(playing)) = (&mut self.live_cursor, &self.playing) else { return };
        if self.live_for.as_deref() != Some(playing.id.as_str()) {
            return;
        }
        if let Some(line) = cursor.advance(self.player.position().as_secs_f64()) {
            speech::say_error(&line);
        }
    }

    // ── The playing-song cue ────────────────────────────────────────────

    /// The song lists say "playing" after the playing song, when Settings asks for it.
    pub(crate) fn show_playing_cue(&self) {
        let says = cue_says(&self.config.current.playing_cue);
        views::set_cue(self.playing.as_ref().map(|p| p.id.clone()), says);
        self.refresh_rows(quiet_list::Select::Keep);
    }

    /// The arrow keys reached the playing song: a click, when Settings asks for one.
    pub(crate) fn cue_selection(&self) {
        if cue_clicks(&self.config.current.playing_cue)
            && let (Some(playing), Some(selected)) = (&self.playing, self.list().selected())
            && playing.id == selected
        {
            self.player.click();
        }
    }
}
