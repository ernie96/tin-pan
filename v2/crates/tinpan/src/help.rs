//! The Help menu, as Tin Pan 1's: F1 says a short line for where you are, Ctrl+F1 lists every command (Enter
//! runs one; the first item is the full key guide), the Speech submenu, Report a problem and Suggest a feature,
//! Licenses and notices, and About. Resume where I left off (Ctrl+Shift+F8) is here too.

use tinpan_core::i18n::{t, tf};
use wxdragon::prelude::*;
use tinpan_core::report::{self, Report};

use crate::commands::{self, Action};
use crate::form::{self, Field, Value};
use crate::net::Command;
use crate::settings_page::SPEECH_LEVELS;
use crate::views::{ACCOUNT, DISCOVER, SETTINGS};
use crate::{App, dialogs, logging, speech};

/// Tin Pan's notices and licenses, built into the exe as Tin Pan 1's are; legal texts stay in English.
const NOTICE: &str = include_str!("../../../../NOTICE.md");
const LICENSE: &str = include_str!("../../../../LICENSE");
const THIRD_PARTY: &str = include_str!("../../../THIRD_PARTY_NOTICES.md");
const SUNSCREEN_LICENSE: &str = include_str!("../../../../deps/sunscreen/LICENSE");

/// The Attribution Notice from NOTICE.md; the license's terms require it in About.
const ATTRIBUTION: &str = "Tin Pan was created by Ernesto Melendez (github.com/ernie96/tin-pan).";
const DEDICATION: &str = "Tin Pan, like all my apps, is dedicated to Jenni and Diana.";

/// Ctrl+F1's first item: the whole guide, each section translated on its own. Sections Tin Pan 2 does as Tin
/// Pan 1 did use Tin Pan 1's texts.
pub fn full_guide() -> String {
    let section = |title: &str, parts: &[&str]| format!("{}\n{}", t(title), parts.iter().map(|p| t(p)).collect::<Vec<_>>().join("\n"));
    [
        section("Anywhere", &["Alt or F10: the menu bar. Every command is in it, with its keys. The Song menu acts on the song selected in the tab you're on.\nTab stops on the search box, the lists, the songs and the volume; the buttons on each page work with their Alt letters.\nF1: short help for where you are. Ctrl+F1: all commands in a list; Enter runs one. Its first item is this guide.\nCtrl+R: repeat the last message. Ctrl+Shift+R: message history, newest first.\nEscape: step back one level: from songs to the lists, from the lists to the tab headers.\nF2: credits left. Shift+F2: your plan, when it renews, credits and downloads left.\nF3: what's playing and how far in.\nF5: refresh what's shown.\nCtrl+1 Library, Ctrl+2 Discover, Ctrl+4 Account, Ctrl+5 Settings.\nCtrl+N: create a song, in a window that stays open. F6: to that window and back.\nCtrl+F: search your songs."]),
        section("Tabs and lists", &["The Library and Discover tabs each have their lists on the left and the chosen list's songs on the right. The Library's lists are your songs and each of your playlists; Discover's are Trending, From people you follow, Explore and each of its shelves, and Contests. A list loads a moment after you arrow onto it; Enter loads it now and says how many songs it has. A song carries on through the list it started from, whichever tab is shown.\nSimilar songs, a creator's songs, covers and remixes and the original song open as lists in the Discover tab; Delete in the lists closes one. Ctrl+G opens a song from a Suno link someone sent you, the same way. In a playlist's songs, Delete removes a song and Ctrl+Up and Ctrl+Down move it; Delete on a playlist in the lists moves it to the trash (asks first). Ctrl+P adds the selected song to a playlist."]),
        section("Playing", &["F8 or the Play/Pause media key: play or pause; with nothing playing it starts the selected song.\nShift+F8 or the Stop media key: stop.\nF7 and F9, or the Previous and Next media keys: previous and next song.\nShift+F7 and Shift+F9: back and forward 10 seconds. Left and Right in a song list: 5 seconds.\nCtrl+F7 and Ctrl+F9: back and forward 10 percent of the song.\nCtrl+F8: back to the start of the song.\nCtrl+Shift+F8: resume the last song where you left off, even after closing Tin Pan.\nF11 and F12: volume down and up.\nSongs play in Tin Pan itself, and the next song in the list is fetched ahead, so it starts at once."]),
        section("Audio output", &["Playback menu, Audio output: choose the output songs play on (headphones, speakers, a USB box), or the Windows default. Second audio output: one more, say external speakers to show someone a song; Ctrl+Shift+F12 switches to it and back, even mid-song. Both are also in Settings. Tin Pan always starts on the main output, and if an output is unplugged it says so and plays on the Windows default."]),
        section("Library list", &[
            "Enter: play.\nSpace: pause or resume.\nLeft and Right arrows: back and forward 5 seconds.\nCtrl+I: details (style, model, plays, link and lyrics) as one text.\nAlt+Enter: song information, one fact per line.\nCtrl+L: lyrics.\nCtrl+C: copy the song's link.\nCtrl+D: download in the format chosen in Settings. It uses the song's free unencrypted file when Suno has one; if the direct download fails, the app offers Suno's own download instead.\nCtrl+E: extend.\nDelete: move to Suno trash (asks first).\nApplications key or Shift+F10 for everything: Download WAV, Download video, Extend, Cover, Remaster, Get whole song, Stems, Edit title and lyrics, Make public or private, Add to playlist, Copy Suno share link, Allow or stop remixes, Similar songs, Show comments, Write a comment.\nWAV needs a Pro or Premier plan. Free plans get a few covers and remasters; the Song menu says how many are left, and Shift+F2 reads them out.\nReaching the end of the list loads more songs.\nAlt+F, Filters: workspace, disliked songs, trash, stems, Suno Studio songs, liked, public or private, uploaded, covers, extended, remasters, whole songs, personas, songs in a playlist and songs downloaded before. Each can show those songs, show only them, or hide them. Sort newest, oldest, most liked or least liked first. Kept between runs.\nAlt+L, Download all: every finished song in the list, using the free direct download only; songs already in the folder are skipped.\nSongs you liked say \"liked\".",
            "In every song list (the Library, a playlist's songs, Trending and song windows): Ctrl+Shift+U like, Ctrl+Shift+N dislike, Ctrl+P add to a playlist, Ctrl+T edit the title and lyrics, Ctrl+Shift+C cover, Ctrl+Shift+M remaster, Ctrl+J get the whole song, Ctrl+Shift+S stems, Ctrl+Shift+D download MP3, Ctrl+Shift+W download WAV, Ctrl+Shift+V download the video, Ctrl+Shift+H clean up the download, Ctrl+M similar songs, Ctrl+U the creator's songs, Ctrl+O the original song, Ctrl+Shift+O covers and remixes of it, Ctrl+K comments, Ctrl+Shift+K write a comment, Ctrl+Shift+L copy the Suno share link, Ctrl+Shift+P public or private, Ctrl+Shift+X allow or stop remixes. The Song menu shows each key.",
        ]),
        section("Search box", &["Enter searches, Escape clears the search."]),
        section("Create window", &[
            "Ctrl+N or New song on the Library page opens this window, which stays open beside the Library until you close it (Escape or Close). F6 goes to the main window and back, and the playback keys work here, so you can listen to what you made and make another.\nModel, style, styles to exclude, voice, instrumental, max mode, weirdness, style influence and persona (pick one of yours, or type an ID) apply to both ways of making a song.\nDescribe a song lets Suno write the words. Your own lyrics uses your title and lyrics. Write lyrics makes lyrics from a topic, and Use these lyrics copies them in.\nStyle boost rewrites the Style box into a fuller description.\nChange my lyrics: say what to change, then Rewrite my lyrics now. The result goes into Generated lyrics; Use these lyrics copies it in.\nNew songs appear at the top of your library. You'll hear when they're ready.\nIf Suno asks for a captcha, the app passes it itself. If Suno wants a click, a Suno check window comes forward with the check box.",
            "Style ideas lists styles Suno suggests; Enter puts one in the Style box. Lyrics drafts lists your drafts from Suno's lyrics editor. Its first line saves the Lyrics box as a new draft; Enter on a draft puts it in the Lyrics box (asking first if it has words in it), saves the Lyrics box into it, renames it or deletes it.",
        ]),
        section("Extend, Cover and Remaster", &["Song menu, Make from it. Each opens a form that stays open until you close it (Escape or Close); its button starts it, and you can change something and start another. The playback keys work in the form, so you can listen while you fine-tune.\nExtend continues a song from a time you choose; Get whole song joins the parts afterwards. Cover keeps the melody in a new style; its lyrics start as the original words, Rewrite the lyrics now changes them the way the How to change box says, and Back to the original words undoes it. Remaster makes a cleaner version with a newer model. Use as inspiration makes a new song that takes after the selected one. Each form only offers the models your plan can use for it.\nWeirdness, style influence and audio influence are sliders, each followed by Let Suno choose, ticked until you move the slider."]),
        section("Changing a song", &["Song menu, Make from it: Change speed (keeping the pitch or not, like a tape), Reverse, Crop (keep only a part, or take a part out) and Fade in or out. Each opens a small form like Remaster and makes a new song; the original stays as it is. You hear when the new one is ready."]),
        section("Account tab", &["Your plan, credits and what the plan includes, then Suno's notifications.", "Notifications you haven't seen say New. Mark all as read (also in the Account menu) marks them read on Suno too."]),
        section("Speech", &[
            "Help menu or Settings, Tin Pan speaks: everything; what you ask for and errors (no background news); or only errors and what you ask to hear. F1, F2, F3 and Ctrl+R always speak, and every message is kept in the history.\nSettings, When a list reaches the playing song: a short click (the default), say playing (also in braille), both, or nothing.",
            "Shift+F3, or Settings, Live lyrics: when a song has timed lyrics, Tin Pan says each line as it starts, also in braille. Off to start with. Section tags such as [Chorus] aren't read.",
        ]),
        section("Settings", &["Sign in opens Suno's own sign-in page; it closes by itself once you're in. It remembers your Google account and can save your password. Sign out only signs out of Suno. The first time, or when Suno ends your sign-in, Tin Pan asks you to sign in.\nLanguage: English, Romanian, Ukrainian or Spanish, or the same as Windows. It changes the next time Tin Pan starts.\nDownload folder, download format, and extra files saved next to each download: lyrics, timed lyrics (LRC), song details and cover art.\nCheck for new versions when Tin Pan starts: on by default, and quiet unless there is one."]),
        section("Clean-up", &["Download (Ctrl+D) asks every time whether to clean the song up with sunscreen, which removes the haze and buzz in Suno songs. Download MP3, Download WAV and Download all don't ask; they clean up when Settings says to. Clean up download, in the Song menu, cleans up a song you already have.\nThe result is a 24-bit WAV next to the download, with (clean) added to its name, often 10 to 20 times bigger. sunscreen reads MP3 and WAV, so Suno's M4A files are converted first. It runs on this computer and takes about 10 seconds for a 5-minute song.\nSettings has each step: decoder buzz, hearing model, flutter, widening, top octave, tone matching and loudness, with their values, and whether to keep the original download."]),
        section("Uploading", &["Library menu, Upload a song (Alt+U on the Library page), sends an audio file from this computer to your Suno account, where it becomes a song you can play, extend, cover and remaster. MP3, M4A, WAV, FLAC, OGG and AAC files can be uploaded, and you can add its lyrics. You confirm you have the rights to it and accept Suno's upload terms. Suno checks each upload, which can take a minute or two, and may refuse copyrighted music. The length limits are your plan's."]),
        section("Problems and ideas", &["Help menu, Report a problem (Ctrl+Shift+B) and Suggest a feature (Ctrl+Shift+F) send a message privately to Tin Pan's author; no account is needed. A problem report can include Tin Pan's log, which has no passwords; your Windows user name is taken out first."]),
        section("Updates", &["Help, Check for updates looks for a new version now. When one is out, Tin Pan says what's new and asks; Yes downloads it, checks its signature, closes Tin Pan, swaps in the new TinPan.exe and opens it again. Saying No at startup means that version isn't offered again by itself."]),
    ]
    .join("\n\n")
}

impl App {
    /// F1: one or two sentences for where you are.
    pub(crate) fn context_help(&self) {
        let line = match usize::try_from(self.notebook.selection()).unwrap_or(0) {
            DISCOVER => t("Discover. Lists on the left: Trending, From people you follow, Explore and its shelves, Contests, and the lists you open from a song. Enter opens one; in its songs Enter plays, Space pauses, Left and Right seek. Delete in the lists closes a list you opened. Ctrl+G opens a Suno link. F5 refreshes."),
            ACCOUNT => t("Account. Your plan and credits, then Suno's notifications. F2 and Shift+F2 read them from anywhere. F5 refreshes."),
            SETTINGS => t("Settings. Sign in or out, choose the language, and choose where songs are saved."),
            _ => t("Library. Arrow through your songs. Enter plays, Space pauses, Left and Right seek. Applications key for extend, cover, downloads and more. Ctrl+F searches. Ctrl+Shift+F8 resumes where you left off. Alt+F for filters."),
        };
        let not_signed_in = if self.signed_in || self.notebook.selection() == SETTINGS as i32 { String::new() } else { format!(" {}", t("You're not signed in. Ctrl+5 opens Settings.")) };
        // The other windows open, as Tin Pan 1 says them first.
        let mut open: Vec<String> = self.shown_create().map(|_| t("Create a song")).into_iter().collect();
        open.extend(crate::tool_form::open_forms().into_iter().filter(|d| d.is_shown()).filter_map(|d| d.get_label()));
        let windows = if open.is_empty() { t("Ctrl+N opens Create; F6 then moves between it and the main window.") } else { tf("F6 moves between the main window and {0}.", &[&open.join(", ")]) };
        speech::say_error(&format!("{windows} {line}{not_signed_in} {}", t("Ctrl+F1 for all commands.")));
    }

    /// Ctrl+F1: every command; Enter runs one, and the first item is the full key guide.
    pub(crate) fn all_commands(&mut self) {
        let commands = commands::command_lines();
        let mut lines = vec![t("Read the full key guide")];
        lines.extend(commands.iter().map(|(_, line)| line.clone()));
        match dialogs::pick(&self.frame, &t("All commands. Enter runs one."), &lines) {
            Some(0) => dialogs::show_document(&self.frame, &t("Tin Pan keys"), &full_guide()),
            Some(i) => {
                let action = commands[i - 1].0;
                if action != Action::AllCommands {
                    self.run_from_button(action);
                }
            }
            None => {}
        }
    }

    /// Help menu, Speech: as Settings' Tin Pan speaks, which shows it too.
    pub(crate) fn set_speech_level(&mut self, level: &str) {
        let index = SPEECH_LEVELS.iter().position(|(k, _)| *k == level).unwrap_or(0);
        self.settings_speech_level(index);
        speech::say_error(&match level {
            "Everything" => t("Speaking everything."),
            "AskedFor" => t("Speaking what you ask for and errors, not background news."),
            _ => t("Speaking only errors and what you ask to hear."),
        });
    }

    pub(crate) fn licenses(&self) {
        let notices = THIRD_PARTY.replace("Its license text follows these notices", "Its license text follows");
        let text = format!("{NOTICE}\n\n{notices}\n\nsunscreen license\n\n{SUNSCREEN_LICENSE}\n\nTin Pan's license\n\n{LICENSE}");
        dialogs::show_document(&self.frame, &t("Licenses and notices"), text.trim());
    }

    pub(crate) fn about(&self) {
        let title = self.frame.get_title();
        let account = match (self.signed_in, title.strip_prefix("Tin Pan - ")) {
            (false, _) => t("Not signed in."),
            (true, Some(name)) => tf("Signed in as {0}.", &[&name]),
            (true, None) => t("Signed in."),
        };
        let lines = vec![
            tf("Tin Pan {0}.", &[&tinpan_core::version()]),
            t(ATTRIBUTION),
            t(DEDICATION),
            t("Copyright (C) 2026 Ernesto Melendez. Tin Pan is free software under the GNU General Public License, version 3 or later, with terms that keep this credit and dedication visible. It comes with no warranty. Help, Licenses and notices has the license and its terms."),
            t("An accessible, unofficial client for Suno. Not affiliated with Suno."),
            tf("Speech goes through {0}.", &[&speech::output_name()]),
            account,
            self.billing.as_ref().map_or_else(|| t("Plan not loaded yet."), |b| format!("{}.", b.plan_name())),
            tf("Settings: {0}", &[&self.config.path().display()]),
            tf("Log: {0}", &[&logging::path().display()]),
            t("Built with ideas and Suno API notes from Nsomnia's chadvis-projectm-qt, suno-power-exporter and suno-api-local-manager-frontend, paperfoot/suno-cli and gcui-art/suno-api, and the BrailleNote QT's KeySoft."),
        ];
        dialogs::show_lines(&self.frame, &t("About Tin Pan"), &lines);
    }

    /// Report a problem (Ctrl+Shift+B) or Suggest a feature (Ctrl+Shift+F).
    pub(crate) fn send_report(&mut self, problem: bool) {
        let describe = if problem { "_What happened, and what did you expect?" } else { "_What would you like Tin Pan to do?" };
        let mut fields = vec![
            Field { label: describe, value: Value::Text { value: String::new(), multiline: true, max_len: report::MAX_DESCRIPTION } },
            Field { label: "_Email or other way to reach you (optional)", value: Value::Text { value: String::new(), multiline: false, max_len: report::MAX_CONTACT } },
        ];
        if problem {
            fields.push(if cfg!(windows) {
                Field::check("Include Tin Pan's _log (it has no passwords, and your Windows user name is taken out)", true)
            } else {
                Field::check("Include Tin Pan's _log (it has no passwords, and your user name is taken out)", true)
            });
        }
        let (title, intro) = if problem {
            (t("Report a problem"), t("Goes privately to Tin Pan's author. The log helps find the cause."))
        } else {
            (t("Suggest a feature"), t("Goes privately to Tin Pan's author."))
        };
        if !form::show(&self.frame, &title, &intro, &t("Send"), &mut fields) {
            return;
        }
        if fields[0].text().trim().is_empty() {
            speech::say_error(&t("Nothing sent: the description was empty."));
            return;
        }
        let log = (problem && fields[2].checked()).then(read_recent_log);
        let contact = Some(fields[1].text()).filter(|c| !c.is_empty());
        speech::say(&t("Sending."));
        let report = Report {
            kind: if problem { "bug" } else { "feature" },
            description: fields[0].text(),
            contact,
            // The system goes in the version too, so the report's title says which one it came from.
            version: format!("{} {}", tinpan_core::version(), system_name()),
            language: tinpan_core::i18n::current().to_string(),
            windows: system_description(),
            log,
        };
        self.net.send(Command::SendReport { report: Box::new(report) });
    }

    pub(crate) fn report_sent(&self, kind: &str, number: i64) {
        speech::say_error(&if kind == "bug" { tf("Sent as report {0}. Thank you.", &[&number]) } else { tf("Sent as suggestion {0}. Thank you.", &[&number]) });
    }

    // ── Resume where I left off ─────────────────────────────────────────

    /// Every few seconds: the playing song and where it is, so Resume works after closing Tin Pan.
    pub(crate) fn track_resume(&mut self) {
        let Some(clip) = &self.playing else { return };
        let seconds = self.player.position().as_secs_f64().floor();
        let total = self.player.total().map_or(0.0, |d| d.as_secs_f64());
        let r = &mut self.config.current.resume;
        if r.clip_id == clip.id && (r.seconds - seconds).abs() < 1.0 {
            return;
        }
        r.clip_id = clip.id.clone();
        r.title = clip.display_title();
        // Near the end, start over next time instead of resuming at the last second.
        r.seconds = if total > 0.0 && total - seconds < 15.0 { 0.0 } else { seconds };
        self.save_settings();
    }

    /// Ctrl+Shift+F8: the last song, where it was.
    pub(crate) fn resume(&mut self) {
        let saved = self.config.current.resume.clone();
        if saved.clip_id.is_empty() {
            speech::say_error(&t("Nothing to resume yet."));
            return;
        }
        if !self.signed_in {
            speech::say_error(&t("Sign in from Settings first."));
            return;
        }
        self.resume_at = Some((saved.clip_id.clone(), saved.seconds));
        match self.find_song(&saved.clip_id) {
            Some(song) => self.play(song),
            None => self.net.send(Command::ResumeSong { clip_id: saved.clip_id }),
        }
    }
}

/// The end of Tin Pan's log, with the user name taken out.
/// "Windows", "macOS" or "Linux".
fn system_name() -> &'static str {
    if cfg!(windows) {
        "Windows"
    } else if cfg!(target_os = "macos") {
        "macOS"
    } else if cfg!(target_os = "linux") {
        "Linux"
    } else {
        std::env::consts::OS
    }
}

/// What a report says about the system, so a problem on one Tin Pan's author has no device for can be found:
/// the system and its version, the processor, on Linux the desktop and display server, and the screen reader
/// speech goes through. At most 120 characters, as the report relay keeps.
fn system_description() -> String {
    let mut parts = vec![format!("{} ({})", system_version(), std::env::consts::ARCH)];
    if cfg!(target_os = "linux") {
        let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
        let session = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
        let shown = [desktop.as_str(), session.as_str()].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(" on ");
        if !shown.is_empty() {
            parts.push(shown);
        }
    }
    parts.push(format!("speech: {}", speech::output_name()));
    parts.join(", ").chars().take(120).collect()
}

/// The system's own name for its version: "macOS 15.1", "Ubuntu 24.04.1 LTS", or just the system.
fn system_version() -> String {
    #[cfg(target_os = "macos")]
    if let Ok(out) = std::process::Command::new("sw_vers").arg("-productVersion").output()
        && let Ok(version) = String::from_utf8(out.stdout)
        && !version.trim().is_empty()
    {
        return format!("macOS {}", version.trim());
    }
    #[cfg(target_os = "linux")]
    if let Ok(release) = std::fs::read_to_string("/etc/os-release")
        && let Some(name) = release.lines().find_map(|l| l.strip_prefix("PRETTY_NAME="))
    {
        return name.trim().trim_matches('"').to_string();
    }
    system_name().to_string()
}

fn read_recent_log() -> String {
    let path = logging::path();
    let old = path.with_extension("log.old");
    let mut text = String::new();
    for p in [old, path] {
        if let Ok(part) = std::fs::read_to_string(&p) {
            text.push_str(&part);
        }
    }
    if text.len() > report::MAX_LOG {
        let mut cut = text.len() - report::MAX_LOG;
        while !text.is_char_boundary(cut) {
            cut += 1;
        }
        text = text[cut..].to_string();
    }
    let user = std::env::var("USERNAME").or_else(|_| std::env::var("USER")).ok();
    report::anonymize(&text, user.as_deref())
}
