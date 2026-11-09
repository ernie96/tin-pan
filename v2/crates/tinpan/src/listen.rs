//! Account, Listen for Credits: rating songs for Suno, which pays credits for it. Suno's rater profile is
//! asked first when it wants one; then each task opens in a window with its songs, played here, and its
//! questions. A rating goes when every question has an answer and every song has been listened to for the
//! time Suno asks. Suno's Turnstile check is passed in the background when Suno wants it.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use tinpan_core::i18n::{t, tf};
use tinpan_core::listen::{BACKGROUNDS, CREATOR_TYPES, GENRES, LANGUAGES, Profile, SETUPS, Task};
use wxdragon::prelude::*;

use crate::form::{self, Field, Value};
use crate::net::Command;
use crate::player::Player;
use crate::{App, captcha, speech, with_app};

/// The Turnstile key Suno's Listen for Credits page uses (captcha.rs).
pub const CHECK_PROVIDER: i32 = 3;

/// What was done in a task's window, with the seconds listened to each song by its key.
pub enum Rated {
    Send { answers: BTreeMap<String, String>, listened: BTreeMap<String, u32> },
    Skip { listened: BTreeMap<String, u32> },
    Close,
}

fn letter(i: usize) -> String {
    char::from(b'A' + i.min(25) as u8).to_string()
}

/// A task's window. Songs play on a player of its own; the main song is paused first by the caller.
fn rate(parent: &Frame, task: &Task, songs: Vec<Vec<u8>>, volume: f32, device: &str) -> Rated {
    const SEND: i32 = 10_101;
    const SKIP: i32 = 10_102;
    let dialog = Dialog::builder(parent, &t("Listen for Credits")).with_style(DialogStyle::DefaultDialogStyle | DialogStyle::ResizeBorder).with_size(640, 620).build();
    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    let mut intro = vec![task.question.clone(), task.instructions.clone()];
    intro.push(tf("Listen to each song for at least {0} seconds, answer each question, then Send rating.", &[&task.min_listen_seconds]));
    if let Some(credits) = task.credits {
        intro.push(tf("This one earns {0} credits.", &[&credits]));
    }
    let about = TextCtrl::builder(&dialog)
        .with_value(&intro.into_iter().filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\r\n\r\n"))
        .with_style(TextCtrlStyle::MultiLine | TextCtrlStyle::ReadOnly | TextCtrlStyle::Rich2)
        .build();
    sizer.add(&about, 1, SizerFlag::Expand | SizerFlag::All, 8);

    let (player, _) = Player::new(volume, device);
    let player = Rc::new(RefCell::new(player));
    let playing: Rc<RefCell<Option<usize>>> = Rc::new(RefCell::new(None));
    let listened = Rc::new(RefCell::new(vec![0_u32; task.songs.len()]));
    let songs = Rc::new(songs);
    let mut song_buttons = Vec::new();
    let row = BoxSizer::builder(Orientation::Horizontal).build();
    for i in 0..task.songs.len() {
        let button = Button::builder(&dialog).with_label(&tf("Song _{0}: play or pause", &[&letter(i)]).replacen('_', "&", 1)).build();
        let (player, playing, songs) = (player.clone(), playing.clone(), songs.clone());
        button.on_click(move |_| {
            let mut player = player.borrow_mut();
            if *playing.borrow() == Some(i) && player.is_loaded() {
                speech::say(&if player.toggle_pause() { t("Playing.") } else { t("Paused.") });
                return;
            }
            match player.play(songs[i].clone()) {
                Ok(()) => *playing.borrow_mut() = Some(i),
                Err(e) => {
                    log::warn!("Listen for Credits: song {i} didn't play: {e}");
                    speech::say_error(&t("That song didn't play."));
                }
            }
        });
        row.add(&button, 0, SizerFlag::All, 4);
        song_buttons.push(button);
    }
    sizer.add_sizer(&row, 0, SizerFlag::All, 4);

    let mut choices = Vec::new();
    for question in &task.questions {
        let label = if question.description.is_empty() { question.name.clone() } else { format!("{}: {}", question.name, question.description) };
        sizer.add(&StaticText::builder(&dialog).with_label(&label).build(), 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 8);
        let mut items = vec![t("Not answered")];
        items.extend((0..task.songs.len()).map(|i| tf("Song {0}", &[&letter(i)])));
        if question.allow_neither {
            items.push(t("Neither"));
        }
        let choice = Choice::builder(&dialog).with_choices(items).build();
        choice.set_selection(0);
        sizer.add(&choice, 0, SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right, 8);
        choices.push(choice);
    }

    let buttons = BoxSizer::builder(Orientation::Horizontal).build();
    let send = Button::builder(&dialog).with_id(SEND).with_label(&form::wx_label("_Send rating")).build();
    let skip = Button::builder(&dialog).with_id(SKIP).with_label(&form::wx_label("S_kip this one")).build();
    let close = Button::builder(&dialog).with_id(ID_CANCEL).with_label(&t("Close")).build();
    for button in [&send, &skip, &close] {
        buttons.add(button, 0, SizerFlag::All, 4);
    }
    sizer.add_sizer(&buttons, 0, SizerFlag::AlignRight | SizerFlag::All, 4);
    dialog.set_sizer(sizer, true);
    dialog.set_escape_id(ID_CANCEL);

    // Seconds listened, counted while a song plays; each song says once when it's been heard long enough.
    let timer = Timer::new(&dialog);
    {
        let (player, playing, listened) = (player.clone(), playing.clone(), listened.clone());
        let min = task.min_listen_seconds;
        timer.on_tick(move |_| {
            let Ok(player) = player.try_borrow() else { return };
            let Some(i) = *playing.borrow() else { return };
            if player.is_loaded() && !player.is_paused() {
                let mut listened = listened.borrow_mut();
                listened[i] += 1;
                if listened[i] == min {
                    speech::say_quietly(&tf("Song {0} has been heard long enough.", &[&letter(i)]));
                }
            }
        });
    }
    timer.start(1000, false);

    let (min, count, questions) = (task.min_listen_seconds, task.songs.len(), choices.clone());
    // `count` is used again below, once the window has closed.
    let heard = listened.clone();
    send.on_click(move |_| {
        if let Some(i) = heard.borrow().iter().position(|s| *s < min) {
            speech::say_error(&tf("Listen to song {0} for at least {1} seconds first.", &[&letter(i), &min]));
            if let Some(button) = song_buttons.get(i) {
                button.set_focus();
            }
            return;
        }
        if let Some(choice) = questions.iter().find(|c| c.get_selection().unwrap_or(0) == 0) {
            speech::say_error(&t("Answer each question first."));
            choice.set_focus();
            return;
        }
        dialog.end_modal(SEND);
    });
    skip.on_click(move |_| dialog.end_modal(SKIP));
    about.set_insertion_point(0);
    about.set_focus();
    let answer = dialog.show_modal();
    timer.stop();
    player.borrow_mut().stop();

    let seconds: BTreeMap<String, u32> = task.songs.iter().zip(listened.borrow().iter()).map(|(s, n)| (s.key.clone(), *n)).collect();
    let answers: BTreeMap<String, String> = task
        .questions
        .iter()
        .zip(&choices)
        .map(|(q, c)| {
            let picked = c.get_selection().unwrap_or(0) as usize;
            (q.key.clone(), if picked >= 1 && picked <= count { task.songs[picked - 1].key.clone() } else { "neither".to_string() })
        })
        .collect();
    dialog.destroy();
    match answer {
        SEND => Rated::Send { answers, listened: seconds },
        SKIP => Rated::Skip { listened: seconds },
        _ => Rated::Close,
    }
}

fn pick(items: &[(&str, &'static str)], current: &str) -> (Vec<String>, usize) {
    let mut names = vec![t("Choose one")];
    names.extend(items.iter().map(|(_, name)| t(name)));
    (names, items.iter().position(|(key, _)| *key == current).map_or(0, |i| i + 1))
}

impl App {
    /// Account menu, Listen for Credits.
    pub(crate) fn listen_for_credits(&mut self) {
        if !self.signed_in {
            speech::say(&t("Not signed in."));
            return;
        }
        speech::say(&t("Getting a song to rate."));
        self.net.send(Command::ListenStart);
    }

    /// Suno wants the rater profile before the first rating: its own questions, with what you answered before.
    pub(crate) fn listen_profile(&mut self, known: Option<Profile>) {
        let known = known.unwrap_or_default();
        let (creators, creator) = pick(&CREATOR_TYPES, &known.creator_type);
        let (backgrounds, background) = pick(&BACKGROUNDS, &known.musical_background);
        let (setups, setup) = pick(&SETUPS, &known.listening_setup);
        let mut fields = vec![
            Field::choice("_Which describes you best?", creators, creator),
            Field::choice("Your _musical background", backgrounds, background),
            Field::choice("What you _listen on", setups, setup),
            Field::heading("Genres you know well"),
        ];
        fields.extend(GENRES.iter().map(|(key, name)| Field::check(name, known.genres.iter().any(|g| g == key))));
        fields.push(Field { label: "_Other genres", value: Value::Text { value: known.other_genres.clone(), multiline: false, max_len: 500 } });
        fields.push(Field::heading("Languages you speak fluently"));
        fields.extend(LANGUAGES.iter().map(|(key, name)| Field::check(name, known.languages.iter().any(|l| l == key))));
        fields.push(Field { label: "Other lan_guages", value: Value::Text { value: known.other_languages.clone(), multiline: false, max_len: 500 } });
        let intro = t("Before the first rating, Suno asks about you as a listener, so it can give you songs you can judge. You can change this later.");
        if !form::show(&self.frame, &t("About you as a listener"), &intro, &t("Save and continue"), &mut fields) {
            return;
        }
        let chosen = |field: &Field, items: &[(&str, &str)]| field.selected().checked_sub(1).and_then(|i| items.get(i)).map_or(String::new(), |(key, _)| key.to_string());
        let genre_start = 4;
        let language_start = genre_start + GENRES.len() + 2;
        let profile = Profile {
            creator_type: chosen(&fields[0], &CREATOR_TYPES),
            musical_background: chosen(&fields[1], &BACKGROUNDS),
            listening_setup: chosen(&fields[2], &SETUPS),
            genres: GENRES.iter().enumerate().filter(|(i, _)| fields[genre_start + i].checked()).map(|(_, (key, _))| key.to_string()).collect(),
            other_genres: fields[genre_start + GENRES.len()].text(),
            languages: LANGUAGES.iter().enumerate().filter(|(i, _)| fields[language_start + i].checked()).map(|(_, (key, _))| key.to_string()).collect(),
            other_languages: fields[language_start + LANGUAGES.len()].text(),
        };
        if !profile.is_complete() {
            speech::say_error(&t("Suno needs an answer to each question, and at least one genre and one language."));
            return;
        }
        self.net.send(Command::ListenSaveProfile { profile: Box::new(profile) });
    }

    /// A task to rate, with its songs' files.
    pub(crate) fn listen_task(&mut self, task: Task, songs: Vec<Vec<u8>>, check_due: bool) {
        if self.player.is_loaded() && !self.player.is_paused() {
            self.player.toggle_pause();
        }
        let device = self.config.current.audio.output_device.clone();
        match rate(&self.frame, &task, songs, self.player.volume(), &device) {
            Rated::Send { answers, listened } => {
                if check_due {
                    self.listen_check(task.id, answers, listened);
                } else {
                    self.net.send(Command::ListenSend { task_id: task.id, answers, listened, check: None });
                }
            }
            Rated::Skip { listened } => self.net.send(Command::ListenSkip { task_id: task.id, listened }),
            Rated::Close => self.net.send(Command::ListenRelease),
        }
    }

    /// Suno wants its Turnstile check with this rating: passed here, then the rating goes again with it.
    pub(crate) fn listen_check(&mut self, task_id: String, answers: BTreeMap<String, String>, listened: BTreeMap<String, u32>) {
        captcha::solve(&self.frame, CHECK_PROVIDER, move |result| match result {
            Ok((token, _)) => with_app(|app| app.net.send(Command::ListenSend { task_id, answers, listened, check: Some(token) })),
            Err(why) => speech::say_error(&why),
        });
    }
}
