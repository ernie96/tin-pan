//! Making new versions of a song (the Song menu's Make from it): Extend, Cover and Remaster, each in a form
//! that stays open, as in Tin Pan 1, so you can make one, listen, change something and make another. The
//! songs they start go to the top of your songs and are followed until they're ready, like new songs.

use tinpan_core::clip::SunoClip;
use tinpan_core::create::{self, GenerateRequest, Mode, RemasterRequest, SunoModel};
use tinpan_core::i18n::{t, tf};
use wxdragon::prelude::*;

use crate::net::Command;
use crate::tool_form::{self, ToolForm};
use crate::{App, speech, with_app};

/// Suno's own values for remaster strength (variation_category) and kind (style_profile), with their words.
const STRENGTHS: [(&str, &str); 3] = [("subtle", "Subtle"), ("normal", "Normal"), ("high", "High")];
const PROFILES: [(&str, &str); 3] = [("natural", "Natural"), ("boost", "Boost"), ("clarity", "Clarity")];

/// Suno only remasters songs up to this long.
const LONGEST_REMASTER: f64 = 960.0;

impl App {
    /// Suno only extends, covers or remasters a song once it's finished.
    pub(crate) fn finished(&self, song: &SunoClip) -> bool {
        if song.is_complete() {
            return true;
        }
        speech::say(&if song.has_failed() { t("That song failed to generate.") } else { t("That song isn't finished yet.") });
        false
    }

    fn plan_models(&self) -> Vec<SunoModel> {
        self.billing.as_ref().map(|b| create::usable(&b.models)).unwrap_or_default()
    }

    /// The models that can make `what` with these sources, as Suno's web client offers them, and the one to
    /// start on; None (said) when none can. Picking one Suno can't use was a way to get "song failed".
    pub(crate) fn models_for(&self, task: &str, what: &str, conditions: &[&str]) -> Option<(Vec<SunoModel>, usize)> {
        let plan = self.plan_models();
        if plan.is_empty() {
            speech::say(&t("Models haven't loaded yet. Press F5 in the Create window (Ctrl+N), then try again."));
            return None;
        }
        let fits = create::models_for(&plan, task, conditions);
        if fits.is_empty() {
            speech::say(&tf("None of the models on your plan can make {0}.", &[&t(what)]));
            return None;
        }
        let at = create::starting_model(&fits, &self.config.current.last_model);
        Some((fits, at))
    }

    fn persona_items(&self) -> Vec<String> {
        let mut items = vec![t("No persona")];
        items.extend(self.personas.iter().map(|p| p.name.clone()));
        items
    }

    fn persona_at(&self, choice: &Choice) -> Option<String> {
        choice.get_selection().and_then(|i| (i as usize).checked_sub(1)).and_then(|i| self.personas.get(i)).map(|p| p.id.clone())
    }

    /// Says so and returns false when numbers were given for sliders the model doesn't take.
    pub(crate) fn sliders_fit(model: &SunoModel, request: &GenerateRequest) -> bool {
        if !request.has_sliders() || model.takes_sliders() != Some(false) {
            return true;
        }
        speech::say(&tf("{0} doesn't take weirdness or influence numbers. Tick Let Suno choose for each, or choose another model.", &[&model.display_name()]));
        false
    }

    /// Style boost and Style ideas inside a form put their result in this form's Style box.
    pub(crate) fn form_style_buttons(form: &ToolForm, style: TextCtrl, lyrics: TextCtrl, instrumental: Option<CheckBox>) {
        form.button("Style _boost", move || {
            with_app(|app| {
                let text = style.get_value().trim().to_string();
                if text.is_empty() {
                    speech::say(&t("Type a style first, even a word or two."));
                    return;
                }
                let no_words = instrumental.is_some_and(|i| i.get_value());
                app.form_text = Some(style);
                speech::say(&t("Boosting the style."));
                app.net.send(Command::BoostStyle { style: text, lyrics: (!no_words).then(|| lyrics.get_value()), instrumental: no_words, to_form: true });
            });
        });
        form.button("Style i_deas...", move || {
            with_app(|app| {
                app.form_text = Some(style);
                app.net.send(Command::StyleIdeas { to_form: true });
            });
        });
    }

    // ── Extend ──────────────────────────────────────────────────────────

    /// Ctrl+E: continues the song from a point you choose.
    pub(crate) fn extend(&mut self) {
        self.with_song(|app, song| {
            if !app.finished(&song) {
                return;
            }
            // Uploaded audio is extended with its own task.
            let task = if song.metadata.as_ref().and_then(|m| m.kind.as_deref()) == Some("upload") { "upload_extend" } else { "extend" };
            let Some((models, at)) = app.models_for(task, "extensions", &["extend"]) else { return };
            let length = song.duration().unwrap_or(0.0);
            let intro = tf("Continues {0} from a point you choose. Leave lyrics empty to let Suno carry on. Use Get whole song afterwards to join the parts.", &[&song.display_title()]);
            let form = ToolForm::new(&app.frame, &tf("Extend {0}", &[&song.display_title()]), &intro);
            let from = form.time("Extend f_rom (seconds, or minutes:seconds)", (length > 0.0).then_some(length));
            let title = form.text("_Title", song.title.as_deref().unwrap_or(""), 1);
            let lyrics = form.text("_Lyrics for the new part", "", 6);
            let style = form.text("_Style", song.metadata.as_ref().and_then(|m| m.tags.as_deref()).unwrap_or(""), 1);
            Self::form_style_buttons(&form, style, lyrics, None);
            let exclude = form.text("_Exclude styles", "", 1);
            let model = form.choice("_Model", models.iter().map(SunoModel::display_name).collect(), at);
            let (weirdness, suno_weirdness) = form.slider("_Weirdness", "Let Suno choose the weirdness", 50);
            let (influence, suno_influence) = form.slider("Style in_fluence", "Let Suno choose the style influence", 50);
            let persona = form.choice("_Persona", app.persona_items(), 0);
            let max_mode = form.check("Ma_x mode", false);
            let max = if length > 0.0 { length } else { 3600.0 };
            form.show(&t("Extend"), move || {
                with_app(|app| {
                    let Some(at) = tool_form::seconds(&from).filter(|s| (0.0..=max).contains(s)) else {
                        speech::say(&tf("{0} must be between {1} and {2}.", &[&t("Extend from"), &tinpan_core::clip::format_time(0.0), &tinpan_core::clip::format_time(max)]));
                        from.set_focus();
                        return;
                    };
                    let chosen = models[model.get_selection().unwrap_or(0) as usize].clone();
                    let mut request = GenerateRequest::new(&chosen.external_key, Mode::Custom);
                    request.title = title.get_value().trim().to_string();
                    request.prompt = lyrics.get_value().trim().to_string();
                    request.tags = style.get_value().trim().to_string();
                    request.negative_tags = exclude.get_value().trim().to_string();
                    request.continue_clip_id = Some(song.id.clone());
                    request.continue_at = Some(at);
                    request.task = Some(task.to_string());
                    request.persona_id = app.persona_at(&persona);
                    request.weirdness = tool_form::percent(&weirdness, &suno_weirdness);
                    request.style_influence = tool_form::percent(&influence, &suno_influence);
                    request.max_mode = max_mode.get_value();
                    request.is_remix = true;
                    if Self::sliders_fit(&chosen, &request) {
                        app.start_job(&tf("Extending {0}.", &[&song.display_title()]), request, &chosen);
                    }
                });
            });
        });
    }

    // ── Cover ───────────────────────────────────────────────────────────

    /// Ctrl+Shift+C: the song's words come first (stored, or its timed lyrics), then the form opens.
    pub(crate) fn cover(&mut self) {
        self.with_song(|app, song| {
            if app.finished(&song) && app.models_for("cover", "covers", &["cover"]).is_some() {
                app.net.send(Command::CoverLyrics { clip: Box::new(song) });
            }
        });
    }

    pub(crate) fn cover_form(&mut self, song: SunoClip, original: String) {
        let Some((models, at)) = self.models_for("cover", "covers", &["cover"]) else { return };
        let was_instrumental = song.metadata.as_ref().and_then(|m| m.make_instrumental) == Some(true) || original.is_empty();
        let intro = tf("Makes a new version of {0} in another style, keeping its melody. The lyrics start as the original words.", &[&song.display_title()]);
        let form = ToolForm::new(&self.frame, &tf("Cover {0}", &[&song.display_title()]), &intro);
        let start_title = song.title.as_deref().map(str::trim).filter(|t| !t.is_empty()).map(|t| tf("{0} (Cover)", &[&t])).unwrap_or_default();
        let title = form.text("_Title", &start_title, 1);
        let lyrics = form.text("_Lyrics (change them, or keep the original words)", &original, 8);
        let change = form.text("_How to change the lyrics (for example: make the chorus rhyme)", "", 1);
        let style = form.text("New _style", song.metadata.as_ref().and_then(|m| m.tags.as_deref()).unwrap_or(""), 1);
        form.button("_Rewrite the lyrics now", move || {
            with_app(|app| {
                if lyrics.get_value().trim().is_empty() {
                    speech::say(&t("Put lyrics in the Lyrics box first."));
                    return;
                }
                let how = change.get_value().trim().to_string();
                if how.is_empty() {
                    speech::say(&t("Say how to change them in the How to change box first."));
                    return;
                }
                app.form_text = Some(lyrics);
                speech::say(&t("Rewriting the lyrics."));
                app.net.send(Command::Cowrite { lyrics: lyrics.get_value(), instruction: how, title: title.get_value(), style: style.get_value(), to_form: true });
            });
        });
        let words = original.clone();
        form.button("Back to the ori_ginal words", move || {
            lyrics.set_value(&words.replace('\n', "\r\n"));
            speech::say(&t("The original words are back in the Lyrics box."));
        });
        let instrumental = form.check("_Instrumental (no singing)", was_instrumental);
        Self::form_style_buttons(&form, style, lyrics, Some(instrumental));
        let exclude = form.text("_Exclude styles", "", 1);
        let (audio, suno_audio) = form.slider("_Audio influence: how closely to follow the original", "Let Suno choose the audio influence", 50);
        let (weirdness, suno_weirdness) = form.slider("_Weirdness", "Let Suno choose the weirdness", 50);
        let (influence, suno_influence) = form.slider("Style in_fluence", "Let Suno choose the style influence", 50);
        let model = form.choice("_Model", models.iter().map(SunoModel::display_name).collect(), at);
        let persona = form.choice("_Persona", self.persona_items(), 0);
        let max_mode = form.check("Ma_x mode", false);
        form.show(&t("Make cover"), move || {
            with_app(|app| {
                let no_words = instrumental.get_value();
                let words = lyrics.get_value().trim().to_string();
                if !no_words && words.is_empty() {
                    speech::say(&t("Add lyrics, or tick Instrumental."));
                    return;
                }
                let chosen = models[model.get_selection().unwrap_or(0) as usize].clone();
                let persona_id = app.persona_at(&persona);
                if persona_id.is_some() && !chosen.supports("cover", &["persona", "cover"]) && !chosen.supports("cover", &["vox", "cover"]) {
                    speech::say(&tf("{0} can't make a cover with a persona. Choose No persona, or another model.", &[&chosen.display_name()]));
                    return;
                }
                let mut request = GenerateRequest::new(&chosen.cover_key(), Mode::Custom);
                request.title = title.get_value().trim().to_string();
                request.prompt = if no_words { String::new() } else { words };
                request.instrumental = no_words;
                request.tags = style.get_value().trim().to_string();
                request.negative_tags = exclude.get_value().trim().to_string();
                request.cover_clip_id = Some(song.id.clone());
                request.task = Some("cover".into());
                request.persona_id = persona_id;
                request.audio_influence = tool_form::percent(&audio, &suno_audio);
                request.weirdness = tool_form::percent(&weirdness, &suno_weirdness);
                request.style_influence = tool_form::percent(&influence, &suno_influence);
                request.max_mode = max_mode.get_value();
                request.is_remix = true;
                if Self::sliders_fit(&chosen, &request) {
                    app.start_job(&tf("Covering {0}.", &[&song.display_title()]), request, &chosen);
                }
            });
        });
    }

    // ── Remaster ────────────────────────────────────────────────────────

    /// Ctrl+Shift+M: a cleaner, newer-sounding version of the same song.
    pub(crate) fn remaster(&mut self) {
        self.with_song(|app, song| {
            if !app.finished(&song) {
                return;
            }
            let Some(billing) = &app.billing else {
                speech::say(&t("Plan details haven't loaded yet. Press F5 in the Create window (Ctrl+N), then try again."));
                return;
            };
            if billing.is_paid() == Some(false) && billing.free_remasters_left.unwrap_or(0) <= 0 {
                speech::say(&t("Remasters need a Pro or Premier plan."));
                return;
            }
            let models = create::remaster_models(&billing.remaster_models);
            if models.is_empty() {
                speech::say(&t("Suno offers no remaster models to this account."));
                return;
            }
            if song.duration().is_some_and(|d| d > LONGEST_REMASTER) {
                speech::say(&t("Suno only remasters songs up to 16 minutes long."));
                return;
            }
            let intro = t("Makes a cleaner, newer-sounding version of the same song. Strength is how much it may change. Kind: Natural stays close to the original, Boost lifts it, Clarity sharpens it. The five numbers are Suno's fine controls.");
            let form = ToolForm::new(&app.frame, &tf("Remaster {0}", &[&song.display_title()]), &intro);
            let at = models.iter().position(|m| m.is_default).unwrap_or(0);
            let model = form.choice("Remaster _model", models.iter().map(SunoModel::display_name).collect(), at);
            let strength = form.choice("Remaster s_trength", STRENGTHS.iter().map(|(_, l)| t(l)).collect(), 1);
            let profile = form.choice("Remaster _kind", PROFILES.iter().map(|(_, l)| t(l)).collect(), 1);
            let style = form.text("_Style (optional)", "", 1);
            // Each starts at Suno's own value, which isn't sent; moving it sends the new one.
            let freedom = form.plain_slider("_Freedom (Suno uses 0)", 0);
            let tone = form.plain_slider("T_one (Suno uses 50)", 50);
            let clarity = form.plain_slider("_Clarity (Suno uses 50)", 50);
            let fine = form.plain_slider("Stre_ngth (Suno uses 50)", 50);
            let stereo = form.plain_slider("Stereo _width (Suno uses 50)", 50);
            form.show(&t("Remaster"), move || {
                let request = RemasterRequest {
                    clip_id: song.id.clone(),
                    model: models[model.get_selection().unwrap_or(0) as usize].external_key.clone(),
                    style: style.get_value(),
                    strength: STRENGTHS[strength.get_selection().unwrap_or(1) as usize].0.into(),
                    profile: PROFILES[profile.get_selection().unwrap_or(1) as usize].0.into(),
                    freedom: Some(f64::from(freedom.get_value())),
                    tone: Some(f64::from(tone.get_value())),
                    clarity: Some(f64::from(clarity.get_value())),
                    fine_strength: Some(f64::from(fine.get_value())),
                    stereo_width: Some(f64::from(stereo.get_value())),
                };
                let starting = tf("Remastering {0}.", &[&song.display_title()]);
                with_app(|app| {
                    speech::say(&starting);
                    app.net.send(Command::Remaster { request: Box::new(request) });
                });
            });
        });
    }

    /// Sends a made-from-a-song request (extend, cover), saying what started.
    pub(crate) fn start_job(&mut self, starting: &str, request: GenerateRequest, model: &SunoModel) {
        if let Some(problem) = request.check_lengths(model.limits()) {
            speech::say(&problem);
            return;
        }
        speech::say(starting);
        self.net.send(Command::Generate { request: Box::new(request), model_takes_voice: model.takes_voice_personas(), job: true });
    }

    /// A Style boost, Style ideas or rewrite asked for in a form: its box, while the form is still open.
    pub(crate) fn form_box(&self) -> Option<TextCtrl> {
        self.form_text.filter(|b| b.is_valid())
    }
}
