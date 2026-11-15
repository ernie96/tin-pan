//! Suno's newer pages, as Suno's own site has them: Speech and Sounds (Library menu), your listen history and
//! Hooks (Discover menu, each opening as a list), playlists in the trash, and your Listen for Credits numbers.

use tinpan_core::create::{self, SunoModel};
use tinpan_core::i18n::{t, tf};
use tinpan_core::listen::Stats;
use tinpan_core::playlist::Playlist;
use wxdragon::prelude::*;

use crate::net::Command;
use crate::tool_form::ToolForm;
use crate::{App, dialogs, speech, with_app};

impl App {
    fn signed_in_or_say(&self) -> bool {
        if !self.signed_in {
            speech::say(&t("Not signed in."));
        }
        self.signed_in
    }

    // ── Speech and Sounds ───────────────────────────────────────────────

    /// Library menu, Make speech: words read aloud by a Suno voice, with music under them or not. A form that
    /// stays open, like Extend and Cover.
    pub(crate) fn make_speech(&mut self) {
        if !self.signed_in_or_say() {
            return;
        }
        let model = self.plan_models().into_iter().find(|m| m.external_key == create::SPEECH_MODEL);
        let form = ToolForm::new(&self.frame, &t("Make speech"), &t("Suno reads your words aloud in a voice, with music under them or not. It makes two versions, as with songs."));
        let script = form.text("_Words to speak", "", 8);
        let tone = form.text("_Tone (for example: warm, calm narrator)", "", 1);
        let music = form.check("With _music under the words", true);
        let voice = form.choice("_Voice", vec![t("Let Suno choose"), t("Male"), t("Female")], 0);
        form.show(&t("Make speech"), move || {
            with_app(|app| {
                let words = script.get_value().trim().to_string();
                if words.is_empty() {
                    speech::say(&t("Type the words to speak first."));
                    script.set_focus();
                    return;
                }
                let gender = match voice.get_selection() {
                    Some(1) => Some("m"),
                    Some(2) => Some("f"),
                    _ => None,
                };
                let request = create::speech_request(&words, &tone.get_value(), music.get_value(), gender);
                let model = model.clone().unwrap_or_else(|| SunoModel { external_key: create::SPEECH_MODEL.to_string(), ..SunoModel::default() });
                app.start_job(&t("Making speech."), request, &model);
            });
        });
    }

    /// Library menu, Make sounds: a short sound or loop from a description, with a tempo and key if you want.
    pub(crate) fn make_sounds(&mut self) {
        if !self.signed_in_or_say() {
            return;
        }
        let plan = self.plan_models();
        if plan.is_empty() {
            speech::say(&t("Models haven't loaded yet. Press F5 in the Create window (Ctrl+N), then try again."));
            return;
        }
        let models = create::sound_models(&plan);
        if models.is_empty() {
            speech::say(&tf("None of the models on your plan can make {0}.", &[&t("sounds")]));
            return;
        }
        let at = models.iter().position(|m| m.is_default).unwrap_or(0);
        let form = ToolForm::new(&self.frame, &t("Make sounds"), &t("Makes a sound effect, a sample or a loop from a description, such as rain on a tin roof or a funky drum loop."));
        let description = form.text("_Sound", "", 2);
        let kind = form.choice("_Type", vec![t("One-shot"), t("Loop")], 0);
        let tempo = form.text("_Beats a minute (empty lets Suno choose)", "", 1);
        let mut keys = vec![t("Let Suno choose")];
        keys.extend(create::SOUND_KEYS.iter().map(|k| k.to_string()));
        let key = form.choice("_Key", keys, 0);
        let model = form.choice("_Model", models.iter().map(SunoModel::display_name).collect(), at);
        form.show(&t("Make sounds"), move || {
            with_app(|app| {
                let text = description.get_value().trim().to_string();
                if text.is_empty() {
                    speech::say(&t("Describe the sound first."));
                    description.set_focus();
                    return;
                }
                let bpm = tempo.get_value().trim().to_string();
                let bpm = if bpm.is_empty() {
                    None
                } else {
                    match bpm.parse::<u32>().ok().filter(|b| (1..=300).contains(b)) {
                        Some(b) => Some(b),
                        None => {
                            speech::say(&tf("{0} must be between {1} and {2}.", &[&t("Beats a minute"), &1, &300]));
                            tempo.set_focus();
                            return;
                        }
                    }
                };
                let chosen_key = key.get_selection().and_then(|i| (i as usize).checked_sub(1)).map(|i| create::SOUND_KEYS[i]);
                let chosen = models[model.get_selection().unwrap_or(0) as usize].clone();
                let request = create::sound_request(&chosen.external_key, &text, kind.get_selection() == Some(1), bpm, chosen_key);
                app.start_job(&t("Making sounds."), request, &chosen);
            });
        });
    }

    // ── Lists ───────────────────────────────────────────────────────────

    /// Discover menu, Listen history: the songs you played on Suno, as a list in the Discover tab.
    pub(crate) fn listen_history(&mut self) {
        if self.signed_in_or_say() {
            self.net.send(Command::ListenHistory);
        }
    }

    /// Discover menu, Your hooks and Hooks you liked.
    pub(crate) fn hooks(&mut self, liked: bool) {
        if self.signed_in_or_say() {
            self.net.send(Command::Hooks { liked });
        }
    }

    // ── Playlists in the trash ──────────────────────────────────────────

    pub(crate) fn trashed_playlists(&mut self) {
        if self.signed_in_or_say() {
            self.net.send(Command::TrashedPlaylists);
        }
    }

    /// Enter on one brings it back out of the trash.
    pub(crate) fn trashed_playlists_loaded(&mut self, playlists: Vec<Playlist>) {
        if playlists.is_empty() {
            speech::say(&t("No playlists in the trash."));
            return;
        }
        let lines: Vec<String> = playlists.iter().map(Playlist::spoken_text).collect();
        let title = tf("Playlists in the trash ({0}). Enter restores one.", &[&playlists.len()]);
        if let Some(i) = dialogs::pick(&self.frame, &title, &lines) {
            let p = &playlists[i];
            self.net.send(Command::RestorePlaylist { playlist_id: p.id.clone(), name: p.display_name() });
        }
    }

    // ── Listen for Credits numbers ──────────────────────────────────────

    /// Account menu, Listen for credits stats.
    pub(crate) fn listen_stats(&mut self) {
        if self.signed_in_or_say() {
            self.net.send(Command::ListenStats);
        }
    }

    pub(crate) fn listen_stats_loaded(&self, stats: &Stats) {
        let lines = stats.lines();
        if lines.is_empty() {
            speech::say(&t("Suno has no Listen for Credits numbers for you yet."));
            return;
        }
        dialogs::show_lines(&self.frame, &t("Listen for credits stats"), &lines);
    }
}
