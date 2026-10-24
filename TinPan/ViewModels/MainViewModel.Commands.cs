using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using System.Reflection;
using System.Threading.Tasks;
using System.Windows.Input;
using System.Windows.Threading;
using CommunityToolkit.Mvvm.Input;
using TinPan.Models;
using TinPan.Services;
using static TinPan.Localization.Loc;

namespace TinPan.ViewModels
{
    /// <summary>
    /// One action, its group, its keys and the command that runs it. The menu bar and the Ctrl+F1
    /// command list are both built from these, so they can't drift apart.
    /// HeaderPath names a LibraryPanelViewModel property when the label changes with the plan.
    /// Submenu puts it in a submenu of its group (see MainViewModel.SubmenuLabel); SeparatorBefore draws a line above it.
    /// </summary>
    public sealed record CommandEntry(string Group, string Label, string Keys, ICommand Command, object? Parameter = null, string? HeaderPath = null,
        string? Submenu = null, bool SeparatorBefore = false)
    {
        public string SpokenLabel => Label.Replace("_", "").Replace("...", "");
    }

    // Menu bar, command list, repeat and history, resume, skips, speech level and About.
    public partial class MainViewModel
    {
        /// <summary>The Attribution Notice from NOTICE.md; the license's terms require it in About.</summary>
        public const string Attribution =
            "Tin Pan was created by Ernesto Melendez (github.com/ernie96/tin-pan).";

        public const string Dedication =
            "Tin Pan, like all my apps, is dedicated to Jenni and Diana.";

        private DispatcherTimer? _resumeTimer;

        /// <summary>Asks the window to put focus in the Library search box (Ctrl+F).</summary>
        public event EventHandler? SearchRequested;

        private List<CommandEntry>? _commands;

        /// <summary>A menu group's name as shown; the code itself matches on the English name.</summary>
        public static string GroupLabel(string group) => group switch
        {
            "Playback" => T("Playback"),
            "Song" => T("Song"),
            "Library" => T("Library"),
            "View" => T("View"),
            "Discover" => T("Discover"),
            "Account" => T("Account"),
            "Help" => T("Help"),
            _ => group
        };

        /// <summary>A submenu's name as shown, with its access key.</summary>
        public static string SubmenuLabel(string submenu) => submenu switch
        {
            "Download" => T("D_ownload"),
            "Make" => T("_Make from it"),
            "Related" => T("Related so_ngs"),
            "Comments" => T("_Comments"),
            "Share" => T("S_haring"),
            "Speech" => T("_Speech"),
            _ => submenu
        };

        /// <summary>
        /// A label that depends on the song the menu acts on, decided as the menu opens: public or private,
        /// remixes allowed or not. Null keeps the entry's own label.
        /// </summary>
        public string? SongLabel(CommandEntry entry, SunoClip? song) =>
            entry.Command == LibraryVM.ToggleVisibilityCommand ? LibraryPanelViewModel.VisibilityLabel(song)
            : entry.Command == LibraryVM.ToggleRemixCommand ? LibraryPanelViewModel.RemixLabel(song)
            : null;

        public IReadOnlyList<CommandEntry> Commands => _commands ??= BuildCommands();

        private List<CommandEntry> BuildCommands()
        {
            var lib = LibraryVM;
            return new List<CommandEntry>
            {
                new("Playback", T("_Play or pause"), "F8", PlayPauseCommand),
                new("Playback", T("_Stop"), "Shift+F8", lib.StopCommand),
                new("Playback", T("Pre_vious song"), "F7", PreviousCommand),
                new("Playback", T("_Next song"), "F9", NextCommand),
                new("Playback", T("_Back 10 seconds"), "Shift+F7", lib.SeekCommand, "-10", SeparatorBefore: true),
                new("Playback", T("_Forward 10 seconds"), "Shift+F9", lib.SeekCommand, "10"),
                new("Playback", T("Bac_k 10 percent"), "Ctrl+F7", SeekPercentCommand, "-10"),
                new("Playback", T("Forwar_d 10 percent"), "Ctrl+F9", SeekPercentCommand, "10"),
                new("Playback", T("Back to the s_tart"), "Ctrl+F8", RestartCommand),
                new("Playback", T("_Resume where I left off"), "Ctrl+Shift+F8", ResumeCommand),
                new("Playback", T("_What's playing"), "F3", NowPlayingCommand, SeparatorBefore: true),
                new("Playback", T("Read _lyrics as they're sung, on or off"), "Shift+F3", ToggleLiveLyricsCommand),
                new("Playback", T("Volume d_own"), "F11", lib.VolumeDownCommand, SeparatorBefore: true),
                new("Playback", T("Volume _up"), "F12", lib.VolumeUpCommand),

                new("Song", T("_Play"), "Enter", PlaySongCommand),
                new("Song", T("_Details"), "Ctrl+I", lib.DetailsCommand),
                new("Song", T("Song in_formation"), "Alt+Enter", lib.SongInfoCommand),
                new("Song", T("_Lyrics"), "Ctrl+L", lib.LyricsCommand),
                new("Song", T("_Download..."), "Ctrl+D", lib.DownloadCommand, Submenu: "Download", SeparatorBefore: true),
                new("Song", T("Download MP_3"), "", lib.DownloadMp3Command, Submenu: "Download"),
                new("Song", T("Download _WAV"), "", lib.DownloadWavCommand, HeaderPath: nameof(LibraryPanelViewModel.WavMenuText), Submenu: "Download"),
                new("Song", T("Download _video"), "", lib.DownloadVideoCommand, Submenu: "Download"),
                new("Song", T("Clean up download (remove ha_ze)"), "", lib.CleanUpDownloadCommand, Submenu: "Download"),
                new("Song", T("_Extend..."), "Ctrl+E", lib.ExtendCommand, Submenu: "Make"),
                new("Song", T("_Cover..."), "", lib.CoverCommand, HeaderPath: nameof(LibraryPanelViewModel.CoverMenuText), Submenu: "Make"),
                new("Song", T("Re_master..."), "", lib.RemasterCommand, HeaderPath: nameof(LibraryPanelViewModel.RemasterMenuText), Submenu: "Make"),
                new("Song", T("_Get whole song"), "", lib.WholeSongCommand, Submenu: "Make"),
                new("Song", T("S_tems"), "", lib.StemsCommand, Submenu: "Make"),
                new("Song", T("Similar so_ngs..."), "", lib.SimilarCommand, Submenu: "Related"),
                new("Song", T("_Creator's songs..."), "", lib.CreatorCommand, Submenu: "Related"),
                new("Song", T("Covers and _remixes of it..."), "", lib.RemixesCommand, Submenu: "Related"),
                new("Song", T("_Original song..."), "", lib.OriginalCommand, Submenu: "Related"),
                new("Song", T("S_how comments"), "", lib.CommentsCommand, Submenu: "Comments"),
                new("Song", T("_Write a comment..."), "", lib.WriteCommentCommand, Submenu: "Comments"),
                new("Song", T("Copy l_ink"), "Ctrl+C", lib.CopyLinkCommand, Submenu: "Share"),
                new("Song", T("Copy Suno share lin_k"), "", lib.ShareLinkCommand, Submenu: "Share"),
                new("Song", T("Make p_ublic"), "", lib.ToggleVisibilityCommand, Submenu: "Share"),
                new("Song", T("Stop remi_xes"), "", lib.ToggleRemixCommand, Submenu: "Share"),
                new("Song", T("Edit title and l_yrics..."), "", lib.EditCommand, SeparatorBefore: true),
                new("Song", T("Add to playli_st..."), "", lib.AddToPlaylistCommand),
                new("Song", T("Move to tr_ash"), "Delete", lib.TrashCommand),

                new("Library", T("_Refresh"), "F5", RefreshCommand),
                new("Library", T("_Search"), "Ctrl+F", SearchCommand),
                new("Library", T("_Filters..."), "", lib.FiltersCommand),
                new("Library", T("Download _all..."), "", lib.DownloadAllCommand, SeparatorBefore: true),
                new("Library", T("_Upload a song..."), "", lib.UploadCommand),

                new("View", T("_Library"), "Ctrl+1", ShowTabCommand, "0"),
                new("View", T("_Playlists"), "Ctrl+2", ShowTabCommand, "1"),
                new("View", T("_Create"), "Ctrl+3", ShowTabCommand, "2"),
                new("View", T("_Trending"), "Ctrl+4", ShowTabCommand, "3"),
                new("View", T("_Account"), "Ctrl+5", ShowTabCommand, "4"),
                new("View", T("_Settings"), "Ctrl+6", ShowTabCommand, "5"),

                new("Discover", T("_Explore..."), "", lib.ExploreCommand),
                new("Discover", T("From people I _follow..."), "", lib.FollowingCommand),
                new("Discover", T("Find a c_reator..."), "", lib.FindCreatorCommand),
                new("Discover", T("_Contests..."), "", lib.ContestsCommand),

                new("Account", T("_Credits left"), "F2", ReadCreditsCommand),
                new("Account", T("_Plan and downloads"), "Shift+F2", ReadSubscriptionCommand),
                new("Account", T("Mark _notifications as read"), "", SunoVM.MarkNotificationsReadCommand),
                new("Account", T("Sign _in"), "", SettingsVM.SignInCommand, SeparatorBefore: true),
                new("Account", T("Sign _out"), "", SettingsVM.SignOutCommand),

                new("Help", T("_Help for here"), "F1", ContextHelpCommand),
                new("Help", T("_All commands"), "Ctrl+F1", FullHelpCommand),
                new("Help", T("_Repeat last message"), "Ctrl+R", RepeatCommand),
                new("Help", T("_Message history"), "Ctrl+Shift+R", HistoryCommand),
                new("Help", T("Speak e_verything"), "", SetSpeechLevelCommand, nameof(SpeechLevel.Everything), Submenu: "Speech"),
                new("Help", T("Speak only what I as_k for"), "", SetSpeechLevelCommand, nameof(SpeechLevel.AskedFor), Submenu: "Speech"),
                new("Help", T("Speak only _errors"), "", SetSpeechLevelCommand, nameof(SpeechLevel.ErrorsOnly), Submenu: "Speech"),
                new("Help", T("Report a _problem..."), "Ctrl+Shift+B", ReportProblemCommand, SeparatorBefore: true),
                new("Help", T("Suggest a _feature..."), "Ctrl+Shift+F", SuggestFeatureCommand),
                new("Help", T("Check for _updates"), "", CheckForUpdatesCommand, SeparatorBefore: true),
                new("Help", T("_Licenses and notices"), "", LicensesCommand),
                new("Help", T("About Tin Pa_n"), "", AboutCommand)
            };
        }

        // ── Help: the runnable command list (Ctrl+F1) ───────────────────────

        [RelayCommand]
        private void FullHelp()
        {
            var entries = Commands;
            var lines = new List<string> { T("Read the full key guide") };
            lines.AddRange(entries.Select(c =>
                $"{GroupLabel(c.Group)}{(c.Submenu == null ? "" : ", " + SubmenuLabel(c.Submenu).Replace("_", ""))}: {c.SpokenLabel}{(c.Keys.Length > 0 ? ", " + c.Keys : "")}"));
            _dialogs.ShowLines(T("All commands. Enter runs one."), lines, index =>
            {
                if (index == 0)
                {
                    _dialogs.ShowDocument(T("Tin Pan keys"), HelpText.Full);
                    return;
                }
                var entry = entries[index - 1];
                var parameter = SongParameter(entry);
                if (entry.Command.CanExecute(parameter)) entry.Command.Execute(parameter);
                else Announcer.Announce(T("{0} isn't available right now.", entry.SpokenLabel), always: true);
            });
        }

        /// <summary>
        /// The song the Song menu acts on: the one selected in the tab you're on (a playlist's song,
        /// a trending song), or the Library's.
        /// </summary>
        public SunoClip? CurrentSong => SelectedTab switch
        {
            PlaylistsTab => PlaylistsVM.SelectedSong,
            TrendingTab => SunoVM.SelectedTrending ?? LibraryVM.SelectedClip,
            _ => LibraryVM.SelectedClip
        };

        /// <summary>What a Song menu entry runs with: the current song, unless the entry has its own parameter.</summary>
        public object? SongParameter(CommandEntry entry) =>
            entry.Group == "Song" && entry.Parameter == null && entry.Command != PlaySongCommand ? CurrentSong : entry.Parameter;

        /// <summary>Plays the selected song of the tab you're on, so the list it's in carries on afterwards.</summary>
        [RelayCommand(AllowConcurrentExecutions = true)]
        private Task PlaySongAsync() => SelectedTab switch
        {
            PlaylistsTab => PlaylistsVM.PlayCommand.ExecuteAsync(null),
            TrendingTab when SunoVM.SelectedTrending != null => SunoVM.PlayTrendingCommand.ExecuteAsync(null),
            _ => LibraryVM.PlayCommand.ExecuteAsync(null)
        };

        [RelayCommand]
        private void Search()
        {
            SelectedTab = LibraryTab;
            SearchRequested?.Invoke(this, EventArgs.Empty);
        }

        // ── Repeat and history ──────────────────────────────────────────────

        [RelayCommand]
        private void Repeat() => Announcer.RepeatLast();

        [RelayCommand]
        private void History()
        {
            var history = Announcer.History;
            if (history.Count == 0)
            {
                Announcer.Announce(T("No messages yet."), always: true);
                return;
            }
            _dialogs.ShowLines(T("Message history, newest first"), history);
        }

        // ── Speech level ────────────────────────────────────────────────────

        [RelayCommand]
        private void SetSpeechLevel(string level)
        {
            if (!Enum.TryParse<SpeechLevel>(level, out var parsed)) return;
            SettingsVM.SpeechLevel = SettingsPanelViewModel.SpeechLevelLabel(parsed);
            Announcer.Announce(parsed switch
            {
                SpeechLevel.Everything => T("Speaking everything."),
                SpeechLevel.AskedFor => T("Speaking what you ask for and errors, not background news."),
                _ => T("Speaking only errors and what you ask to hear.")
            }, always: true);
        }

        // ── Skips, restart, resume ──────────────────────────────────────────

        /// <summary>Percent of the song, as text from a key binding: "-10", "10".</summary>
        [RelayCommand]
        private void SeekPercent(string percent)
        {
            if (_audio.CurrentClipId == null || _audio.TotalDuration <= TimeSpan.Zero ||
                !double.TryParse(percent, NumberStyles.Float, CultureInfo.InvariantCulture, out var p)) return;
            _audio.CurrentPosition += TimeSpan.FromSeconds(_audio.TotalDuration.TotalSeconds * p / 100);
            Announcer.Announce(SunoClip.FormatTime(_audio.CurrentPosition));
        }

        [RelayCommand]
        private void Restart()
        {
            if (_audio.CurrentClipId == null) return;
            _audio.CurrentPosition = TimeSpan.Zero;
            Announcer.Announce(T("From the start."));
        }

        [RelayCommand]
        private async Task ResumeAsync()
        {
            var saved = _config.Current.Resume;
            if (string.IsNullOrEmpty(saved.ClipId))
            {
                Announcer.Announce(T("Nothing to resume yet."), always: true);
                return;
            }
            if (!_auth.IsAuthenticated)
            {
                Announcer.Announce(T("Sign in from Settings first."), always: true);
                return;
            }
            await LibraryVM.ResumeAsync(saved.ClipId, saved.Title, saved.Seconds);
        }

        /// <summary>Remembers the song and position every few seconds, so Resume works after closing the app.</summary>
        private void StartResumeTracking()
        {
            _resumeTimer = new DispatcherTimer { Interval = TimeSpan.FromSeconds(5) };
            _resumeTimer.Tick += (_, _) =>
            {
                if (_audio.CurrentClipId is not { } id) return;
                var r = _config.Current.Resume;
                var seconds = Math.Floor(_audio.CurrentPosition.TotalSeconds);
                if (r.ClipId == id && Math.Abs(r.Seconds - seconds) < 1) return;
                r.ClipId = id;
                r.Title = _audio.CurrentTitle ?? "";
                // Near the end, start over next time instead of resuming at the last second.
                r.Seconds = _audio.TotalDuration.TotalSeconds - seconds < 15 ? 0 : seconds;
                _config.SaveSoon();
            };
            _resumeTimer.Start();
        }

        // ── About ───────────────────────────────────────────────────────────

        /// <summary>Tin Pan's copyright, license terms and attribution, the components and their licenses, built into the exe.</summary>
        [RelayCommand]
        private void Licenses()
        {
            var own = EmbeddedFiles.ReadText("NOTICE.md") ?? "";
            var gpl = EmbeddedFiles.ReadText("LICENSE.txt") ?? "";
            var notices = EmbeddedFiles.ReadText("THIRD_PARTY_NOTICES.md") ?? "";
            var nvda = EmbeddedFiles.ReadText("nvdaControllerClient-license.txt") ?? "";
            var sunscreen = EmbeddedFiles.ReadText("sunscreen-license.txt") ?? "";
            // The notices and licenses are legal texts; they stay in English.
            var text = own + Environment.NewLine + Environment.NewLine + notices.Replace("Its license text is in `deps/nvda/license.txt`.", "Its license text follows.")
                       + Environment.NewLine + Environment.NewLine + "NVDA Controller Client license" + Environment.NewLine + Environment.NewLine + nvda
                       + Environment.NewLine + Environment.NewLine + "sunscreen license" + Environment.NewLine + Environment.NewLine + sunscreen
                       + Environment.NewLine + Environment.NewLine + "Tin Pan's license" + Environment.NewLine + Environment.NewLine + gpl;
            _dialogs.ShowDocument(T("Licenses and notices"), text.Trim());
        }

        [RelayCommand]
        private void About()
        {
            var version = Assembly.GetExecutingAssembly().GetName().Version?.ToString(3) ?? "?";
            var lines = new List<string>
            {
                T("Tin Pan {0}.", version),
                T(Attribution),
                T(Dedication),
                T("Copyright (C) 2026 Ernesto Melendez. Tin Pan is free software under the GNU General Public License, version 3 or later, with terms that keep this credit and dedication visible. It comes with no warranty. Help, Licenses and notices has the license and its terms."),
                T("An accessible, unofficial client for Suno. Not affiliated with Suno."),
                T("Speech goes through {0}.", Announcer.OutputName),
                !_auth.IsAuthenticated ? T("Not signed in.")
                    : string.IsNullOrEmpty(_auth.UserName) ? T("Signed in.") : T("Signed in as {0}.", _auth.UserName),
                SunoVM.Account.Billing == null ? T("Plan not loaded yet.") : BillingText.PlanName(SunoVM.Account.Billing) + ".",
                T("Settings: {0}", AppFolders.Settings),
                T("Log: {0}", Log.FilePath),
                T("Built with ideas and Suno API notes from Nsomnia's chadvis-projectm-qt, suno-power-exporter and suno-api-local-manager-frontend, paperfoot/suno-cli and gcui-art/suno-api, and the BrailleNote QT's KeySoft.")
            };
            _dialogs.ShowLines(T("About Tin Pan"), lines);
        }
    }
}
