//! The rest of Tin Pan 1's Song menu and playlist actions: Its stems, Credits and remix count, the creator's
//! playlists (like and save), Show the list of remixes, Cover picture, a playlist's cover and Make an album,
//! the Applications key menu of the lists box, and About this creator.

use std::path::PathBuf;

use tinpan_core::clip::SunoClip;
use tinpan_core::discover::{CreatorPage, CreditedSong};
use tinpan_core::i18n::{plural, t, tf};
use tinpan_core::playlist::Playlist;
use wxdragon::prelude::*;

use crate::commands::{self, Action};
use crate::form::{self, Field, Value};
use crate::net::{Command, CoverSource, PlaylistChange};
use crate::views::{LIBRARY, ViewKey};
use crate::{App, dialogs, speech};

const COVER_SOURCES: [&str; 3] = ["A picture from this computer", "A picture Suno draws from your description", "Suno's own cover again"];
const ALBUM_KINDS: [(&str, &str); 2] = [("lp", "Album"), ("ep", "EP")];

fn pick_picture(parent: &Frame) -> Option<PathBuf> {
    let wildcard = format!("{} (*.jpg;*.jpeg;*.png;*.webp)|*.jpg;*.jpeg;*.png;*.webp", t("Pictures"));
    let picker = FileDialog::builder(parent).with_message(&t("Choose a picture")).with_wildcard(&wildcard).with_style(FileDialogStyle::Open | FileDialogStyle::FileMustExist).build();
    (picker.show_modal() == ID_OK).then(|| picker.get_path().map(PathBuf::from)).flatten()
}

impl App {
    /// Related songs, Its stems: the stems Suno already made from the song.
    pub(crate) fn its_stems(&mut self) {
        self.with_song(|app, song| app.net.send(Command::ExistingStems { clip: Box::new(song) }));
    }

    /// Related songs, Credits and remix count: the songs it credits; Enter opens one.
    pub(crate) fn credits(&mut self) {
        self.with_song(|app, song| app.net.send(Command::Credits { clip: Box::new(song) }));
    }

    pub(crate) fn credits_loaded(&mut self, song: &SunoClip, sources: &[CreditedSong], count: Option<i64>, capped: bool) {
        let mut lines = vec![match count {
            Some(n) if capped => tf("Remixes and covers: {0} or more.", &[&n]),
            Some(n) => plural(n, "{0} remix or cover.", "{0} remixes and covers."),
            None => t("Suno didn't say how many remixes it has."),
        }];
        if sources.is_empty() {
            lines.push(t("It credits no other songs."));
        }
        lines.extend(sources.iter().map(CreditedSong::spoken_text));
        let Some(i) = dialogs::pick(&self.frame, &tf("Credits for {0}. Enter plays a credited song.", &[&song.display_title()]), &lines) else { return };
        if let Some(source) = i.checked_sub(1).and_then(|i| sources.get(i)).filter(|s| !s.deleted)
            && let Some(id) = &source.clip_id
        {
            self.net.send(Command::CreditedSong { clip_id: id.clone(), title: source.title.clone() });
        }
    }

    /// Related songs, Creator's playlists: their public playlists, to open, like or save.
    pub(crate) fn creator_playlists(&mut self) {
        self.with_song(|app, song| match song.handle.as_deref().map(str::trim).filter(|h| !h.is_empty()) {
            Some(handle) => app.net.send(Command::CreatorPlaylists { handle: handle.to_string() }),
            None => speech::say(&t("Suno didn't say who made this song.")),
        });
    }

    pub(crate) fn creator_playlists_loaded(&mut self, page: &CreatorPage) {
        if page.playlists.is_empty() {
            speech::say(&tf("{0} has no public playlists.", &[&page.name()]));
            return;
        }
        let lines: Vec<String> = page.playlists.iter().map(Playlist::spoken_text).collect();
        let Some(i) = dialogs::pick(&self.frame, &tf("Playlists of {0}. Enter for what you can do with one.", &[&page.name()]), &lines) else { return };
        let list = page.playlists[i].clone();
        let actions = [t("Its songs"), t("Like it"), t("Remove your like"), t("Save it to your library"), t("Remove it from your library")];
        let change = match dialogs::pick(&self.frame, &list.display_name(), &actions) {
            Some(0) => PlaylistChange::Songs,
            Some(1) => PlaylistChange::Like(true),
            Some(2) => PlaylistChange::Like(false),
            Some(3) => PlaylistChange::Save(true),
            Some(4) => PlaylistChange::Save(false),
            _ => return,
        };
        self.net.send(Command::OtherPlaylist { id: list.id.clone(), name: list.display_name(), change });
    }

    /// Sharing, Show or hide the list of remixes on the song's page.
    pub(crate) fn toggle_show_remixes(&mut self) {
        self.with_song(|app, song| {
            let show = song.metadata.as_ref().and_then(|m| m.show_remix) != Some(true);
            app.net.send(Command::ShowRemixes { clip_id: song.id, show });
        });
    }

    pub(crate) fn show_remixes_changed(&mut self, clip_id: &str, show: bool) {
        self.each_copy(clip_id, |s| s.metadata.get_or_insert_with(Default::default).show_remix = Some(show));
        self.selection_changed();
        speech::say(&if show { t("The song's page now lists its remixes.") } else { t("The song's page no longer lists its remixes.") });
    }

    /// Cover picture: from this computer, drawn by Suno from a description, or Suno's own again.
    pub(crate) fn cover_picture(&mut self) {
        self.with_song(|app, song| {
            let mut fields = [
                Field::choice("_New cover", COVER_SOURCES.iter().map(|s| t(s)).collect(), 0),
                Field { label: "_Description, for a picture Suno draws", value: Value::Text { value: String::new(), multiline: false, max_len: 0 } },
            ];
            if !form::show(&app.frame, &tf("Cover picture of {0}", &[&song.display_title()]), &t("Changes the picture Suno shows for this song."), &t("Change"), &mut fields) {
                return;
            }
            let source = match fields[0].selected() {
                0 => match pick_picture(&app.frame) {
                    Some(path) => {
                        speech::say(&t("Sending the picture."));
                        CoverSource::File(path)
                    }
                    None => return,
                },
                1 if fields[1].text().is_empty() => {
                    speech::say(&t("Describe the picture first."));
                    return;
                }
                1 => {
                    speech::say(&t("Suno is drawing the picture."));
                    CoverSource::Drawn(fields[1].text())
                }
                _ => CoverSource::SunosOwn,
            };
            let title = song.display_title();
            app.net.send(Command::SongCover { clip_id: song.id, title, source });
        });
    }

    pub(crate) fn song_covered(&mut self, clip_id: &str, url: Option<String>, said: &str) {
        if let Some(url) = url {
            self.each_copy(clip_id, |s| s.image_url = Some(url.clone()));
        }
        speech::say(said);
    }

    // ── A playlist's own actions ────────────────────────────────────────

    pub(crate) fn playlist_cover(&mut self) {
        let Some(p) = self.sections[LIBRARY].view().playlist.clone() else { return speech::say(&t("Choose a playlist first.")) };
        let Some(path) = pick_picture(&self.frame) else { return };
        speech::say(&t("Sending the picture."));
        let name = p.display_name();
        self.net.send(Command::PlaylistCover { id: p.id, name, path });
    }

    /// An album from the playlist's songs: made as a draft, then published if you choose.
    pub(crate) fn make_album(&mut self) {
        let Some(p) = self.sections[LIBRARY].view().playlist.clone() else { return speech::say(&t("Choose a playlist first.")) };
        let mut fields = [
            Field { label: "_Name", value: Value::Text { value: p.name.clone(), multiline: false, max_len: 80 } },
            Field { label: "_Description", value: Value::Text { value: p.description.clone(), multiline: true, max_len: 0 } },
            Field::choice("_Kind", ALBUM_KINDS.iter().map(|(_, l)| t(l)).collect(), 0),
            Field::check("_Publish it now (otherwise it stays a draft on Suno)", false),
        ];
        let intro = t("Makes an album on Suno with this playlist's songs. Your plan has to allow albums.");
        if !form::show(&self.frame, &tf("Make an album from {0}", &[&p.display_name()]), &intro, &t("Make album"), &mut fields) || fields[0].text().is_empty() {
            return;
        }
        let kind = ALBUM_KINDS[fields[2].selected()].0.to_string();
        self.net.send(Command::MakeAlbum { playlist_id: p.id, name: fields[0].text(), description: fields[1].text(), kind, publish: fields[3].checked() });
    }

    /// The Applications key in a lists box: what can be done with the list chosen there.
    pub(crate) fn lists_context_menu(&mut self) {
        let key = self.section().view().key.clone();
        let creator = self.section().view().creator.is_some();
        let menu = commands::lists_menu(|action| match action {
            Action::OpenList | Action::Refresh => true,
            Action::NewPlaylist => self.tab() == LIBRARY,
            Action::RenamePlaylist | Action::PlaylistCover | Action::MakeAlbum => matches!(key, ViewKey::Playlist(_)),
            Action::DeleteList => matches!(key, ViewKey::Playlist(_) | ViewKey::Result(_)),
            Action::CreatorInfo => creator,
            _ => false,
        });
        let mut menu = menu;
        self.section().selector.popup_menu(&mut menu, None);
    }

    /// About this creator: who made the list's songs, their numbers and their page.
    pub(crate) fn creator_info(&mut self) {
        match self.section().view().creator.clone() {
            Some(page) => dialogs::show_lines(&self.frame, &page.name(), &page.info_lines()),
            None => speech::say(&t("Open a creator's songs first: Creator's songs, in the Song menu, or Find a creator.")),
        }
    }
}
