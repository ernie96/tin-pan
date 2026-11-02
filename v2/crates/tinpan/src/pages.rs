//! The main window's layout: tabs for Suno's sections, each with its views list (as in fedra) and the
//! chosen view's songs, and below them the playback buttons and the volume, as on Tin Pan 1's pages.
//!
//! The page buttons run the same commands as the menus and, as in Tin Pan 1, are reached by their Alt
//! letters or the mouse, not by Tab. Controls are made in Tab order (search box, views, songs, volume)
//! and placed by the sizers, so each label's Alt letter moves to its own control and NVDA names each
//! control after its label.

use quiet_list::QuietList;
use tinpan_core::i18n::t;
use wxdragon::prelude::*;

use crate::commands::Action;
use crate::form::wx_label;
use crate::account_page::{self, AccountPage};
use crate::settings_page::{self, SettingsPage};
use crate::views::{DISCOVER, LIBRARY, Section, View, ViewKey};
use crate::{App, LIST_SEEK_SECONDS, with_app};

pub struct Layout {
    pub notebook: Notebook,
    pub sections: Vec<Section>,
    pub search_box: TextCtrl,
    pub volume: Slider,
    pub account: AccountPage,
    pub settings: SettingsPage,
}

/// A button that runs a command, reached by its Alt letter or the mouse, not by Tab.
pub(crate) fn command_button(parent: &impl WxWidget, label: &str, action: Action) -> Button {
    let button = Button::builder(parent).with_label(label).build();
    button.disable_focus_from_keyboard();
    button.on_click(move |_| with_app(|app| app.run_from_button(action)));
    button
}

pub(crate) fn heading(parent: &impl WxWidget, label: &str) -> StaticText {
    let text = StaticText::builder(parent).with_label(label).build();
    if let Some(mut font) = text.get_font() {
        font.make_bold();
        font.set_point_size(font.get_point_size() + 4);
        text.set_font(&font);
    }
    text
}

/// The views list and the song list, side by side, made in that Tab order.
fn views_and_songs(page: &Panel, tab: usize, views: Vec<View>) -> (Section, BoxSizer) {
    let views_label = StaticText::builder(page).with_label(&wx_label("L_ists")).build();
    let selector = ListBox::builder(page).build();
    let songs_label = heading(page, &wx_label("_Songs"));
    let list = QuietList::new(page, &t("Songs"));

    let left = BoxSizer::builder(Orientation::Vertical).build();
    left.add(&views_label, 0, SizerFlag::Bottom, 4);
    left.add(&selector, 1, SizerFlag::Expand, 0);
    let right = BoxSizer::builder(Orientation::Vertical).build();
    right.add(&songs_label, 0, SizerFlag::Bottom, 4);
    right.add(&list, 1, SizerFlag::Expand, 0);
    let body = BoxSizer::builder(Orientation::Horizontal).build();
    body.add_sizer(&left, 1, SizerFlag::Expand | SizerFlag::Right, 10);
    body.add_sizer(&right, 3, SizerFlag::Expand, 0);

    selector.on_selection_changed(move |event| {
        if let Some(index) = event.get_selection().and_then(|i| usize::try_from(i).ok()) {
            with_app(|app| app.view_chosen(tab, index, false));
        }
    });
    selector.on_key_down(move |event| {
        let WindowEventData::Keyboard(ref key) = event else { return };
        let handled = match (key.control_down(), key.get_key_code()) {
            (false, Some(WXK_RETURN | WXK_NUMPAD_ENTER)) => {
                with_app(App::open_chosen_view);
                true
            }
            (false, Some(WXK_DELETE)) => {
                with_app(App::delete_view);
                true
            }
            // The Applications key or Shift+F10: what can be done with the chosen list.
            (false, Some(WXK_WINDOWS_MENU)) => {
                with_app(App::lists_context_menu);
                true
            }
            (false, Some(WXK_F10)) if key.shift_down() => {
                with_app(App::lists_context_menu);
                true
            }
            (false, Some(WXK_ESCAPE)) => {
                with_app(|app| app.notebook.set_focus());
                true
            }
            (true, Some(code)) => tab_key(code),
            _ => false,
        };
        event.skip(!handled);
    });
    attach_song_keys(&list);

    let section = Section { views, shown: 0, selector, list };
    section.show_titles();
    let _ = tab;
    (section, body)
}

/// Ctrl+1 and Ctrl+2: the tabs, from anywhere in a page.
fn tab_key(code: i32) -> bool {
    let tab = match code {
        c if c == i32::from(b'1') => LIBRARY,
        c if c == i32::from(b'2') => DISCOVER,
        _ => return false,
    };
    with_app(|app| app.show_tab(tab));
    true
}

/// The song list's own keys, as Tin Pan 1's lists have them. The list leaves Enter to the app.
fn attach_song_keys(list: &QuietList<String>) {
    list.on_activate(|| with_app(App::play_selected));
    list.on_space(|| with_app(App::play_pause));
    list.on_selection_changed(|| {
        with_app(|app| {
            app.selection_changed();
            app.cue_selection();
        });
    });
    list.on_context_menu(|| with_app(App::song_context_menu));
    list.on_key_down(|event| {
        let WindowEventData::Keyboard(key) = event else { return false };
        if key.alt_down() || key.shift_down() {
            return false;
        }
        let code = key.get_key_code();
        if key.control_down() {
            return match code {
                Some(WXK_UP) => {
                    with_app(|app| app.playlist_key(-1));
                    true
                }
                Some(WXK_DOWN) => {
                    with_app(|app| app.playlist_key(1));
                    true
                }
                Some(c) => tab_key(c),
                None => false,
            };
        }
        let seconds = match code {
            Some(WXK_RETURN | WXK_NUMPAD_ENTER) => {
                with_app(App::play_selected);
                return true;
            }
            Some(WXK_DELETE) => {
                with_app(App::delete_key);
                return true;
            }
            Some(WXK_ESCAPE) => {
                with_app(|app| app.section().selector.set_focus());
                return true;
            }
            Some(WXK_LEFT) => -LIST_SEEK_SECONDS,
            Some(WXK_RIGHT) => LIST_SEEK_SECONDS,
            _ => return false,
        };
        let mut handled = false;
        with_app(|app| handled = app.seek(seconds));
        handled
    });
}

pub fn build(frame: &Frame, volume_now: f32, config: &tinpan_core::config::AppConfig) -> Layout {
    let window = Panel::builder(frame).build();
    let notebook = Notebook::builder(&window).build();

    // ── Library: your songs and your playlists ──
    let library = Panel::builder(&notebook).build();
    let search_label = StaticText::builder(&library).with_label(&wx_label("Searc_h")).build();
    let search_box = TextCtrl::builder(&library).with_style(TextCtrlStyle::ProcessEnter).build();
    let search_button = command_button(&library, &t("Search"), Action::SearchNow);
    let clear_button = command_button(&library, &t("Clear search"), Action::ClearSearch);
    let new_song = command_button(&library, &wx_label("_New song..."), Action::NewSong);
    let refresh = command_button(&library, &wx_label("_Refresh"), Action::Refresh);
    let filters = command_button(&library, &wx_label("_Filters..."), Action::Filters);
    let upload = command_button(&library, &wx_label("_Upload..."), Action::UploadSong);
    let download_all = command_button(&library, &wx_label("Download a_ll..."), Action::DownloadAll);
    let (library_section, library_body) = views_and_songs(&library, LIBRARY, vec![View::new(ViewKey::Library, t("Songs"))]);

    let buttons = BoxSizer::builder(Orientation::Horizontal).build();
    for button in [&new_song, &upload, &refresh, &filters, &download_all] {
        buttons.add(button, 0, SizerFlag::Right, 8);
    }
    let search = BoxSizer::builder(Orientation::Horizontal).build();
    search.add(&search_label, 0, SizerFlag::AlignCenterVertical | SizerFlag::Right, 8);
    search.add(&search_box, 1, SizerFlag::AlignCenterVertical, 0);
    search.add(&search_button, 0, SizerFlag::AlignCenterVertical | SizerFlag::Left, 8);
    search.add(&clear_button, 0, SizerFlag::AlignCenterVertical | SizerFlag::Left, 8);
    let library_page = BoxSizer::builder(Orientation::Vertical).build();
    library_page.add_sizer(&buttons, 0, SizerFlag::All, 10);
    library_page.add_sizer(&search, 0, SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right | SizerFlag::Bottom, 10);
    library_page.add_sizer(&library_body, 1, SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right | SizerFlag::Bottom, 10);
    library.set_sizer(library_page, true);

    // ── Discover: Suno's own lists ──
    let discover = Panel::builder(&notebook).build();
    let (discover_section, discover_body) = views_and_songs(&discover, DISCOVER, crate::discover::first_views());
    let discover_page = BoxSizer::builder(Orientation::Vertical).build();
    discover_page.add_sizer(&discover_body, 1, SizerFlag::Expand | SizerFlag::All, 10);
    discover.set_sizer(discover_page, true);

    notebook.add_page(&library, &t("Library"), true, None);
    notebook.add_page(&discover, &t("Discover"), false, None);
    let account = account_page::build(&notebook);
    notebook.add_page(&account.panel, &t("Account"), false, None);
    let settings = settings_page::build(&notebook, config);
    notebook.add_page(&settings.panel, &t("Settings"), false, None);
    notebook.on_page_changed(|event| {
        if let Some(tab) = event.get_selection().and_then(|i| usize::try_from(i).ok()) {
            with_app(|app| app.tab_shown(tab));
        }
    });

    // ── Playback, under every tab ──
    let previous = command_button(&window, &t("Previous"), Action::Previous);
    let play_pause = command_button(&window, &t("Play or pause"), Action::PlayPause);
    let stop = command_button(&window, &t("Stop"), Action::Stop);
    let next = command_button(&window, &t("Next"), Action::Next);
    let volume_label = StaticText::builder(&window).with_label(&wx_label("_Volume")).build();
    let volume = Slider::builder(&window).with_value((volume_now * 100.0).round() as i32).with_min_value(0).with_max_value(100).with_size(Size::new(160, -1)).build();
    volume.on_slider(move |event| {
        let value = event.get_value();
        with_app(|app| app.set_volume_from_slider(value));
    });
    let playback = BoxSizer::builder(Orientation::Horizontal).build();
    for button in [&previous, &play_pause, &stop, &next] {
        playback.add(button, 0, SizerFlag::All, 5);
    }
    playback.add(&volume_label, 0, SizerFlag::AlignCenterVertical | SizerFlag::Left, 16);
    playback.add(&volume, 0, SizerFlag::AlignCenterVertical | SizerFlag::Left, 8);

    let outer = BoxSizer::builder(Orientation::Vertical).build();
    outer.add(&notebook, 1, SizerFlag::Expand | SizerFlag::All, 6);
    outer.add_sizer(&playback, 0, SizerFlag::AlignCenterHorizontal | SizerFlag::Bottom, 6);
    window.set_sizer(outer, true);

    Layout { notebook, sections: vec![library_section, discover_section], search_box, volume, account, settings }
}
