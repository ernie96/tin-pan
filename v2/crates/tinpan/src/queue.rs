//! Up next: songs you queue to play after the current one, before the list it came from carries on, and,
//! when Settings allows, songs Suno picks to keep playing once a list ends (its player's autoplay). The
//! queue is a list in the Discover tab; Delete there takes a song out.

use quiet_list::Select;
use tinpan_core::clip::SunoClip;
use tinpan_core::i18n::{t, tf};

use crate::views::{DISCOVER, ViewKey};
use crate::{App, marks, speech};

impl App {
    pub(crate) fn up_next(&self) -> &[SunoClip] {
        self.find_view(&ViewKey::Queue).map_or(&[], |v| v.songs.as_slice())
    }

    fn change_queue(&mut self, change: impl FnOnce(&mut Vec<SunoClip>)) {
        if let Some(view) = self.find_view_mut(&ViewKey::Queue) {
            change(&mut view.songs);
            view.loaded = true;
        }
        if self.sections[DISCOVER].view().key == ViewKey::Queue {
            self.sections[DISCOVER].refresh(Select::Keep);
        }
    }

    /// Song menu, Add to queue (`first` false) and Play next (`first` true): the song, or the marked songs in
    /// their order. Songs not ready to play are left out.
    pub(crate) fn add_to_queue(&mut self, first: bool) {
        self.with_songs(|app, songs| {
            let asked = marks::songs_text(&songs);
            let one_asked = songs.len() == 1;
            let songs: Vec<SunoClip> = songs.into_iter().filter(SunoClip::can_play).collect();
            if songs.is_empty() {
                speech::say(&if one_asked { tf("{0} can't be played yet.", &[&asked]) } else { t("None of the marked songs can be played yet.") });
                return;
            }
            let ids: Vec<String> = songs.iter().map(|s| s.id.clone()).collect();
            let named = marks::songs_text(&songs);
            let one = songs.len() == 1;
            app.change_queue(|queue| {
                queue.retain(|s| !ids.contains(&s.id));
                if first {
                    queue.splice(0..0, songs);
                } else {
                    queue.extend(songs);
                }
            });
            app.unmark(&ids);
            speech::say(&match (first, one) {
                (true, true) => tf("{0} plays next.", &[&named]),
                (true, false) => tf("{0} play next.", &[&named]),
                (false, _) => tf("Added {0} to Up next.", &[&named]),
            });
        });
    }

    /// Delete in Up next: the song, or the marked songs.
    pub(crate) fn remove_from_queue(&mut self) {
        self.with_songs(|app, songs| {
            let ids: Vec<String> = songs.iter().map(|s| s.id.clone()).collect();
            app.change_queue(|queue| queue.retain(|s| !ids.contains(&s.id)));
            app.unmark(&ids);
            speech::say(&tf("Removed {0} from Up next.", &[&marks::songs_text(&songs)]));
        });
    }

    /// Playback menu, Clear Up next.
    pub(crate) fn clear_queue(&mut self) {
        if self.up_next().is_empty() {
            speech::say(&t("Up next is empty."));
            return;
        }
        self.change_queue(Vec::clear);
        speech::say(&t("Up next cleared."));
    }

    /// The first playable queued song, taken out of the queue.
    pub(crate) fn take_queued(&mut self) -> Option<SunoClip> {
        let index = self.up_next().iter().position(SunoClip::can_play)?;
        let mut taken = None;
        self.change_queue(|queue| taken = Some(queue.remove(index)));
        taken
    }

    /// Takes a song out of the queue because it's being played from there.
    pub(crate) fn unqueue(&mut self, id: &str) {
        self.change_queue(|queue| queue.retain(|s| s.id != id));
    }

    /// Suno's picks to follow a list that ended: queued and started, or the end said when there are none.
    pub(crate) fn autoplay_loaded(&mut self, result: Result<Vec<SunoClip>, tinpan_core::Error>) {
        self.autoplay_pending = false;
        // Something else started meanwhile: the picks aren't wanted any more.
        if self.wanted.is_some() || self.player.is_loaded() || self.playing.is_none() {
            return;
        }
        let songs: Vec<SunoClip> = match result {
            Ok(songs) => songs.into_iter().filter(|s| s.can_play() && !self.not_interested.contains(&s.id)).collect(),
            Err(e) => {
                log::warn!("Autoplay: {e}");
                Vec::new()
            }
        };
        if songs.is_empty() {
            self.list_ended();
            return;
        }
        self.change_queue(|queue| queue.extend(songs));
        self.step_on(1, true);
    }
}
