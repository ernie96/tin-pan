//! Every command, its group, its keys and its text, as in Tin Pan 1's MainViewModel.Commands: the menu
//! bar and the Applications key menu are both built from this table, so they can't drift apart. Texts
//! are Tin Pan 1's own, with "_" before the access key, so its checked translations and access keys
//! apply; they become wxWidgets' "&" here.

use tinpan_core::clip::SunoClip;
use tinpan_core::i18n::{t, tf};
use wxdragon::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    // Playback
    PlayPause,
    Stop,
    Previous,
    Next,
    Back,
    Forward,
    BackPercent,
    ForwardPercent,
    Restart,
    Resume,
    NowPlaying,
    LiveLyrics,
    VolumeDown,
    VolumeUp,
    ChooseOutput,
    ChooseSecondOutput,
    ToggleOutput,
    // Song
    Play,
    Details,
    Information,
    Lyrics,
    Like,
    Dislike,
    NotInterested,
    Extend,
    Cover,
    Inspire,
    Remaster,
    WholeSong,
    Stems,
    ChangeSpeed,
    Reverse,
    Crop,
    Fade,
    MakeVideo,
    MakePersona,
    Download,
    DownloadMp3,
    DownloadWav,
    DownloadVideo,
    CleanUpDownload,
    CopyLink,
    ShareLink,
    Visibility,
    Remixes,
    Similar,
    Creator,
    ItsStems,
    SongCredits,
    CreatorPlaylists,
    ShowRemixes,
    CoverPicture,
    /// Covers and remixes others made of the song (Remixes is whether they may).
    CoversAndRemixes,
    Original,
    ShowComments,
    WriteComment,
    Edit,
    AddToPlaylist,
    Trash,
    DeleteForever,
    // A playlist's songs (the Applications key menu only)
    RemoveFromPlaylist,
    MoveUp,
    MoveDown,
    // The lists box's own menu
    OpenList,
    PlaylistCover,
    MakeAlbum,
    DeleteList,
    CreatorInfo,
    // Library
    NewSong,
    UploadSong,
    Personas,
    CustomModels,
    LyricsMashup,
    Refresh,
    Search,
    /// The Library page's Search and Clear search buttons (Enter and Escape in the box do the same).
    SearchNow,
    ClearSearch,
    Filters,
    DownloadAll,
    NewPlaylist,
    RenamePlaylist,
    DeletePlaylist,
    Exit,
    // Discover
    Explore,
    Following,
    FindCreator,
    Contests,
    // View
    ShowLibrary,
    ShowDiscover,
    NextWindow,
    ShowAccount,
    Settings,
    // Account
    Credits,
    PlanText,
    MarkNotificationsRead,
    SignIn,
    SignOut,
    // Help
    Help,
    AllCommands,
    Repeat,
    History,
    SpeakEverything,
    SpeakAskedFor,
    SpeakErrorsOnly,
    ReportProblem,
    SuggestFeature,
    CheckForUpdates,
    Licenses,
    About,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Playback,
    Song,
    /// Not in the menu bar: put first in the Applications key menu of a playlist's songs.
    PlaylistSong,
    /// Not in the menu bar: the Applications key menu of a lists box.
    Lists,
    Library,
    Discover,
    View,
    Account,
    Help,
}

pub struct Entry {
    pub group: Group,
    /// Tin Pan 1's text, with "_" before the access key.
    pub label: &'static str,
    /// Shown in the menu and made a shortcut there; empty for none.
    pub keys: &'static str,
    pub action: Action,
    pub separator_before: bool,
    /// Shown in the menu but handled by the song list itself, so it only works there (Enter).
    pub list_only: bool,
    /// Tin Pan 1's submenu text ("S_haring"), for entries in a submenu of their group.
    pub submenu: Option<&'static str>,
}

const fn entry(group: Group, label: &'static str, keys: &'static str, action: Action) -> Entry {
    Entry { group, label, keys, action, separator_before: false, list_only: false, submenu: None }
}

const fn sharing(mut e: Entry) -> Entry {
    e.submenu = Some("S_haring");
    e
}

const fn download(mut e: Entry) -> Entry {
    e.submenu = Some("D_ownload");
    e
}

const fn make(mut e: Entry) -> Entry {
    e.submenu = Some("_Make from it");
    e
}

const fn related(mut e: Entry) -> Entry {
    e.submenu = Some("Related so_ngs");
    e
}

const fn comments(mut e: Entry) -> Entry {
    e.submenu = Some("_Comments");
    e
}

const fn speech(mut e: Entry) -> Entry {
    e.submenu = Some("_Speech");
    e
}

const fn after_line(mut e: Entry) -> Entry {
    e.separator_before = true;
    e
}

const fn in_list(mut e: Entry) -> Entry {
    e.list_only = true;
    e
}

use Action as A;
use Group as G;

/// In Tin Pan 1's order; the menu bar's groups come in the order they first appear.
pub const COMMANDS: &[Entry] = &[
    entry(G::Playback, "_Play or pause", "F8", A::PlayPause),
    entry(G::Playback, "_Stop", "Shift+F8", A::Stop),
    entry(G::Playback, "Pre_vious song", "F7", A::Previous),
    entry(G::Playback, "_Next song", "F9", A::Next),
    after_line(entry(G::Playback, "_Back 10 seconds", "Shift+F7", A::Back)),
    entry(G::Playback, "_Forward 10 seconds", "Shift+F9", A::Forward),
    entry(G::Playback, "Bac_k 10 percent", "Ctrl+F7", A::BackPercent),
    entry(G::Playback, "Forwar_d 10 percent", "Ctrl+F9", A::ForwardPercent),
    entry(G::Playback, "Back to the s_tart", "Ctrl+F8", A::Restart),
    entry(G::Playback, "_Resume where I left off", "Ctrl+Shift+F8", A::Resume),
    after_line(entry(G::Playback, "_What's playing", "F3", A::NowPlaying)),
    entry(G::Playback, "Read _lyrics as they're sung, on or off", "Shift+F3", A::LiveLyrics),
    after_line(entry(G::Playback, "Volume d_own", "F11", A::VolumeDown)),
    entry(G::Playback, "Volume _up", "F12", A::VolumeUp),
    after_line(entry(G::Playback, "_Audio output...", "", A::ChooseOutput)),
    entry(G::Playback, "Se_cond audio output...", "", A::ChooseSecondOutput),
    entry(G::Playback, "Switc_h to the other output", "Ctrl+Shift+F12", A::ToggleOutput),
    in_list(entry(G::Song, "_Play", "Enter", A::Play)),
    entry(G::Song, "_Details", "Ctrl+I", A::Details),
    entry(G::Song, "Song in_formation", "Alt+Enter", A::Information),
    entry(G::Song, "_Lyrics", "Ctrl+L", A::Lyrics),
    entry(G::Song, "Li_ke", "Ctrl+Shift+U", A::Like),
    entry(G::Song, "D_islike", "Ctrl+Shift+N", A::Dislike),
    entry(G::Song, "Not interes_ted: fewer like this", "", A::NotInterested),
    after_line(make(entry(G::Song, "_Extend...", "Ctrl+E", A::Extend))),
    make(entry(G::Song, "_Cover...", "Ctrl+Shift+C", A::Cover)),
    make(entry(G::Song, "Use as _inspiration...", "", A::Inspire)),
    make(entry(G::Song, "Re_master...", "Ctrl+Shift+M", A::Remaster)),
    make(entry(G::Song, "_Get whole song", "Ctrl+J", A::WholeSong)),
    make(entry(G::Song, "S_tems", "Ctrl+Shift+S", A::Stems)),
    make(after_line(entry(G::Song, "Change s_peed...", "", A::ChangeSpeed))),
    make(entry(G::Song, "Re_verse...", "", A::Reverse)),
    make(entry(G::Song, "Cr_op...", "", A::Crop)),
    make(entry(G::Song, "_Fade in or out...", "", A::Fade)),
    make(after_line(entry(G::Song, "Make a vi_deo", "", A::MakeVideo))),
    make(entry(G::Song, "Make a pe_rsona...", "", A::MakePersona)),
    download(entry(G::Song, "_Download...", "Ctrl+D", A::Download)),
    download(entry(G::Song, "Download MP_3", "Ctrl+Shift+D", A::DownloadMp3)),
    download(entry(G::Song, "Download _WAV", "Ctrl+Shift+W", A::DownloadWav)),
    download(entry(G::Song, "Download _video", "Ctrl+Shift+V", A::DownloadVideo)),
    download(entry(G::Song, "Clean up download (remove ha_ze)", "Ctrl+Shift+H", A::CleanUpDownload)),
    related(entry(G::Song, "Similar so_ngs...", "Ctrl+M", A::Similar)),
    related(entry(G::Song, "_Creator's songs...", "Ctrl+U", A::Creator)),
    related(entry(G::Song, "Covers and _remixes of it...", "Ctrl+Shift+O", A::CoversAndRemixes)),
    related(entry(G::Song, "_Original song...", "Ctrl+O", A::Original)),
    related(entry(G::Song, "Its _stems...", "", A::ItsStems)),
    related(entry(G::Song, "Cre_dits and remix count...", "", A::SongCredits)),
    related(entry(G::Song, "Creator's play_lists...", "", A::CreatorPlaylists)),
    comments(entry(G::Song, "S_how comments", "Ctrl+K", A::ShowComments)),
    comments(entry(G::Song, "_Write a comment...", "Ctrl+Shift+K", A::WriteComment)),
    sharing(entry(G::Song, "Copy l_ink", "Ctrl+C", A::CopyLink)),
    sharing(entry(G::Song, "Copy Suno share lin_k", "Ctrl+Shift+L", A::ShareLink)),
    sharing(entry(G::Song, "Make p_ublic", "Ctrl+Shift+P", A::Visibility)),
    sharing(entry(G::Song, "Stop remi_xes", "Ctrl+Shift+X", A::Remixes)),
    sharing(entry(G::Song, "Show the list of re_mixes", "", A::ShowRemixes)),
    after_line(entry(G::Song, "Add to playli_st...", "Ctrl+P", A::AddToPlaylist)),
    entry(G::Song, "Edit title and l_yrics...", "Ctrl+T", A::Edit),
    entry(G::Song, "Co_ver picture...", "", A::CoverPicture),
    // Delete is the list's own key, so it still deletes text in the search box.
    in_list(entry(G::Song, "Move to tr_ash", "Delete", A::Trash)),
    entry(G::Song, "Delete fo_rever...", "Shift+Delete", A::DeleteForever),
    in_list(entry(G::PlaylistSong, "Remove from playlist", "Delete", A::RemoveFromPlaylist)),
    in_list(entry(G::PlaylistSong, "Move up", "Ctrl+Up", A::MoveUp)),
    in_list(entry(G::PlaylistSong, "Move down", "Ctrl+Down", A::MoveDown)),
    in_list(entry(G::Lists, "_Open", "Enter", A::OpenList)),
    entry(G::Lists, "_New playlist...", "", A::NewPlaylist),
    entry(G::Lists, "Rena_me...", "", A::RenamePlaylist),
    entry(G::Lists, "Cover _picture...", "", A::PlaylistCover),
    entry(G::Lists, "Make an _album from it...", "", A::MakeAlbum),
    in_list(entry(G::Lists, "_Delete", "Delete", A::DeleteList)),
    entry(G::Lists, "About this cr_eator", "", A::CreatorInfo),
    in_list(entry(G::Lists, "_Refresh", "F5", A::Refresh)),
    entry(G::Library, "_New song...", "Ctrl+N", A::NewSong),
    entry(G::Library, "_Upload a song...", "", A::UploadSong),
    after_line(entry(G::Library, "_Refresh", "F5", A::Refresh)),
    entry(G::Library, "_Search", "Ctrl+F", A::Search),
    entry(G::Library, "_Filters...", "Alt+F", A::Filters),
    after_line(entry(G::Library, "Download _all...", "Alt+L", A::DownloadAll)),
    after_line(entry(G::Library, "New pla_ylist...", "", A::NewPlaylist)),
    entry(G::Library, "Rena_me playlist...", "", A::RenamePlaylist),
    entry(G::Library, "Dele_te playlist...", "", A::DeletePlaylist),
    after_line(entry(G::Library, "_Personas...", "", A::Personas)),
    entry(G::Library, "Custom mo_dels...", "", A::CustomModels),
    entry(G::Library, "Lyrics mas_hup...", "", A::LyricsMashup),
    after_line(entry(G::Library, "E_xit", "Alt+F4", A::Exit)),
    entry(G::Discover, "_Explore...", "", A::Explore),
    entry(G::Discover, "From people I _follow...", "", A::Following),
    entry(G::Discover, "Find a c_reator...", "", A::FindCreator),
    entry(G::Discover, "_Contests...", "", A::Contests),
    entry(G::View, "_Library", "Ctrl+1", A::ShowLibrary),
    entry(G::View, "_Discover", "Ctrl+2", A::ShowDiscover),
    entry(G::View, "_Account", "Ctrl+4", A::ShowAccount),
    entry(G::View, "_Settings", "Ctrl+5", A::Settings),
    after_line(entry(G::View, "Next _window", "F6", A::NextWindow)),
    entry(G::Account, "_Credits left", "F2", A::Credits),
    entry(G::Account, "_Plan and downloads", "Shift+F2", A::PlanText),
    entry(G::Account, "Mark _notifications as read", "", A::MarkNotificationsRead),
    after_line(entry(G::Account, "Sign _in", "", A::SignIn)),
    entry(G::Account, "Sign _out", "", A::SignOut),
    entry(G::Help, "_Help for here", "F1", A::Help),
    entry(G::Help, "_All commands", "Ctrl+F1", A::AllCommands),
    entry(G::Help, "_Repeat last message", "Ctrl+R", A::Repeat),
    entry(G::Help, "_Message history", "Ctrl+Shift+R", A::History),
    speech(entry(G::Help, "Speak e_verything", "", A::SpeakEverything)),
    speech(entry(G::Help, "Speak only what I as_k for", "", A::SpeakAskedFor)),
    speech(entry(G::Help, "Speak only _errors", "", A::SpeakErrorsOnly)),
    after_line(entry(G::Help, "Report a _problem...", "Ctrl+Shift+B", A::ReportProblem)),
    entry(G::Help, "Suggest a _feature...", "Ctrl+Shift+F", A::SuggestFeature),
    after_line(entry(G::Help, "Check for _updates", "", A::CheckForUpdates)),
    entry(G::Help, "_Licenses and notices", "", A::Licenses),
    entry(G::Help, "About Tin Pa_n", "", A::About),
];

const FIRST_ID: i32 = 10_100;

pub fn id(action: Action) -> i32 {
    FIRST_ID + COMMANDS.iter().position(|e| e.action == action).unwrap_or(0) as i32
}

pub fn action_for(id: i32) -> Option<Action> {
    usize::try_from(id - FIRST_ID).ok().and_then(|i| COMMANDS.get(i)).map(|e| e.action)
}

pub fn group_name(group: Group) -> String {
    t(match group {
        G::Playback => "Playback",
        G::Song | G::PlaylistSong => "Song",
        G::Lists => "Lists",
        G::Library => "Library",
        G::Discover => "Discover",
        G::View => "View",
        G::Account => "Account",
        G::Help => "Help",
    })
}

/// What the plan allows, for the Cover and Remaster texts: paid or not, and the free ones left.
#[derive(Clone, Copy, Default)]
pub struct PlanLimits {
    pub paid: Option<bool>,
    pub free_covers: Option<i64>,
    pub free_remasters: Option<i64>,
}

thread_local! {
    static PLAN: std::cell::Cell<PlanLimits> = std::cell::Cell::new(PlanLimits::default());
}

/// Sets what the Cover and Remaster menu texts say; the next relabel shows it.
pub fn set_plan(limits: PlanLimits) {
    PLAN.set(limits);
}

/// A Song entry's text for this song, where it changes with the song or the plan (Tin Pan 1's SongLabel,
/// CoverMenuText and RemasterMenuText).
fn song_label(entry: &Entry, song: Option<&SunoClip>, not_interested: bool) -> String {
    let plan = PLAN.get();
    let free = plan.paid == Some(false);
    t(match entry.action {
        A::NotInterested if not_interested => "Suggest songs like this a_gain",
        A::Like if song.is_some_and(|s| s.is_liked) => "Remove li_ke",
        A::Dislike if song.is_some_and(SunoClip::is_disliked) => "Remove d_islike",
        A::Trash if song.is_some_and(|s| s.is_trashed == Some(true)) => "Restore from tr_ash",
        A::Visibility if song.is_some_and(|s| s.is_public) => "Make p_rivate",
        A::Remixes if song.is_some_and(|s| s.metadata.as_ref().and_then(|m| m.can_remix) == Some(false)) => "Allow remi_xes",
        A::ShowRemixes if song.is_some_and(|s| s.metadata.as_ref().and_then(|m| m.show_remix) == Some(true)) => "Hide the list of re_mixes",
        A::Cover if free && plan.free_covers.is_some() => return tf("_Cover... ({0} free left)", &[&plan.free_covers.unwrap_or(0)]),
        A::Remaster if free && plan.free_remasters.is_some_and(|n| n > 0) => return tf("Re_master... ({0} free left)", &[&plan.free_remasters.unwrap_or(0)]),
        A::Remaster if plan.paid != Some(true) => return tf("{0} (needs Pro or Premier)", &[&t("Re_master...")]),
        _ => entry.label,
    })
}

/// A playback command's key, pressed where the menu bar's shortcuts don't reach (the Create window and the
/// forms that stay open), or a media key (Play/Pause, Stop, Previous, Next), as in Tin Pan 1.
pub fn playback_key(code: i32, ctrl: bool, shift: bool, alt: bool) -> Option<Action> {
    match code {
        WXK_MEDIA_PLAY_PAUSE => return Some(A::PlayPause),
        WXK_MEDIA_STOP => return Some(A::Stop),
        WXK_MEDIA_PREV_TRACK => return Some(A::Previous),
        WXK_MEDIA_NEXT_TRACK => return Some(A::Next),
        _ => {}
    }
    let name = match code {
        WXK_F3 => "F3",
        WXK_F7 => "F7",
        WXK_F8 => "F8",
        WXK_F9 => "F9",
        WXK_F11 => "F11",
        WXK_F12 => "F12",
        _ => return None,
    };
    let mut keys = String::new();
    if ctrl {
        keys.push_str("Ctrl+");
    }
    if alt {
        keys.push_str("Alt+");
    }
    if shift {
        keys.push_str("Shift+");
    }
    keys.push_str(name);
    COMMANDS.iter().find(|e| e.group == G::Playback && e.keys == keys).map(|e| e.action)
}

/// Ctrl+F1's list: every command as "Group, submenu: text, keys", in the menus' order.
pub fn command_lines() -> Vec<(Action, String)> {
    COMMANDS
        .iter()
        .filter(|e| !matches!(e.group, G::PlaylistSong | G::Lists))
        .map(|e| {
            let sub = e.submenu.map(|s| format!(", {}", t(s).replace('_', ""))).unwrap_or_default();
            let keys = if e.keys.is_empty() { String::new() } else { format!(", {}", e.keys) };
            (e.action, format!("{}{sub}: {}{keys}", group_name(e.group), t(e.label).replace('_', "")))
        })
        .collect()
}

/// Tin Pan 1's "_" access key as wxWidgets' "&" (a real "&" doubled), with the keys after a tab.
fn menu_text(label: &str, keys: &str, shortcut: bool) -> String {
    // Labels come in English or already translated; either way t() gives the shown text.
    let text = t(label).replace('&', "&&").replacen('_', "&", 1);
    match (keys.is_empty(), shortcut) {
        (true, _) => text,
        // A tab makes the keys a shortcut; list-only keys are only named, so they don't fire elsewhere.
        (false, true) => format!("{text}\t{keys}"),
        (false, false) => format!("{text} ({keys})"),
    }
}

fn fill(menu: &Menu, group: Group, song: Option<&SunoClip>, not_interested: bool) {
    let entries: Vec<&Entry> = COMMANDS.iter().filter(|e| e.group == group).collect();
    let mut done_submenus: Vec<&str> = Vec::new();
    for entry in &entries {
        // A line before a later entry of a submenu goes inside that submenu, not here.
        let starts_here = entry.submenu.is_none_or(|name| !done_submenus.contains(&name));
        if entry.separator_before && starts_here && menu.get_item_count() > 0 {
            menu.append_separator();
        }
        let item = |into: &Menu, e: &Entry| {
            into.append(id(e.action), &menu_text(&song_label(e, song, not_interested), e.keys, !e.list_only), "", ItemKind::Normal);
        };
        match entry.submenu {
            None => item(menu, entry),
            // A submenu goes where its first entry is, with all its entries.
            Some(name) if !done_submenus.contains(&name) => {
                done_submenus.push(name);
                let sub = Menu::builder().build();
                for e in entries.iter().filter(|e| e.submenu == Some(name)) {
                    if e.separator_before && sub.get_item_count() > 0 {
                        sub.append_separator();
                    }
                    item(&sub, e);
                }
                menu.append_submenu(sub, &menu_text(name, "", true), "");
            }
            Some(_) => {}
        }
    }
}

/// The menu bar. As in Tin Pan 1, its menus have no Alt letters of their own (Alt or F10, then the arrows,
/// open them), so every Alt letter on the pages stays the page's: _Songs, Searc_h, Download a_ll, _Volume.
pub fn menu_bar() -> MenuBar {
    let mut bar = MenuBar::builder();
    for group in [G::Library, G::Song, G::Discover, G::Playback, G::View, G::Account, G::Help] {
        let menu = Menu::builder().build();
        fill(&menu, group, None, false);
        bar = bar.append(menu, &group_name(group).replace('&', "&&"));
    }
    bar.build()
}

/// The Applications key menu of a song list: the Song menu, for the selected song, with a playlist's own
/// Remove and Move first in a playlist. `not_interested` says whether it was marked so in this run (Suno
/// doesn't say which songs were).
pub fn song_menu(song: Option<&SunoClip>, not_interested: bool, in_playlist: bool) -> Menu {
    let menu = Menu::builder().build();
    if in_playlist {
        fill(&menu, G::PlaylistSong, song, not_interested);
        menu.append_separator();
    }
    fill(&menu, G::Song, song, not_interested);
    menu
}

/// The Applications key menu of a lists box: the Lists entries `shown` lets through (Tin Pan 1's Playlists tab menu).
pub fn lists_menu(shown: impl Fn(Action) -> bool) -> Menu {
    let menu = Menu::builder().build();
    // A Lists entry may share its action with a menu bar entry; the id is the one the menu bar uses, which runs it the same.
    for e in COMMANDS.iter().filter(|e| e.group == G::Lists && shown(e.action)) {
        menu.append(id(e.action), &menu_text(e.label, e.keys, false), "", ItemKind::Normal);
    }
    menu
}

/// Brings the menu bar's Song entries up to date with the selected song.
pub fn relabel(bar: &MenuBar, song: Option<&SunoClip>, not_interested: bool) {
    for entry in COMMANDS.iter().filter(|e| e.group == G::Song) {
        // Submenu entries are found too: find_item looks inside submenus.
        if let Some(item) = bar.find_item(id(entry.action)) {
            item.set_label(&menu_text(&song_label(entry, song, not_interested), entry.keys, !entry.list_only));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_and_actions_match_and_access_keys_are_unique_per_menu() {
        for entry in COMMANDS {
            assert_eq!(action_for(id(entry.action)), Some(entry.action));
        }
        for group in [G::Library, G::Song, G::PlaylistSong, G::Discover, G::Playback, G::View, G::Account, G::Help] {
            let mut keys: Vec<char> = COMMANDS
                .iter()
                .filter(|e| e.group == group)
                .map(|e| e.submenu.unwrap_or(e.label))
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .filter_map(|label| label.split_once('_').and_then(|(_, rest)| rest.chars().next()))
                .map(|c| c.to_ascii_lowercase())
                .collect();
            let count = keys.len();
            keys.sort_unstable();
            keys.dedup();
            assert_eq!(keys.len(), count, "access keys clash in a menu");
        }
    }

    #[test]
    fn playback_keys_reach_their_commands_outside_the_main_window() {
        assert_eq!(playback_key(WXK_F8, false, false, false), Some(A::PlayPause));
        assert_eq!(playback_key(WXK_F8, true, true, false), Some(A::Resume));
        assert_eq!(playback_key(WXK_F12, true, true, false), Some(A::ToggleOutput));
        assert_eq!(playback_key(WXK_MEDIA_NEXT_TRACK, false, false, false), Some(A::Next));
        assert_eq!(playback_key(WXK_F5, false, false, false), None);
        assert_eq!(command_lines().len(), COMMANDS.iter().filter(|e| !matches!(e.group, G::PlaylistSong | G::Lists)).count());
    }

    #[test]
    fn labels_become_wx_text() {
        assert_eq!(menu_text("Li_ke", "Ctrl+Shift+U", true), "Li&ke\tCtrl+Shift+U");
        assert_eq!(menu_text("_Play", "Enter", false), "&Play (Enter)");
        assert_eq!(menu_text("Rock & roll", "", true), "Rock && roll");
    }
}
