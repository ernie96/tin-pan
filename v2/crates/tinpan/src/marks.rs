//! Marking several songs in a list, so one Song command acts on them all: Ctrl+Space marks or unmarks the
//! selected song, Shift+Up and Shift+Down mark as they move, Ctrl+A marks the whole list and Escape clears
//! the marks. The list control has one selection only, so marks are Tin Pan's own: each list keeps its
//! marked songs, and a marked row says "marked" after the song.

use quiet_list::{Select, Step};
use tinpan_core::clip::SunoClip;
use tinpan_core::i18n::{plural, t, tf};

use crate::views::View;
use crate::{App, discover, speech};

/// "Title" for one song, "5 songs" for more, to fill the messages that name what was acted on.
pub fn songs_text(songs: &[SunoClip]) -> String {
    match songs {
        [one] => one.display_title(),
        _ => plural(songs.len() as i64, "{0} song", "{0} songs"),
    }
}

impl View {
    /// The marked songs, in the list's order.
    pub fn marked_songs(&self) -> Vec<SunoClip> {
        self.songs.iter().filter(|s| self.marked.contains(&s.id)).cloned().collect()
    }

    pub fn marked_count(&self) -> usize {
        self.songs.iter().filter(|s| self.marked.contains(&s.id)).count()
    }
}

impl App {
    fn shown_mut(&mut self) -> &mut View {
        let tab = self.tab();
        let section = &mut self.sections[tab];
        let shown = section.shown.min(section.views.len() - 1);
        &mut section.views[shown]
    }

    /// Runs a Song command on the marked songs of the list shown, or on the selected song when none are marked.
    pub(crate) fn with_songs(&mut self, f: impl FnOnce(&mut Self, Vec<SunoClip>)) {
        let marked: Vec<SunoClip> = self.shown().marked_songs().into_iter().filter(|s| !s.id.starts_with(discover::NO_SONG)).collect();
        if marked.is_empty() {
            self.with_song(|app, song| f(app, vec![song]));
        } else {
            f(self, marked);
        }
    }

    /// Ctrl+Space: marks the selected song, or unmarks it.
    pub(crate) fn toggle_mark(&mut self) {
        self.with_song(|app, song| {
            let view = app.shown_mut();
            let marked = view.marked.insert(song.id.clone());
            if !marked {
                view.marked.remove(&song.id);
            }
            let count = view.marked_count();
            app.refresh_rows(Select::Keep);
            let said = if marked { t("Marked.") } else { t("Not marked.") };
            speech::say(&format!("{said} {}", tf("{0} marked.", &[&count])));
        });
    }

    /// Shift+Up and Shift+Down: mark the song selected and the one moved to, as Shift+arrows extend a
    /// selection elsewhere in Windows. The row moved to then says it's marked.
    pub(crate) fn mark_step(&mut self, step: Step) {
        let Some(index) = self.list().selected_index() else { return };
        let next = match step {
            Step::Up => index.checked_sub(1),
            _ => Some(index + 1),
        };
        let view = self.shown_mut();
        let ids: Vec<String> = [Some(index), next].into_iter().flatten().filter_map(|i| view.songs.get(i)).map(|s| s.id.clone()).collect();
        view.marked.extend(ids);
        self.refresh_rows(Select::Keep);
        if !self.list().step(step) {
            // At the end of the list: nothing new is read, so say what happened.
            speech::say(&tf("{0} marked.", &[&self.shown().marked_count()]));
        }
    }

    /// Ctrl+A: every song loaded in the list.
    pub(crate) fn mark_all(&mut self) {
        let view = self.shown_mut();
        let ids: Vec<String> = view.songs.iter().filter(|s| !s.id.starts_with(discover::NO_SONG)).map(|s| s.id.clone()).collect();
        if ids.is_empty() {
            speech::say(&t("No songs."));
            return;
        }
        let count = ids.len();
        view.marked.extend(ids);
        self.refresh_rows(Select::Keep);
        speech::say(&tf("Marked all {0}.", &[&plural(count as i64, "{0} song", "{0} songs")]));
    }

    /// Escape in a song list with songs marked, or Song menu, Clear marks. False when nothing was marked.
    pub(crate) fn clear_marks(&mut self) -> bool {
        let view = self.shown_mut();
        if view.marked.is_empty() {
            return false;
        }
        view.marked.clear();
        self.refresh_rows(Select::Keep);
        speech::say(&t("Marks cleared."));
        true
    }

    /// After a command has acted on these songs, they're no longer marked anywhere.
    pub(crate) fn unmark(&mut self, ids: &[String]) {
        let mut changed = false;
        for view in self.sections.iter_mut().flat_map(|s| s.views.iter_mut()) {
            for id in ids {
                changed |= view.marked.remove(id);
            }
        }
        if changed {
            self.refresh_rows(Select::Keep);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::views::ViewKey;

    fn song(id: &str) -> SunoClip {
        SunoClip { id: id.into(), title: Some(format!("Song {id}")), ..Default::default() }
    }

    #[test]
    fn marked_songs_come_in_the_list_order_and_skip_songs_gone_from_it() {
        let mut view = View::new(ViewKey::Library, "Songs".into());
        view.songs = vec![song("a"), song("b"), song("c")];
        view.marked.extend(["c".to_string(), "a".to_string(), "gone".to_string()]);
        let ids: Vec<String> = view.marked_songs().into_iter().map(|s| s.id).collect();
        assert_eq!(ids, ["a", "c"]);
        assert_eq!(view.marked_count(), 2);
    }

    #[test]
    fn one_song_is_named_and_more_are_counted() {
        assert_eq!(songs_text(&[song("a")]), "Song a");
        assert_eq!(songs_text(&[song("a"), song("b")]), "2 songs");
    }
}
