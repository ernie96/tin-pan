//! The Discover tab's own lists (From people you follow, Explore's shelves, Contests) and the lists opened
//! from a song: similar songs, its creator's songs, covers and remixes, and the song it was made from. In
//! Tin Pan 1 those opened in windows of their own; here each joins the Discover tab's lists until Delete
//! closes it. Comments, from the Song menu, are here too.

use quiet_list::Select;
use tinpan_core::clip::SunoClip;
use tinpan_core::discover::{Comment, Contest, ExploreShelf, creator_handle};
use tinpan_core::i18n::{plural, t, tf};
use tinpan_core::link::read_song_link;
use wxdragon::prelude::*;

use crate::net::Command;
use crate::views::{DISCOVER, View, ViewKey};
use crate::{App, dialogs, form, speech};

/// The id given to a contest that has no song, so it can still be listed.
pub(crate) const NO_SONG: &str = "tinpan-contest-without-song-";

/// The Discover tab's lists before anything is opened from a song.
pub fn first_views() -> Vec<View> {
    vec![
        View::new(ViewKey::Trending, t("Trending")),
        View::new(ViewKey::Following, t("From people you follow")),
        View::new(ViewKey::Explore, t("Explore")),
        View::new(ViewKey::Contests, t("Suno contests")),
        View::new(ViewKey::Queue, t("Up next")),
    ]
}

impl App {
    /// The Discover menu's Explore, From people I follow and Contests: that list, loaded and said.
    pub(crate) fn open_discover(&mut self, key: &ViewKey) {
        let Some(index) = self.sections[DISCOVER].views.iter().position(|v| v.key == *key) else { return };
        self.show_tab(DISCOVER);
        self.sections[DISCOVER].selector.set_selection(index as u32, true);
        self.view_chosen(DISCOVER, index, true);
        if self.sections[DISCOVER].view().loaded {
            let text = self.view_count_text(self.sections[DISCOVER].view());
            speech::say(&text);
        }
    }

    /// Delete in a lists box: a playlist goes to the trash (asking first); a list opened from a song closes.
    pub(crate) fn delete_view(&mut self) {
        let tab = self.tab();
        match self.sections[tab].view().key.clone() {
            ViewKey::Playlist(_) => self.delete_playlist(),
            ViewKey::Result(_) => {
                let section = &mut self.sections[tab];
                let closed = section.views.remove(section.shown);
                section.shown = section.shown.min(section.views.len() - 1);
                section.show_titles();
                section.refresh(Select::Start);
                speech::say(&tf("Closed {0}.", &[&closed.title]));
            }
            _ => {}
        }
    }

    fn set_discover_view(&mut self, key: &ViewKey, songs: Vec<SunoClip>, announce: bool) {
        let Some(view) = self.find_view_mut(key) else { return };
        view.songs = songs;
        view.loaded = true;
        let text = self.find_view(key).map(|v| self.view_count_text(v));
        if self.sections[DISCOVER].view().key == *key {
            self.sections[DISCOVER].refresh(Select::Start);
        }
        if announce && let Some(text) = text {
            speech::say(&text);
        }
    }

    pub(crate) fn following_loaded(&mut self, songs: Vec<SunoClip>, announce: bool) {
        self.set_discover_view(&ViewKey::Following, songs, announce);
    }

    /// Explore's shelves come in: each becomes a list after Explore, whose own list has all their songs.
    pub(crate) fn explore_loaded(&mut self, shelves: Vec<ExploreShelf>, announce: bool) {
        let section = &mut self.sections[DISCOVER];
        let shown_key = section.view().key.clone();
        section.views.retain(|v| !matches!(v.key, ViewKey::Shelf(_)));
        let at = section.views.iter().position(|v| v.key == ViewKey::Explore).map_or(section.views.len(), |i| i + 1);
        let mut all: Vec<SunoClip> = Vec::new();
        for (i, shelf) in shelves.into_iter().enumerate() {
            for song in &shelf.songs {
                if !all.iter().any(|s| s.id == song.id) {
                    all.push(song.clone());
                }
            }
            let mut view = View::new(ViewKey::Shelf(shelf.title.clone()), tf("{0}, {1} songs", &[&shelf.title, &shelf.songs.len()]));
            view.songs = shelf.songs;
            view.loaded = true;
            section.views.insert(at + i, view);
        }
        section.shown = section.views.iter().position(|v| v.key == shown_key).unwrap_or(0);
        section.show_titles();
        self.set_discover_view(&ViewKey::Explore, all, announce);
        // A shelf on screen shows its songs too.
        self.sections[DISCOVER].refresh(Select::Keep);
    }

    /// Contests: each contest's song, said as the contest (its title, when it ends, its entries).
    pub(crate) fn contests_loaded(&mut self, contests: Vec<Contest>, songs: Vec<SunoClip>, announce: bool) {
        let mut ordered = Vec::new();
        let mut labels = std::collections::HashMap::new();
        for (i, contest) in contests.iter().enumerate() {
            match contest.base_clip_id.as_deref().and_then(|id| songs.iter().find(|s| s.id == id)) {
                Some(song) if !ordered.iter().any(|s: &SunoClip| s.id == song.id) => {
                    labels.insert(song.id.clone(), contest.spoken_text());
                    ordered.push(song.clone());
                }
                Some(_) => {}
                // A contest without a song stays in the list, as in Tin Pan 1; Enter says it has none.
                None => {
                    let id = format!("{NO_SONG}{i}");
                    labels.insert(id.clone(), contest.spoken_text());
                    ordered.push(SunoClip { id, title: Some(contest.title.clone()), ..SunoClip::default() });
                }
            }
        }
        if let Some(view) = self.find_view_mut(&ViewKey::Contests) {
            view.labels = labels;
        }
        self.set_discover_view(&ViewKey::Contests, ordered, announce);
    }

    /// A list opened from a song or a creator: it joins the Discover tab's lists, shown and said.
    pub(crate) fn open_result(&mut self, title: String, songs: Vec<SunoClip>, with_creator: bool, creator: Option<tinpan_core::discover::CreatorPage>) {
        let section = &mut self.sections[DISCOVER];
        let next = section.views.iter().filter_map(|v| if let ViewKey::Result(n) = v.key { Some(n) } else { None }).max().map_or(0, |n| n + 1);
        let mut view = View::new(ViewKey::Result(next), title);
        view.with_creator = with_creator;
        view.creator = creator;
        view.loaded = true;
        let said = format!("{}, {}", view.title, plural(songs.len() as i64, "{0} song.", "{0} songs."));
        view.songs = songs;
        section.views.push(view);
        section.shown = section.views.len() - 1;
        section.show_titles();
        section.refresh(Select::Start);
        self.notebook.set_selection(DISCOVER);
        self.sections[DISCOVER].list.set_focus();
        speech::say(&said);
    }

    // ── Related songs (the Song menu) ───────────────────────────────────

    pub(crate) fn similar(&mut self) {
        self.with_song(|app, song| app.net.send(Command::Similar { clip: Box::new(song) }));
    }

    pub(crate) fn creator_of_song(&mut self) {
        self.with_song(|app, song| match song.handle.as_deref().map(str::trim).filter(|h| !h.is_empty()) {
            Some(handle) => app.net.send(Command::Creator { handle: handle.to_string() }),
            None => speech::say(&t("Suno didn't say who made this song.")),
        });
    }

    pub(crate) fn remixes(&mut self) {
        self.with_song(|app, song| app.net.send(Command::Remixes { clip: Box::new(song) }));
    }

    pub(crate) fn original(&mut self) {
        self.with_song(|app, song| app.net.send(Command::Original { clip: Box::new(song) }));
    }

    /// Discover menu, Find a creator: by handle or link.
    pub(crate) fn find_creator(&mut self) {
        if !self.signed_in {
            speech::say(&t("Not signed in."));
            return;
        }
        let mut fields = [form::Field { label: "_Handle: the name after suno.com/@, or the whole link", value: form::Value::Text { value: String::new(), multiline: false, max_len: 200 } }];
        if !form::show(&self.frame, &t("Find a creator"), &t("Shows a Suno creator's songs, newest first."), &t("Find"), &mut fields) {
            return;
        }
        let handle = creator_handle(&fields[0].text());
        if !handle.is_empty() {
            self.net.send(Command::Creator { handle });
        }
    }

    /// Discover menu, Search Suno: everyone's public songs, as Suno's own search finds them.
    pub(crate) fn search_suno(&mut self) {
        if !self.signed_in {
            speech::say(&t("Not signed in."));
            return;
        }
        let mut fields = [form::Field { label: "_Search for", value: form::Value::Text { value: String::new(), multiline: false, max_len: 256 } }];
        if !form::show(&self.frame, &t("Search Suno"), &t("Finds songs from everyone on Suno, not only yours. They open as a list in the Discover tab."), &t("Search"), &mut fields) {
            return;
        }
        let term = fields[0].text().trim().to_string();
        if !term.is_empty() {
            speech::say(&t("Searching."));
            self.net.send(Command::SearchSuno { term });
        }
    }

    /// Discover menu, Open a Suno link: a song someone shared. A link on the clipboard is already filled in.
    pub(crate) fn open_link(&mut self) {
        if !self.signed_in {
            speech::say(&t("Not signed in."));
            return;
        }
        let copied = Clipboard::get().get_text().filter(|text| read_song_link(text).is_some()).unwrap_or_default();
        let mut fields = [form::Field { label: "_Link to the song", value: form::Value::Text { value: copied, multiline: false, max_len: 2000 } }];
        if !form::show(&self.frame, &t("Open a Suno link"), &t("Paste a link to a Suno song. It opens as a list in the Discover tab."), &t("Open"), &mut fields) {
            return;
        }
        match read_song_link(&fields[0].text()) {
            Some(link) => self.net.send(Command::OpenLink { link }),
            None => speech::say(&t("That isn't a link to a Suno song.")),
        }
    }

    // ── Comments ────────────────────────────────────────────────────────

    pub(crate) fn show_comments(&mut self) {
        self.with_song(|app, song| app.net.send(Command::Comments { clip: Box::new(song) }));
    }

    pub(crate) fn comments_loaded(&self, song: &SunoClip, comments: &[Comment]) {
        if comments.is_empty() {
            speech::say(&t("No comments."));
            return;
        }
        let text = comments.iter().map(Comment::spoken_text).collect::<Vec<_>>().join("\n\n");
        dialogs::show_document(&self.frame, &tf("Comments on {0} ({1})", &[&song.display_title(), &comments.len()]), &text);
    }

    pub(crate) fn write_comment(&mut self) {
        self.with_song(|app, song| {
            let mut fields = [form::Field { label: "_Comment", value: form::Value::Text { value: String::new(), multiline: true, max_len: 1000 } }];
            let title = tf("Comment on {0}", &[&song.display_title()]);
            if form::show(&app.frame, &title, &t("Comments are public on Suno."), &t("Post"), &mut fields) && !fields[0].text().is_empty() {
                app.net.send(Command::PostComment { clip_id: song.id, text: fields[0].text() });
            }
        });
    }
}
