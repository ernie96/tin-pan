//! Account, Listen for Credits: rating songs for Suno, which pays credits for it. Suno's rater profile is
//! asked first when it wants one; then each task opens in a window with its songs, played here, and its
//! questions. A rating goes when every question has an answer and every song has been listened to for the
//! time Suno asks. Suno's Turnstile check is passed in the background when Suno wants it.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use tinpan_core::i18n::{t, tf};
use tinpan_core::listen::{Answer, BACKGROUNDS, CREATOR_TYPES, GENRES, LANGUAGES, OTHER, Profile, SETUPS, Task, letter};
use wxdragon::prelude::*;

use crate::form::{self, Field, Value};
use crate::net::Command;
use crate::player::Player;
use crate::{App, captcha, speech, with_app};

/// The Turnstile key Suno's Listen for Credits page uses (captcha.rs).
pub const CHECK_PROVIDER: i32 = 3;

/// What was done in a task's window, with the seconds listened to each song by its key.
pub enum Rated {
    Send { answers: BTreeMap<String, serde_json::Value>, listened: BTreeMap<String, u32> },
    Skip { listened: BTreeMap<String, u32> },
    Close,
}

/// A question's controls in the window.
enum Asked {
    /// A list whose first line is "Not answered" and whose last may be "Other".
    One(Choice),
    /// A check box per choice, and one for "Other".
    Many(Vec<CheckBox>),
    Text(TextCtrl),
}

struct Control {
    asked: Asked,
    /// The words for "Other", when the question has it.
    other: Option<TextCtrl>,
    /// Greys out (false) or brings back everything belonging to the question.
    parts: Vec<Box<dyn Fn(bool)>>,
}

/// The values chosen or typed for each question, for deciding which questions are asked.
fn chosen(task: &Task, controls: &[Control]) -> BTreeMap<String, Vec<String>> {
    task.questions
        .iter()
        .zip(controls)
        .map(|(q, c)| {
            let values = match (&q.answer, &c.asked) {
                (Answer::One(choices), Asked::One(list)) => match list.get_selection().unwrap_or(0) as usize {
                    0 => Vec::new(),
                    i if i <= choices.len() => vec![choices[i - 1].value.clone()],
                    _ => vec![OTHER.to_string()],
                },
                (Answer::Many(choices), Asked::Many(boxes)) => boxes
                    .iter()
                    .enumerate()
                    .filter(|(_, b)| b.get_value())
                    .map(|(i, _)| choices.get(i).map_or_else(|| OTHER.to_string(), |c| c.value.clone()))
                    .collect(),
                (_, Asked::Text(text)) => Some(text.get_value().trim().to_string()).filter(|t| !t.is_empty()).into_iter().collect(),
                _ => Vec::new(),
            };
            (q.key.clone(), values)
        })
        .collect()
}

/// Greys out the questions not asked for the answers so far.
fn update_shown(task: &Task, controls: &[Control]) {
    let answers = chosen(task, controls);
    for (q, c) in task.questions.iter().zip(controls) {
        let shown = q.is_shown(&answers);
        for part in &c.parts {
            part(shown);
        }
        if let Some(other) = &c.other {
            other.enable(shown && answers.get(&q.key).is_some_and(|v| v.iter().any(|x| x == OTHER)));
        }
    }
}

/// The answers to send, or the question still to answer (its index and what to say).
fn answers_to_send(task: &Task, controls: &[Control]) -> Result<BTreeMap<String, serde_json::Value>, (usize, String)> {
    let chosen = chosen(task, controls);
    let mut out = BTreeMap::new();
    for (i, (q, c)) in task.questions.iter().zip(controls).enumerate() {
        if !q.is_shown(&chosen) {
            continue;
        }
        let values = chosen.get(&q.key).cloned().unwrap_or_default();
        if values.is_empty() {
            if q.required {
                return Err((i, tf("Answer {0} first.", &[&q.label])));
            }
            continue;
        }
        if values.iter().any(|v| v == OTHER) {
            let words = c.other.as_ref().map(|o| o.get_value().trim().to_string()).unwrap_or_default();
            if words.is_empty() {
                return Err((i, tf("Say what Other means for {0} first.", &[&q.label])));
            }
            out.insert(format!("{}_other", q.key), serde_json::Value::String(words));
        }
        let value = match q.answer {
            Answer::Many(_) => serde_json::Value::Array(values.into_iter().map(serde_json::Value::String).collect()),
            _ => serde_json::Value::String(values.into_iter().next().unwrap_or_default()),
        };
        out.insert(q.key.clone(), value);
    }
    Ok(out)
}

/// A task's window. Songs play on a player of its own; the main song is paused first by the caller.
fn rate(parent: &Frame, task: &Task, songs: Vec<Vec<u8>>, volume: f32, device: &str) -> Rated {
    const SEND: i32 = 10_101;
    const SKIP: i32 = 10_102;
    let dialog = Dialog::builder(parent, &t("Listen for Credits")).with_style(DialogStyle::DefaultDialogStyle | DialogStyle::ResizeBorder).with_size(640, 680).build();
    let outer = BoxSizer::builder(Orientation::Vertical).build();
    let panel = ScrolledWindow::builder(&dialog).build();
    panel.set_scroll_rate(0, 20);
    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    let mut intro = vec![task.question.clone(), task.instructions.clone()];
    intro.push(tf("Listen to each song for at least {0} seconds, answer the questions, then Send rating.", &[&task.min_listen_seconds]));
    if let Some(credits) = task.credits {
        intro.push(tf("This one earns {0} credits.", &[&credits]));
    }
    let about = TextCtrl::builder(&panel)
        .with_value(&intro.into_iter().filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\r\n\r\n").replace('\n', "\r\n").replace("\r\r\n", "\r\n"))
        .with_style(TextCtrlStyle::MultiLine | TextCtrlStyle::ReadOnly | TextCtrlStyle::Rich2)
        .with_size(Size::new(-1, 160))
        .build();
    sizer.add(&about, 0, SizerFlag::Expand | SizerFlag::All, 8);

    let (player, _) = Player::new(volume, device);
    let player = Rc::new(RefCell::new(player));
    let playing: Rc<RefCell<Option<usize>>> = Rc::new(RefCell::new(None));
    let listened = Rc::new(RefCell::new(vec![0_u32; task.songs.len()]));
    let songs = Rc::new(songs);
    let mut song_buttons = Vec::new();
    let row = BoxSizer::builder(Orientation::Horizontal).build();
    for i in 0..task.songs.len() {
        let button = Button::builder(&panel).with_label(&tf("Song _{0}: play or pause", &[&letter(i)]).replacen('_', "&", 1)).build();
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

    let mut controls = Vec::new();
    for question in &task.questions {
        let label = if question.description.is_empty() { question.label.clone() } else { format!("{}: {}", question.label, question.description) };
        let heading = StaticText::builder(&panel).with_label(&label).build();
        sizer.add(&heading, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 8);
        let mut parts: Vec<Box<dyn Fn(bool)>> = vec![Box::new(move |on| heading.enable(on))];
        let asked = match &question.answer {
            Answer::One(choices) => {
                let mut items = vec![t("Not answered")];
                items.extend(choices.iter().map(|c| c.label.clone()));
                if question.allow_other {
                    items.push(t("Other"));
                }
                let list = Choice::builder(&panel).with_choices(items).build();
                list.set_selection(0);
                sizer.add(&list, 0, SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right, 8);
                parts.push(Box::new(move |on| list.enable(on)));
                Asked::One(list)
            }
            Answer::Many(choices) => {
                let mut labels: Vec<String> = choices.iter().map(|c| c.label.clone()).collect();
                if question.allow_other {
                    labels.push(t("Other"));
                }
                let boxes: Vec<CheckBox> = labels.iter().map(|l| CheckBox::builder(&panel).with_label(l).build()).collect();
                for b in &boxes {
                    sizer.add(b, 0, SizerFlag::Left | SizerFlag::Right, 16);
                    let b = *b;
                    parts.push(Box::new(move |on| b.enable(on)));
                }
                Asked::Many(boxes)
            }
            Answer::Text => {
                let text = TextCtrl::builder(&panel).with_style(TextCtrlStyle::MultiLine).with_size(Size::new(-1, 60)).build();
                sizer.add(&text, 0, SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right, 8);
                parts.push(Box::new(move |on| text.enable(on)));
                Asked::Text(text)
            }
        };
        let other = question.allow_other.then(|| {
            let label = StaticText::builder(&panel).with_label(&tf("Other, for {0}:", &[&question.label])).build();
            let text = TextCtrl::builder(&panel).build();
            sizer.add(&label, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 8);
            sizer.add(&text, 0, SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right, 8);
            text.enable(false);
            text
        });
        controls.push(Control { asked, other, parts });
    }
    panel.set_sizer(sizer, true);
    outer.add(&panel, 1, SizerFlag::Expand, 0);

    let buttons = BoxSizer::builder(Orientation::Horizontal).build();
    let send = Button::builder(&dialog).with_id(SEND).with_label(&form::wx_label("_Send rating")).build();
    let skip = Button::builder(&dialog).with_id(SKIP).with_label(&form::wx_label("S_kip this one")).build();
    let close = Button::builder(&dialog).with_id(ID_CANCEL).with_label(&t("Close")).build();
    for button in [&send, &skip, &close] {
        buttons.add(button, 0, SizerFlag::All, 4);
    }
    outer.add_sizer(&buttons, 0, SizerFlag::AlignRight | SizerFlag::All, 4);
    dialog.set_sizer(outer, true);
    dialog.set_escape_id(ID_CANCEL);

    // Questions asked only after certain answers are greyed out until then.
    let task_rc = Rc::new(task.clone());
    let controls = Rc::new(controls);
    for c in controls.iter() {
        let (task, all) = (task_rc.clone(), controls.clone());
        let refresh = move || update_shown(&task, &all);
        match &c.asked {
            Asked::One(list) => {
                let refresh = refresh.clone();
                list.on_selection_changed(move |_| refresh());
            }
            Asked::Many(boxes) => {
                for b in boxes {
                    let refresh = refresh.clone();
                    b.on_toggled(move |_| refresh());
                }
            }
            Asked::Text(text) => {
                let refresh = refresh.clone();
                text.on_text_changed(move |_| refresh());
            }
        }
    }
    update_shown(&task_rc, &controls);

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

    let min = task.min_listen_seconds;
    let heard = listened.clone();
    let answers: Rc<RefCell<BTreeMap<String, serde_json::Value>>> = Rc::new(RefCell::new(BTreeMap::new()));
    let (task_send, controls_send, answers_send) = (task_rc.clone(), controls.clone(), answers.clone());
    send.on_click(move |_| {
        if let Some(i) = heard.borrow().iter().position(|s| *s < min) {
            speech::say_error(&tf("Listen to song {0} for at least {1} seconds first.", &[&letter(i), &min]));
            if let Some(button) = song_buttons.get(i) {
                button.set_focus();
            }
            return;
        }
        match answers_to_send(&task_send, &controls_send) {
            Ok(found) => {
                *answers_send.borrow_mut() = found;
                dialog.end_modal(SEND);
            }
            Err((i, why)) => {
                speech::say_error(&why);
                let c = &controls_send[i];
                match &c.asked {
                    Asked::One(list) => list.set_focus(),
                    Asked::Many(boxes) => {
                        if let Some(b) = boxes.first() {
                            b.set_focus();
                        }
                    }
                    Asked::Text(text) => text.set_focus(),
                }
            }
        }
    });
    skip.on_click(move |_| dialog.end_modal(SKIP));
    about.set_insertion_point(0);
    about.set_focus();
    let answer = dialog.show_modal();
    timer.stop();
    player.borrow_mut().stop();

    let seconds: BTreeMap<String, u32> = task.songs.iter().zip(listened.borrow().iter()).map(|(s, n)| (s.key.clone(), *n)).collect();
    dialog.destroy();
    match answer {
        SEND => Rated::Send { answers: answers.take(), listened: seconds },
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
    pub(crate) fn listen_check(&mut self, task_id: String, answers: BTreeMap<String, serde_json::Value>, listened: BTreeMap<String, u32>) {
        captcha::solve(&self.frame, CHECK_PROVIDER, move |result| match result {
            Ok((token, _)) => with_app(|app| app.net.send(Command::ListenSend { task_id, answers, listened, check: Some(token) })),
            Err(why) => speech::say_error(&why),
        });
    }
}
