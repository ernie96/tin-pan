//! Tin Pan's network thread. The window sends it work; it does the work with the blocking Suno client
//! and sends back what happened, then wakes the window to read it. The window never waits on Suno.

use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::path::{Path, PathBuf};
use std::thread;

use tinpan_core::api::{LibraryQuery, SunoApi};
use tinpan_core::clip::SunoClip;
use tinpan_core::config::CleanupConfig;
use tinpan_core::create::{GenerateRequest, LyricsDraft, Persona, RemasterRequest};
use tinpan_core::discover::{Comment, Contest, ExploreShelf, Notification};
use tinpan_core::json::FeedPage;
use tinpan_core::playlist::Playlist;
use tinpan_core::{Error, clerk};
use wxdragon::{ffi, prelude::*};

pub const ID_WAKE: i32 = 10_999;

pub enum Command {
    /// Signs in with cookies: saved ones, or `fresh` ones from the sign-in window.
    SignIn { cookies: String, device_id: String, fresh: bool },
    SignOut,
    /// The first library page (no cursor) or the next one.
    LoadLibrary { query: LibraryQuery, more: bool },
    /// The song's file, decrypted, for playing it, or `ahead` of time so it starts at once later.
    FetchSong { clip: Box<SunoClip>, ahead: bool },
    /// Like ("LIKE"), dislike ("DISLIKE") or clear (None).
    React { clip_id: String, reaction: Option<&'static str> },
    Trash { clip_id: String, trashed: bool },
    /// The song's words: its stored lyrics, or its timed words for songs Suno wrote the words for.
    Lyrics { clip: Box<SunoClip> },
    /// Your workspaces, for the filters.
    Workspaces,
    ShareLink { clip_id: String },
    Change { clip_id: String, change: Change },
    /// Saves a song: the free file for "mp3" (whatever type it is), Suno's WAV, or the video; then the
    /// extras, and a clean-up when `clean` has its options.
    Download { clip: Box<SunoClip>, format: String, path: PathBuf, extras: Extras, clean: Option<CleanupConfig> },
    /// Suno's own download, which spends one of the plan's downloads unless the song was downloaded before
    /// (or the account is a Suno Studio one, which has its own route).
    DownloadThroughSuno { clip: Box<SunoClip>, format: String, path: PathBuf, extras: Extras, studio: bool, clean: Option<CleanupConfig> },
    /// Cleans up a song already downloaded.
    Clean { title: String, path: PathBuf, options: CleanupConfig },
    /// Every song given, to its path, through the free route only (the caller left out those already there).
    DownloadAll { songs: Vec<(SunoClip, PathBuf)>, format: String, dir: PathBuf, extras: Extras, clean: Option<CleanupConfig> },
    /// The plan, credits and downloads; `say` is what to say once they're in (F2, Shift+F2), if anything.
    Account { say: Option<AccountText> },
    /// Your playlists, and a playlist's songs.
    Playlists,
    PlaylistSongs { playlist_id: String, announce: bool },
    Trending { announce: bool },
    NewPlaylist { name: String },
    RenamePlaylist { playlist_id: String, name: String, description: String },
    TrashPlaylist { playlist_id: String, name: String },
    /// Into the playlist given, or a new one with `new_name`.
    AddToPlaylist { playlist: Option<Playlist>, new_name: String, clip_id: String },
    RemoveFromPlaylist { playlist_id: String, clip_id: String },
    MovePlaylistSong { playlist_id: String, clip_id: String, from: usize, to: usize },
    Following { announce: bool },
    Explore { announce: bool },
    Contests { announce: bool },
    /// The lists opened from a song, and a creator's songs.
    Similar { clip: Box<SunoClip> },
    Remixes { clip: Box<SunoClip> },
    Original { clip: Box<SunoClip> },
    Creator { handle: String },
    Comments { clip: Box<SunoClip> },
    PostComment { clip_id: String, text: String },
    Personas,
    /// Starts songs; first asks whether Suno wants a captcha, unless the request already carries its token.
    /// `job` is a song made from another (extend, cover), which says it started differently.
    Generate { request: Box<GenerateRequest>, model_takes_voice: Option<bool>, job: bool },
    Remaster { request: Box<RemasterRequest> },
    /// A song's words for the Cover form: stored, or its timed lyrics.
    CoverLyrics { clip: Box<SunoClip> },
    Notifications { say: bool },
    MarkNotificationsRead,
    /// Makes a new song from a finished one.
    Edit { clip_id: String, job: EditJob },
    /// Starts the song's video and follows it until it's ready (up to 10 minutes).
    MakeVideo { clip: Box<SunoClip> },
    MakePersona { clip_id: String, name: String, description: String, public: bool },
    /// The latest release on GitHub against this version; `manual` from Help, Check for updates.
    CheckForUpdates { manual: bool },
    SendReport { report: Box<tinpan_core::report::Report> },
    ExistingStems { clip: Box<SunoClip> },
    Credits { clip: Box<SunoClip> },
    CreditedSong { clip_id: String, title: String },
    CreatorPlaylists { handle: String },
    /// Someone else's playlist: its songs, a like, or saving it to your library.
    OtherPlaylist { id: String, name: String, change: PlaylistChange },
    ShowRemixes { clip_id: String, show: bool },
    SongCover { clip_id: String, title: String, source: CoverSource },
    PlaylistCover { id: String, name: String, path: PathBuf },
    MakeAlbum { playlist_id: String, name: String, description: String, kind: String, publish: bool },
    PersonaList { list: tinpan_core::personas::PersonaList },
    PersonaSongs { id: String, name: String },
    PersonaSource { clip_id: String, name: String },
    LovePersona { id: String, name: String },
    /// `said` is what to say once it's done.
    EditPersona { id: String, name: String, description: String, public: bool, said: String },
    TrashPersona { id: String, undo: bool, said: String },
    PendingModels,
    ModelBases,
    RenameModel { id: String, name: String, said: String },
    ArchiveModel { id: String, said: String },
    /// Trains a custom model on the finished songs of yours in the playlist.
    TrainModel { playlist_id: String, name: String, base: Option<String> },
    Mashup { first: String, second: String },
    /// The playing song's timed lyrics, for reading them as they're sung.
    TimedLyrics { clip_id: String },
    /// `limits` are the plan's, checked again after a file is converted.
    UploadSong { path: PathBuf, title: String, lyrics: String, limits: Option<tinpan_core::billing::UploadLimits> },
    /// The saved song for Resume, when it isn't in a list here.
    ResumeSong { clip_id: String },
    /// Downloads the update and checks its signature.
    DownloadUpdate { update: Box<tinpan_core::update::UpdateInfo> },
    WriteLyrics { topic: String },
    /// How songs Suno is making are doing.
    CheckSongs { ids: Vec<String> },
    /// `to_form`: for the open Extend or Cover form, not the Create window.
    BoostStyle { style: String, lyrics: Option<String>, instrumental: bool, to_form: bool },
    StyleIdeas { to_form: bool },
    Cowrite { lyrics: String, instruction: String, title: String, style: String, to_form: bool },
    LyricsDrafts,
    DraftText { draft: LyricsDraft },
    NewDraft { title: String, lyrics: String },
    SaveDraft { draft: LyricsDraft, lyrics: String },
    RenameDraft { draft: LyricsDraft, title: String },
    DeleteDraft { draft: LyricsDraft },
}

/// What to do with someone else's playlist.
pub enum PlaylistChange {
    Songs,
    Like(bool),
    Save(bool),
}

/// Where a song's new cover comes from.
pub enum CoverSource {
    File(PathBuf),
    Drawn(String),
    SunosOwn,
}

/// The edits Suno makes into a new song.
pub enum EditJob {
    Speed { multiplier: f64, keep_pitch: bool, title: String },
    Reverse { title: String },
    Crop { start: f64, end: f64, remove: bool, title: String },
    Fade { fade_in: Option<f64>, fade_out: Option<f64>, title: String },
    WholeSong,
    Stems,
}

#[derive(Clone, Copy)]
pub enum AccountText {
    Credits,
    Plan,
}

/// The files chosen in Settings to sit next to each download.
#[derive(Clone, Copy)]
pub struct Extras {
    pub lyrics: bool,
    pub timed_lyrics: bool,
    pub details: bool,
    pub cover: bool,
}

/// Something about a song that Suno changes and Tin Pan then shows.
#[derive(Clone)]
pub enum Change {
    /// The new title and lyrics, each only when it changed.
    Details { title: Option<String>, lyrics: Option<String> },
    Public(bool),
    RemixAllowed(bool),
    NotInterested(bool),
    DeletedForever,
}

pub enum Response {
    /// `fresh` cookies worked and should be saved.
    SignedIn { user_name: String, fresh: Option<String> },
    SignInFailed { error: Error, fresh: bool },
    Library { page: FeedPage, more: bool },
    Song { clip: Box<SunoClip>, data: Vec<u8>, ahead: bool },
    /// The song asked for couldn't be fetched to play here.
    SongFailed { clip: Box<SunoClip>, error: Error },
    /// A song fetched ahead that couldn't be; only the log hears about it.
    AheadFailed { clip_id: String },
    /// Suno's reaction now: "L", "D" or none.
    Reacted { clip_id: String, state: Option<String>, asked: Option<&'static str> },
    Trashed { clip_id: String, trashed: bool },
    Lyrics { clip: Box<SunoClip>, text: String },
    /// Empty when they didn't load; the filters still open, as in Tin Pan 1.
    Workspaces(Vec<tinpan_core::api::Workspace>),
    Changed { clip_id: String, change: Change },
    ShareLink(String),
    Downloaded { clip: Box<SunoClip>, through_suno: bool },
    /// A clean-up finished: the cleaned file, or why it didn't work.
    Cleaned { title: String, result: Result<PathBuf, Error> },
    /// Something to say while a long job runs (Download all's halfway point), and when it ends.
    Progress(String),
    /// The plan as it is now, or the reason it didn't load.
    Account { info: Result<Box<tinpan_core::billing::BillingInfo>, Error>, personalization: Option<bool>, say: Option<AccountText> },
    /// The free route didn't work; `offer` says whether Suno's own download may (a refused file, not a
    /// sign-in or disk problem, which would fail that way too).
    DownloadFailed { clip: Box<SunoClip>, format: String, path: PathBuf, error: Error, offer: bool, clean: Option<CleanupConfig> },
    /// Clerk rotated the sign-in cookies; save these.
    CookiesRotated(String),
    Playlists(Vec<Playlist>),
    PlaylistSongs { playlist_id: String, songs: Vec<SunoClip>, announce: bool },
    Trending { songs: Vec<SunoClip>, announce: bool },
    PlaylistMade(Playlist),
    PlaylistRenamed,
    PlaylistTrashed { playlist_id: String, name: String },
    AddedToPlaylist(Playlist),
    RemovedFromPlaylist { playlist_id: String, clip_id: String },
    /// Suno took a move the list had made already.
    MovedInPlaylist,
    Following { songs: Vec<SunoClip>, announce: bool },
    Explore { shelves: Vec<ExploreShelf>, announce: bool },
    Contests { contests: Vec<Contest>, songs: Vec<SunoClip>, announce: bool },
    /// A list to open in the Discover tab; a creator's songs bring who they are.
    ResultList { title: String, songs: Vec<SunoClip>, with_creator: bool, creator: Option<Box<tinpan_core::discover::CreatorPage>> },
    Credits { clip: Box<SunoClip>, sources: Vec<tinpan_core::discover::CreditedSong>, count: Option<i64>, capped: bool },
    CreatorPlaylists(Box<tinpan_core::discover::CreatorPage>),
    ShowRemixes { clip_id: String, show: bool },
    SongCovered { clip_id: String, url: Option<String>, said: String },
    Comments { clip: Box<SunoClip>, comments: Vec<Comment> },
    /// Something to say, such as why a list has nothing to open.
    Say(String),
    Personas(Vec<Persona>),
    CaptchaNeeded { request: Box<GenerateRequest>, provider: i32, model_takes_voice: Option<bool>, job: bool },
    SongsStarted { clips: Vec<SunoClip>, job: bool },
    CoverLyrics { clip: Box<SunoClip>, lyrics: String },
    Notifications { list: Vec<Notification>, say: bool },
    NotificationsRead,
    VideoReady { clip_id: String, url: String },
    UpdateChecked { result: Result<Option<tinpan_core::update::UpdateInfo>, String>, manual: bool },
    ReportSent { kind: &'static str, number: i64 },
    TimedLyrics { clip_id: String, words: Vec<tinpan_core::lyrics::AlignedWord> },
    PersonaList { list: tinpan_core::personas::PersonaList, personas: Vec<tinpan_core::personas::PersonaDetails> },
    PendingModels(Vec<tinpan_core::personas::PendingModel>),
    ModelBases(Vec<tinpan_core::personas::ModelBase>),
    TooFewSongs(usize),
    Mashup(String),
    Uploaded { clip: Box<SunoClip>, title: String },
    /// Something to say whatever the speech level, such as why a report wasn't sent.
    SayError(String),
    ResumeSong(Box<SunoClip>),
    /// The verified file and its version, or what to say.
    UpdateDownloaded(Result<(PathBuf, String), String>),
    LyricsWritten { title: Option<String>, text: String },
    SongsChecked(Result<Vec<SunoClip>, Error>),
    StyleBoosted { style: String, to_form: bool },
    StyleIdeas { ideas: Vec<String>, to_form: bool },
    Cowritten { text: String, to_form: bool },
    LyricsDrafts(Vec<LyricsDraft>),
    DraftText { draft: LyricsDraft, text: Option<String> },
    Failed(Error),
}

/// Wakes the window when there's something to read, by posting it a menu command.
#[derive(Clone, Copy)]
struct Waker(usize);

impl Waker {
    fn wake(self) {
        // SAFETY: the frame outlives the network thread (the thread ends when the window does, since
        // its command channel closes then); posting a command is safe from any thread.
        unsafe { ffi::wxd_Window_PostMenuCommand(self.0 as *mut ffi::wxd_Window_t, ID_WAKE) };
    }
}

pub struct Network {
    commands: Sender<Command>,
    responses: Receiver<Response>,
}

impl Network {
    pub fn start(frame: &Frame) -> Self {
        let (commands, inbox) = mpsc::channel::<Command>();
        let (outbox, responses) = mpsc::channel();
        let waker = Waker(frame.handle_ptr() as usize);
        thread::spawn(move || {
            let api = Arc::new(SunoApi::new(clerk::http_client()));
            for command in inbox {
                // Downloads and clean-ups take a while; each gets its own thread, so playing and paging
                // never wait behind them.
                if command.is_long() {
                    let (api, outbox) = (api.clone(), outbox.clone());
                    thread::spawn(move || {
                        let report = |response| {
                            if outbox.send(response).is_ok() {
                                waker.wake();
                            }
                        };
                        for response in run(&api, command, &report) {
                            report(response);
                        }
                    });
                    continue;
                }
                let report = |response| {
                    if outbox.send(response).is_ok() {
                        waker.wake();
                    }
                };
                for response in run(&api, command, &report) {
                    report(response);
                }
            }
        });
        Self { commands, responses }
    }

    pub fn send(&self, command: Command) {
        let _ = self.commands.send(command);
    }

    /// Somewhere to send work from elsewhere, such as the sign-in window.
    pub fn sender(&self) -> Sender<Command> {
        self.commands.clone()
    }

    /// Everything that has come back so far.
    pub fn drain(&self) -> Vec<Response> {
        self.responses.try_iter().collect()
    }
}

impl Command {
    fn is_long(&self) -> bool {
        // Fetching a song to play (several MB), and lyrics that may need Suno to time them first (up to a minute
        // or more), run apart too, so a like or the next page never waits behind them.
        matches!(self, Command::FetchSong { .. } | Command::Lyrics { .. } | Command::CoverLyrics { .. } | Command::ExistingStems { .. } | Command::Creator { .. } | Command::TrainModel { .. } | Command::Download { .. } | Command::DownloadThroughSuno { .. } | Command::Clean { .. } | Command::DownloadAll { .. } | Command::WriteLyrics { .. } | Command::Cowrite { .. } | Command::BoostStyle { .. } | Command::MakeVideo { .. } | Command::DownloadUpdate { .. } | Command::UploadSong { .. } | Command::TimedLyrics { .. } | Command::Mashup { .. } | Command::SongCover { .. } | Command::PlaylistCover { .. })
    }
}

/// Where Tin Pan keeps tools it unpacks (sunscreen), the same folder as Tin Pan 1.
fn native_dir() -> PathBuf {
    dirs::data_local_dir().unwrap_or_default().join(if cfg!(any(windows, target_os = "macos")) { "Tin Pan" } else { "tin-pan" }).join("native")
}

fn clean(title: &str, path: &Path, options: &CleanupConfig) -> Response {
    let result = tinpan_core::cleanup::clean(path, options, &native_dir(), crate::song_source::decode_to_wav);
    if let Err(e) = &result {
        log::warn!("Clean-up of {title} failed: {e}");
    }
    Response::Cleaned { title: title.to_string(), result }
}

/// Download all: each song through the free route, then cleaned up when chosen; one line halfway through a
/// long list, and a summary at the end, as Tin Pan 1 says them.
fn download_all(api: &SunoApi, songs: &[(SunoClip, PathBuf)], format: &str, dir: &Path, extras: Extras, options: Option<&CleanupConfig>, report: &dyn Fn(Response)) -> Response {
    use tinpan_core::i18n::{plural, t, tf};
    let (mut saved, mut cleaned, mut failed) = (0usize, 0usize, Vec::new());
    for (i, (clip, path)) in songs.iter().enumerate() {
        let path = path.as_path();
        let done = if format == "wav" { api.wav_url(&clip.id).and_then(|url| api.download_to(&url, path)) } else { api.download_free(clip, path) };
        match done {
            Ok(()) => {
                saved += 1;
                save_extras(api, clip, path, extras);
                if let Some(options) = options
                    && tinpan_core::cleanup::clean(path, options, &native_dir(), crate::song_source::decode_to_wav).is_ok()
                {
                    cleaned += 1;
                }
            }
            Err(e) => {
                log::warn!("Download all: one song failed: {e}");
                failed.push(clip.display_title());
                // Signed out, or the folder can't be written: every other song would fail the same way.
                if matches!(e, Error::SignInRequired(_) | Error::Io(_)) {
                    report(Response::Failed(e));
                    break;
                }
            }
        }
        if i + 1 == songs.len() / 2 && songs.len() >= 10 {
            report(Response::Progress(tf("Halfway: {0} saved.", &[&saved])));
        }
    }
    let folder = dir.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
    let mut summary = tf("Downloaded {0} of {1} into {2}.", &[&saved, &plural(songs.len() as i64, "{0} song", "{0} songs"), &folder]);
    if options.is_some() && saved > 0 {
        summary += " ";
        summary += &if cleaned == saved { t("All of them are cleaned up.") } else { tf("{0} of them cleaned up; the log says why the others weren't.", &[&cleaned]) };
    }
    if !failed.is_empty() {
        summary += " ";
        summary += &if failed.len() <= 3 {
            tf("Couldn't download directly: {0}. Ctrl+D on one offers Suno's own download.", &[&failed.join(", ")])
        } else {
            tf("{0} couldn't be downloaded directly. Ctrl+D on one offers Suno's own download.", &[&failed.len()])
        };
    }
    Response::Progress(summary)
}

fn run(api: &SunoApi, command: Command, report: &dyn Fn(Response)) -> Vec<Response> {
    use tinpan_core::i18n::{t, tf};
    let mut out = match command {
        Command::SignIn { cookies, device_id, fresh } => match api.sign_in(&cookies, &device_id) {
            Ok(user_name) => vec![Response::SignedIn { user_name, fresh: fresh.then_some(cookies) }],
            Err(error) => vec![Response::SignInFailed { error, fresh }],
        },
        Command::SignOut => {
            api.sign_out();
            Vec::new()
        }
        Command::LoadLibrary { query, more } => match api.library_page(&query) {
            Ok(page) => vec![Response::Library { page, more }],
            Err(e) => vec![Response::Failed(e)],
        },
        Command::FetchSong { clip, ahead } => match api.song_audio(&clip) {
            Ok(data) => vec![Response::Song { clip, data, ahead }],
            Err(e) if ahead => {
                log::warn!("Fetching {} ahead didn't work: {e}", clip.id);
                vec![Response::AheadFailed { clip_id: clip.id.clone() }]
            }
            // Suno's own player may still play it.
            Err(error) => vec![Response::SongFailed { clip, error }],
        },
        Command::React { clip_id, reaction } => match api.set_reaction(&clip_id, reaction) {
            Ok(state) => vec![Response::Reacted { clip_id, state, asked: reaction }],
            Err(e) => vec![Response::Failed(e)],
        },
        Command::Trash { clip_id, trashed } => match api.set_trashed(&clip_id, trashed) {
            Ok(()) => vec![Response::Trashed { clip_id, trashed }],
            Err(e) => vec![Response::Failed(e)],
        },
        Command::Change { clip_id, change } => {
            let done = match &change {
                Change::Details { title, lyrics } => api.set_details(&clip_id, title.as_deref(), lyrics.as_deref()),
                Change::Public(public) => api.set_public(&clip_id, *public),
                Change::RemixAllowed(allowed) => api.set_remix_allowed(&clip_id, *allowed),
                Change::NotInterested(yes) => api.set_not_interested(&clip_id, *yes),
                Change::DeletedForever => api.delete_forever(&[clip_id.as_str()]),
            };
            match done {
                Ok(()) => vec![Response::Changed { clip_id, change }],
                Err(e) => vec![Response::Failed(e)],
            }
        }
        Command::Clean { title, path, options } => vec![clean(&title, &path, &options)],
        Command::DownloadAll { songs, format, dir, extras, clean } => vec![download_all(api, &songs, &format, &dir, extras, clean.as_ref(), report)],
        Command::Download { clip, format, path, extras, clean: cleaning } => {
            let saved = match format.as_str() {
                "wav" => api.wav_url(&clip.id).and_then(|url| api.download_to(&url, &path)),
                "mp4" => match clip.video_url.as_deref().filter(|u| !u.is_empty()) {
                    Some(url) => api.download_to(url, &path),
                    None => Err(Error::api(tinpan_core::i18n::t("Suno gave no file address for this song."))),
                },
                _ => api.download_free(&clip, &path),
            };
            match saved {
                Ok(()) => {
                    save_extras(api, &clip, &path, extras);
                    let title = clip.display_title();
                    let mut out = vec![Response::Downloaded { clip, through_suno: false }];
                    if let Some(options) = cleaning {
                        report(out.remove(0));
                        report(Response::Progress(tinpan_core::i18n::tf("Cleaning up {0}.", &[&title])));
                        out.push(clean(&title, &path, &options));
                    }
                    out
                }
                Err(error) => {
                    log::warn!("Download {} ({format}) failed directly: {error}", clip.id);
                    let offer = format != "wav" && !matches!(error, Error::SignInRequired(_) | Error::Io(_));
                    vec![Response::DownloadFailed { clip, format, path, error, offer, clean: cleaning }]
                }
            }
        }
        Command::DownloadThroughSuno { clip, format, path, extras, studio, clean: cleaning } => {
            let unlocked = studio || clip.is_download_unlocked == Some(true);
            let saved = (if unlocked { Ok(()) } else { api.authorize_download(&clip.id) })
                .and_then(|()| api.prepare_download(&clip.id, &format, studio))
                .and_then(|url| api.download_to(&url, &path));
            match saved {
                Ok(()) => {
                    save_extras(api, &clip, &path, extras);
                    let title = clip.display_title();
                    let mut out = vec![Response::Downloaded { clip, through_suno: true }];
                    if let Some(options) = cleaning {
                        report(out.remove(0));
                        report(Response::Progress(tinpan_core::i18n::tf("Cleaning up {0}.", &[&title])));
                        out.push(clean(&title, &path, &options));
                    }
                    out
                }
                Err(e) => vec![Response::Failed(e)],
            }
        }
        Command::Account { say } => {
            let personalization = api.style_personalization().unwrap_or_else(|e| {
                log::warn!("Personalization setting didn't load: {e}");
                None
            });
            vec![Response::Account { info: api.billing_info().map(Box::new), personalization, say }]
        }
        Command::ShareLink { clip_id } => match api.share_link(&clip_id) {
            Ok(link) => vec![Response::ShareLink(link)],
            Err(e) => vec![Response::Failed(e)],
        },
        Command::Workspaces => vec![Response::Workspaces(api.workspaces().unwrap_or_else(|e| {
            log::warn!("Workspaces didn't load: {e}");
            Vec::new()
        }))],
        Command::Playlists => match api.playlists() {
            Ok(list) => vec![Response::Playlists(list)],
            // At sign-in this runs quietly; only the log hears, and F5 on a playlist tries again.
            Err(e) => {
                log::warn!("Playlists didn't load: {e}");
                Vec::new()
            }
        },
        Command::PlaylistSongs { playlist_id, announce } => match api.playlist_songs(&playlist_id) {
            Ok(songs) => vec![Response::PlaylistSongs { playlist_id, songs, announce }],
            Err(e) => vec![Response::Failed(e)],
        },
        Command::Trending { announce } => match api.trending() {
            Ok(songs) => vec![Response::Trending { songs, announce }],
            Err(e) => vec![Response::Failed(e)],
        },
        Command::NewPlaylist { name } => match api.create_playlist(&name) {
            Ok(made) => vec![Response::PlaylistMade(made)],
            Err(e) => vec![Response::Failed(e)],
        },
        Command::RenamePlaylist { playlist_id, name, description } => match api.rename_playlist(&playlist_id, &name, &description) {
            Ok(()) => vec![Response::PlaylistRenamed],
            Err(e) => vec![Response::Failed(e)],
        },
        Command::TrashPlaylist { playlist_id, name } => match api.trash_playlist(&playlist_id) {
            Ok(()) => vec![Response::PlaylistTrashed { playlist_id, name }],
            Err(e) => vec![Response::Failed(e)],
        },
        Command::AddToPlaylist { playlist, new_name, clip_id } => {
            let target = match playlist {
                Some(p) => Ok(p),
                None => api.create_playlist(&new_name),
            };
            match target.and_then(|p| api.add_to_playlist(&p.id, &[clip_id.as_str()]).map(|()| p)) {
                Ok(p) => vec![Response::AddedToPlaylist(p)],
                Err(e) => vec![Response::Failed(e)],
            }
        }
        Command::RemoveFromPlaylist { playlist_id, clip_id } => match api.remove_from_playlist(&playlist_id, &[clip_id.as_str()]) {
            Ok(()) => vec![Response::RemovedFromPlaylist { playlist_id, clip_id }],
            Err(e) => vec![Response::Failed(e)],
        },
        // The song moved in Tin Pan already; if Suno refused, the playlist as Suno has it comes back too.
        Command::MovePlaylistSong { playlist_id, clip_id, from, to } => match api.move_in_playlist(&playlist_id, &clip_id, to) {
            Ok(()) => vec![Response::MovedInPlaylist],
            Err(e) => {
                log::warn!("Moving {clip_id} from {from} to {to} wasn't taken: {e}");
                let mut out = vec![Response::Failed(e)];
                if let Ok(songs) = api.playlist_songs(&playlist_id) {
                    out.push(Response::PlaylistSongs { playlist_id, songs, announce: false });
                }
                out
            }
        },
        Command::Personas => match api.personas() {
            Ok(list) => vec![Response::Personas(list)],
            Err(e) => {
                log::warn!("Personas didn't load: {e}");
                Vec::new()
            }
        },
        Command::Generate { request, model_takes_voice, job } => {
            let provider = if request.token.is_some() { Ok(None) } else { api.captcha_needed() };
            vec![match provider {
                Ok(Some(provider)) => Response::CaptchaNeeded { request, provider, model_takes_voice, job },
                Ok(None) => match api.generate(*request, model_takes_voice) {
                    Ok(clips) => Response::SongsStarted { clips, job },
                    Err(e) => Response::Failed(e),
                },
                Err(e) => Response::Failed(e),
            }]
        }
        Command::WriteLyrics { topic } => vec![match api.write_lyrics(&topic) {
            Ok(result) => Response::LyricsWritten { title: result.title, text: result.text.unwrap_or_default() },
            Err(e) => Response::Failed(e),
        }],
        Command::BoostStyle { style, lyrics, instrumental, to_form } => vec![match api.boost_style(&style, lyrics.as_deref(), instrumental) {
            Ok(style) => Response::StyleBoosted { style, to_form },
            Err(e) => Response::Failed(e),
        }],
        Command::StyleIdeas { to_form } => vec![match api.style_ideas() {
            Ok(ideas) => Response::StyleIdeas { ideas, to_form },
            Err(e) => Response::Failed(e),
        }],
        Command::Cowrite { lyrics, instruction, title, style, to_form } => vec![match api.cowrite_lyrics(&lyrics, &instruction, &title, &style) {
            Ok(text) => Response::Cowritten { text, to_form },
            Err(e) => Response::Failed(e),
        }],
        Command::Remaster { request } => vec![match api.remaster(&request) {
            Ok(clips) => Response::SongsStarted { clips, job: true },
            Err(e) => Response::Failed(e),
        }],
        Command::CoverLyrics { clip } => {
            let lyrics = match clip.lyrics() {
                Some(words) => words.trim().to_string(),
                None => api.timed_words(&clip.id).map(|w| tinpan_core::clip::join_words(&w)).unwrap_or_else(|e| {
                    log::warn!("Couldn't fetch lyrics for a cover: {e}");
                    String::new()
                }),
            };
            vec![Response::CoverLyrics { clip, lyrics }]
        }
        Command::Notifications { say } => match api.notifications() {
            Ok(list) => vec![Response::Notifications { list, say }],
            Err(e) if say => vec![Response::Failed(e)],
            Err(e) => {
                log::warn!("Notifications didn't load: {e}");
                Vec::new()
            }
        },
        Command::Edit { clip_id, job } => {
            let made = match job {
                EditJob::Speed { multiplier, keep_pitch, title } => api.change_speed(&clip_id, multiplier, keep_pitch, &title).map(|c| vec![c]),
                EditJob::Reverse { title } => api.reverse(&clip_id, &title).map(|c| vec![c]),
                EditJob::Crop { start, end, remove, title } => api.crop(&clip_id, start, end, remove, &title).map(|c| vec![c]),
                EditJob::Fade { fade_in, fade_out, title } => api.fade(&clip_id, fade_in, fade_out, &title).map(|c| vec![c]),
                EditJob::WholeSong => api.whole_song(&clip_id),
                EditJob::Stems => api.stems(&clip_id),
            };
            vec![match made {
                Ok(clips) => Response::SongsStarted { clips, job: true },
                Err(e) => Response::Failed(e),
            }]
        }
        Command::MakeVideo { clip } => vec![make_video(api, &clip)],
        Command::ExistingStems { clip } => vec![match api.existing_stems(&clip.id) {
            Ok(stems) if stems.is_empty() => Response::Say(tf("No stems have been made from {0} yet. Stems, under Make from it, makes them.", &[&clip.display_title()])),
            Ok(stems) => Response::ResultList { title: tf("Stems of {0}", &[&clip.display_title()]), songs: stems, with_creator: false, creator: None },
            Err(e) => Response::Failed(e),
        }],
        Command::Credits { clip } => vec![match api.credits(&clip.id) {
            Ok((sources, count, capped)) => Response::Credits { clip, sources, count, capped },
            Err(e) => Response::Failed(e),
        }],
        Command::CreditedSong { clip_id, title } => vec![match api.clips(&[clip_id.as_str()]) {
            Ok(songs) if !songs.is_empty() => Response::ResultList { title, songs, with_creator: true, creator: None },
            Ok(_) => Response::SayError(t("That song isn't on Suno anymore.")),
            Err(e) => Response::Failed(e),
        }],
        Command::CreatorPlaylists { handle } => vec![match api.creator(&handle, 1) {
            Ok(page) => Response::CreatorPlaylists(Box::new(page)),
            Err(e) => Response::Failed(e),
        }],
        Command::OtherPlaylist { id, name, change } => vec![match change {
            PlaylistChange::Songs => match api.playlist_songs(&id) {
                Ok(songs) if songs.is_empty() => Response::Say(tf("{0} is empty.", &[&name])),
                Ok(songs) => Response::ResultList { title: name, songs, with_creator: true, creator: None },
                Err(e) => Response::Failed(e),
            },
            PlaylistChange::Like(like) => match api.set_playlist_liked(&id, like) {
                Ok(()) => Response::Say(if like { tf("Liked {0}.", &[&name]) } else { tf("Removed your like from {0}.", &[&name]) }),
                Err(e) => Response::Failed(e),
            },
            PlaylistChange::Save(save) => match api.set_playlist_saved(&id, save) {
                Ok(()) => Response::Say(if save { tf("Saved {0} to your library.", &[&name]) } else { tf("Removed {0} from your library.", &[&name]) }),
                Err(e) => Response::Failed(e),
            },
        }],
        Command::ShowRemixes { clip_id, show } => vec![match api.set_show_remixes(&clip_id, show) {
            Ok(()) => Response::ShowRemixes { clip_id, show },
            Err(e) => Response::Failed(e),
        }],
        Command::SongCover { clip_id, title, source } => {
            let storage = tinpan_core::update::client();
            let done = match source {
                CoverSource::File(path) => api.upload_picture(&storage, &path).and_then(|id| api.set_song_cover(&clip_id, Some(&id), None)).map(|()| (None, tf("{0} has its new cover.", &[&title]))),
                CoverSource::Drawn(prompt) => api.make_picture(&prompt).and_then(|url| api.set_song_cover(&clip_id, None, Some(&url)).map(|()| (Some(url), tf("{0} has its new cover.", &[&title])))),
                CoverSource::SunosOwn => api.set_song_cover(&clip_id, None, None).map(|()| (None, tf("{0} has Suno's own cover again.", &[&title]))),
            };
            vec![match done {
                Ok((url, said)) => Response::SongCovered { clip_id, url, said },
                Err(e) => Response::Failed(e),
            }]
        }
        Command::PlaylistCover { id, name, path } => vec![match api.upload_picture(&tinpan_core::update::client(), &path).and_then(|upload| api.set_playlist_cover(&id, &upload)) {
            Ok(()) => Response::Say(tf("{0} has its new cover.", &[&name])),
            Err(e) => Response::Failed(e),
        }],
        Command::MakeAlbum { playlist_id, name, description, kind, publish } => match api.create_album(&name, &description, &kind, &playlist_id) {
            Ok(id) => {
                let said = if !publish {
                    tf("Made the album {0} as a draft.", &[&name])
                } else if api.publish_album(&id).is_ok() {
                    tf("Made and published the album {0}.", &[&name])
                } else {
                    tf("Made {0} as a draft, but Suno didn't publish it.", &[&name])
                };
                vec![Response::Say(said), Response::Playlists(api.playlists().unwrap_or_default())]
            }
            Err(e) => vec![Response::Failed(e)],
        },
        Command::PersonaList { list } => vec![match api.persona_list(list) {
            Ok(personas) => Response::PersonaList { list, personas },
            Err(e) => Response::Failed(e),
        }],
        Command::PersonaSongs { id, name } => vec![match api.persona_songs(&id) {
            Ok(songs) if songs.is_empty() => Response::Say(tf("No songs made with {0} yet.", &[&name])),
            Ok(songs) => Response::ResultList { title: tf("Songs made with {0}", &[&name]), songs, with_creator: true, creator: None },
            Err(e) => Response::Failed(e),
        }],
        Command::PersonaSource { clip_id, name } => vec![match api.clips(&[clip_id.as_str()]) {
            Ok(songs) if !songs.is_empty() => Response::ResultList { title: tf("Source song of {0}", &[&name]), songs, with_creator: true, creator: None },
            Ok(_) => Response::SayError(t("That song isn't on Suno anymore.")),
            Err(e) => Response::Failed(e),
        }],
        Command::LovePersona { id, name } => vec![match api.toggle_persona_love(&id) {
            Ok(true) => Response::Say(tf("You love {0}.", &[&name])),
            Ok(false) => Response::Say(tf("You no longer love {0}.", &[&name])),
            Err(e) => Response::Failed(e),
        }],
        Command::EditPersona { id, name, description, public, said } => match api.edit_persona(&id, &name, &description, public) {
            Ok(()) => vec![Response::Say(said), Response::Personas(api.personas().unwrap_or_default())],
            Err(e) => vec![Response::Failed(e)],
        },
        Command::TrashPersona { id, undo, said } => match api.trash_persona(&id, undo) {
            Ok(()) => vec![Response::Say(said), Response::Personas(api.personas().unwrap_or_default())],
            Err(e) => vec![Response::Failed(e)],
        },
        Command::PendingModels => vec![match api.pending_models() {
            Ok(pending) => Response::PendingModels(pending),
            Err(e) => Response::Failed(e),
        }],
        Command::ModelBases => vec![Response::ModelBases(api.custom_model_bases().unwrap_or_else(|e| {
            log::warn!("Custom model bases didn't load: {e}");
            Vec::new()
        }))],
        Command::RenameModel { id, name, said } => match api.rename_custom_model(&id, &name) {
            Ok(()) => vec![Response::Say(said), Response::Account { info: api.billing_info().map(Box::new), personalization: None, say: None }],
            Err(e) => vec![Response::Failed(e)],
        },
        Command::ArchiveModel { id, said } => match api.archive_custom_model(&id) {
            Ok(()) => vec![Response::Say(said), Response::Account { info: api.billing_info().map(Box::new), personalization: None, say: None }],
            Err(e) => vec![Response::Failed(e)],
        },
        Command::TrainModel { playlist_id, name, base } => vec![match api.playlist_songs(&playlist_id) {
            Ok(songs) => {
                let mut ids: Vec<String> = Vec::new();
                for song in songs.iter().filter(|s| s.is_complete() && s.is_trashed != Some(true)) {
                    if !ids.contains(&song.id) && ids.len() < 100 {
                        ids.push(song.id.clone());
                    }
                }
                if ids.len() < 6 {
                    Response::TooFewSongs(ids.len())
                } else {
                    match api.create_custom_model(&ids, &name, base.as_deref()) {
                        Ok(()) => Response::Say(tf("Suno is training {0} on {1} songs. It joins your model list when it's ready.", &[&name, &ids.len()])),
                        Err(e) => Response::Failed(e),
                    }
                }
            }
            Err(e) => Response::Failed(e),
        }],
        Command::Mashup { first, second } => vec![match api.mashup_lyrics(&first, &second) {
            Ok(text) => Response::Mashup(text),
            Err(e) => Response::Failed(e),
        }],
        Command::TimedLyrics { clip_id } => {
            let words = api.timed_words(&clip_id).unwrap_or_else(|e| {
                log::warn!("Timed lyrics for {clip_id} not loaded: {e}");
                Vec::new()
            });
            vec![Response::TimedLyrics { clip_id, words }]
        }
        Command::UploadSong { path, title, lyrics, limits } => {
            let stage = |stage: tinpan_core::upload::Stage| {
                if stage == tinpan_core::upload::Stage::Checking {
                    report(Response::Progress(tinpan_core::i18n::tf("Suno is checking {0}. This can take a minute or two.", &[&title])));
                }
            };
            let work = std::env::temp_dir().join("TinPanUpload").join(uuid_like());
            let ready = uploadable(&path, &work, limits);
            let uploaded = ready.and_then(|file| api.upload_song(&tinpan_core::update::client(), &file, &title, &lyrics, &stage)).and_then(|id| {
                api.clips(&[id.as_str()])?.into_iter().find(|c| c.id == id).ok_or_else(|| Error::api(tinpan_core::i18n::t("Suno processed the upload but didn't make a song from it. Refresh the library to check.")))
            });
            if work.exists()
                && let Err(e) = std::fs::remove_dir_all(&work)
            {
                log::warn!("Couldn't remove an upload work folder: {e}");
            }
            vec![match uploaded {
                Ok(clip) => Response::Uploaded { clip: Box::new(clip), title },
                Err(e) => Response::Failed(e),
            }]
        }
        Command::SendReport { report } => vec![match tinpan_core::report::send(&tinpan_core::update::client(), &report) {
            Ok(number) => Response::ReportSent { kind: report.kind, number },
            Err(why) => Response::SayError(why),
        }],
        Command::ResumeSong { clip_id } => vec![match api.clips(&[clip_id.as_str()]) {
            Ok(clips) => match clips.into_iter().find(|c| c.id == clip_id) {
                Some(clip) => Response::ResumeSong(Box::new(clip)),
                None => Response::SayError(tinpan_core::i18n::t("That song isn't on Suno anymore.")),
            },
            Err(e) => Response::Failed(e),
        }],
        Command::CheckForUpdates { manual } => {
            let result = tinpan_core::update::check(&tinpan_core::update::client(), &crate::updates::current());
            vec![Response::UpdateChecked { result, manual }]
        }
        Command::DownloadUpdate { update } => {
            let halfway = std::cell::Cell::new(false);
            let progress = |part: f64| {
                if part >= 0.5 && !halfway.replace(true) {
                    report(Response::Progress(tinpan_core::i18n::t("Update download halfway.")));
                }
            };
            let version = update.version.short();
            vec![Response::UpdateDownloaded(tinpan_core::update::download(&tinpan_core::update::client(), &update, &progress).map(|file| (file, version)))]
        }
        Command::MakePersona { clip_id, name, description, public } => match api.create_persona(&clip_id, &name, &description, public) {
            Ok(()) => vec![Response::Say(tf("Made the persona {0}.", &[&name])), Response::Personas(api.personas().unwrap_or_default())],
            Err(e) => vec![Response::Failed(e)],
        },
        Command::MarkNotificationsRead => vec![match api.mark_notifications_read() {
            Ok(()) => Response::NotificationsRead,
            Err(e) => Response::Failed(e),
        }],
        Command::LyricsDrafts => vec![match api.lyrics_drafts() {
            Ok(drafts) => Response::LyricsDrafts(drafts),
            Err(e) => Response::Failed(e),
        }],
        Command::DraftText { draft } => vec![match api.lyrics_draft_text(&draft.id) {
            Ok(text) => Response::DraftText { draft, text },
            Err(e) => Response::Failed(e),
        }],
        Command::NewDraft { title, lyrics } => vec![match api.new_lyrics_draft(&title, &lyrics) {
            Ok(()) => Response::Say(tf("Saved as the draft {0}.", &[&if title.trim().is_empty() { t("Untitled") } else { title.trim().to_string() }])),
            Err(e) => Response::Failed(e),
        }],
        Command::SaveDraft { draft, lyrics } => vec![match api.save_lyrics_draft(&draft.id, &lyrics) {
            Ok(()) => Response::Say(tf("Saved into {0}.", &[&draft.name()])),
            Err(e) => Response::Failed(e),
        }],
        Command::RenameDraft { draft, title } => vec![match api.rename_lyrics_draft(&draft.id, &title) {
            Ok(()) => Response::Say(tf("Renamed to {0}.", &[&title])),
            Err(e) => Response::Failed(e),
        }],
        Command::DeleteDraft { draft } => vec![match api.delete_lyrics_draft(&draft.id) {
            Ok(()) => Response::Say(tf("Deleted the draft {0}.", &[&draft.name()])),
            Err(e) => Response::Failed(e),
        }],
        Command::CheckSongs { ids } => {
            let ids: Vec<&str> = ids.iter().map(String::as_str).collect();
            vec![Response::SongsChecked(api.clips(&ids))]
        }
        Command::Following { announce } => match api.following() {
            Ok(songs) => vec![Response::Following { songs, announce }],
            Err(e) => vec![Response::Failed(e)],
        },
        Command::Explore { announce } => match api.explore() {
            Ok(shelves) => vec![Response::Explore { shelves, announce }],
            Err(e) => vec![Response::Failed(e)],
        },
        Command::Contests { announce } => match api.contests() {
            Ok((contests, songs)) => vec![Response::Contests { contests, songs, announce }],
            Err(e) => vec![Response::Failed(e)],
        },
        Command::Similar { clip } => vec![match api.similar(&clip.id) {
            Ok(songs) if songs.is_empty() => Response::Say(t("Suno found no similar songs.")),
            Ok(songs) => Response::ResultList { title: tf("Songs similar to {0}", &[&clip.display_title()]), songs, with_creator: true, creator: None },
            Err(e) => Response::Failed(e),
        }],
        Command::Remixes { clip } => vec![match api.remixes(&clip.id) {
            Ok(songs) if songs.is_empty() => Response::Say(tf("No one has covered or remixed {0} yet.", &[&clip.display_title()])),
            Ok(songs) => Response::ResultList { title: tf("Covers and remixes of {0}", &[&clip.display_title()]), songs, with_creator: true, creator: None },
            Err(e) => Response::Failed(e),
        }],
        Command::Original { clip } => vec![match api.original(&clip) {
            Ok(None) => Response::Say(tf("{0} wasn't made from another song.", &[&clip.display_title()])),
            Ok(Some(song)) => Response::ResultList { title: tf("{0} was made from", &[&clip.display_title()]), songs: vec![song], with_creator: true, creator: None },
            Err(e) => Response::Failed(e),
        }],
        Command::Creator { handle } => vec![match api.creator_songs(&handle) {
            Ok(page) if page.songs.is_empty() => Response::Say(tf("{0} has no public songs.", &[&page.name()])),
            Ok(page) => Response::ResultList { title: page.summary(), songs: page.songs.clone(), with_creator: false, creator: Some(Box::new(page)) },
            Err(e) => Response::Failed(e),
        }],
        Command::Comments { clip } => match api.comments(&clip.id) {
            Ok(comments) => vec![Response::Comments { clip, comments }],
            Err(e) => vec![Response::Failed(e)],
        },
        Command::PostComment { clip_id, text } => vec![match api.post_comment(&clip_id, &text) {
            Ok(()) => Response::Say(t("Comment posted.")),
            Err(e) => Response::Failed(e),
        }],
        Command::Lyrics { clip } => {
            if let Some(lyrics) = clip.lyrics() {
                let text = lyrics.to_string();
                vec![Response::Lyrics { clip, text }]
            } else {
                // Description-mode songs keep their words only in the timed lyrics.
                match api.timed_words(&clip.id) {
                    Ok(words) => vec![Response::Lyrics { text: tinpan_core::clip::join_words(&words), clip }],
                    Err(e) => vec![Response::Failed(e)],
                }
            }
        }
    };
    if let Some(cookies) = api.rotated_cookies() {
        out.push(Response::CookiesRotated(cookies));
    }
    out
}

/// A folder name of its own for one upload's work.
fn uuid_like() -> String {
    format!("{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos()))
}

/// The file to send: as it is when Suno takes its type, otherwise converted to M4A in `work` (named like the
/// original), and then checked against the plan's lengths, as Tin Pan 1 does.
fn uploadable(path: &Path, work: &Path, limits: Option<tinpan_core::billing::UploadLimits>) -> Result<PathBuf, Error> {
    use tinpan_core::clip::format_time;
    use tinpan_core::i18n::{t, tf};
    let extension = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    if tinpan_core::upload::EXTENSIONS.contains(&extension.as_str()) {
        return Ok(path.to_path_buf());
    }
    std::fs::create_dir_all(work).map_err(|e| Error::Io(e.to_string()))?;
    let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "upload".into());
    let target = work.join(format!("{stem}.m4a"));
    if let Err(e) = crate::to_m4a::convert(path, &target) {
        log::warn!("Couldn't convert {extension} to M4A: {e}");
        return Err(Error::api(if cfg!(windows) {
            t("Windows couldn't turn this file into M4A for uploading. Try an MP3, WAV or M4A file.")
        } else {
            t("Tin Pan can't turn this kind of file into M4A on this system. Try an MP3, WAV or M4A file.")
        }));
    }
    let seconds = std::fs::read(&target).ok().and_then(|d| crate::song_source::SongSource::new(d).ok()).and_then(|s| rodio::Source::total_duration(&s)).map(|d| d.as_secs_f64());
    if let (Some(seconds), Some(limits)) = (seconds, limits) {
        if let Some(min) = limits.min_seconds().filter(|m| seconds < *m) {
            return Err(Error::api(tf("That song is {0}. Suno takes uploads of at least {1}.", &[&format_time(seconds), &format_time(min)])));
        }
        if let Some(max) = limits.max_seconds().filter(|m| seconds > *m) {
            return Err(Error::api(tf("That song is {0}. Suno takes uploads of up to {1} on this account.", &[&format_time(seconds), &format_time(max)])));
        }
    }
    Ok(target)
}

/// Starts the song's video, then asks how it's doing every 6 seconds for up to 10 minutes, as Tin Pan 1 does.
fn make_video(api: &SunoApi, clip: &SunoClip) -> Response {
    use tinpan_core::i18n::tf;
    let title = clip.display_title();
    if let Err(e) = api.start_video(&clip.id) {
        return Response::Failed(e);
    }
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10 * 60);
    while std::time::Instant::now() < until {
        thread::sleep(std::time::Duration::from_secs(6));
        match api.video_status(&clip.id) {
            Ok((status, _)) if status == "failed" || status == "error" => return Response::Say(tf("Suno couldn't make the video for {0}.", &[&title])),
            Ok((status, Some(url))) if status == "complete" => return Response::VideoReady { clip_id: clip.id.clone(), url },
            Ok(_) => {}
            Err(e) => log::warn!("Video status for {}: {e}", clip.id),
        }
    }
    Response::Say(tf("The video for {0} is taking long. Check again later with Download video.", &[&title]))
}

/// The files chosen in Settings next to a download: lyrics, timed lyrics (LRC), details and cover. The
/// song itself is saved; a missing extra file only goes to the log, as in Tin Pan 1.
fn save_extras(api: &SunoApi, clip: &SunoClip, audio: &Path, extras: Extras) {
    let stem = audio.with_extension("");
    let with = |ending: &str| PathBuf::from(format!("{}{ending}", stem.display()));
    let mut problems = Vec::new();
    if extras.lyrics && let Some(lyrics) = clip.lyrics() {
        problems.extend(std::fs::write(with(".txt"), lyrics).err().map(|e| e.to_string()));
    }
    if extras.details {
        problems.extend(std::fs::write(with(".details.txt"), clip.details_text()).err().map(|e| e.to_string()));
    }
    if extras.timed_lyrics {
        match api.timed_words(&clip.id) {
            Ok(words) if !words.is_empty() => {
                problems.extend(std::fs::write(with(".lrc"), tinpan_core::lyrics::lrc(clip, &words)).err().map(|e| e.to_string()));
            }
            Ok(_) => {}
            Err(e) => problems.push(e.to_string()),
        }
    }
    if extras.cover && let Some(cover) = clip.image_large_url.as_deref().or(clip.image_url.as_deref()).filter(|c| !c.is_empty()) {
        let path = with(&format!(".{}", tinpan_core::files::extension_of(Some(cover), "jpg")));
        problems.extend(api.download_to(cover, &path).err().map(|e| e.to_string()));
    }
    for problem in problems {
        log::warn!("Extra files for a download failed: {problem}");
    }
}
