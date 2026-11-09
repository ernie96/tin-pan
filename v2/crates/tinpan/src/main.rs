//! Tin Pan 2: the unofficial Suno client, rebuilt on wxWidgets (through wxDragon) so the
//! same app runs on Windows, macOS and Linux with each system's own controls.
#![windows_subsystem = "windows"]

mod account_page;
mod captcha;
mod commands;
mod contact;
mod create_window;
mod dialogs;
mod editing;
mod discover;
mod downloads;
mod filters;
mod form;
mod help;
mod instance;
mod listen;
mod live;
mod logging;
mod making;
mod net;
mod personas;
mod out_of_reach;
mod outputs;
mod pages;
mod queue;
mod player;
mod settings_page;
mod signin;
mod song_more;
mod song_source;
mod speech;
mod to_m4a;
mod tool_form;
mod updates;
mod uploads;
mod views;
mod voice;
mod web_player;

use std::cell::RefCell;
use std::collections::{HashSet, VecDeque};

use quiet_list::Select;
use tinpan_core::api::LibraryQuery;
use tinpan_core::clip::{SunoClip, format_time};
use tinpan_core::config::ConfigStore;
use tinpan_core::i18n::{plural, t, tf, use_language};
use tinpan_core::json::FeedPage;
use tinpan_core::{Error, secrets};
use wxdragon::prelude::*;

use crate::commands::Action;
use crate::form::{Field, Value};
use crate::net::{AccountText, Change, Command, Network, Response};
use crate::player::Player;
use crate::signin::SignInWindow;
use crate::views::{LIBRARY, Section, ViewKey};

/// Shift+F7 and Shift+F9; Left and Right in a list move half as far.
const SEEK_SECONDS: f64 = 10.0;
pub(crate) const LIST_SEEK_SECONDS: f64 = 5.0;
const VOLUME_STEP: f32 = 0.1;
/// Songs kept in memory after playing or fetching ahead, so going back and forth starts at once.
const KEPT_SONGS: usize = 4;

/// Everything the window keeps. Lives on the window's thread only.
struct App {
    frame: Frame,
    notebook: Notebook,
    /// The tabs, each with its views and song list (views.rs).
    sections: Vec<Section>,
    search_box: TextCtrl,
    volume: Slider,
    /// The search the Library shows, if any.
    search: Option<String>,
    /// Said before the number of songs the next load brings (the filters just chosen, say).
    say_before_count: Option<String>,
    net: Network,
    player: Player,
    config: ConfigStore,
    next_cursor: Option<String>,
    has_more: bool,
    loading_more: bool,
    /// The song asked for and not started yet.
    wanted: Option<String>,
    playing: Option<SunoClip>,
    /// The view the playing song started from; F7, F9 and a song's end carry on through it.
    playing_from: ViewKey,
    /// While songs from Up next play: the song in `playing_from` the list carries on after.
    resume: Option<(ViewKey, String)>,
    /// Suno's autoplay picks are on their way, after a list ended.
    autoplay_pending: bool,
    /// Song files already here, newest first, and the ones being fetched ahead.
    kept: VecDeque<(String, Vec<u8>)>,
    fetching_ahead: HashSet<String>,
    signed_in: bool,
    sign_in_window: Option<SignInWindow>,
    /// The plan as last loaded (at sign-in, and with F2 or Shift+F2), and Suno's style personalization.
    billing: Option<tinpan_core::billing::BillingInfo>,
    personalization: Option<bool>,
    /// Pages loaded so far to find the next song, and whether a step is waiting for one (true when a song ended).
    pages_for_next: u8,
    waiting_for_next: Option<bool>,
    /// Songs marked Not interested in this run; Suno doesn't say which ones were, as in Tin Pan 1.
    not_interested: HashSet<String>,
    /// A tab whose newly chosen view loads once arrowing stops, and the timer that waits for that.
    pending_view: Option<usize>,
    view_timer: Timer<Frame>,
    /// The Create window once opened (Ctrl+N), your personas for it, and the songs being made.
    create: Option<create_window::CreateWindow>,
    personas: Vec<tinpan_core::create::Persona>,
    watching: create_window::Watching,
    checking_watched: bool,
    watch_failures: u8,
    /// The open Extend or Cover form's box that a Style boost, Style ideas or rewrite asked for goes into.
    form_text: Option<TextCtrl>,
    account: account_page::AccountPage,
    settings: settings_page::SettingsPage,
    notifications: Vec<tinpan_core::discover::Notification>,
    /// Playing on the second output (Ctrl+Shift+F12); Tin Pan always starts on the main one.
    on_second_output: bool,
    /// What each choice in the Settings tab's two output lists stands for (None: no second output).
    output_names: (Vec<Option<String>>, Vec<Option<String>>),
    /// An update check or download is under way; and the timer for the quiet one after startup.
    updating: bool,
    update_timer: Timer<Frame>,
    /// Resume (Ctrl+Shift+F8): the song asked for and the second to start it at.
    resume_at: Option<(String, f64)>,
    /// The song starting on Suno's own player, until it has.
    web_song: Option<SunoClip>,
    /// Live lyrics (Shift+F3): the song they were asked for, its lines once they're in, and whether to say none.
    live_for: Option<String>,
    live_cursor: Option<tinpan_core::lyrics::LyricsCursor>,
    live_say_none: bool,
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

/// Runs `f` with the app, unless it's already busy (an event raised from inside another handler).
fn with_app(f: impl FnOnce(&mut App)) {
    with_app_value(f);
}

/// Runs `f` on the app and gives back what it returns; None when the app is busy (a re-entrant event) or `f`
/// failed. A failure is logged and said, and Tin Pan carries on, as Tin Pan 1 does with an unexpected error.
fn with_app_value<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    APP.with(|app| {
        let Ok(mut app) = app.try_borrow_mut() else { return None };
        let app = app.as_mut()?;
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(app))) {
            Ok(value) => Some(value),
            Err(_) => {
                speech::say_error(&t("Something went wrong. Details are in the log."));
                None
            }
        }
    })
}

fn main() {
    SystemOptions::set_option_by_int("msw.no-manifest-check", 1);
    logging::init();
    // What went wrong, and where, goes in the log; with_app says so and carries on.
    std::panic::set_hook(Box::new(|info| log::error!("Unexpected error: {info}")));
    log::info!("Starting Tin Pan {}", tinpan_core::version());
    signin::prepare_browser_profile();
    let _ = wxdragon::main(|_| {
        let config = ConfigStore::open();
        // One Tin Pan at a time: a second start brings the open one forward and ends.
        let single = match instance::start() {
            instance::Start::First(checker, server) => (checker, server),
            instance::Start::AnotherIsOpen { asked } => {
                use_language(&config.current.language, &sys_locale::get_locale().unwrap_or_default());
                let frame = Frame::builder().with_title("Tin Pan").with_size(Size::new(1, 1)).build();
                if !asked {
                    MessageDialog::builder(&frame, &t("Tin Pan is already running."), "Tin Pan").with_style(MessageDialogStyle::OK | MessageDialogStyle::IconInformation).build().show_modal();
                }
                frame.close(true);
                return;
            }
        };
        // They have to live as long as Tin Pan does.
        std::mem::forget(single);
        use_language(&config.current.language, &sys_locale::get_locale().unwrap_or_default());

        let frame = Frame::builder().with_title("Tin Pan").with_size(Size::new(1000, 720)).build();
        wxdragon::app::set_top_window(&frame);
        frame.set_menu_bar(commands::menu_bar());
        tool_form::track_active(&frame, None);

        let pages::Layout { notebook, sections, search_box, volume, account, settings } = pages::build(&frame, config.current.audio.default_volume, &config.current);
        speech::init(&notebook);
        speech::set_level(&config.current.speech_level);

        search_box.on_text_enter(move |_| with_app(App::search));
        // Escape clears the search, if there is one, as in Tin Pan 1; otherwise it steps back as anywhere. The
        // key hook, as the page's dialog handling takes Escape before the box's own key-down.
        search_box.on_char_hook(move |event| {
            if let WindowEventData::Keyboard(ref key) = event
                && key.get_key_code() == Some(WXK_ESCAPE)
                && with_app_value(App::has_search).unwrap_or(false)
            {
                with_app(App::clear_search);
                event.skip(false);
            } else {
                event.skip(true);
            }
        });
        // The media keys (Play/Pause, Stop, Previous, Next) while Tin Pan is in front, as in Tin Pan 1, and
        // Shift+F6 back through Tin Pan's windows (F6 is the View menu's shortcut).
        frame.on_char_hook(|event| {
            let WindowEventData::Keyboard(ref key) = event else { return };
            let code = key.get_key_code().unwrap_or(0);
            if (WXK_MEDIA_NEXT_TRACK..=WXK_MEDIA_PLAY_PAUSE).contains(&code)
                && let Some(action) = commands::playback_key(code, false, false, false)
            {
                with_app(|app| app.run_from_button(action));
                event.skip(false);
            } else if code == WXK_F6 && key.shift_down() && !key.control_down() && !key.alt_down() {
                with_app(|app| app.next_window_from(None, true));
                event.skip(false);
            } else {
                event.skip(true);
            }
        });
        frame.on_menu_selected(|event| {
            let id = event.get_id();
            with_app(|app| app.menu(id));
        });

        // Notices when a song has played to its end, to start the next one.
        let timer = Timer::new(&frame);
        timer.on_tick(|_| with_app(App::check_song_ended));
        timer.start(500, false);
        // The timer has to live as long as the window.
        std::mem::forget(timer);
        let watch_timer = Timer::new(&frame);
        watch_timer.on_tick(|_| with_app(App::check_watched));
        watch_timer.start(create_window::WATCH_EVERY.as_millis() as i32, false);
        std::mem::forget(watch_timer);
        let update_timer = updates::startup_timer(&frame);
        let live_timer = Timer::new(&frame);
        live_timer.on_tick(|_| with_app(App::live_tick));
        live_timer.start(live::LIVE_EVERY_MS, false);
        std::mem::forget(live_timer);
        let view_timer = Timer::new(&frame);
        view_timer.on_tick(|_| with_app(App::load_pending_view));

        frame.show(true);
        updates::bring_to_front_after_update(&frame);
        sections[LIBRARY].list.set_focus();
        let (player, output_unusable) = Player::new(config.current.audio.default_volume, &config.current.audio.output_device);
        let mut app = App {
            frame,
            notebook,
            sections,
            search_box,
            volume,
            search: None,
            say_before_count: None,
            net: Network::start(&frame),
            player,
            config,
            next_cursor: None,
            has_more: false,
            loading_more: false,
            wanted: None,
            playing: None,
            playing_from: ViewKey::Library,
            resume: None,
            autoplay_pending: false,
            kept: VecDeque::new(),
            fetching_ahead: HashSet::new(),
            signed_in: false,
            sign_in_window: None,
            billing: None,
            personalization: None,
            pages_for_next: 0,
            waiting_for_next: None,
            not_interested: HashSet::new(),
            pending_view: None,
            view_timer,
            create: None,
            personas: Vec::new(),
            watching: Default::default(),
            checking_watched: false,
            watch_failures: 0,
            form_text: None,
            account,
            settings,
            notifications: Vec::new(),
            on_second_output: false,
            output_names: Default::default(),
            updating: false,
            update_timer,
            resume_at: None,
            web_song: None,
            live_for: None,
            live_cursor: None,
            live_say_none: false,
        };
        app.settings_show_outputs();
        if output_unusable {
            speech::say_error(&outputs::fell_back(&app.main_output()));
        }
        app.start();
        app.after_startup_updates();
        APP.with(|cell| *cell.borrow_mut() = Some(app));
    });
}

impl App {
    fn start(&mut self) {
        match secrets::load_sign_in(&self.config.current.user) {
            Some(cookies) => {
                speech::say_quietly(&t("Tin Pan ready. Press F1 for help."));
                self.net.send(Command::SignIn { cookies, device_id: self.config.current.user.device_id.clone(), fresh: false });
            }
            None => {
                speech::say_quietly(&t("Tin Pan ready."));
                self.offer_sign_in(false);
            }
        }
    }

    fn menu(&mut self, id: i32) {
        if let Some(action) = commands::action_for(id) {
            self.run(action);
        }
        // Answers that came while a window of Tin Pan's was open couldn't wake it; read them now.
        self.read_network();
    }

    /// A page button: the same command as its menu entry.
    pub(crate) fn run_from_button(&mut self, action: Action) {
        self.run(action);
        self.read_network();
    }

    /// The volume slider (0 to 100), kept like F11 and F12 keep it.
    pub(crate) fn set_volume_from_slider(&mut self, value: i32) {
        self.player.set_volume(value as f32 / 100.0);
        self.config.current.audio.default_volume = self.player.volume();
        self.save_settings();
    }

    /// A tab came forward: its view loads the first time, as Tin Pan 1's Trending did.
    pub(crate) fn tab_shown(&mut self, tab: usize) {
        if tab == views::SETTINGS {
            // Outputs plugged in or out since are shown.
            self.settings_show_outputs();
        }
        if tab >= self.sections.len() {
            return;
        }
        if self.signed_in && !self.sections[tab].view().loaded {
            self.load_view(tab, true);
        }
        self.selection_changed();
    }

    /// Delete in a song list: out of the playlist when a playlist is shown, otherwise to the trash (as in Tin Pan 1).
    pub(crate) fn delete_key(&mut self) {
        if matches!(self.shown().key, ViewKey::Playlist(_)) {
            self.remove_from_playlist();
        } else if self.shown().key == ViewKey::Queue {
            self.remove_from_queue();
        } else {
            self.trash();
        }
    }

    /// Ctrl+Up and Ctrl+Down in a song list: they move songs in a playlist.
    pub(crate) fn playlist_key(&mut self, delta: i64) {
        if matches!(self.shown().key, ViewKey::Playlist(_)) {
            self.move_in_playlist(delta);
        }
    }

    fn run(&mut self, action: Action) {
        match action {
            Action::PlayPause => self.play_pause(),
            Action::Stop => self.stop(),
            Action::Previous => self.step(-1),
            Action::Next => self.step(1),
            Action::Back => {
                self.seek(-SEEK_SECONDS);
            }
            Action::Forward => {
                self.seek(SEEK_SECONDS);
            }
            Action::BackPercent => self.seek_percent(-0.1),
            Action::ForwardPercent => self.seek_percent(0.1),
            Action::Restart => {
                if self.playing.is_some() {
                    self.player.seek_by(-f64::MAX);
                    speech::say(&t("From the start."));
                }
            }
            Action::NowPlaying => self.now_playing(),
            Action::VolumeDown => self.change_volume(-VOLUME_STEP),
            Action::VolumeUp => self.change_volume(VOLUME_STEP),
            Action::Play => self.play_selected(),
            Action::Details => self.with_song(|app, song| dialogs::show_document(&app.frame, &tf("Details: {0}", &[&song.display_title()]), &song.details_text())),
            Action::Information => self.with_song(|app, song| dialogs::show_lines(&app.frame, &tf("Information: {0}", &[&song.display_title()]), &song.info_lines())),
            Action::Lyrics => self.with_song(|app, song| app.net.send(Command::Lyrics { clip: Box::new(song) })),
            Action::Like => self.react(true),
            Action::Dislike => self.react(false),
            Action::CopyLink => self.with_song(|_, song| {
                let copied = Clipboard::get().set_text(&song.web_link());
                speech::say(&if copied { t("Link copied.") } else { t("Couldn't use the clipboard. Try again.") });
            }),
            Action::Trash => self.trash(),
            Action::RemoveFromPlaylist => self.remove_from_playlist(),
            Action::MoveUp => self.move_in_playlist(-1),
            Action::MoveDown => self.move_in_playlist(1),
            Action::ShareLink => self.with_song(|app, song| app.net.send(Command::ShareLink { clip_id: song.id })),
            Action::Download => self.download(),
            Action::DownloadMp3 => self.download_as("mp3"),
            Action::DownloadWav => self.download_as("wav"),
            Action::DownloadVideo => self.download_as("mp4"),
            Action::CleanUpDownload => self.clean_up_download(),
            Action::DownloadAll => {
                if self.signed_in {
                    self.download_all();
                } else {
                    speech::say(&t("Not signed in."));
                }
            }
            Action::Edit => self.edit_details(),
            Action::AddToPlaylist => self.add_to_playlist(),
            Action::NotInterested => self.with_song(|app, song| {
                let mark = !app.not_interested.contains(&song.id);
                app.net.send(Command::Change { clip_id: song.id, change: Change::NotInterested(mark) });
            }),
            Action::DeleteForever => self.with_song(|app, song| {
                if song.is_trashed != Some(true) {
                    speech::say(&t("Only songs in the trash can be deleted for good. Move it to the trash first."));
                    return;
                }
                let question = tf("Delete \"{0}\" for good? This can't be undone.", &[&song.display_title()]);
                if dialogs::confirm(&app.frame, &question, &t("Delete forever")) {
                    app.net.send(Command::Change { clip_id: song.id, change: Change::DeletedForever });
                }
            }),
            Action::Visibility => self.with_song(|app, song| {
                app.net.send(Command::Change { clip_id: song.id, change: Change::Public(!song.is_public) });
            }),
            Action::Remixes => self.with_song(|app, song| {
                let allow = song.metadata.as_ref().and_then(|m| m.can_remix) == Some(false);
                app.net.send(Command::Change { clip_id: song.id, change: Change::RemixAllowed(allow) });
            }),
            Action::Refresh => self.refresh_shown(),
            Action::Search => {
                self.show_tab(LIBRARY);
                self.search_box.set_focus();
                self.search_box.select_all();
            }
            Action::SearchNow => self.search(),
            Action::ClearSearch => self.clear_search(),
            Action::Filters => {
                if self.signed_in {
                    self.net.send(Command::Workspaces);
                } else {
                    speech::say(&t("Not signed in."));
                }
            }
            Action::NewPlaylist => self.new_playlist(),
            Action::RenamePlaylist => self.rename_playlist(),
            Action::DeletePlaylist => self.delete_playlist(),
            Action::ShowLibrary => self.show_tab(LIBRARY),
            Action::ShowDiscover => self.show_tab(views::DISCOVER),
            Action::NewSong => self.open_create(),
            Action::ChooseOutput => self.choose_output(),
            Action::ChooseSecondOutput => self.choose_second_output(),
            Action::ToggleOutput => self.toggle_output(),
            Action::CheckForUpdates => self.check_for_updates(true),
            Action::AllCommands => self.all_commands(),
            Action::SpeakEverything => self.set_speech_level("Everything"),
            Action::SpeakAskedFor => self.set_speech_level("AskedFor"),
            Action::SpeakErrorsOnly => self.set_speech_level("ErrorsOnly"),
            Action::ReportProblem => self.send_report(true),
            Action::SuggestFeature => self.send_report(false),
            Action::Contact => contact::show_contact(&self.frame),
            Action::Donate => contact::show_donate(&self.frame),
            Action::Licenses => self.licenses(),
            Action::About => self.about(),
            Action::Resume => self.resume(),
            Action::LiveLyrics => self.toggle_live_lyrics(),
            Action::UploadSong => self.upload_song(),
            Action::Personas => self.personas(),
            Action::ItsStems => self.its_stems(),
            Action::SongCredits => self.credits(),
            Action::CreatorPlaylists => self.creator_playlists(),
            Action::ShowRemixes => self.toggle_show_remixes(),
            Action::CoverPicture => self.cover_picture(),
            Action::OpenList => self.open_chosen_view(),
            Action::PlaylistCover => self.playlist_cover(),
            Action::MakeAlbum => self.make_album(),
            Action::DeleteList => self.delete_view(),
            Action::CreatorInfo => self.creator_info(),
            Action::CustomModels => self.custom_models(),
            Action::LyricsMashup => self.lyrics_mashup(),
            Action::MarkNotificationsRead => self.mark_notifications_read(),
            Action::Extend => self.extend(),
            Action::Cover => self.cover(),
            Action::Remaster => self.remaster(),
            Action::Inspire => self.inspire(),
            Action::WholeSong => self.whole_song(),
            Action::Stems => self.stems(),
            Action::ChangeSpeed => self.change_speed(),
            Action::Reverse => self.reverse(),
            Action::Crop => self.crop(),
            Action::Fade => self.fade(),
            Action::MakeVideo => self.make_video(),
            Action::MakePersona => self.make_persona(),
            Action::NextWindow => self.next_window_from(None, false),
            Action::Similar => self.similar(),
            Action::Creator => self.creator_of_song(),
            Action::CoversAndRemixes => self.remixes(),
            Action::Original => self.original(),
            Action::ShowComments => self.show_comments(),
            Action::WriteComment => self.write_comment(),
            Action::Explore => self.open_discover(&ViewKey::Explore),
            Action::Following => self.open_discover(&ViewKey::Following),
            Action::FindCreator => self.find_creator(),
            Action::OpenLink => self.open_link(),
            Action::SearchSuno => self.search_suno(),
            Action::UpNext => self.open_discover(&ViewKey::Queue),
            Action::AddToQueue => self.add_to_queue(false),
            Action::PlayNext => self.add_to_queue(true),
            Action::ClearQueue => self.clear_queue(),
            Action::ListenForCredits => self.listen_for_credits(),
            Action::MakeVoice => self.make_voice(),
            Action::Contests => self.open_discover(&ViewKey::Contests),
            Action::Repeat => speech::repeat_last(),
            Action::History => {
                let history = speech::history();
                if history.is_empty() {
                    speech::say(&t("No messages yet."));
                } else {
                    dialogs::show_lines(&self.frame, &t("Message history, newest first"), &history);
                }
            }
            Action::Exit => {
                self.frame.close(true);
            }
            Action::ShowAccount => self.show_tab(views::ACCOUNT),
            Action::Settings => self.show_tab(views::SETTINGS),
            Action::Credits => self.read_account(AccountText::Credits),
            Action::PlanText => self.read_account(AccountText::Plan),
            Action::SignIn => self.sign_in(),
            Action::SignOut => self.sign_out(),
            Action::Help => self.context_help(),
        }
    }

    /// F5: the view on screen, from Suno again.
    fn refresh_shown(&mut self) {
        if !self.signed_in {
            speech::say(&t("Not signed in."));
            return;
        }
        match usize::try_from(self.notebook.selection()).unwrap_or(0) {
            // The Account tab: the plan, said, and the notifications.
            views::ACCOUNT => {
                self.net.send(Command::Account { say: Some(AccountText::Credits) });
                self.load_notifications(false);
                return;
            }
            views::SETTINGS => {
                self.settings_show_outputs();
                return;
            }
            _ => {}
        }
        let tab = self.tab();
        if matches!(self.shown().key, ViewKey::Playlist(_)) {
            self.net.send(Command::Playlists);
        }
        self.load_view(tab, true);
    }

    fn read_network(&mut self) {
        loop {
            let responses = self.net.drain();
            if responses.is_empty() {
                return;
            }
            for response in responses {
                self.respond(response);
            }
        }
    }

    fn respond(&mut self, response: Response) {
        match response {
            Response::SignedIn { user_name, fresh } => {
                self.signed_in = true;
                if let Some(cookies) = fresh {
                    secrets::save_sign_in(&mut self.config.current.user, Some(&cookies));
                    self.config.current.user.clear_browser_on_next_sign_in = false;
                    self.save_settings();
                    if let Some(window) = self.sign_in_window.take() {
                        window.close();
                    }
                    self.list().set_focus();
                    speech::say(&if user_name.is_empty() { t("Signed in.") } else { tf("Signed in as {0}.", &[&user_name]) });
                }
                let title = if user_name.is_empty() { t("Tin Pan - signed in") } else { format!("Tin Pan - {user_name}") };
                self.frame.set_title(&title);
                self.show_sign_in_status(Some(&user_name));
                self.load_notifications(false);
                self.load_library(true);
                self.net.send(Command::Playlists);
                self.net.send(Command::Account { say: None });
                self.net.send(Command::Personas);
            }
            // Cookies from a page that isn't signed in yet: the window keeps waiting.
            Response::SignInFailed { error, fresh: true } => log::info!("Sign-in cookies not accepted yet: {error}"),
            Response::SignInFailed { error: Error::SignInRequired(message), fresh: false } => {
                secrets::save_sign_in(&mut self.config.current.user, None);
                self.save_settings();
                log::info!("Saved sign-in is no longer valid: {message}");
                self.offer_sign_in(true);
            }
            Response::Failed(Error::SignInRequired(message)) if self.signed_in => {
                log::warn!("{message}");
                self.sign_in_ended();
            }
            Response::SignInFailed { error: e, .. } | Response::Failed(e) => {
                log::warn!("{e}");
                self.loading_more = false;
                // A song on its way to play has its own answer (SongFailed); a failure elsewhere doesn't stop it.
                // A song that ended was waiting for this page: it's over, rather than asking again every moment.
                if self.waiting_for_next.take() == Some(true) {
                    self.playing = None;
                    self.show_playing_cue();
                }
                self.pages_for_next = 0;
                speech::say_error(&e.to_string());
            }
            Response::Notifications { list, say } => self.notifications_loaded(list, say),
            Response::NotificationsRead => self.notifications_read(),
            Response::VideoReady { clip_id, url } => self.video_ready(&clip_id, url),
            Response::UpdateChecked { result, manual } => self.update_checked(result, manual),
            Response::ReportSent { kind, number } => self.report_sent(kind, number),
            Response::TimedLyrics { clip_id, words } => self.live_lyrics_loaded(&clip_id, &words),
            Response::PersonaList { list, personas } => self.persona_list_loaded(list, personas),
            Response::PendingModels(pending) => self.custom_models_loaded(pending),
            Response::ModelBases(bases) => self.train_model_form(bases),
            Response::TooFewSongs(count) => self.too_few_songs(count),
            Response::Mashup(text) => self.mashup_done(&text),
            Response::Uploaded { clip, title } => self.song_uploaded(*clip, &title),
            Response::SayError(text) => speech::say_error(&text),
            Response::ResumeSong(clip) => self.play(*clip),
            Response::UpdateDownloaded(result) => self.update_downloaded(result),
            // A page asked for before signing out is dropped.
            Response::Library { page, more } if self.signed_in => self.show_page(page, more),
            Response::Library { .. } => {}
            Response::Song { clip, data, ahead } => self.song_arrived(*clip, data, ahead),
            Response::SongFailed { clip, error } => self.song_failed(*clip, error),
            Response::AheadFailed { clip_id } => {
                self.fetching_ahead.remove(&clip_id);
                // If it was asked for meanwhile, ask again as an ordinary fetch, so a failure is said.
                if self.wanted.as_deref() == Some(clip_id.as_str())
                    && let Some(clip) = self.find_song(&clip_id)
                {
                    self.net.send(Command::FetchSong { clip: Box::new(clip), ahead: false });
                }
            }
            Response::Reacted { clip_id, state, asked } => self.reacted(&clip_id, state.as_deref(), asked),
            Response::Trashed { clip_id, trashed } => self.trashed(&clip_id, trashed),
            Response::Lyrics { clip, text } => {
                if text.trim().is_empty() {
                    speech::say(&t("This song has no lyrics."));
                } else {
                    dialogs::show_document(&self.frame, &tf("Lyrics: {0}", &[&clip.display_title()]), &text);
                }
            }
            Response::Changed { clip_id, change } => self.changed(&clip_id, change),
            Response::ShareLink(link) => {
                let copied = Clipboard::get().set_text(&link);
                speech::say(&if copied { t("Share link copied.") } else { t("Couldn't use the clipboard. Try again.") });
            }
            Response::Account { info, personalization, say } => self.account_loaded(info, personalization, say),
            Response::Downloaded { clip, through_suno } => self.downloaded(&clip, through_suno),
            Response::DownloadFailed { clip, format, path, error, offer, clean } => self.download_failed(*clip, format, path, &error.to_string(), offer, clean),
            Response::Cleaned { title, result } => self.cleaned(&title, result),
            Response::Progress(text) => speech::say_quietly(&text),
            Response::Workspaces(workspaces) => {
                if filters::edit(&self.frame, &mut self.config.current.library, &workspaces) {
                    self.save_settings();
                    self.say_before_count = Some(filters::summary(&self.config.current.library));
                    self.load_library(true);
                }
            }
            Response::Playlists(playlists) => self.playlists_loaded(playlists),
            Response::PlaylistSongs { playlist_id, songs, announce } => self.playlist_songs_loaded(&playlist_id, songs, announce),
            Response::Trending { songs, announce } => self.trending_loaded(songs, announce),
            Response::PlaylistMade(playlist) => {
                speech::say(&tf("Made playlist {0}.", &[&playlist.display_name()]));
                self.net.send(Command::Playlists);
            }
            Response::PlaylistRenamed => {
                speech::say(&t("Saved."));
                self.net.send(Command::Playlists);
            }
            Response::PlaylistTrashed { playlist_id, name } => {
                let section = &mut self.sections[LIBRARY];
                // The view shown stays the same one, wherever it moved; the deleted one's place goes to your songs.
                let shown = section.view().key.clone();
                section.views.retain(|v| v.key != ViewKey::Playlist(playlist_id.clone()));
                section.shown = section.views.iter().position(|v| v.key == shown).unwrap_or(0);
                section.show_titles();
                section.refresh(Select::Start);
                speech::say(&tf("Moved {0} to trash.", &[&name]));
            }
            Response::AddedToPlaylist(playlist) => self.added_to_playlist(playlist),
            Response::Following { songs, announce } => self.following_loaded(songs, announce),
            Response::Explore { shelves, announce } => self.explore_loaded(shelves, announce),
            Response::Contests { contests, songs, announce } => self.contests_loaded(contests, songs, announce),
            Response::Autoplay(result) => self.autoplay_loaded(result),
            Response::ListenProfile(known) => self.listen_profile(known.map(|p| *p)),
            Response::ListenTask { task, songs, check_due } => self.listen_task(*task, songs, check_due),
            Response::ListenCheck { task_id, answers, listened } => self.listen_check(task_id, answers, listened),
            Response::VoicePhrase { singing, phrase, details, attempt } => self.voice_phrase(*singing, *phrase, *details, attempt),
            Response::ResultList { title, songs, with_creator, creator } => self.open_result(title, songs, with_creator, creator.map(|c| *c)),
            Response::Credits { clip, sources, count, capped } => self.credits_loaded(&clip, &sources, count, capped),
            Response::CreatorPlaylists(page) => self.creator_playlists_loaded(&page),
            Response::ShowRemixes { clip_id, show } => self.show_remixes_changed(&clip_id, show),
            Response::SongCovered { clip_id, url, said } => self.song_covered(&clip_id, url, &said),
            Response::Comments { clip, comments } => self.comments_loaded(&clip, &comments),
            Response::Say(text) => speech::say(&text),
            Response::Personas(list) => self.personas_loaded(list),
            Response::CaptchaNeeded { request, provider, model_takes_voice, job } => self.captcha_needed(*request, provider, model_takes_voice, job),
            Response::SongsStarted { clips, job } => self.songs_started(clips, job),
            Response::CoverLyrics { clip, lyrics } => self.cover_form(*clip, lyrics),
            Response::LyricsWritten { title, text } => self.lyrics_written(title, text),
            Response::SongsChecked(result) => self.watched_checked(result),
            Response::StyleBoosted { style, to_form } => self.style_boosted(style, to_form),
            Response::StyleIdeas { ideas, to_form } => self.style_ideas_loaded(ideas, to_form),
            Response::Cowritten { text, to_form } => self.cowritten(text, to_form),
            Response::LyricsDrafts(drafts) => self.lyrics_drafts_loaded(drafts),
            Response::DraftText { draft, text } => self.draft_text_loaded(draft, text),
            Response::RemovedFromPlaylist { playlist_id, clip_id } => self.removed_from_playlist(&playlist_id, &clip_id),
            // Moved here already, when the key was pressed.
            Response::MovedInPlaylist => {}
            // Saved only while signed in: a rotation that arrives just after signing out mustn't sign you back in.
            Response::CookiesRotated(cookies) if self.signed_in => {
                secrets::save_sign_in(&mut self.config.current.user, Some(&cookies));
                self.save_settings();
            }
            Response::CookiesRotated(_) => {}
        }
    }

    // ── Signing in ──────────────────────────────────────────────────────

    /// The first start (nothing saved): a welcome that offers to sign in. A sign-in Suno has ended: asks to
    /// sign in again. No either way says where Sign in is. As in Tin Pan 1.
    fn offer_sign_in(&mut self, expired: bool) {
        let yes = if expired {
            dialogs::confirm(&self.frame, &t("Suno has ended your sign-in, so Tin Pan can't reach your songs. This happens now and then, for example after changing your password. Sign in again now?"), &t("Sign in again"))
        } else {
            dialogs::confirm(&self.frame, &t("Welcome to Tin Pan. To browse, play and make songs, sign in to your Suno account. Suno's own sign-in page opens, and closes by itself once you're in. Sign in now?"), &t("Welcome to Tin Pan"))
        };
        if yes {
            self.sign_in();
        } else {
            speech::say(&t("Not signed in. You can sign in any time from Settings, Ctrl+5. Press F1 for help."));
        }
    }

    /// Opens Suno's sign-in page; the window closes by itself once the sign-in works.
    fn sign_in(&mut self) {
        if self.signed_in {
            speech::say(&t("Already signed in. Sign out first to use another account."));
            return;
        }
        if self.sign_in_window.is_some() {
            return;
        }
        let forget = self.config.current.user.clear_browser_on_next_sign_in;
        let device_id = self.config.current.user.device_id.clone();
        let commands = self.net.sender();
        let try_cookies = move |cookies| {
            let _ = commands.send(Command::SignIn { cookies, device_id: device_id.clone(), fresh: true });
        };
        let closed = || {
            with_app(|app| {
                app.sign_in_window = None;
                if !app.signed_in {
                    speech::say(&t("Not signed in. You can sign in any time from Settings, Ctrl+5. Press F1 for help."));
                }
            });
        };
        if !signin::web_pages_work(&t("Signing in needs the Microsoft Edge WebView2 Runtime. Install it from Microsoft, then try again.")) {
            return;
        }
        self.sign_in_window = Some(SignInWindow::open(&self.frame, forget, try_cookies, closed));
    }

    fn sign_out(&mut self) {
        if !self.signed_in {
            speech::say(&t("Not signed in."));
            return;
        }
        self.forget_account();
        // The next sign-in page starts signed out of Suno, so another account can be used.
        self.config.current.user.clear_browser_on_next_sign_in = true;
        self.save_settings();
        speech::say(&t("Signed out."));
    }

    /// Suno ended the sign-in while Tin Pan was running: it's forgotten, and you're asked to sign in again at
    /// once, as in Tin Pan 1.
    fn sign_in_ended(&mut self) {
        if !self.signed_in {
            return;
        }
        log::info!("Suno ended the sign-in");
        self.forget_account();
        self.offer_sign_in(true);
    }

    /// Everything that belongs to the signed-in account goes: songs, lists, plan, the saved sign-in.
    fn forget_account(&mut self) {
        // Suno's own player forgets the sign-in at once, as in Tin Pan 1.
        self.player.forget_sign_in();
        self.playing = None;
        self.wanted = None;
        self.web_song = None;
        self.waiting_for_next = None;
        self.pages_for_next = 0;
        self.resume = None;
        self.autoplay_pending = false;
        self.kept.clear();
        self.fetching_ahead.clear();
        self.signed_in = false;
        for section in &mut self.sections {
            // Your playlists, Explore's shelves and the lists opened from songs go; the rest stay, empty until the next sign-in.
            section.views.retain(|v| !matches!(v.key, ViewKey::Playlist(_) | ViewKey::Shelf(_) | ViewKey::Result(_)));
            section.shown = 0;
            for view in &mut section.views {
                view.songs.clear();
                view.loaded = false;
            }
            section.show_titles();
        }
        self.billing = None;
        self.personalization = None;
        // The account's personas, the songs being followed and the rest of what was its own go too.
        self.personas.clear();
        self.watching.clear();
        self.checking_watched = false;
        self.not_interested.clear();
        self.live_for = None;
        self.live_cursor = None;
        self.next_cursor = None;
        self.has_more = false;
        self.loading_more = false;
        self.fill_create_choices();
        self.refresh_rows(Select::Start);
        self.net.send(Command::SignOut);
        secrets::save_sign_in(&mut self.config.current.user, None);
        self.save_settings();
        self.frame.set_title("Tin Pan");
        self.show_sign_in_status(None);
        self.clear_account_page();
    }

    // ── The library ─────────────────────────────────────────────────────

    fn load_library(&mut self, quietly: bool) {
        if !self.signed_in {
            speech::say(&t("Not signed in."));
            return;
        }
        // Quiet, as in Tin Pan 1: the number of songs that came back is what's said.
        let _ = quietly;
        // A fresh first page is coming: no more pages of the old list (or the old search) until it's here.
        self.has_more = false;
        self.next_cursor = None;
        let query = LibraryQuery { search: self.search.clone(), library: self.config.current.library.clone(), ..LibraryQuery::default() };
        self.net.send(Command::LoadLibrary { query, more: false });
    }

    /// Enter in the search box: your songs that match, or all of them again when it's empty.
    fn search(&mut self) {
        let text = self.search_box.get_value().trim().to_string();
        self.search = (!text.is_empty()).then_some(text);
        // The search is of your songs: show them.
        let section = &mut self.sections[LIBRARY];
        if section.shown != 0 {
            section.shown = 0;
            section.show_titles();
            section.refresh(Select::Start);
        }
        self.load_library(true);
        self.sections[LIBRARY].list.set_focus();
    }

    fn has_search(&mut self) -> bool {
        self.search.is_some() || !self.search_box.get_value().is_empty()
    }

    fn clear_search(&mut self) {
        if !self.has_search() {
            return;
        }
        self.search_box.set_value("");
        self.search = None;
        self.load_library(true);
    }

    pub(crate) fn selection_changed(&mut self) {
        if let Some(bar) = self.frame.get_menu_bar() {
            let marked = self.selected().is_some_and(|s| self.not_interested.contains(&s.id));
            commands::relabel(&bar, self.selected().as_ref(), marked);
        }
        self.load_more_near_the_end();
    }

    /// Reaching the last of your songs loads the next page.
    fn load_more_near_the_end(&mut self) {
        if self.shown().key != ViewKey::Library {
            return;
        }
        let at_end = self.list().selected_index().is_some_and(|i| i + 1 >= self.shown().songs.len());
        if at_end && self.has_more && !self.loading_more {
            self.loading_more = true;
            let query = LibraryQuery { cursor: self.next_cursor.clone(), search: self.search.clone(), library: self.config.current.library.clone() };
            self.net.send(Command::LoadLibrary { query, more: true });
        }
    }

    fn show_page(&mut self, page: FeedPage, more: bool) {
        let library = self.library();
        let before = if more { library.songs.len() } else { 0 };
        if more {
            let known: HashSet<String> = library.songs.iter().map(|s| s.id.clone()).collect();
            library.songs.extend(page.clips.into_iter().filter(|c| !known.contains(&c.id)));
        } else {
            library.songs = page.clips;
        }
        library.loaded = true;
        let added = library.songs.len() - before;
        // A cursor that never ends is caught by a page adding nothing.
        self.has_more = added > 0 && page.has_more.unwrap_or(page.next_cursor.is_some());
        self.next_cursor = page.next_cursor;
        self.loading_more = false;
        self.sections[LIBRARY].refresh(if more { Select::Keep } else { Select::Start });
        // Songs Suno is still making are followed until they're ready, as are ones started here.
        let songs = self.library().songs.clone();
        self.watch(&songs);
        // F9 or a song's end was waiting for this page: carry on now, saying nothing about the page.
        if let Some(ended) = self.waiting_for_next.take() {
            self.step_on(1, ended);
            return;
        }
        let before_count = if more { String::new() } else { self.say_before_count.take().map(|s| s + " ").unwrap_or_default() };
        speech::say_quietly(&(before_count + &match (more, added) {
            (true, 0) => t("End of list."),
            (true, n) => tf("{0} more.", &[&n]),
            (false, 0) => match &self.search {
                Some(search) => tf("No songs match {0}.", &[search]),
                None => t("No songs."),
            },
            (false, n) => match &self.search {
                Some(search) => tf("{0} matching {1}.", &[&plural(n as i64, "{0} song", "{0} songs"), search]),
                None => plural(n as i64, "{0} song.", "{0} songs."),
            },
        }));
    }

    /// The song selected in the view on screen.
    pub(crate) fn selected(&self) -> Option<SunoClip> {
        let id = self.list().selected()?;
        self.shown().songs.iter().find(|s| s.id == id).cloned()
    }

    /// Runs a Song command on the selected song, if there is one.
    pub(crate) fn with_song(&mut self, f: impl FnOnce(&mut Self, SunoClip)) {
        match self.selected() {
            // A contest without a song has nothing for the Song menu to act on.
            Some(song) if song.id.starts_with(discover::NO_SONG) => speech::say(&tf("{0} has no song to play.", &[&song.display_title()])),
            Some(song) => f(self, song),
            None => speech::say(&t("Choose a song first.")),
        }
    }

    /// The Applications key in a song list: the Song menu, with a playlist's own Remove and Move first.
    fn song_context_menu(&mut self) {
        let marked = self.selected().is_some_and(|s| self.not_interested.contains(&s.id));
        let in_playlist = matches!(self.shown().key, ViewKey::Playlist(_));
        let mut menu = commands::song_menu(self.selected().as_ref(), marked, in_playlist);
        self.list().popup_menu(&mut menu);
    }

    // ── The account ─────────────────────────────────────────────────────

    /// F2 and Shift+F2: always fetched, so the numbers are current, as in Tin Pan 1.
    fn read_account(&mut self, say: AccountText) {
        if !self.signed_in {
            speech::say(&t("Not signed in."));
            return;
        }
        self.net.send(Command::Account { say: Some(say) });
    }

    fn account_loaded(&mut self, info: Result<Box<tinpan_core::billing::BillingInfo>, Error>, personalization: Option<bool>, say: Option<AccountText>) {
        match info {
            Ok(info) => {
                self.billing = Some(*info);
                if personalization.is_some() {
                    self.personalization = personalization;
                }
                // The Create window's models and the Account tab come from the plan.
                self.fill_create_choices();
                self.show_account_text();
                // The Cover and Remaster texts say the free ones left.
                if let Some(b) = &self.billing {
                    commands::set_plan(commands::PlanLimits { paid: b.is_paid(), free_covers: b.free_covers_left, free_remasters: b.free_remasters_left });
                }
                self.selection_changed();
            }
            // The last known numbers are said when there are any; otherwise why they didn't load.
            Err(e) => {
                log::warn!("Account refresh failed: {e}");
                if self.billing.is_none() {
                    if say.is_some() {
                        speech::say(&e.to_string());
                    }
                    return;
                }
            }
        }
        let (Some(say), Some(billing)) = (say, &self.billing) else { return };
        speech::say_error(&match say {
            AccountText::Credits => billing.credits_text(),
            AccountText::Plan => billing.subscription_text(self.personalization),
        });
    }

    // ── Song commands ───────────────────────────────────────────────────

    /// Like (Ctrl+Shift+U) or dislike (Ctrl+Shift+N); again takes it back.
    fn react(&mut self, like: bool) {
        self.with_song(|app, song| {
            let undo = if like { song.is_liked } else { song.is_disliked() };
            let reaction = if undo { None } else if like { Some("LIKE") } else { Some("DISLIKE") };
            app.net.send(Command::React { clip_id: song.id, reaction });
        });
    }

    fn reacted(&mut self, clip_id: &str, state: Option<&str>, asked: Option<&str>) {
        let Some(song) = self.find_song(clip_id) else { return };
        let had = (song.is_liked, song.is_disliked());
        self.each_copy(clip_id, |s| s.set_reaction(state));
        let title = song.display_title();
        speech::say(&match (asked, had) {
            (Some("LIKE"), _) => tf("Liked {0}.", &[&title]),
            (Some(_), _) => tf("Disliked {0}.", &[&title]),
            (None, (true, _)) => tf("Like removed from {0}.", &[&title]),
            (None, _) => tf("Dislike removed from {0}.", &[&title]),
        });
        self.refresh_rows(Select::Keep);
        self.selection_changed();
    }

    /// Ctrl+T: the song's title and lyrics, as Suno shows them. The audio stays the same.
    fn edit_details(&mut self) {
        self.with_song(|app, song| {
            let old_lyrics = song.lyrics().unwrap_or("").to_string();
            let mut fields = [
                Field { label: "_Title", value: Value::Text { value: song.title.clone().unwrap_or_default(), multiline: false, max_len: 100 } },
                Field { label: "_Lyrics", value: Value::Text { value: old_lyrics.clone(), multiline: true, max_len: 0 } },
            ];
            let title = tf("Edit {0}", &[&song.display_title()]);
            if !form::show(&app.frame, &title, &t("Changes what Suno shows for this song. The audio stays the same."), &t("Save"), &mut fields) {
                return;
            }
            let new_title = Some(fields[0].text()).filter(|t| *t != song.title.clone().unwrap_or_default());
            let new_lyrics = Some(fields[1].text()).filter(|l| *l != old_lyrics);
            if new_title.is_some() || new_lyrics.is_some() {
                app.net.send(Command::Change { clip_id: song.id, change: Change::Details { title: new_title, lyrics: new_lyrics } });
            }
        });
    }

    fn changed(&mut self, clip_id: &str, change: Change) {
        let Some(song) = self.find_song(clip_id) else { return };
        let title = song.display_title();
        let said = match change {
            Change::DeletedForever => {
                if self.playing.as_ref().is_some_and(|p| p.id == clip_id) {
                    self.stop();
                }
                self.remove_everywhere(clip_id);
                tf("Deleted {0} for good.", &[&title])
            }
            Change::NotInterested(mark) => {
                if mark {
                    self.not_interested.insert(clip_id.to_string());
                } else {
                    self.not_interested.remove(clip_id);
                }
                if mark { tf("Suno will suggest fewer songs like {0}.", &[&title]) } else { tf("Suno may suggest songs like {0} again.", &[&title]) }
            }
            Change::Details { title: new_title, lyrics } => {
                self.each_copy(clip_id, |s| {
                    if let Some(new_title) = &new_title {
                        s.title = Some(new_title.clone());
                    }
                    if let Some(lyrics) = &lyrics {
                        s.metadata.get_or_insert_with(Default::default).prompt = Some(lyrics.clone());
                    }
                });
                t("Saved.")
            }
            Change::Public(public) => {
                self.each_copy(clip_id, |s| s.is_public = public);
                if public { tf("{0} is public.", &[&title]) } else { tf("{0} is private.", &[&title]) }
            }
            Change::RemixAllowed(allowed) => {
                self.each_copy(clip_id, |s| s.metadata.get_or_insert_with(Default::default).can_remix = Some(allowed));
                if allowed { t("Others may now cover and remix it.") } else { t("Others can no longer cover or remix it.") }
            }
        };
        self.refresh_rows(Select::Keep);
        self.selection_changed();
        speech::say(&said);
    }

    /// Moves the song to Suno's trash (asking first), or restores one that's in it.
    fn trash(&mut self) {
        self.with_song(|app, song| {
            if song.is_trashed == Some(true) {
                app.net.send(Command::Trash { clip_id: song.id, trashed: false });
                return;
            }
            let question = tf("Move \"{0}\" to your Suno trash?", &[&song.display_title()]);
            if dialogs::confirm(&app.frame, &question, &t("Move to trash")) {
                app.net.send(Command::Trash { clip_id: song.id, trashed: true });
            }
        });
    }

    fn trashed(&mut self, clip_id: &str, trashed: bool) {
        let Some(song) = self.find_song(clip_id) else { return };
        let title = song.display_title();
        if !trashed {
            self.each_copy(clip_id, |s| s.is_trashed = Some(false));
            self.refresh_rows(Select::Keep);
            speech::say(&tf("Restored {0} from the trash.", &[&title]));
            return;
        }
        if self.playing.as_ref().is_some_and(|p| p.id == clip_id) {
            self.stop();
        }
        self.remove_everywhere(clip_id);
        self.refresh_rows(Select::Keep);
        speech::say(&tf("Moved {0} to trash.", &[&title]));
    }

    // ── Playing ─────────────────────────────────────────────────────────

    pub(crate) fn play_selected(&mut self) {
        if let Some(clip) = self.selected() {
            if self.shown().key == ViewKey::Queue {
                // From Up next: the song leaves the queue, and the playing list carries on after it.
                if self.playing.is_some() && self.resume.is_none() {
                    self.resume = self.step_from();
                }
                self.unqueue(&clip.id);
            } else {
                // The view on screen becomes the one F9 and a song's end carry on through.
                self.playing_from = self.shown().key.clone();
                self.resume = None;
            }
            self.play(clip);
        }
    }

    fn play(&mut self, clip: SunoClip) {
        if clip.id.starts_with(discover::NO_SONG) {
            speech::say(&tf("{0} has no song to play.", &[&clip.display_title()]));
            return;
        }
        if !clip.can_play() {
            speech::say(&if clip.has_failed() { t("That song failed to generate.") } else { t("That song isn't ready yet.") });
            return;
        }
        self.player.stop();
        self.playing = None;
        self.web_song = None;
        self.wanted = Some(clip.id.clone());
        // Settings can have every song play through Suno's own player.
        if !self.config.current.audio.play_directly {
            self.play_on_web(clip);
            return;
        }
        if let Some(at) = self.kept.iter().position(|(id, _)| *id == clip.id) {
            let (_, data) = self.kept.remove(at).unwrap_or_default();
            self.start_song(clip, data);
        } else if self.fetching_ahead.contains(&clip.id) {
            log::info!("{} is on its way already", clip.id);
        } else {
            log::info!("Fetching {} to play", clip.id);
            self.net.send(Command::FetchSong { clip: Box::new(clip), ahead: false });
        }
    }

    fn song_arrived(&mut self, clip: SunoClip, data: Vec<u8>, ahead: bool) {
        if ahead {
            self.fetching_ahead.remove(&clip.id);
        }
        // Only the song asked for last starts; one fetched ahead, or that took longer, is kept for later.
        if self.wanted.as_deref() == Some(clip.id.as_str()) {
            self.start_song(clip, data);
        } else {
            self.keep(clip.id, data);
        }
    }

    fn keep(&mut self, id: String, data: Vec<u8>) {
        self.kept.retain(|(k, _)| *k != id);
        self.kept.push_front((id, data));
        self.kept.truncate(KEPT_SONGS);
    }

    fn start_song(&mut self, clip: SunoClip, data: Vec<u8>) {
        self.wanted = None;
        log::info!("Starting {} ({} KB)", clip.id, data.len() / 1024);
        // A copy stays, so going back to it starts at once.
        self.keep(clip.id.clone(), data.clone());
        match self.player.play(data) {
            Ok(()) => {
                // Resume (Ctrl+Shift+F8) starts where the song was.
                match self.resume_at.take().filter(|(id, _)| *id == clip.id) {
                    Some((_, seconds)) if seconds > 5.0 => {
                        self.player.seek_by(seconds);
                        speech::say(&tf("Resuming {0} at {1}.", &[&clip.display_title(), &format_time(seconds)]));
                    }
                    _ => speech::say(&tf("Playing: {0}.", &[&clip.display_title()])),
                }
                self.playing = Some(clip);
                self.show_playing_cue();
                self.load_live_lyrics(false);
                self.fetch_next_ahead();
            }
            Err(e) => {
                log::warn!("Playing {} in Tin Pan didn't work ({e}); using Suno's player", clip.id);
                self.play_on_web(clip);
            }
        }
    }

    /// Fetches the next playable song in the playing view while this one plays, so it starts at once.
    fn fetch_next_ahead(&mut self) {
        let Some(next) = self.next_song() else { return };
        if self.kept.iter().any(|(id, _)| *id == next.id) || !self.fetching_ahead.insert(next.id.clone()) {
            return;
        }
        self.net.send(Command::FetchSong { clip: Box::new(next), ahead: true });
    }

    fn play_pause(&mut self) {
        if self.player.is_loaded() {
            speech::say(&if self.player.toggle_pause() { t("Playing.") } else { t("Paused.") });
        } else {
            self.play_selected();
        }
    }

    fn stop(&mut self) {
        // A song still on its way, here or in Suno's player, is called off too, so it can't start later.
        let pending = self.wanted.take().is_some() | self.web_song.take().is_some();
        if self.playing.take().is_some() || pending {
            self.player.stop();
            self.show_playing_cue();
            speech::say(&t("Stopped."));
        }
    }

    /// Where F7 and F9 start from: the playing song in its view, or the selected song in the view on screen.
    fn step_from(&self) -> Option<(ViewKey, String)> {
        match &self.playing {
            Some(_) if self.resume.is_some() => self.resume.clone(),
            Some(p) => Some((self.playing_from.clone(), p.id.clone())),
            None => Some((self.shown().key.clone(), self.list().selected()?)),
        }
    }

    /// The next (or previous) playable song in the view the playing song came from. When your songs' next
    /// one isn't loaded yet, up to 3 more pages are loaded first, as in Tin Pan 1; `ended` says it's because
    /// a song played to its end, which says "End of library." quietly when there's no more.
    fn step(&mut self, direction: i32) {
        self.step_on(direction, false);
    }

    pub(crate) fn step_on(&mut self, direction: i32, ended: bool) {
        // Up next comes first; once it's empty the list carries on from where it was.
        if direction > 0
            && let Some(clip) = self.take_queued()
        {
            if self.resume.is_none() {
                self.resume = self.step_from();
            }
            self.play(clip);
            return;
        }
        let Some((key, from)) = self.step_from() else { return };
        if direction > 0 && key == ViewKey::Library && self.next_song().is_none() && self.has_more && self.pages_for_next < 3 {
            self.pages_for_next += 1;
            self.waiting_for_next = Some(ended);
            if !self.loading_more {
                self.loading_more = true;
                let query = LibraryQuery { cursor: self.next_cursor.clone(), search: self.search.clone(), library: self.config.current.library.clone() };
                self.net.send(Command::LoadLibrary { query, more: true });
            }
            return;
        }
        self.pages_for_next = 0;
        if direction > 0 && ended && self.next_song().is_none() {
            if self.config.current.audio.autoplay
                && let Some(seed) = &self.playing
            {
                self.autoplay_pending = true;
                self.net.send(Command::Autoplay { seed_id: seed.id.clone() });
                return;
            }
            self.list_ended();
            return;
        }
        let songs = self.find_view(&key).map(|v| v.songs.clone()).unwrap_or_default();
        let Some(index) = songs.iter().position(|s| s.id == from) else {
            // The list was reloaded or closed without the song that ended: nothing to go on to.
            if ended {
                self.playing = None;
                self.resume = None;
                self.show_playing_cue();
            }
            return;
        };
        let next = if direction > 0 { songs[index + 1..].iter().find(|s| s.can_play()) } else { songs[..index].iter().rev().find(|s| s.can_play()) };
        match next.cloned() {
            Some(clip) => {
                // The view keeps up when it's on screen.
                if self.shown().key == key {
                    self.list().select(Some(&clip.id));
                }
                self.playing_from = key;
                self.resume = None;
                self.play(clip);
            }
            None => speech::say(&match (direction > 0, &key) {
                (true, ViewKey::Playlist(_)) => t("End of playlist."),
                (true, _) => t("End of list."),
                (false, _) => t("First song."),
            }),
        }
    }

    /// The playing list has no more songs: playing stops, and says so quietly.
    pub(crate) fn list_ended(&mut self) {
        let key = self.step_from().map(|(key, _)| key);
        self.playing = None;
        self.resume = None;
        self.show_playing_cue();
        speech::say_quietly(&match key {
            Some(ViewKey::Library) => t("End of library."),
            Some(ViewKey::Playlist(_)) => t("End of playlist."),
            _ => t("End of list."),
        });
    }

    /// The song couldn't be fetched to play here: Suno's own player tries, unless the sign-in is the problem.
    fn song_failed(&mut self, clip: SunoClip, error: Error) {
        if self.wanted.as_deref() != Some(clip.id.as_str()) {
            return;
        }
        if matches!(error, Error::SignInRequired(_)) {
            self.wanted = None;
            self.sign_in_ended();
            return;
        }
        log::warn!("Fetching {} to play didn't work ({error}); using Suno's player", clip.id);
        self.play_on_web(clip);
    }

    /// Plays the song through Suno's own player, off screen; it's said once it starts.
    fn play_on_web(&mut self, clip: SunoClip) {
        if !signin::web_pages_work(&t("Playing needs the Microsoft Edge WebView2 Runtime. Install it from Microsoft, then try again.")) {
            self.wanted = None;
            return;
        }
        self.wanted = Some(clip.id.clone());
        self.player.play_web(&self.frame, &clip.id);
        self.web_song = Some(clip);
    }

    /// What Suno's player did: started (said, as a song here is), or couldn't (said why).
    fn poll_web_player(&mut self) {
        let Some(event) = self.player.poll_web() else { return };
        match event {
            web_player::Event::Started => {
                let Some(clip) = self.web_song.take() else { return };
                if self.wanted.as_deref() != Some(clip.id.as_str()) {
                    return;
                }
                self.wanted = None;
                // Resume (Ctrl+Shift+F8) starts where the song was here too.
                match self.resume_at.take().filter(|(id, _)| *id == clip.id) {
                    Some((_, seconds)) if seconds > 5.0 => {
                        self.player.seek_by(seconds);
                        speech::say(&tf("Resuming {0} at {1}.", &[&clip.display_title(), &format_time(seconds)]));
                    }
                    _ => speech::say(&tf("Playing: {0}.", &[&clip.display_title()])),
                }
                self.playing = Some(clip);
                self.show_playing_cue();
                self.load_live_lyrics(false);
            }
            // The song's end is noticed below, as for songs played here.
            web_player::Event::Ended => {}
            web_player::Event::OutputFailed { not_found } => {
                let name = if self.on_second_output { self.second_output().unwrap_or_default() } else { self.main_output() };
                speech::say_error(&if not_found {
                    if cfg!(windows) { tf("{0} isn't connected, so the song plays on the Windows default output.", &[&name]) } else { tf("{0} isn't connected, so the song plays on the default output.", &[&name]) }
                } else {
                    if cfg!(windows) { tf("Couldn't switch to {0}, so the song plays on the Windows default output.", &[&name]) } else { tf("Couldn't switch to {0}, so the song plays on the default output.", &[&name]) }
                });
            }
            web_player::Event::Failed { no_button } => {
                self.wanted = None;
                self.web_song = None;
                speech::say_error(&if no_button {
                    t("Suno's player didn't offer this song. It may be private to another account, or still being made.")
                } else {
                    t("Suno's player didn't start this song. Try again.")
                });
            }
        }
    }

    fn check_song_ended(&mut self) {
        self.poll_web_player();
        if self.playing.is_some() && self.wanted.is_none() && !self.player.is_loaded() && self.waiting_for_next.is_none() && !self.autoplay_pending {
            self.step_on(1, true);
        }
    }

    /// The next playable song after the playing one (or the selected one), if it's loaded.
    fn next_song(&self) -> Option<SunoClip> {
        if let Some(queued) = self.up_next().iter().find(|s| s.can_play()) {
            return Some(queued.clone());
        }
        let (key, from) = self.step_from()?;
        let songs = if key == self.playing_from && self.playing.is_some() { self.queue() } else { self.find_view(&key)?.songs.as_slice() };
        let index = songs.iter().position(|s| s.id == from)?;
        songs[index + 1..].iter().find(|s| s.can_play()).cloned()
    }

    /// Moves through the song; false when nothing is playing.
    pub(crate) fn seek(&mut self, seconds: f64) -> bool {
        if self.playing.is_none() {
            return false;
        }
        let at = self.player.seek_by(seconds);
        speech::say(&format_time(at.as_secs_f64()));
        true
    }

    fn seek_percent(&mut self, part: f64) {
        if let Some(total) = self.player.total() {
            self.seek(total.as_secs_f64() * part);
        }
    }

    fn change_volume(&mut self, delta: f32) {
        self.player.set_volume(self.player.volume() + delta);
        self.config.current.audio.default_volume = self.player.volume();
        self.volume.set_value((self.player.volume() * 100.0).round() as i32);
        self.save_settings();
        speech::say(&tf("Volume {0:0}.", &[&(self.player.volume() * 100.0).round()]));
    }

    fn now_playing(&self) {
        let Some(clip) = &self.playing else {
            speech::say_error(&t("Nothing playing."));
            return;
        };
        let at = format_time(self.player.position().as_secs_f64());
        let place = match self.player.total() {
            Some(total) => tf("{0} of {1}", &[&at, &format_time(total.as_secs_f64())]),
            None => at,
        };
        let line = if self.player.is_paused() { "Paused: {0}, {1}." } else { "Playing: {0}, {1}." };
        speech::say_error(&tf(line, &[&clip.display_title(), &place]));
    }

    fn save_settings(&self) {
        if let Err(e) = self.config.save() {
            log::error!("Couldn't save settings: {e}");
        }
    }
}
