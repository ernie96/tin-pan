//! Making a song, in a window of its own (Ctrl+N), as in Tin Pan 1: it stays open beside the Library, so you
//! can make a song, listen to it, change the style and make another. Escape or Close hides it; the fields keep
//! what you typed until Tin Pan closes. Starting songs, the captcha and following them until they're ready
//! are App methods at the end.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use quiet_list::Select;
use tinpan_core::clip::SunoClip;
use tinpan_core::create::{self, GenerateRequest, LyricsDraft, Mode, Persona, SunoModel};
use tinpan_core::i18n::{plural, t, tf};
use wxdragon::prelude::*;

use crate::form::wx_label;
use crate::net::Command;
use crate::{App, captcha, dialogs, form, speech, with_app};

/// Songs Suno is still making are checked this often, and given up on after this long, as in Tin Pan 1.
pub const WATCH_EVERY: Duration = Duration::from_secs(5);
const GIVE_UP_AFTER: Duration = Duration::from_secs(10 * 60);
const FAILURES_IN_A_ROW: u8 = 5;

#[derive(Clone, Copy)]
pub struct CreateWindow {
    dialog: Dialog,
    model: Choice,
    style: TextCtrl,
    exclude: TextCtrl,
    voice: Choice,
    instrumental: CheckBox,
    max_mode: CheckBox,
    weirdness: Slider,
    suno_weirdness: CheckBox,
    influence: Slider,
    suno_influence: CheckBox,
    persona: Choice,
    persona_id: TextCtrl,
    description: TextCtrl,
    title: TextCtrl,
    lyrics: TextCtrl,
    topic: TextCtrl,
    generated: TextCtrl,
    cowrite: TextCtrl,
}

/// A 0 to 100 slider and, after it, "Let Suno choose", ticked to start. Moving the slider unticks it, so the
/// number is sent; ticking it again leaves the choice to Suno. The screen reader says the slider's number.
fn slider(parent: &ScrolledWindow, sizer: &BoxSizer, label: &str, suno_chooses: &str) -> (Slider, CheckBox) {
    let label = StaticText::builder(parent).with_label(&wx_label(label)).build();
    sizer.add(&label, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 10);
    let slider = Slider::builder(parent).with_value(50).with_min_value(0).with_max_value(100).with_size(Size::new(300, -1)).build();
    let let_suno = CheckBox::builder(parent).with_label(&t(suno_chooses)).build();
    let_suno.set_value(true);
    slider.on_slider(move |_| let_suno.set_value(false));
    let row = BoxSizer::builder(Orientation::Horizontal).build();
    row.add(&slider, 0, SizerFlag::AlignCenterVertical, 0);
    row.add(&let_suno, 0, SizerFlag::AlignCenterVertical | SizerFlag::Left, 12);
    sizer.add_sizer(&row, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 6);
    (slider, let_suno)
}

/// Two buttons side by side.
fn buttons(parent: &ScrolledWindow, sizer: &BoxSizer, first: (&str, fn(&mut App)), second: (&str, fn(&mut App))) {
    let row = BoxSizer::builder(Orientation::Horizontal).build();
    for (label, run) in [first, second] {
        let button = Button::builder(parent).with_label(&wx_label(label)).build();
        button.on_click(move |_| with_app(run));
        row.add(&button, 0, SizerFlag::Right, 10);
    }
    sizer.add_sizer(&row, 0, SizerFlag::Left | SizerFlag::Top, 6);
}

fn heading(parent: &ScrolledWindow, sizer: &BoxSizer, text: &str) {
    let label = StaticText::builder(parent).with_label(&t(text)).build();
    if let Some(mut font) = label.get_font() {
        font.make_bold();
        font.set_point_size(font.get_point_size() + 2);
        label.set_font(&font);
    }
    sizer.add(&label, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 16);
}

/// A label and its box; NVDA names the box after the label made just before it.
fn text_box(parent: &ScrolledWindow, sizer: &BoxSizer, label: &str, lines: i32, read_only: bool) -> TextCtrl {
    let label = StaticText::builder(parent).with_label(&wx_label(label)).build();
    sizer.add(&label, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 10);
    let mut style = if lines > 1 { TextCtrlStyle::MultiLine } else { TextCtrlStyle::Default };
    if read_only {
        style |= TextCtrlStyle::ReadOnly;
    }
    let text = TextCtrl::builder(parent).with_style(style).with_size(Size::new(-1, if lines > 1 { 22 * lines } else { -1 })).build();
    sizer.add(&text, 0, SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 6);
    text
}

fn choice(parent: &ScrolledWindow, sizer: &BoxSizer, label: &str, items: Vec<String>) -> Choice {
    let label = StaticText::builder(parent).with_label(&wx_label(label)).build();
    sizer.add(&label, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 10);
    let choice = Choice::builder(parent).with_choices(items).build();
    choice.set_selection(0);
    sizer.add(&choice, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 6);
    choice
}

fn check(parent: &ScrolledWindow, sizer: &BoxSizer, label: &str) -> CheckBox {
    let check = CheckBox::builder(parent).with_label(&wx_label(label)).build();
    sizer.add(&check, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 10);
    check
}

fn button(parent: &ScrolledWindow, sizer: &BoxSizer, label: &str, run: impl Fn(&mut App) + 'static) {
    let button = Button::builder(parent).with_label(&wx_label(label)).build();
    button.on_click(move |_| with_app(|app| run(app)));
    sizer.add(&button, 0, SizerFlag::Left | SizerFlag::Top, 6);
}

pub fn voice_choices() -> Vec<String> {
    vec![t("Any voice"), t("Male vocals"), t("Female vocals")]
}

impl CreateWindow {
    fn build(parent: &Frame) -> Self {
        let dialog = Dialog::builder(parent, &t("Create a song")).with_style(DialogStyle::DefaultDialogStyle | DialogStyle::ResizeBorder).with_size(720, 760).build();
        let panel = ScrolledWindow::builder(&dialog).build();
        panel.set_scroll_rate(0, 20);
        let s = BoxSizer::builder(Orientation::Vertical).build();

        heading(&panel, &s, "Settings for both ways of making a song");
        let model = choice(&panel, &s, "_Model", vec![t("Models haven't loaded yet.")]);
        let style = text_box(&panel, &s, "_Style", 1, false);
        buttons(&panel, &s, ("Style _boost", App::boost_style), ("Style ideas...", App::style_ideas));
        let exclude = text_box(&panel, &s, "_Exclude styles", 1, false);
        let voice = choice(&panel, &s, "_Voice", voice_choices());
        let instrumental = check(&panel, &s, "_Instrumental (no singing)");
        let max_mode = check(&panel, &s, "Ma_x mode (Suno's highest quality)");
        let (weirdness, suno_weirdness) = slider(&panel, &s, "_Weirdness", "Let Suno choose the weirdness");
        let (influence, suno_influence) = slider(&panel, &s, "Style in_fluence", "Let Suno choose the style influence");
        let persona = choice(&panel, &s, "_Persona", vec![t("No persona")]);
        let persona_id = text_box(&panel, &s, "Or type _a persona ID", 1, false);

        heading(&panel, &s, "Describe a song");
        let description = text_box(&panel, &s, "_Description", 2, false);
        button(&panel, &s, "_Create from description", |app| app.start_songs(Mode::Simple));

        heading(&panel, &s, "Your own lyrics");
        let title = text_box(&panel, &s, "_Title", 1, false);
        let lyrics = text_box(&panel, &s, "_Lyrics", 6, false);
        button(&panel, &s, "C_reate with my lyrics", |app| app.start_songs(Mode::Custom));

        heading(&panel, &s, "Write lyrics for me");
        let topic = text_box(&panel, &s, "T_opic", 1, false);
        buttons(&panel, &s, ("Write l_yrics", App::write_lyrics), ("Lyrics drafts...", App::lyrics_drafts));
        let generated = text_box(&panel, &s, "_Generated lyrics", 4, true);
        button(&panel, &s, "_Use these lyrics", App::use_generated_lyrics);

        heading(&panel, &s, "Change my lyrics");
        let cowrite = text_box(&panel, &s, "What to c_hange (for example: make the chorus rhyme)", 1, false);
        button(&panel, &s, "Rewrite my lyrics _now", App::cowrite);
        panel.set_sizer(s, true);

        let close = Button::builder(&dialog).with_id(ID_CANCEL).with_label(&t("Close")).build();
        let outer = BoxSizer::builder(Orientation::Vertical).build();
        outer.add(&panel, 1, SizerFlag::Expand, 0);
        outer.add(&close, 0, SizerFlag::AlignRight | SizerFlag::All, 10);
        dialog.set_sizer(outer, true);
        dialog.set_escape_id(ID_CANCEL);
        crate::tool_form::track_active(&dialog, Some(dialog));
        // Closing only hides it, so what you typed is still there next time.
        close.on_click(move |_| dialog.show(false));
        dialog.on_close(move |event| {
            dialog.show(false);
            event.skip(false);
        });
        // F5 reloads the models and personas here, as on Tin Pan 1's Create tab; F6 goes to the main window.
        let window_handle = dialog.get_handle();
        dialog.on_char_hook(move |event| {
            let WindowEventData::Keyboard(ref key) = event else { return };
            let Some(code) = key.get_key_code() else { return event.skip(true) };
            match (code, key.control_down() || key.alt_down()) {
                (WXK_F1, false) => speech::say(&t("Create a song. Set the model, style and voice, then describe a song or use your own lyrics. Tab moves between fields. F5 reloads the models. F8 plays or pauses from here. The window stays open until you press Escape.")),
                (WXK_F5, false) => with_app(App::refresh_create_choices),
                (WXK_F6, false) => with_app(|app| app.next_window_from(Some(window_handle), key.shift_down())),
                _ => match crate::commands::playback_key(code, key.control_down(), key.shift_down(), key.alt_down()) {
                    // The playback keys work here too, so you can listen to what you made.
                    Some(action) => with_app(|app| app.run_from_button(action)),
                    None => {
                        event.skip(true);
                        return;
                    }
                },
            }
            event.skip(false);
        });
        Self {
            dialog,
            model,
            style,
            exclude,
            voice,
            instrumental,
            max_mode,
            weirdness,
            suno_weirdness,
            influence,
            suno_influence,
            persona,
            persona_id,
            description,
            title,
            lyrics,
            topic,
            generated,
            cowrite,
        }
    }

    fn set_items(choice: &Choice, items: &[String], selected: usize) {
        choice.clear();
        for item in items {
            choice.append(item);
        }
        choice.set_selection(selected.min(items.len().saturating_sub(1)) as u32);
    }
}

impl App {
    /// Ctrl+N: the Create window, opened where it was left.
    pub(crate) fn open_create(&mut self) {
        if !self.signed_in {
            speech::say(&t("Not signed in."));
            return;
        }
        let window = *self.create.get_or_insert_with(|| CreateWindow::build(&self.frame));
        self.fill_create_choices();
        window.dialog.show(true);
        window.dialog.raise();
        // Focus starts on the description, the quickest way to a song, unless you were in lyrics.
        if window.lyrics.get_value().trim().is_empty() {
            window.description.set_focus();
        } else {
            window.lyrics.set_focus();
        }
    }

    pub(crate) fn refresh_create_choices(&mut self) {
        self.net.send(Command::Account { say: None });
        self.net.send(Command::Personas);
        speech::say_quietly(&t("Refreshing."));
    }

    fn usable_models(&self) -> Vec<SunoModel> {
        self.billing.as_ref().map(|b| create::usable(&b.models)).unwrap_or_default()
    }

    /// The model and persona lists, from the plan and your personas as last loaded; the choices stay where they were.
    pub(crate) fn fill_create_choices(&mut self) {
        let Some(window) = self.create else { return };
        let models = self.usable_models();
        if !models.is_empty() {
            let keep = window.model.get_selection().and_then(|i| models.get(i as usize)).map(|m| m.external_key.clone());
            let names: Vec<String> = models.iter().map(SunoModel::display_name).collect();
            let at = create::starting_model(&models, keep.as_deref().unwrap_or(&self.config.current.last_model));
            CreateWindow::set_items(&window.model, &names, at);
        }
        let keep = window.persona.get_selection().unwrap_or(0) as usize;
        let mut names = vec![t("No persona")];
        names.extend(self.personas.iter().map(|p| p.name.clone()));
        CreateWindow::set_items(&window.persona, &names, keep);
    }

    pub(crate) fn personas_loaded(&mut self, personas: Vec<Persona>) {
        self.personas = personas;
        self.fill_create_choices();
    }

    /// Builds the request from the window; None (having said why) when something's off.
    fn build_request(&mut self, window: &CreateWindow, mode: Mode) -> Option<(GenerateRequest, Option<bool>)> {
        let models = self.usable_models();
        let Some(model) = window.model.get_selection().and_then(|i| models.get(i as usize)).cloned() else {
            speech::say(&t("Models haven't loaded yet. Press F5 to refresh, then try again."));
            return None;
        };
        let percent = |slider: &Slider, suno_chooses: &CheckBox| (!suno_chooses.get_value()).then(|| f64::from(slider.get_value()));
        let weirdness = percent(&window.weirdness, &window.suno_weirdness);
        let influence = percent(&window.influence, &window.suno_influence);
        let simple = mode == Mode::Simple;
        let instrumental = window.instrumental.get_value();
        let voice = create::voice_tag(window.voice.get_selection().unwrap_or(0) as usize);
        let tags = [window.style.get_value().trim().to_string(), voice.to_string()].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(", ");
        let mut request = GenerateRequest::new(&model.external_key, mode);
        // Suno sings whatever `prompt` holds, so a description goes in gpt_description_prompt instead, as Suno's
        // own Create form sends it; a description left in prompt comes back as lyrics.
        request.prompt = if simple || instrumental { String::new() } else { window.lyrics.get_value().trim().to_string() };
        request.description = simple.then(|| window.description.get_value().trim().to_string());
        request.title = if simple { String::new() } else { window.title.get_value().trim().to_string() };
        request.tags = tags;
        request.negative_tags = window.exclude.get_value().trim().to_string();
        request.instrumental = instrumental;
        request.max_mode = window.max_mode.get_value();
        request.weirdness = weirdness;
        request.style_influence = influence;
        let typed = window.persona_id.get_value().trim().to_string();
        let chosen = window.persona.get_selection().and_then(|i| (i as usize).checked_sub(1)).and_then(|i| self.personas.get(i)).map(|p| p.id.clone());
        request.persona_id = if typed.is_empty() { chosen } else { Some(typed) };
        if let Some(problem) = request.check_lengths(model.limits()) {
            speech::say(&problem);
            return None;
        }
        if self.config.current.last_model != model.external_key {
            self.config.current.last_model = model.external_key.clone();
            self.save_settings();
        }
        Some((request, model.takes_voice_personas()))
    }

    /// Create from description, or Create with my lyrics.
    pub(crate) fn start_songs(&mut self, mode: Mode) {
        let Some(window) = self.create else { return };
        if !self.signed_in {
            speech::say(&t("Not signed in."));
            return;
        }
        match mode {
            Mode::Simple if window.description.get_value().trim().is_empty() => {
                speech::say(&t("Describe the song first."));
                window.description.set_focus();
                return;
            }
            Mode::Custom if window.lyrics.get_value().trim().is_empty() && !window.instrumental.get_value() => {
                speech::say(&t("Add lyrics, or tick Instrumental."));
                window.lyrics.set_focus();
                return;
            }
            _ => {}
        }
        let Some((request, takes_voice)) = self.build_request(&window, mode) else { return };
        speech::say(&t("Sending to Suno."));
        self.net.send(Command::Generate { request: Box::new(request), model_takes_voice: takes_voice, job: false });
    }

    /// Suno wants a captcha first: it's passed in a web view here, then the request goes again with its token.
    pub(crate) fn captcha_needed(&mut self, mut request: GenerateRequest, provider: i32, model_takes_voice: Option<bool>, job: bool) {
        log::info!("Captcha required (provider {provider})");
        captcha::solve(&self.frame, provider, move |result| match result {
            Ok((token, used)) => with_app(|app| {
                request.token = Some(token);
                request.token_provider = Some(used);
                app.net.send(Command::Generate { request: Box::new(request), model_takes_voice, job });
            }),
            Err(why) => speech::say(&why),
        });
    }

    /// Suno started the songs: they go to the top of your songs and are followed until they're ready.
    pub(crate) fn songs_started(&mut self, clips: Vec<SunoClip>, job: bool) {
        let count = clips.len();
        self.upsert_songs(&clips);
        self.watch(&clips);
        speech::say_quietly(&match (job, count) {
            (true, 0) => t("Suno accepted it. New songs will show up when you refresh."),
            (true, 1) => t("Started. You'll hear when it's ready."),
            (true, _) => t("Started. You'll hear when they're ready."),
            (false, 1) => t("Making 1 song. You'll hear when it's ready."),
            (false, _) => tf("Making {0} songs. You'll hear when they're ready.", &[&count]),
        });
        self.net.send(Command::Account { say: None });
    }

    /// New songs go at the top of your songs; ones already shown are brought up to date wherever they are.
    pub(crate) fn upsert_songs(&mut self, clips: &[SunoClip]) {
        for clip in clips.iter().rev() {
            if self.find_song(&clip.id).is_some() {
                self.each_copy(&clip.id, |s| *s = clip.clone());
            } else {
                self.library().songs.insert(0, clip.clone());
            }
        }
        self.refresh_rows(Select::Keep);
    }

    /// Follows songs Suno is still making ("streaming" can play, but only "complete" is the finished song).
    pub(crate) fn watch(&mut self, clips: &[SunoClip]) {
        for clip in clips.iter().filter(|c| c.status.as_deref() != Some("complete") && !c.has_failed()) {
            self.watching.entry(clip.id.clone()).or_insert_with(Instant::now);
        }
    }

    /// Every few seconds: how the songs being made are doing.
    pub(crate) fn check_watched(&mut self) {
        self.track_resume();
        self.watching.retain(|_, since| since.elapsed() < GIVE_UP_AFTER);
        if self.watching.is_empty() || !self.signed_in || self.checking_watched {
            return;
        }
        self.checking_watched = true;
        self.net.send(Command::CheckSongs { ids: self.watching.keys().cloned().collect() });
    }

    pub(crate) fn watched_checked(&mut self, result: Result<Vec<SunoClip>, tinpan_core::Error>) {
        self.checking_watched = false;
        let clips = match result {
            Ok(clips) => {
                self.watch_failures = 0;
                clips
            }
            Err(e) => {
                log::warn!("Checking on new songs failed: {e}");
                self.watch_failures += 1;
                if matches!(e, tinpan_core::Error::SignInRequired(_)) || self.watch_failures >= FAILURES_IN_A_ROW {
                    self.watching.clear();
                }
                return;
            }
        };
        self.upsert_songs(&clips);
        let finished: Vec<&SunoClip> = clips.iter().filter(|c| c.status.as_deref() == Some("complete") || c.has_failed()).collect();
        for clip in &finished {
            self.watching.remove(&clip.id);
        }
        if !finished.is_empty() {
            speech::say_quietly(&finished_text(&finished));
        }
    }

    pub(crate) fn write_lyrics(&mut self) {
        let Some(window) = self.create else { return };
        let topic = window.topic.get_value().trim().to_string();
        if topic.is_empty() {
            speech::say(&t("Type a topic first."));
            window.topic.set_focus();
            return;
        }
        speech::say(&t("Writing lyrics."));
        self.net.send(Command::WriteLyrics { topic });
    }

    pub(crate) fn lyrics_written(&mut self, title: Option<String>, text: String) {
        let Some(window) = self.create else { return };
        window.generated.set_value(&text);
        if window.title.get_value().trim().is_empty()
            && let Some(title) = title.filter(|t| !t.trim().is_empty())
        {
            window.title.set_value(&title);
        }
        speech::say(&t("Lyrics ready."));
    }

    pub(crate) fn use_generated_lyrics(&mut self) {
        let Some(window) = self.create else { return };
        let text = window.generated.get_value();
        if text.trim().is_empty() {
            return;
        }
        window.lyrics.set_value(&text);
        speech::say(&t("Copied into Lyrics."));
    }
}

impl App {
    /// The Create window, if it's open.
    pub(crate) fn shown_create(&self) -> Option<CreateWindow> {
        self.create.filter(|w| w.dialog.is_shown())
    }

    /// F6 and Shift+F6: on to the next of Tin Pan's open windows (the main one, Create, the forms that stay
    /// open), or back, as in Tin Pan 1. `from` is the window it was pressed in; None is the main window.
    pub(crate) fn next_window_from(&mut self, from: Option<*mut std::ffi::c_void>, back: bool) {
        let mut ring: Vec<Option<Dialog>> = vec![None];
        ring.extend(self.shown_create().map(|w| Some(w.dialog)));
        ring.extend(crate::tool_form::open_forms().into_iter().filter(|d| d.is_shown()).map(Some));
        if ring.len() < 2 {
            speech::say(&t("No other window is open."));
            return;
        }
        let at = from.and_then(|h| ring.iter().position(|w| w.is_some_and(|d| d.get_handle() == h))).unwrap_or(0);
        let next = (at + if back { ring.len() - 1 } else { 1 }) % ring.len();
        match ring[next] {
            None => {
                self.frame.raise();
                self.list().set_focus();
            }
            Some(dialog) => {
                dialog.raise();
                dialog.set_focus();
                // The Create window starts on the description, as when it opens.
                if let Some(create) = self.create.filter(|w| w.dialog.get_handle() == dialog.get_handle()) {
                    create.description.set_focus();
                }
            }
        }
    }

    pub(crate) fn boost_style(&mut self) {
        let Some(window) = self.shown_create() else { return };
        let style = window.style.get_value().trim().to_string();
        if style.is_empty() {
            speech::say(&t("Type a style first, even a word or two."));
            window.style.set_focus();
            return;
        }
        let instrumental = window.instrumental.get_value();
        let lyrics = (!instrumental).then(|| window.lyrics.get_value()).filter(|l| !l.trim().is_empty());
        speech::say(&t("Boosting the style."));
        self.net.send(Command::BoostStyle { style, lyrics, instrumental, to_form: false });
    }

    /// The Style box a boost or an idea goes into: the open form's, or the Create window's.
    fn style_box(&self, to_form: bool) -> Option<TextCtrl> {
        if to_form { self.form_box() } else { self.create.map(|w| w.style) }
    }

    pub(crate) fn style_boosted(&mut self, style: String, to_form: bool) {
        let Some(target) = self.style_box(to_form) else { return };
        target.set_value(&style);
        speech::say(&tf("Style boosted: {0}", &[&style]));
    }

    pub(crate) fn style_ideas(&mut self) {
        self.net.send(Command::StyleIdeas { to_form: false });
    }

    /// Style ideas from Suno; Enter puts one in the Style box.
    pub(crate) fn style_ideas_loaded(&mut self, ideas: Vec<String>, to_form: bool) {
        let Some(target) = self.style_box(to_form) else { return };
        if ideas.is_empty() {
            speech::say(&t("Suno has no style ideas right now."));
            return;
        }
        if let Some(i) = dialogs::pick(&self.frame, &t("Style ideas. Enter puts one in the Style box."), &ideas) {
            target.set_value(&ideas[i]);
            speech::say(&tf("Style: {0}", &[&ideas[i]]));
        }
        target.set_focus();
    }

    /// Change my lyrics: Suno changes the Lyrics the way you ask; the result lands in Generated lyrics.
    pub(crate) fn cowrite(&mut self) {
        let Some(window) = self.shown_create() else { return };
        let lyrics = window.lyrics.get_value();
        if lyrics.trim().is_empty() {
            speech::say(&t("Put lyrics in the Lyrics box first. Write lyrics makes some from a topic."));
            window.lyrics.set_focus();
            return;
        }
        let instruction = window.cowrite.get_value().trim().to_string();
        if instruction.is_empty() {
            speech::say(&t("Say what to change, for example: make the chorus rhyme."));
            window.cowrite.set_focus();
            return;
        }
        speech::say(&t("Rewriting the lyrics."));
        self.net.send(Command::Cowrite { lyrics, instruction, title: window.title.get_value(), style: window.style.get_value(), to_form: false });
    }

    /// Changed lyrics: into the Cover form's Lyrics box when it asked, otherwise into Generated lyrics.
    pub(crate) fn cowritten(&mut self, text: String, to_form: bool) {
        if to_form {
            if let Some(lyrics) = self.form_box() {
                lyrics.set_value(&text.replace('\n', "\r\n"));
                speech::say(&t("New lyrics are in the Lyrics box. Back to the original words undoes it."));
            }
            return;
        }
        let Some(window) = self.create else { return };
        window.generated.set_value(&text);
        speech::say(&t("New lyrics ready in Generated lyrics. Use these lyrics copies them in."));
    }

    pub(crate) fn lyrics_drafts(&mut self) {
        self.net.send(Command::LyricsDrafts);
    }

    /// Drafts from Suno's lyrics editor. The first line saves the Lyrics box as a new draft; Enter on a draft
    /// offers putting it in the Lyrics box, saving the Lyrics box into it, renaming and deleting.
    pub(crate) fn lyrics_drafts_loaded(&mut self, drafts: Vec<LyricsDraft>) {
        let Some(window) = self.create else { return };
        let mut lines = vec![t("Save the Lyrics box as a new draft...")];
        lines.extend(drafts.iter().map(LyricsDraft::spoken_text));
        let title = if drafts.is_empty() {
            t("Lyrics drafts. You have none on Suno yet.")
        } else {
            plural(drafts.len() as i64, "Lyrics drafts. {0} draft; Enter on one for what to do with it.", "Lyrics drafts. {0} drafts; Enter on one for what to do with it.")
        };
        let Some(chosen) = dialogs::pick(&window.dialog, &title, &lines) else { return };
        let Some(draft) = chosen.checked_sub(1).and_then(|i| drafts.get(i)).cloned() else {
            self.new_lyrics_draft(&window);
            return;
        };
        let actions = [t("Put it in the Lyrics box"), t("Save the Lyrics box into it"), t("Rename..."), t("Delete...")];
        match dialogs::pick(&window.dialog, &draft.spoken_text(), &actions) {
            Some(0) => self.net.send(Command::DraftText { draft }),
            Some(1)
                if self.lyrics_to_save(&window)
                    && dialogs::confirm(&window.dialog, &tf("Replace the words in the draft {0} with the Lyrics box?", &[&draft.name()]), &t("Lyrics drafts")) =>
            {
                self.net.send(Command::SaveDraft { draft, lyrics: window.lyrics.get_value() });
            }
            Some(2) => {
                let mut fields = [form::Field { label: "_Name", value: form::Value::Text { value: draft.title.clone(), multiline: false, max_len: 200 } }];
                if form::show(&window.dialog, &tf("Rename {0}", &[&draft.name()]), "", &t("Rename"), &mut fields) && !fields[0].text().is_empty() {
                    self.net.send(Command::RenameDraft { draft, title: fields[0].text() });
                }
            }
            Some(3) if dialogs::confirm(&window.dialog, &tf("Delete the draft {0} from Suno? This can't be undone.", &[&draft.name()]), &t("Delete draft")) => {
                self.net.send(Command::DeleteDraft { draft });
            }
            _ => {}
        }
    }

    fn lyrics_to_save(&self, window: &CreateWindow) -> bool {
        if window.lyrics.get_value().trim().is_empty() {
            speech::say(&t("The Lyrics box is empty, so there's nothing to save."));
            return false;
        }
        true
    }

    fn new_lyrics_draft(&mut self, window: &CreateWindow) {
        if !self.lyrics_to_save(window) {
            return;
        }
        let mut fields = [form::Field { label: "_Draft name", value: form::Value::Text { value: window.title.get_value().trim().to_string(), multiline: false, max_len: 200 } }];
        if form::show(&window.dialog, &t("Save as a new draft"), &t("Saves the Lyrics box to Suno as a lyrics draft."), &t("Save"), &mut fields) {
            self.net.send(Command::NewDraft { title: fields[0].text(), lyrics: window.lyrics.get_value() });
        }
    }

    pub(crate) fn draft_text_loaded(&mut self, draft: LyricsDraft, text: Option<String>) {
        let Some(window) = self.create else { return };
        let Some(text) = text.filter(|t| !t.trim().is_empty()) else {
            speech::say(&t("That draft has no lyrics yet."));
            return;
        };
        if !window.lyrics.get_value().trim().is_empty() && !dialogs::confirm(&window.dialog, &t("Replace the lyrics in the Lyrics box?"), &t("Lyrics drafts")) {
            return;
        }
        window.lyrics.set_value(&text);
        if window.title.get_value().trim().is_empty() && !draft.title.trim().is_empty() {
            window.title.set_value(draft.title.trim());
        }
        window.lyrics.set_focus();
        speech::say(&tf("Lyrics from {0} are in the Lyrics box.", &[&draft.spoken_text()]));
    }
}

/// "Ready: A, B." and, for songs that failed, how many and Suno's reasons, each once; as Tin Pan 1 says it.
pub fn finished_text(clips: &[&SunoClip]) -> String {
    let mut ready: Vec<String> = Vec::new();
    for clip in clips.iter().filter(|c| !c.has_failed()) {
        let title = clip.display_title();
        if !ready.contains(&title) {
            ready.push(title);
        }
    }
    let failed: Vec<&&SunoClip> = clips.iter().filter(|c| c.has_failed()).collect();
    let mut parts = Vec::new();
    if !ready.is_empty() {
        parts.push(tf("Ready: {0}.", &[&ready.join(", ")]));
    }
    for clip in &failed {
        let m = clip.metadata.as_ref();
        log::warn!(
            "Song {} failed: task {}, error_type {}, error_message {}",
            clip.id,
            m.and_then(|m| m.task.as_deref()).unwrap_or("none"),
            m.and_then(|m| m.error_type.as_deref()).unwrap_or("none"),
            m.and_then(|m| m.error_message.as_deref()).unwrap_or("none")
        );
    }
    if !failed.is_empty() {
        let mut reasons: Vec<String> = Vec::new();
        for reason in failed.iter().filter_map(|c| c.failure_reason()) {
            let reason = format!("{}.", reason.trim_end_matches('.'));
            if !reasons.contains(&reason) {
                reasons.push(reason);
            }
        }
        let why = if reasons.is_empty() { t("Suno gave no reason.") } else { reasons.join(" ") };
        parts.push(format!("{} {why}", plural(failed.len() as i64, "{0} song failed.", "{0} songs failed.")));
    }
    parts.join(" ")
}

/// Songs being made, and since when.
pub type Watching = HashMap<String, Instant>;

#[cfg(test)]
mod tests {
    use super::*;

    fn song(id: &str, title: &str, status: &str) -> SunoClip {
        SunoClip { id: id.into(), title: Some(title.into()), status: Some(status.into()), ..SunoClip::default() }
    }

    #[test]
    fn finished_songs_are_said_once_each() {
        let (a, b, c) = (song("1", "Rain", "complete"), song("2", "Rain", "complete"), song("3", "Sun", "error"));
        let text = finished_text(&[&a, &b, &c]);
        assert!(text.starts_with("Ready: Rain. 1 song failed."), "{text}");
    }
}
