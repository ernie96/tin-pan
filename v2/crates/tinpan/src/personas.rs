//! Library menu: Personas (yours, loved or followed: their songs, source song, love, rename, public or private,
//! trash), Custom models (train one from a playlist of 6 to 100 of your songs, rename, archive) and Lyrics
//! mashup, as Tin Pan 1 has them. Lists are chosen with Enter; a song list opens in the Discover tab.

use tinpan_core::create::SunoModel;
use tinpan_core::i18n::{plural, t, tf};
use tinpan_core::personas::{ModelBase, PendingModel, PersonaDetails, PersonaList};
use tinpan_core::playlist::Playlist;

use crate::form::{self, Field, Value};
use crate::net::Command;
use crate::tool_form::ToolForm;
use crate::{App, dialogs, speech, with_app};

const LISTS: [(PersonaList, &str); 3] = [(PersonaList::Mine, "My personas"), (PersonaList::Loved, "Personas I love"), (PersonaList::Followed, "Personas I follow")];

/// What to do with a persona, said and run.
#[derive(Clone, Copy)]
enum PersonaAction {
    Songs,
    SourceSong,
    Love,
    Edit,
    Visibility,
    Trash,
}

impl App {
    /// Library menu, Personas: which list, then its personas.
    pub(crate) fn personas(&mut self) {
        if !self.signed_in {
            speech::say(&t("Not signed in."));
            return;
        }
        let names: Vec<String> = LISTS.iter().map(|(_, n)| t(n)).collect();
        if let Some(i) = dialogs::pick(&self.frame, &t("Personas. Enter opens a list."), &names) {
            self.net.send(Command::PersonaList { list: LISTS[i].0 });
        }
    }

    pub(crate) fn persona_list_loaded(&mut self, list: PersonaList, personas: Vec<PersonaDetails>) {
        if personas.is_empty() {
            speech::say(&if list == PersonaList::Mine { t("You have no personas yet. Make a persona, under Make from it, makes one from a song.") } else { t("None yet.") });
            return;
        }
        let name = LISTS.iter().find(|(l, _)| *l == list).map(|(_, n)| t(n)).unwrap_or_default();
        let lines: Vec<String> = personas.iter().map(PersonaDetails::spoken_text).collect();
        let Some(i) = dialogs::pick(&self.frame, &tf("{0}. Enter for what you can do with one.", &[&name]), &lines) else { return };
        self.persona_actions(personas[i].clone());
    }

    fn persona_actions(&mut self, p: PersonaDetails) {
        let mut actions = vec![(t("Songs made with it"), PersonaAction::Songs)];
        if p.root_clip_id.is_some() {
            actions.push((t("Its source song"), PersonaAction::SourceSong));
        }
        actions.push((if p.is_loved { t("Stop loving it") } else { t("Love it") }, PersonaAction::Love));
        if p.is_owned {
            actions.push((t("Rename and describe..."), PersonaAction::Edit));
            actions.push((if p.is_public { t("Make it private") } else { t("Make it public") }, PersonaAction::Visibility));
            actions.push((if p.is_trashed { t("Restore from the trash") } else { t("Move to the trash") }, PersonaAction::Trash));
        }
        let lines: Vec<String> = actions.iter().map(|(l, _)| l.clone()).collect();
        let Some(i) = dialogs::pick(&self.frame, &p.display_name(), &lines) else { return };
        let name = p.display_name();
        match actions[i].1 {
            PersonaAction::Songs => self.net.send(Command::PersonaSongs { id: p.id, name }),
            PersonaAction::SourceSong => {
                if let Some(root) = p.root_clip_id {
                    self.net.send(Command::PersonaSource { clip_id: root, name });
                }
            }
            PersonaAction::Love => self.net.send(Command::LovePersona { id: p.id, name }),
            PersonaAction::Edit => {
                let mut fields = [
                    Field { label: "_Name", value: Value::Text { value: p.name.clone(), multiline: false, max_len: 80 } },
                    Field { label: "_Description (the voice and style, for example)", value: Value::Text { value: p.description.clone(), multiline: false, max_len: 0 } },
                ];
                if form::show(&self.frame, &tf("Rename and describe {0}", &[&name]), "", &t("Save"), &mut fields) && !fields[0].text().is_empty() {
                    let new_name = fields[0].text();
                    self.net.send(Command::EditPersona { id: p.id, name: new_name.clone(), description: fields[1].text(), public: p.is_public, said: tf("Saved {0}.", &[&new_name]) });
                }
            }
            PersonaAction::Visibility => {
                let said = if p.is_public { tf("{0} is private now.", &[&name]) } else { tf("{0} is public now.", &[&name]) };
                self.net.send(Command::EditPersona { id: p.id, name: p.name, description: p.description, public: !p.is_public, said });
            }
            PersonaAction::Trash => {
                if !p.is_trashed && !dialogs::confirm(&self.frame, &tf("Move the persona \"{0}\" to the trash?", &[&name]), &t("Move to trash")) {
                    return;
                }
                let said = if p.is_trashed { tf("Restored {0} from the trash.", &[&name]) } else { tf("Moved {0} to trash.", &[&name]) };
                self.net.send(Command::TrashPersona { id: p.id, undo: p.is_trashed, said });
            }
        }
    }

    // ── Custom models ───────────────────────────────────────────────────

    fn custom_models_now(&self) -> Vec<SunoModel> {
        self.billing.as_ref().map(|b| b.models.iter().filter(|m| m.is_custom()).cloned().collect()).unwrap_or_default()
    }

    /// Library menu, Custom models: train a new one, or your trained ones and those still training.
    pub(crate) fn custom_models(&mut self) {
        if !self.signed_in {
            speech::say(&t("Not signed in."));
            return;
        }
        self.net.send(Command::PendingModels);
    }

    pub(crate) fn custom_models_loaded(&mut self, pending: Vec<PendingModel>) {
        let ready = self.custom_models_now();
        let mut lines = vec![t("Train a new custom model from a playlist...")];
        lines.extend(ready.iter().map(SunoModel::display_name));
        lines.extend(pending.iter().map(|m| tf("{0}, still training", &[&m.name])));
        let Some(i) = dialogs::pick(&self.frame, &t("Custom models. Enter for what you can do with one."), &lines) else { return };
        if i == 0 {
            self.net.send(Command::ModelBases);
        } else if let Some(model) = ready.get(i - 1).cloned() {
            self.custom_model_actions(model);
        } else {
            speech::say(&t("Suno is still training it; it shows up in the model list when it's ready."));
        }
    }

    fn custom_model_actions(&mut self, model: SunoModel) {
        let Some(id) = model.id.clone() else { return };
        let name = model.display_name();
        match dialogs::pick(&self.frame, &name, &[t("Rename..."), t("Archive it")]) {
            Some(0) => {
                let mut fields = [Field { label: "_Name, up to 16 characters", value: Value::Text { value: model.name.clone(), multiline: false, max_len: 16 } }];
                if form::show(&self.frame, &tf("Rename {0}", &[&name]), "", &t("Save"), &mut fields) && !fields[0].text().is_empty() {
                    let new_name = fields[0].text();
                    self.net.send(Command::RenameModel { id, name: new_name.clone(), said: tf("Saved {0}.", &[&new_name]) });
                }
            }
            Some(1) if dialogs::confirm(&self.frame, &tf("Archive the custom model \"{0}\"? It leaves your model list.", &[&name]), &t("Archive")) => {
                self.net.send(Command::ArchiveModel { id, said: tf("Archived {0}.", &[&name]) });
            }
            _ => {}
        }
    }

    /// Train a custom model: a playlist of your songs, a name and the model it starts from.
    pub(crate) fn train_model_form(&mut self, bases: Vec<ModelBase>) {
        let playlists: Vec<Playlist> = self.sections[crate::views::LIBRARY].views.iter().filter_map(|v| v.playlist.clone()).collect();
        if playlists.is_empty() {
            speech::say(&t("Put 6 to 100 of your songs in a playlist first; the model learns from them."));
            return;
        }
        let mut fields = vec![
            Field::choice("_Playlist with the songs to learn from", playlists.iter().map(Playlist::spoken_text).collect(), 0),
            Field { label: "_Name, up to 16 characters", value: Value::Text { value: String::new(), multiline: false, max_len: 16 } },
        ];
        if !bases.is_empty() {
            fields.push(Field::choice("_Base model", bases.iter().map(|b| b.name.clone()).collect(), bases.iter().position(|b| b.is_default).unwrap_or(0)));
        }
        let intro = t("Suno learns your sound from the finished songs you made in a playlist (6 to 100 of them), and the new model joins your model list when it's trained. Your plan has to include custom models.");
        if !form::show(&self.frame, &t("Train a custom model"), &intro, &t("Train"), &mut fields) {
            return;
        }
        let name = fields[1].text();
        if name.is_empty() {
            speech::say(&t("Give the model a name."));
            return;
        }
        let base = fields.get(2).and_then(|f| bases.get(f.selected())).map(|b| b.key.clone());
        let playlist = playlists[fields[0].selected()].id.clone();
        self.net.send(Command::TrainModel { playlist_id: playlist, name, base });
    }

    /// How many finished songs of yours the playlist gave; fewer than 6 isn't enough.
    pub(crate) fn too_few_songs(&self, count: usize) {
        speech::say(&plural(count as i64, "That playlist has {0} finished song of yours; the model needs at least 6.", "That playlist has {0} finished songs of yours; the model needs at least 6."));
    }

    // ── Lyrics mashup ───────────────────────────────────────────────────

    /// Suno writes one set of lyrics from two, in a form that stays open.
    pub(crate) fn lyrics_mashup(&mut self) {
        let form = ToolForm::new(&self.frame, &t("Lyrics mashup"), &t("Suno writes one set of lyrics from two. Paste or type both, then Mash up; the result appears in the Mashup box."));
        let first = form.text("_First lyrics", "", 6);
        let second = form.text("_Second lyrics", "", 6);
        let result = form.text("_Mashup (copy it from here)", "", 6);
        form.show(&t("Mash up"), move || {
            let (a, b) = (first.get_value().trim().to_string(), second.get_value().trim().to_string());
            if a.is_empty() || b.is_empty() {
                speech::say(&t("Put lyrics in both boxes first."));
                return;
            }
            with_app(|app| {
                app.form_text = Some(result);
                speech::say(&t("Mashing up the lyrics."));
                app.net.send(Command::Mashup { first: a, second: b });
            });
        });
    }

    pub(crate) fn mashup_done(&mut self, text: &str) {
        if let Some(target) = self.form_box() {
            target.set_value(&text.replace('\n', "\r\n"));
            speech::say(&t("The mashup is in the Mashup box."));
        }
    }
}
