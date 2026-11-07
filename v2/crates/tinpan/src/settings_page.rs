//! The Settings tab (Ctrl+5), as Tin Pan 1's Settings page: your Suno account, the language, how much Tin Pan
//! says, downloads and clean-up. Each change is saved as it's made. Sliders replace Tin Pan 1's number boxes,
//! with the same limits; the screen reader says each slider's number.

use tinpan_core::cleanup;
use tinpan_core::config::{AppConfig, CleanupConfig};
use tinpan_core::i18n::{t, tf};
use wxdragon::prelude::*;

use crate::commands::Action;
use crate::form::wx_label;
use crate::live::CUES;
use crate::pages::heading;
use crate::{App, speech, with_app};

const FORMATS: [&str; 2] = ["mp3", "wav"];
/// Tin Pan 1's speech levels: Everything, What I ask for and errors, Only errors.
pub const SPEECH_LEVELS: [(&str, &str); 3] = [
    ("Everything", "Everything"),
    ("AskedFor", "What I ask for and errors, not background news"),
    ("ErrorsOnly", "Only errors and what I ask to hear"),
];
/// Language codes, each named in its own words and, when it isn't the one in use, the current language's too.
const LANGUAGES: [(&str, &str, &str); 5] = [
    ("", "Automatic (Windows language)", "Automatic (Windows language)"),
    ("en", "English", "English"),
    ("ro", "Română", "Romanian"),
    ("uk", "Українська", "Ukrainian"),
    ("es", "Español", "Spanish"),
];

#[derive(Clone, Copy)]
pub struct SettingsPage {
    pub panel: ScrolledWindow,
    status: TextCtrl,
    language: Choice,
    speech_level: Choice,
    playing_cue: Choice,
    live_lyrics: CheckBox,
    main_output: Choice,
    second_output: Choice,
    play_directly: CheckBox,
    autoplay: CheckBox,
    check_updates: CheckBox,
    folder: TextCtrl,
    format: Choice,
    save_lyrics: CheckBox,
    save_timed: CheckBox,
    save_details: CheckBox,
    save_cover: CheckBox,
    after_download: CheckBox,
    keep_original: CheckBox,
    debuzz: CheckBox,
    gate: CheckBox,
    smooth: CheckBox,
    smooth_ms: Slider,
    widen: CheckBox,
    widen_tenths: Slider,
    excite: CheckBox,
    excite_db: Slider,
    matching: CheckBox,
    reference: TextCtrl,
    master: CheckBox,
    master_lufs: Slider,
}

fn language_names() -> Vec<String> {
    let current = tinpan_core::i18n::current();
    LANGUAGES
        .iter()
        .map(|(code, name, english)| match *code {
            "" => t(name),
            c if c == current => name.to_string(),
            _ => format!("{name} ({})", t(english)),
        })
        .collect()
}

struct Builder {
    panel: ScrolledWindow,
    sizer: BoxSizer,
}

impl Builder {
    fn group(&self, text: &str) {
        let label = heading(&self.panel, &t(text));
        self.sizer.add(&label, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 14);
    }

    fn label(&self, text: &str) {
        let label = StaticText::builder(&self.panel).with_label(&wx_label(text)).build();
        self.sizer.add(&label, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 8);
    }

    fn check(&self, text: &str, value: bool) -> CheckBox {
        let check = CheckBox::builder(&self.panel).with_label(&wx_label(text)).build();
        check.set_value(value);
        check.on_toggled(|_| with_app(App::settings_changed));
        self.sizer.add(&check, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 6);
        check
    }

    fn choice(&self, text: &str, items: Vec<String>, selected: usize) -> Choice {
        self.label(text);
        let choice = Choice::builder(&self.panel).with_choices(items).build();
        choice.set_selection(selected as u32);
        choice.on_selection_changed(|_| with_app(App::settings_changed));
        self.sizer.add(&choice, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 4);
        choice
    }

    fn slider(&self, text: &str, value: f64, min: i32, max: i32) -> Slider {
        self.label(text);
        let slider = Slider::builder(&self.panel).with_value(value.round() as i32).with_min_value(min).with_max_value(max).with_size(Size::new(280, -1)).build();
        slider.on_slider(|_| with_app(App::settings_changed));
        self.sizer.add(&slider, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 4);
        slider
    }

    fn button(&self, row: &BoxSizer, text: &str, run: fn(&mut App)) {
        let button = Button::builder(&self.panel).with_label(&wx_label(text)).build();
        button.on_click(move |_| with_app(run));
        row.add(&button, 0, SizerFlag::Left | SizerFlag::AlignCenterVertical, 8);
    }
}

pub fn build(notebook: &Notebook, config: &AppConfig) -> SettingsPage {
    let panel = ScrolledWindow::builder(notebook).build();
    panel.set_scroll_rate(0, 20);
    let b = Builder { panel, sizer: BoxSizer::builder(Orientation::Vertical).build() };
    let title = heading(&panel, &t("Settings"));
    b.sizer.add(&title, 0, SizerFlag::All, 10);

    b.group("Suno account");
    let status = TextCtrl::builder(&panel).with_value(&t("Not signed in.")).with_style(TextCtrlStyle::ReadOnly).build();
    b.sizer.add(&status, 0, SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 8);
    let account_row = BoxSizer::builder(Orientation::Horizontal).build();
    for (label, action) in [("_Sign in", Action::SignIn), ("Sign _out", Action::SignOut)] {
        let button = Button::builder(&panel).with_label(&wx_label(label)).build();
        button.on_click(move |_| with_app(|app| app.run_from_button(action)));
        account_row.add(&button, 0, SizerFlag::Right, 8);
    }
    b.sizer.add_sizer(&account_row, 0, SizerFlag::Left | SizerFlag::Top, 8);

    b.group("Language");
    let language = b.choice("Lan_guage (changes the next time Tin Pan starts)", language_names(), LANGUAGES.iter().position(|(c, ..)| *c == config.language).unwrap_or(0));

    b.group("Speech");
    let speech_level = b.choice("Tin Pan s_peaks", SPEECH_LEVELS.iter().map(|(_, l)| t(l)).collect(), SPEECH_LEVELS.iter().position(|(k, _)| *k == config.speech_level).unwrap_or(0));
    let playing_cue = b.choice("_When a list reaches the playing song", CUES.iter().map(|(_, l)| t(l)).collect(), CUES.iter().position(|(k, _)| *k == config.playing_cue).unwrap_or(0));
    let live_lyrics = b.check("Li_ve lyrics: read each line aloud as it is sung (Shift+F3)", config.audio.read_lyrics_live);
    let autoplay = b.check("When a list ends, keep playing songs Suno picks", config.audio.autoplay);

    b.group("Audio output");
    // Filled from the connected outputs once the app is up (App::settings_show_outputs).
    let main_output = b.choice("Output device", vec![crate::outputs::default_name()], 0);
    let second_output = b.choice("_Quick switch output, for Ctrl+Shift+F12", vec![t("None")], 0);
    let play_directly = b.check("Play songs in Tin Pan itself, which starts them sooner (the Suno player takes over when that fails)", config.audio.play_directly);

    b.group("Updates");
    let check_updates = b.check("Check for _new versions when Tin Pan starts", config.update.check_at_startup);

    b.group("Downloads");
    let audio = &config.audio;
    b.label("_Download folder (empty means Music\\Suno)");
    let folder_row = BoxSizer::builder(Orientation::Horizontal).build();
    let folder = TextCtrl::builder(&panel).with_value(&audio.download_dir).build();
    folder.on_text_changed(|_| with_app(App::settings_changed));
    folder_row.add(&folder, 1, SizerFlag::Expand, 0);
    b.button(&folder_row, "_Browse...", App::browse_download_folder);
    b.sizer.add_sizer(&folder_row, 0, SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 4);
    let format = b.choice("Download _format for Ctrl+D (WAV needs Pro or Premier)", FORMATS.iter().map(ToString::to_string).collect(), FORMATS.iter().position(|f| *f == audio.output_format).unwrap_or(0));
    let also = StaticText::builder(&panel).with_label(&t("Also save next to each download:")).build();
    b.sizer.add(&also, 0, SizerFlag::Left | SizerFlag::Top, 10);
    let save_lyrics = b.check("_Lyrics as text", audio.save_lyrics);
    let save_timed = b.check("_Timed lyrics (LRC)", audio.save_timed_lyrics);
    let save_details = b.check("Song d_etails", audio.save_details);
    let save_cover = b.check("_Cover art", audio.save_cover);

    b.group("Clean-up (sunscreen)");
    let about = tf(
        "sunscreen {0}, by Alan Bradburne (MIT license), cleans up on this computer; nothing is uploaded. It reads MP3 and WAV, so Suno's M4A files are converted to WAV first. The result is always a 24-bit WAV, named like the download with (clean) added.",
        &[&cleanup::VERSION],
    );
    let about_box = TextCtrl::builder(&panel).with_value(&about).with_style(TextCtrlStyle::MultiLine | TextCtrlStyle::ReadOnly | TextCtrlStyle::NoVScroll).with_size(Size::new(-1, 70)).build();
    b.sizer.add(&about_box, 0, SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 8);
    let c = &config.cleanup;
    let after_download = b.check("Clean _up after Download MP3, Download WAV and Download all", c.after_download);
    let keep_original = b.check("_Keep the downloaded file next to the cleaned-up WAV", c.keep_original);
    let steps = StaticText::builder(&panel).with_label(&t("Clean-up steps:")).build();
    b.sizer.add(&steps, 0, SizerFlag::Left | SizerFlag::Top, 10);
    let debuzz = b.check("Remove the decoder bu_zz", c.debuzz);
    let gate = b.check("Only remove haze you can _hear (hearing model)", c.gate);
    let smooth = b.check("Stead_y the flutter of what is left", c.smooth);
    let smooth_ms = b.slider("Flutter smoothing window, milliseconds (sunscreen uses 85)", c.smooth_ms, 20, 300);
    let widen = b.check("W_iden the highs", c.widen);
    let widen_tenths = b.slider("Widening amount in tenths (10 matches the rest of the track, 5 is half)", c.widen_amount * 10.0, 0, 20);
    let excite = b.check("Rebuild the top octave above the MP3 cut-off (e_xcite)", c.excite);
    let excite_db = b.slider("Top octave level, dB below the natural slope (sunscreen uses -3)", c.excite_db, -12, 0);
    let matching = b.check("_Match the tone of commercial masters", c.matching);
    b.label("Or match this song instead (MP3 or WAV; empty uses the average of 12 pop masters)");
    let reference_row = BoxSizer::builder(Orientation::Horizontal).build();
    let reference = TextCtrl::builder(&panel).with_value(&c.match_reference).with_style(TextCtrlStyle::ReadOnly).build();
    reference_row.add(&reference, 1, SizerFlag::Expand, 0);
    b.button(&reference_row, "Choose _reference...", App::choose_match_reference);
    b.button(&reference_row, "Clear", App::clear_match_reference);
    b.sizer.add_sizer(&reference_row, 0, SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 4);
    let master = b.check("M_aster the loudness", c.master);
    let master_lufs = b.slider("Loudness target, LUFS (-14 is the streaming standard, -9 is loud like a single)", c.master_lufs, -23, -6);
    let recommended_row = BoxSizer::builder(Orientation::Horizontal).build();
    b.button(&recommended_row, "Use the recommended clean-up steps", App::recommended_cleanup);
    b.sizer.add_sizer(&recommended_row, 0, SizerFlag::Top | SizerFlag::Bottom, 12);

    panel.set_sizer(b.sizer, true);
    SettingsPage {
        panel,
        status,
        language,
        speech_level,
        playing_cue,
        live_lyrics,
        main_output,
        second_output,
        play_directly,
        autoplay,
        check_updates,
        folder,
        format,
        save_lyrics,
        save_timed,
        save_details,
        save_cover,
        after_download,
        keep_original,
        debuzz,
        gate,
        smooth,
        smooth_ms,
        widen,
        widen_tenths,
        excite,
        excite_db,
        matching,
        reference,
        master,
        master_lufs,
    }
}

impl SettingsPage {
    pub fn focus(&self) {
        self.language.set_focus();
    }

    /// The clean-up controls, from these options (after Use the recommended clean-up steps).
    fn show_cleanup(&self, c: &CleanupConfig) {
        self.debuzz.set_value(c.debuzz);
        self.gate.set_value(c.gate);
        self.smooth.set_value(c.smooth);
        self.smooth_ms.set_value(c.smooth_ms.round() as i32);
        self.widen.set_value(c.widen);
        self.widen_tenths.set_value((c.widen_amount * 10.0).round() as i32);
        self.excite.set_value(c.excite);
        self.excite_db.set_value(c.excite_db.round() as i32);
        self.matching.set_value(c.matching);
        self.reference.set_value(&c.match_reference);
        self.master.set_value(c.master);
        self.master_lufs.set_value(c.master_lufs.round() as i32);
    }
}

impl App {
    /// A control on the Settings tab changed: every setting is read from the page and saved.
    pub(crate) fn settings_changed(&mut self) {
        let p = self.settings;
        let config = &mut self.config.current;
        let language = LANGUAGES[p.language.get_selection().unwrap_or(0) as usize].0;
        let language_changed = config.language != language;
        config.language = language.to_string();
        config.speech_level = SPEECH_LEVELS[p.speech_level.get_selection().unwrap_or(0) as usize].0.to_string();
        speech::set_level(&config.speech_level);
        let cue = CUES[p.playing_cue.get_selection().unwrap_or(0) as usize].0;
        let cue_changed = config.playing_cue != cue;
        config.playing_cue = cue.to_string();
        let live = p.live_lyrics.get_value();
        let live_changed = config.audio.read_lyrics_live != live;
        config.update.check_at_startup = p.check_updates.get_value();
        let audio = &mut config.audio;
        audio.play_directly = p.play_directly.get_value();
        audio.autoplay = p.autoplay.get_value();
        audio.download_dir = p.folder.get_value().trim().to_string();
        audio.output_format = FORMATS[p.format.get_selection().unwrap_or(0) as usize].to_string();
        audio.save_lyrics = p.save_lyrics.get_value();
        audio.save_timed_lyrics = p.save_timed.get_value();
        audio.save_details = p.save_details.get_value();
        audio.save_cover = p.save_cover.get_value();
        let c = &mut config.cleanup;
        c.after_download = p.after_download.get_value();
        c.keep_original = p.keep_original.get_value();
        c.debuzz = p.debuzz.get_value();
        c.gate = p.gate.get_value();
        c.smooth = p.smooth.get_value();
        c.smooth_ms = f64::from(p.smooth_ms.get_value());
        c.widen = p.widen.get_value();
        c.widen_amount = f64::from(p.widen_tenths.get_value()) / 10.0;
        c.excite = p.excite.get_value();
        c.excite_db = f64::from(p.excite_db.get_value());
        c.matching = p.matching.get_value();
        c.match_reference = p.reference.get_value();
        c.master = p.master.get_value();
        c.master_lufs = f64::from(p.master_lufs.get_value());
        self.save_settings();
        if cue_changed {
            self.show_playing_cue();
        }
        if live_changed {
            self.set_live_lyrics(live);
        }
        // An output that changed opens now; the lists hold what each choice stands for.
        let (main, second) = (p.main_output.get_selection().unwrap_or(0) as usize, p.second_output.get_selection().unwrap_or(0) as usize);
        if let Some(name) = self.output_names.0.get(main).cloned().flatten()
            && name != self.main_output()
        {
            self.set_main_output(name);
        }
        if let Some(name) = self.output_names.1.get(second).cloned()
            && name != self.second_output()
        {
            self.set_second_output(name);
        }
        if language_changed {
            speech::say(&t("The language changes the next time Tin Pan starts."));
        }
    }

    /// Shift+F3 changed Live lyrics: the box shows it.
    pub(crate) fn settings_show_live_lyrics(&self) {
        self.settings.live_lyrics.set_value(self.config.current.audio.read_lyrics_live);
    }

    /// Tin Pan speaks, set from the Help menu's Speech submenu.
    pub(crate) fn settings_speech_level(&mut self, index: usize) {
        self.settings.speech_level.set_selection(index as u32);
        self.settings_changed();
    }

    /// The two output lists, from the outputs connected now.
    pub(crate) fn settings_show_outputs(&mut self) {
        let p = self.settings;
        let (main_names, main_lines, main_at) = crate::outputs::choices(false, Some(&self.main_output()));
        let second = self.second_output();
        let (second_names, second_lines, second_at) = crate::outputs::choices(true, second.as_deref());
        for (choice, lines, at) in [(p.main_output, &main_lines, main_at), (p.second_output, &second_lines, second_at)] {
            choice.clear();
            for line in lines {
                choice.append(line);
            }
            choice.set_selection(at as u32);
        }
        self.output_names = (main_names, second_names);
    }

    /// The account line at the top of the tab.
    pub(crate) fn show_sign_in_status(&self, user_name: Option<&str>) {
        let text = match user_name {
            Some("") => t("Signed in."),
            Some(name) => tf("Signed in as {0}.", &[&name]),
            None => t("Not signed in."),
        };
        self.settings.status.set_value(&text);
    }

    pub(crate) fn browse_download_folder(&mut self) {
        let p = self.settings;
        let picker = DirDialog::builder(&p.panel, &t("Choose where songs are saved"), &p.folder.get_value()).build();
        if picker.show_modal() == ID_OK
            && let Some(path) = picker.get_path()
        {
            p.folder.set_value(&path);
            self.settings_changed();
            speech::say(&t("Download folder set."));
        }
        p.folder.set_focus();
    }

    pub(crate) fn choose_match_reference(&mut self) {
        let p = self.settings;
        let wildcard = format!("{} (*.mp3;*.wav)|*.mp3;*.wav", t("MP3 or WAV songs"));
        let picker = FileDialog::builder(&p.panel).with_message(&t("Choose a song to match the tone of")).with_wildcard(&wildcard).with_style(FileDialogStyle::Open | FileDialogStyle::FileMustExist).build();
        if picker.show_modal() == ID_OK
            && let Some(path) = picker.get_path()
        {
            p.reference.set_value(&path);
            self.settings_changed();
            let name = std::path::Path::new(&path).file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            speech::say(&tf("Clean-up matches the tone of {0}.", &[&name]));
        }
        p.reference.set_focus();
    }

    pub(crate) fn clear_match_reference(&mut self) {
        self.settings.reference.set_value("");
        self.settings_changed();
        speech::say(&t("Clean-up matches the average of commercial pop masters."));
    }

    /// sunscreen's recommended chain with its own values; when and what to keep stay as they are.
    pub(crate) fn recommended_cleanup(&mut self) {
        let old = &self.config.current.cleanup;
        let fresh = CleanupConfig { after_download: old.after_download, keep_original: old.keep_original, extra: old.extra.clone(), ..CleanupConfig::default() };
        self.settings.show_cleanup(&fresh);
        self.settings_changed();
        speech::say(&t("Clean-up steps set to sunscreen's recommended ones."));
    }
}
