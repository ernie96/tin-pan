//! The rest of the Song menu's Make from it, as in Tin Pan 1: Use as inspiration, Get whole song, Stems,
//! Change speed, Reverse, Crop, Fade, Make a video and Make a persona. The forms stay open (tool_form.rs),
//! except Make a persona's, which is a question to answer once.

use tinpan_core::clip::{SunoClip, format_time};
use tinpan_core::create::{GenerateRequest, Mode, SunoModel};
use tinpan_core::i18n::{t, tf};
use wxdragon::prelude::*;

use crate::form::{self, Field, Value};
use crate::net::{Command, EditJob};
use crate::tool_form::{self, ToolForm};
use crate::{App, speech, with_app};

/// "Rain (Reversed)", or the suffix alone for a song with no title.
fn titled(song: &SunoClip, suffix: &str) -> String {
    match song.title.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
        Some(title) => format!("{title} ({suffix})"),
        None => suffix.to_string(),
    }
}

impl App {
    /// Sends an edit of a finished song, saying what started.
    fn start_edit(&mut self, song: &SunoClip, starting: &str, job: EditJob) {
        speech::say(starting);
        self.net.send(Command::Edit { clip_id: song.id.clone(), job });
    }

    /// Ctrl+J: joins an extended song's parts into one.
    pub(crate) fn whole_song(&mut self) {
        self.with_song(|app, song| {
            if app.finished(&song) {
                app.start_edit(&song, &tf("Joining the parts of {0}.", &[&song.display_title()]), EditJob::WholeSong);
            }
        });
    }

    /// Ctrl+Shift+S: the song split into its stems.
    pub(crate) fn stems(&mut self) {
        self.with_song(|app, song| {
            if app.finished(&song) {
                app.start_edit(&song, &tf("Splitting into stems: {0}.", &[&song.display_title()]), EditJob::Stems);
            }
        });
    }

    /// Use as inspiration: a brand-new song that takes after this one.
    pub(crate) fn inspire(&mut self) {
        self.with_song(|app, song| {
            if !app.finished(&song) {
                return;
            }
            let Some((models, at)) = app.models_for("playlist_condition", "songs inspired by another", &["playlist"]) else { return };
            let intro = tf("Makes a new song that takes after {0}: its sound and feel, with your own lyrics and style.", &[&song.display_title()]);
            let form = ToolForm::new(&app.frame, &tf("Use {0} as inspiration", &[&song.display_title()]), &intro);
            let title = form.text("_Title", "", 1);
            let lyrics = form.text("_Lyrics", "", 6);
            let instrumental = form.check("_Instrumental (no singing)", false);
            let style = form.text("_Style", song.metadata.as_ref().and_then(|m| m.tags.as_deref()).unwrap_or(""), 1);
            Self::form_style_buttons(&form, style, lyrics, Some(instrumental));
            let exclude = form.text("_Exclude styles", "", 1);
            let model = form.choice("_Model", models.iter().map(SunoModel::display_name).collect(), at);
            let (weirdness, suno_weirdness) = form.slider("_Weirdness", "Let Suno choose the weirdness", 50);
            let (influence, suno_influence) = form.slider("Style in_fluence", "Let Suno choose the style influence", 50);
            let max_mode = form.check("Ma_x mode", false);
            form.show(&t("Make song"), move || {
                with_app(|app| {
                    let no_words = instrumental.get_value();
                    let words = lyrics.get_value().trim().to_string();
                    if !no_words && words.is_empty() {
                        speech::say(&t("Add lyrics, or tick Instrumental."));
                        return;
                    }
                    let chosen = models[model.get_selection().unwrap_or(0) as usize].clone();
                    let mut request = GenerateRequest::new(&chosen.external_key, Mode::Custom);
                    request.title = title.get_value().trim().to_string();
                    request.prompt = if no_words { String::new() } else { words };
                    request.instrumental = no_words;
                    request.tags = style.get_value().trim().to_string();
                    request.negative_tags = exclude.get_value().trim().to_string();
                    request.task = Some("playlist_condition".into());
                    request.playlist_id = Some("inspiration".into());
                    request.playlist_clip_ids = Some(vec![song.id.clone()]);
                    request.weirdness = tool_form::percent(&weirdness, &suno_weirdness);
                    request.style_influence = tool_form::percent(&influence, &suno_influence);
                    request.max_mode = max_mode.get_value();
                    if Self::sliders_fit(&chosen, &request) {
                        app.start_job(&tf("Making a song inspired by {0}.", &[&song.display_title()]), request, &chosen);
                    }
                });
            });
        });
    }

    pub(crate) fn change_speed(&mut self) {
        self.with_song(|app, song| {
            if !app.finished(&song) {
                return;
            }
            let intro = tf("Makes a new song from {0} at another speed. Leave the title empty for one that says the speed.", &[&song.display_title()]);
            let form = ToolForm::new(&app.frame, &tf("Change the speed of {0}", &[&song.display_title()]), &intro);
            let percent = form.ranged_slider("_Speed in percent: 100 is as it is, 50 is half speed, 200 is double", 100, 25, 400);
            let keep_pitch = form.check("_Keep the pitch (otherwise it gets higher when faster, like a tape)", true);
            let title = form.text("_Title", "", 1);
            form.show(&t("Change speed"), move || {
                let multiplier = (f64::from(percent.get_value()) / 100.0 * 10_000.0).round() / 10_000.0;
                if (multiplier - 1.0).abs() < f64::EPSILON {
                    speech::say(&t("Choose a speed other than 100."));
                    return;
                }
                let typed = title.get_value().trim().to_string();
                let name = if typed.is_empty() { titled(&song, &format!("{}x", trim(multiplier))) } else { typed };
                with_app(|app| app.start_edit(&song, &tf("Changing the speed of {0}.", &[&song.display_title()]), EditJob::Speed { multiplier, keep_pitch: keep_pitch.get_value(), title: name }));
            });
        });
    }

    pub(crate) fn reverse(&mut self) {
        self.with_song(|app, song| {
            if !app.finished(&song) {
                return;
            }
            let form = ToolForm::new(&app.frame, &tf("Reverse {0}", &[&song.display_title()]), &tf("Makes a new song from {0} played backwards.", &[&song.display_title()]));
            let title = form.text("_Title", &titled(&song, &t("Reversed")), 1);
            form.show(&t("Reverse"), move || {
                let typed = title.get_value().trim().to_string();
                let name = if typed.is_empty() { titled(&song, &t("Reversed")) } else { typed };
                with_app(|app| app.start_edit(&song, &tf("Reversing {0}.", &[&song.display_title()]), EditJob::Reverse { title: name }));
            });
        });
    }

    pub(crate) fn crop(&mut self) {
        self.with_song(|app, song| {
            if !app.finished(&song) {
                return;
            }
            let length = song.duration().unwrap_or(0.0);
            let max = if length > 0.0 { length } else { 3600.0 };
            let intro = tf("Makes a new song from {0} ({1}): keep only the part between From and To, or take that part out.", &[&song.display_title(), &format_time(length)]);
            let form = ToolForm::new(&app.frame, &tf("Crop {0}", &[&song.display_title()]), &intro);
            let from = form.time("_From (seconds, or minutes:seconds)", Some(0.0));
            let to = form.time("_To (seconds, or minutes:seconds)", (length > 0.0).then_some(length));
            let kind = form.choice("_What to do with that part", vec![t("Keep only this part"), t("Take this part out")], 0);
            let title = form.text("T_itle (empty says what was done)", "", 1);
            form.show(&t("Crop"), move || {
                let within = |text: &TextCtrl, name: &str| match tool_form::seconds(text).filter(|s| (0.0..=max).contains(s)) {
                    Some(s) => Some(s),
                    None => {
                        speech::say(&tf("{0} must be between {1} and {2}.", &[&t(name), &format_time(0.0), &format_time(max)]));
                        text.set_focus();
                        None
                    }
                };
                let (Some(start), Some(end)) = (within(&from, "From"), within(&to, "To")) else { return };
                if end <= start {
                    speech::say(&t("To must be later than From."));
                    return;
                }
                let remove = kind.get_selection() == Some(1);
                let typed = title.get_value().trim().to_string();
                let name = if typed.is_empty() { titled(&song, &if remove { t("Section removed") } else { t("Cropped") }) } else { typed };
                with_app(|app| app.start_edit(&song, &tf("Cropping {0}.", &[&song.display_title()]), EditJob::Crop { start, end, remove, title: name }));
            });
        });
    }

    pub(crate) fn fade(&mut self) {
        self.with_song(|app, song| {
            if !app.finished(&song) {
                return;
            }
            let intro = tf("Makes a new song from {0} that fades in at the start, out at the end, or both.", &[&song.display_title()]);
            let form = ToolForm::new(&app.frame, &tf("Fade {0}", &[&song.display_title()]), &intro);
            let fade_in = form.ranged_slider("Fade _in over this many seconds (0 for none)", 0, 0, 60);
            let fade_out = form.ranged_slider("Fade _out over this many seconds (0 for none)", 5, 0, 60);
            let title = form.text("_Title (empty says what was done)", "", 1);
            form.show(&t("Fade"), move || {
                let seconds = |s: &Slider| Some(f64::from(s.get_value())).filter(|v| *v > 0.0);
                let (fade_in, fade_out) = (seconds(&fade_in), seconds(&fade_out));
                let suffix = match (fade_in, fade_out) {
                    (None, None) => {
                        speech::say(&t("Give a fade in, a fade out, or both."));
                        return;
                    }
                    (Some(_), Some(_)) => t("Fade"),
                    (Some(_), None) => t("Fade in"),
                    (None, Some(_)) => t("Fade out"),
                };
                let typed = title.get_value().trim().to_string();
                let name = if typed.is_empty() { titled(&song, &suffix) } else { typed };
                with_app(|app| app.start_edit(&song, &tf("Fading {0}.", &[&song.display_title()]), EditJob::Fade { fade_in, fade_out, title: name }));
            });
        });
    }

    /// Make a video: Suno makes it while you carry on; Tin Pan says when it's ready.
    pub(crate) fn make_video(&mut self) {
        self.with_song(|app, song| {
            if app.finished(&song) {
                speech::say(&tf("Making the video for {0}. You'll hear when it's ready.", &[&song.display_title()]));
                app.net.send(Command::MakeVideo { clip: Box::new(song) });
            }
        });
    }

    pub(crate) fn video_ready(&mut self, clip_id: &str, url: String) {
        let Some(song) = self.find_song(clip_id) else { return };
        self.each_copy(clip_id, |s| s.video_url = Some(url.clone()));
        speech::say_quietly(&tf("The video for {0} is ready. Download video saves it.", &[&song.display_title()]));
    }

    pub(crate) fn make_persona(&mut self) {
        self.with_song(|app, song| {
            if !app.finished(&song) {
                return;
            }
            let mut fields = [
                Field { label: "_Name", value: Value::Text { value: song.title.clone().unwrap_or_default(), multiline: false, max_len: 80 } },
                Field { label: "_Description (the voice and style, for example)", value: Value::Text { value: song.metadata.as_ref().and_then(|m| m.tags.clone()).unwrap_or_default(), multiline: false, max_len: 0 } },
                Field::check("_Public: others can use it too", false),
            ];
            let intro = t("A persona keeps this song's voice and style, so new songs, covers and extends can sound like it. Choose it in the Persona box when you make one.");
            if !form::show(&app.frame, &tf("Make a persona from {0}", &[&song.display_title()]), &intro, &t("Make persona"), &mut fields) {
                return;
            }
            let name = fields[0].text();
            if name.is_empty() {
                speech::say(&t("Give the persona a name."));
                return;
            }
            app.net.send(Command::MakePersona { clip_id: song.id, name, description: fields[1].text(), public: fields[2].checked() });
        });
    }
}

/// 1.5 as "1.5" and 2.0 as "2".
fn trim(value: f64) -> String {
    let text = format!("{value:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}
