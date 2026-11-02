//! Tabs for Suno's sections, and in each a list of views on the left (as in fedra) with the chosen view's
//! songs on the right. The Library tab's views are your songs and each of your playlists; the Discover tab's
//! are Trending, From people you follow, Explore and its shelves, Contests, and the lists opened from a song
//! (discover.rs). A song carries on through the view it started from.

use quiet_list::{QuietList, Select};
use std::collections::HashMap;

use tinpan_core::clip::SunoClip;
use tinpan_core::i18n::{plural, t, tf};
use tinpan_core::playlist::Playlist;
use wxdragon::prelude::*;

use crate::net::Command;
use crate::{App, dialogs, form, speech};

pub const LIBRARY: usize = 0;
pub const DISCOVER: usize = 1;
/// Pages of their own, with no lists of songs.
pub const ACCOUNT: usize = 2;
pub const SETTINGS: usize = 3;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ViewKey {
    /// Your songs, with the search and the filters.
    Library,
    Playlist(String),
    Trending,
    Following,
    /// Every song on Explore's shelves; each shelf is a view of its own after it.
    Explore,
    Shelf(String),
    Contests,
    /// A list opened from a song or a creator (Similar songs and the like), until Delete closes it.
    Result(u32),
}

thread_local! {
    /// The playing song, and whether lists say "playing" after it (Settings, When a list reaches the playing song).
    static CUE: std::cell::RefCell<(Option<String>, bool)> = const { std::cell::RefCell::new((None, false)) };
}

pub fn set_cue(playing: Option<String>, says: bool) {
    CUE.set((playing, says));
}

pub struct View {
    pub key: ViewKey,
    pub title: String,
    pub songs: Vec<SunoClip>,
    pub loaded: bool,
    /// Lists of other people's songs say who made each one.
    pub with_creator: bool,
    /// For a playlist: its own details.
    pub playlist: Option<Playlist>,
    /// Rows said differently from the song (a contest's own text), by song id.
    pub labels: HashMap<String, String>,
    /// For a creator's songs: who they are, for About this creator.
    pub creator: Option<tinpan_core::discover::CreatorPage>,
}

impl View {
    pub fn new(key: ViewKey, title: String) -> Self {
        let with_creator = !matches!(key, ViewKey::Library | ViewKey::Playlist(_));
        Self { key, title, songs: Vec::new(), loaded: false, with_creator, playlist: None, labels: HashMap::new(), creator: None }
    }

    fn for_playlist(playlist: Playlist) -> Self {
        let mut view = Self::new(ViewKey::Playlist(playlist.id.clone()), playlist.spoken_text());
        view.playlist = Some(playlist);
        view
    }

    pub fn rows(&self) -> Vec<(String, String)> {
        self.songs
            .iter()
            .map(|s| {
                let mut label = match self.labels.get(&s.id) {
                    Some(label) => label.clone(),
                    None if self.with_creator => s.spoken_with_creator(),
                    None => s.spoken_text(),
                };
                if CUE.with_borrow(|(playing, says)| *says && playing.as_deref() == Some(s.id.as_str())) {
                    label = format!("{label}, {}", t("playing"));
                }
                (s.id.clone(), label)
            })
            .collect()
    }
}

/// One tab: its views list and its song list.
pub struct Section {
    pub views: Vec<View>,
    pub shown: usize,
    pub selector: ListBox,
    pub list: QuietList<String>,
}

impl Section {
    pub fn view(&self) -> &View {
        &self.views[self.shown.min(self.views.len() - 1)]
    }

    pub fn show_titles(&self) {
        self.selector.clear();
        for view in &self.views {
            self.selector.append(&view.title);
        }
        self.selector.set_selection(self.shown as u32, true);
    }

    pub fn refresh(&self, select: Select<String>) {
        self.list.update_labelled(self.view().rows(), select);
    }
}

impl App {
    /// The song tab in front; on the Account and Settings tabs, the Library's (the Song menu acts on its song).
    pub(crate) fn tab(&self) -> usize {
        usize::try_from(self.notebook.selection()).ok().filter(|t| *t < self.sections.len()).unwrap_or(LIBRARY)
    }

    pub(crate) fn section(&self) -> &Section {
        &self.sections[self.tab()]
    }

    pub(crate) fn list(&self) -> &QuietList<String> {
        &self.section().list
    }

    /// The view on screen.
    pub(crate) fn shown(&self) -> &View {
        self.section().view()
    }

    pub(crate) fn library(&mut self) -> &mut View {
        &mut self.sections[LIBRARY].views[0]
    }

    pub(crate) fn find_view(&self, key: &ViewKey) -> Option<&View> {
        self.sections.iter().flat_map(|s| s.views.iter()).find(|v| v.key == *key)
    }

    pub(crate) fn find_view_mut(&mut self, key: &ViewKey) -> Option<&mut View> {
        self.sections.iter_mut().flat_map(|s| s.views.iter_mut()).find(|v| v.key == *key)
    }

    /// The songs of the view the playing song started from.
    pub(crate) fn queue(&self) -> &[SunoClip] {
        self.find_view(&self.playing_from).map_or(&[], |v| v.songs.as_slice())
    }

    /// A song in any view, by id.
    pub(crate) fn find_song(&self, id: &str) -> Option<SunoClip> {
        self.sections.iter().flat_map(|s| s.views.iter()).flat_map(|v| v.songs.iter()).find(|s| s.id == id).cloned()
    }

    /// Changes a song wherever it's shown.
    pub(crate) fn each_copy(&mut self, id: &str, mut change: impl FnMut(&mut SunoClip)) {
        for song in self.sections.iter_mut().flat_map(|s| s.views.iter_mut()).flat_map(|v| v.songs.iter_mut()).filter(|s| s.id == id) {
            change(song);
        }
    }

    pub(crate) fn remove_everywhere(&mut self, id: &str) {
        for view in self.sections.iter_mut().flat_map(|s| s.views.iter_mut()) {
            view.songs.retain(|s| s.id != id);
        }
    }

    /// Shows every tab's songs again after a change, keeping the place.
    pub(crate) fn refresh_rows(&self, select: Select<String>) {
        for section in &self.sections {
            section.refresh(select.clone());
        }
    }

    /// Ctrl+1, Ctrl+2: a tab, with focus on its song list.
    pub(crate) fn show_tab(&mut self, tab: usize) {
        self.notebook.set_selection(tab);
        match tab {
            ACCOUNT => self.account.focus(),
            SETTINGS => self.settings.focus(),
            _ => self.sections[tab].list.set_focus(),
        }
        // The page-changed event comes while the app is busy with this, so it's done here.
        self.tab_shown(tab);
    }

    /// A view was chosen in a tab's views list. One already loaded shows at once; another loads after a
    /// moment, so arrowing past playlists doesn't fetch each one (as in Tin Pan 1).
    pub(crate) fn view_chosen(&mut self, tab: usize, index: usize, now: bool) {
        let section = &mut self.sections[tab];
        if index >= section.views.len() {
            return;
        }
        section.shown = index;
        section.refresh(Select::Start);
        if !section.view().loaded {
            if now {
                self.load_view(tab, true);
            } else {
                self.pending_view = Some(tab);
                self.view_timer.start(400, true);
            }
        }
    }

    /// The moment after arrowing onto a view has passed: load it if it's still the one chosen.
    pub(crate) fn load_pending_view(&mut self) {
        if let Some(tab) = self.pending_view.take()
            && !self.sections[tab].view().loaded
        {
            self.load_view(tab, false);
        }
    }

    /// Loads the tab's chosen view; `announce` says what came, as Enter on it does.
    pub(crate) fn load_view(&mut self, tab: usize, announce: bool) {
        if !self.signed_in {
            return;
        }
        match self.sections[tab].view().key.clone() {
            ViewKey::Library => self.load_library(!announce),
            ViewKey::Playlist(id) => self.net.send(Command::PlaylistSongs { playlist_id: id, announce }),
            ViewKey::Trending => self.net.send(Command::Trending { announce }),
            ViewKey::Following => self.net.send(Command::Following { announce }),
            ViewKey::Explore | ViewKey::Shelf(_) => self.net.send(Command::Explore { announce }),
            ViewKey::Contests => self.net.send(Command::Contests { announce }),
            // Made with its songs; there's nothing to load again.
            ViewKey::Result(_) => {}
        }
    }

    /// Enter in a views list: its songs, loaded now if they aren't yet.
    pub(crate) fn open_chosen_view(&mut self) {
        let tab = self.tab();
        if self.sections[tab].view().loaded {
            self.sections[tab].list.set_focus();
            let view = self.sections[tab].view();
            speech::say(&self.view_count_text(view));
        } else {
            self.load_view(tab, true);
            self.sections[tab].list.set_focus();
        }
    }

    pub(crate) fn view_count_text(&self, view: &View) -> String {
        match (&view.key, view.songs.len()) {
            (ViewKey::Trending, 0) => t("Nothing trending came back."),
            (ViewKey::Trending, n) => plural(n as i64, "{0} trending song.", "{0} trending songs."),
            (ViewKey::Following, 0) => t("Nothing new from the people you follow."),
            (ViewKey::Explore | ViewKey::Shelf(_), 0) => t("Explore has nothing to show right now."),
            (ViewKey::Contests, 0) => t("Suno has no contests right now."),
            (_, 0) => tf("{0} is empty.", &[&view.title_name()]),
            (_, n) => format!("{}, {}", view.title_name(), plural(n as i64, "{0} song.", "{0} songs.")),
        }
    }

    // ── Playlists ───────────────────────────────────────────────────────

    /// Your playlists came: each becomes a view in the Library tab, after your songs; the one shown stays.
    pub(crate) fn playlists_loaded(&mut self, playlists: Vec<Playlist>) {
        let section = &mut self.sections[LIBRARY];
        let shown_key = section.view().key.clone();
        let mut views: Vec<View> = std::mem::take(&mut section.views);
        let library = views.remove(0);
        let mut old: Vec<View> = views;
        let mut fresh = vec![library];
        for playlist in playlists {
            // A playlist already loaded keeps its songs.
            let view = match old.iter().position(|v| v.key == ViewKey::Playlist(playlist.id.clone())) {
                Some(at) => {
                    let mut kept = old.remove(at);
                    kept.title = playlist.spoken_text();
                    kept.playlist = Some(playlist);
                    kept
                }
                None => View::for_playlist(playlist),
            };
            fresh.push(view);
        }
        section.views = fresh;
        section.shown = section.views.iter().position(|v| v.key == shown_key).unwrap_or(0);
        section.show_titles();
        section.refresh(Select::Keep);
    }

    pub(crate) fn playlist_songs_loaded(&mut self, playlist_id: &str, songs: Vec<SunoClip>, announce: bool) {
        let key = ViewKey::Playlist(playlist_id.to_string());
        let Some(view) = self.find_view_mut(&key) else { return };
        view.songs = songs;
        view.loaded = true;
        if let Some(p) = view.playlist.as_mut() {
            p.song_count = Some(view.songs.len() as i64);
            view.title = p.spoken_text();
        }
        let text = announce.then(|| self.find_view(&key).map(|v| self.view_count_text(v))).flatten();
        self.sections[LIBRARY].show_titles();
        if self.sections[LIBRARY].view().key == key {
            self.sections[LIBRARY].refresh(Select::Start);
        }
        if let Some(text) = text {
            speech::say(&text);
        }
    }

    pub(crate) fn trending_loaded(&mut self, songs: Vec<SunoClip>, announce: bool) {
        let Some(view) = self.find_view_mut(&ViewKey::Trending) else { return };
        view.songs = songs;
        view.loaded = true;
        let text = self.find_view(&ViewKey::Trending).map(|v| self.view_count_text(v));
        self.sections[DISCOVER].refresh(Select::Start);
        if announce && let Some(text) = text {
            speech::say(&text);
        }
    }

    /// The playlist on screen in the Library tab, if a playlist is.
    fn chosen_playlist(&self) -> Option<Playlist> {
        self.sections[LIBRARY].view().playlist.clone()
    }

    pub(crate) fn new_playlist(&mut self) {
        let mut fields = [form::Field { label: "_Name", value: form::Value::Text { value: String::new(), multiline: false, max_len: 100 } }];
        if form::show(&self.frame, &t("New playlist"), "", &t("Make playlist"), &mut fields) && !fields[0].text().is_empty() {
            self.net.send(Command::NewPlaylist { name: fields[0].text() });
        }
    }

    pub(crate) fn rename_playlist(&mut self) {
        let Some(p) = self.chosen_playlist() else { return speech::say(&t("Choose a playlist first.")) };
        let mut fields = [
            form::Field { label: "_Name", value: form::Value::Text { value: p.name.clone(), multiline: false, max_len: 100 } },
            form::Field { label: "_Description", value: form::Value::Text { value: p.description.clone(), multiline: true, max_len: 0 } },
        ];
        if form::show(&self.frame, &tf("Rename {0}", &[&p.display_name()]), "", &t("Save"), &mut fields) && !fields[0].text().is_empty() {
            self.net.send(Command::RenamePlaylist { playlist_id: p.id, name: fields[0].text(), description: fields[1].text() });
        }
    }

    pub(crate) fn delete_playlist(&mut self) {
        let Some(p) = self.chosen_playlist() else { return speech::say(&t("Choose a playlist first.")) };
        let question = tf("Move the playlist \"{0}\" to the trash? The songs in it stay in your library.", &[&p.display_name()]);
        if dialogs::confirm(&self.frame, &question, &t("Delete playlist")) {
            let name = p.display_name();
            self.net.send(Command::TrashPlaylist { playlist_id: p.id, name });
        }
    }

    /// Ctrl+P: the selected song into one of your playlists, or a new one, as in Tin Pan 1.
    pub(crate) fn add_to_playlist(&mut self) {
        self.with_song(|app, song| {
            let playlists: Vec<Playlist> = app.sections[LIBRARY].views.iter().filter_map(|v| v.playlist.clone()).collect();
            let mut choices = vec![t("New playlist")];
            choices.extend(playlists.iter().map(Playlist::spoken_text));
            let mut fields = [
                form::Field::choice("_Playlist", choices, usize::from(!playlists.is_empty())),
                form::Field { label: "Name for a _new playlist", value: form::Value::Text { value: String::new(), multiline: false, max_len: 100 } },
            ];
            let title = tf("Add {0} to a playlist", &[&song.display_title()]);
            if !form::show(&app.frame, &title, &t("Pick a playlist, or New playlist and give it a name."), &t("Add"), &mut fields) {
                return;
            }
            let target = fields[0].selected().checked_sub(1).and_then(|i| playlists.get(i)).cloned();
            let new_name = fields[1].text();
            if target.is_none() && new_name.is_empty() {
                speech::say(&t("Give the new playlist a name."));
                return;
            }
            app.net.send(Command::AddToPlaylist { playlist: target, new_name, clip_id: song.id });
        });
    }

    pub(crate) fn added_to_playlist(&mut self, playlist: Playlist) {
        speech::say(&tf("Added to {0}.", &[&playlist.display_name()]));
        // Its songs load again next time it's shown; the list of playlists says the new count.
        if let Some(view) = self.find_view_mut(&ViewKey::Playlist(playlist.id.clone())) {
            view.loaded = false;
        }
        self.net.send(Command::Playlists);
    }

    /// Delete in a playlist's songs: out of the playlist (the song stays in your library).
    pub(crate) fn remove_from_playlist(&mut self) {
        let (Some(p), Some(song)) = (self.chosen_playlist(), self.selected()) else { return };
        self.net.send(Command::RemoveFromPlaylist { playlist_id: p.id, clip_id: song.id });
    }

    pub(crate) fn removed_from_playlist(&mut self, playlist_id: &str, clip_id: &str) {
        let key = ViewKey::Playlist(playlist_id.to_string());
        let Some(view) = self.find_view_mut(&key) else { return };
        let title = view.songs.iter().find(|s| s.id == clip_id).map(SunoClip::display_title).unwrap_or_default();
        view.songs.retain(|s| s.id != clip_id);
        let name = view.title_name();
        if let Some(p) = view.playlist.as_mut() {
            p.song_count = Some(view.songs.len() as i64);
            view.title = p.spoken_text();
        }
        self.sections[LIBRARY].show_titles();
        self.sections[LIBRARY].refresh(Select::Keep);
        speech::say(&tf("Removed {0} from {1}.", &[&title, &name]));
    }

    /// Ctrl+Up and Ctrl+Down in a playlist's songs.
    pub(crate) fn move_in_playlist(&mut self, delta: i64) {
        let (Some(p), Some(song)) = (self.chosen_playlist(), self.selected()) else { return };
        let songs = &self.sections[LIBRARY].view().songs;
        let Some(from) = songs.iter().position(|s| s.id == song.id) else { return };
        let to = from as i64 + delta;
        if to < 0 || to >= songs.len() as i64 {
            speech::say(&if delta < 0 { t("Already first.") } else { t("Already last.") });
            return;
        }
        self.net.send(Command::MovePlaylistSong { playlist_id: p.id, clip_id: song.id, from, to: to as usize });
    }

    pub(crate) fn moved_in_playlist(&mut self, playlist_id: &str, from: usize, to: usize) {
        let Some(view) = self.find_view_mut(&ViewKey::Playlist(playlist_id.to_string())) else { return };
        if from >= view.songs.len() || to >= view.songs.len() {
            return;
        }
        let song = view.songs.remove(from);
        view.songs.insert(to, song);
        let count = view.songs.len();
        self.sections[LIBRARY].refresh(Select::Keep);
        speech::say(&tf("{0} of {1}.", &[&(to + 1), &count]));
    }
}

impl View {
    /// The view's name without its song count.
    pub fn title_name(&self) -> String {
        self.playlist.as_ref().map_or_else(|| self.title.clone(), Playlist::display_name)
    }
}
