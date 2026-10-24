using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading.Tasks;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using TinPan.Models;
using TinPan.Services;
using static TinPan.Localization.Loc;

namespace TinPan.ViewModels
{
    public partial class MainViewModel : ViewModelBase
    {
        private static string AppTitle => T("Tin Pan, by Ernesto Melendez");
        // Tab order, which is also Ctrl+1 to Ctrl+5. Create is a window of its own (Ctrl+N).
        public const int LibraryTab = 0, PlaylistsTab = 1, TrendingTab = 2, AccountTab = 3, SettingsTab = 4;

        /// <summary>Asks the window to open the Create window, or bring it forward (Ctrl+N).</summary>
        public event EventHandler? CreateRequested;

        // ── Audio output ────────────────────────────────────────────────────

        /// <summary>Choose the main output from the ones Windows has; Enter picks one.</summary>
        [RelayCommand]
        private void ChooseOutput() => PickOutput(T("Audio output. Enter plays songs there."), includeNone: false, _outputs.Main, name =>
        {
            _outputs.SetMain(name ?? "");
            Announcer.Announce(_outputs.OnSecond
                ? T("Main output: {0}. Ctrl+Shift+F12 goes back to it.", AudioOutputs.Spoken(name))
                : T("Playing on {0}.", AudioOutputs.Spoken(name)), always: true);
        });

        /// <summary>Choose the output Ctrl+Shift+F12 flips to and back; or none.</summary>
        [RelayCommand]
        private void ChooseSecondOutput() => PickOutput(T("Second audio output, for Ctrl+Shift+F12. Enter chooses it."), includeNone: true, _outputs.Second, name =>
        {
            _outputs.SetSecond(name);
            Announcer.Announce(name == null
                ? T("No second output. Ctrl+Shift+F12 does nothing.")
                : T("Second output: {0}. Ctrl+Shift+F12 switches to it and back.", AudioOutputs.Spoken(name)), always: true);
        });

        /// <summary>Ctrl+Shift+F12: flip between the main and second output, even while a song plays.</summary>
        [RelayCommand]
        private void ToggleOutput()
        {
            if (!_outputs.Toggle())
            {
                Announcer.Announce(T("Choose a second output first: Playback menu, Second audio output."), always: true);
                return;
            }
            SettingsVM.RefreshOutputs();
            Announcer.Announce(T("Playing on {0}.", AudioOutputs.Spoken(_outputs.Current)), always: true);
        }

        /// <param name="current">The output to start on: a Windows name, "" for the Windows default, null for none.</param>
        private void PickOutput(string title, bool includeNone, string? current, Action<string?> chosen)
        {
            var names = new List<string?>();
            if (includeNone) names.Add(null);
            names.Add("");
            names.AddRange(AudioOutputs.List());
            if (current is { Length: > 0 } && !names.Contains(current)) names.Add(current);
            var lines = names.Select(n => (n == null ? T("None") : AudioOutputs.Spoken(n))
                                          + (n == current ? ", " + T("chosen") : "")
                                          + (n == current && n != null && !AudioOutputs.List().Contains(n) && n.Length > 0 ? ", " + T("not connected") : ""))
                .ToList();
            _dialogs.ShowLines(title, lines, index =>
            {
                chosen(names[index]);
                SettingsVM.RefreshOutputs();
            });
        }

        /// <summary>Speaks a line the user asked for (F1 in a tool window).</summary>
        public void Say(string text) => Announcer.Announce(text, always: true);

        [RelayCommand]
        private void OpenCreate() => CreateRequested?.Invoke(this, EventArgs.Empty);

        /// <summary>F6 and Shift+F6: on to the next open window (Create, a Cover form), or back.</summary>
        [RelayCommand]
        private void NextWindow(string? direction)
        {
            if (Views.ToolWindows.All.Count == 0)
            {
                Announcer.Announce(T("No other Tin Pan windows are open."), always: true);
                return;
            }
            Views.ToolWindows.Next(System.Windows.Application.Current.MainWindow, back: direction == "back");
        }

        private readonly IAuthService _auth;
        private readonly IAudioService _audio;
        private readonly IDialogService _dialogs;
        private readonly IConfigService _config;
        private readonly IUpdateService _updates;
        private readonly LiveLyrics _liveLyrics;
        private readonly IReportService _reports;
        private readonly OutputSwitch _outputs;

        [ObservableProperty] private string title = AppTitle;
        [ObservableProperty] private int selectedTab;

        public SunoPanelViewModel SunoVM { get; }
        public LibraryPanelViewModel LibraryVM { get; }
        public SettingsPanelViewModel SettingsVM { get; }
        public PlaylistsViewModel PlaylistsVM { get; }

        public MainViewModel(SunoPanelViewModel sunoVM, LibraryPanelViewModel libraryVM, SettingsPanelViewModel settingsVM, PlaylistsViewModel playlistsVM,
            IAuthService auth, IAudioService audio, IDialogService dialogs, IConfigService config, IUpdateService updates, SongWatcher watcher, LiveLyrics liveLyrics, IReportService reports, OutputSwitch outputs, IAnnouncer announcer)
            : base(announcer)
        {
            SunoVM = sunoVM;
            LibraryVM = libraryVM;
            SettingsVM = settingsVM;
            PlaylistsVM = playlistsVM;
            _auth = auth;
            _audio = audio;
            _dialogs = dialogs;
            _config = config;
            _updates = updates;
            _liveLyrics = liveLyrics;
            _reports = reports;
            _outputs = outputs;
            _audio.OutputFailed += (_, message) => Announcer.Announce(message, interrupt: false, always: true);

            _auth.AuthStateChanged += (_, _) => OnAuthChanged();
            _auth.SignInExpired += (_, _) => _signInExpired = true;
            SunoVM.SongsStarted += (_, clips) => LibraryVM.Upsert(clips);
            watcher.ClipsFinished += (_, clips) => AnnounceFinished(clips);
        }

        /// <summary>Called once the window is showing.</summary>
        private bool _signInExpired;
        private bool _askingToSignIn;

        public async Task InitializeAsync()
        {
            Announcer.Level = Enum.TryParse<SpeechLevel>(_config.Current.SpeechLevel, out var level) ? level : SpeechLevel.Everything;
            StartResumeTracking();
            bool signedIn;
            try
            {
                signedIn = await _auth.RestoreAsync();
            }
            catch (SunoApiException ex)
            {
                // Suno couldn't be reached: the saved sign-in stays, and there's nothing to ask yet.
                Announcer.Announce(T("Tin Pan ready. {0} Press F1 for help.", ex.Message), always: true);
                _ = AfterStartupUpdatesAsync();
                return;
            }
            if (signedIn)
            {
                Announcer.Announce(T("Tin Pan ready. Press F1 for help."), interrupt: false, always: true);
            }
            else
            {
                Announcer.Announce(T("Tin Pan ready."), interrupt: false, always: true);
                await OfferSignInAsync(expired: _signInExpired);
            }
            _ = AfterStartupUpdatesAsync();
            // From now on, an ended sign-in asks straight away.
            _auth.SignInExpired += (_, _) => System.Windows.Application.Current?.Dispatcher.BeginInvoke(() => _ = OfferSignInAsync(expired: true));
        }

        /// <summary>
        /// First run (nothing saved): a welcome that offers to sign in. An ended sign-in (Suno said so,
        /// not a network problem): asks to sign in again. No either way points to Settings.
        /// </summary>
        private async Task OfferSignInAsync(bool expired)
        {
            if (_askingToSignIn || _auth.IsAuthenticated) return;
            _askingToSignIn = true;
            _signInExpired = false;
            try
            {
                var yes = expired
                    ? _dialogs.Ask(T("Suno has ended your sign-in, so Tin Pan can't reach your songs. This happens now and then, for example after changing your password. Sign in again now?"), T("Sign in again"))
                    : _dialogs.Ask(T("Welcome to Tin Pan. To browse, play and make songs, sign in to your Suno account. Suno's own sign-in page opens, and closes by itself once you're in. Sign in now?"), T("Welcome to Tin Pan"));
                if (yes) await SettingsVM.SignInCommand.ExecuteAsync(null);
                if (!_auth.IsAuthenticated)
                    Announcer.Announce(T("Not signed in. You can sign in any time from Settings, Ctrl+5. Press F1 for help."), always: true);
            }
            finally
            {
                _askingToSignIn = false;
            }
        }

        private void OnAuthChanged()
        {
            if (_auth.IsAuthenticated)
            {
                Title = string.IsNullOrEmpty(_auth.UserName) ? T("Tin Pan - signed in") : $"Tin Pan - {_auth.UserName}";
                // One load of each, quietly; failures still get one spoken line.
                _ = LibraryVM.LoadFirstPageAsync(background: true);
                _ = SunoVM.LoadAccountAsync(background: true);
                _ = SunoVM.LoadNotificationsAsync(background: true);
                _ = PlaylistsVM.LoadAsync(background: true);
                // Starting the hidden browser takes a few seconds; do it now rather than on the first Play.
                _ = _audio.WarmUpAsync();
            }
            else
            {
                Title = AppTitle;
                LibraryVM.Clear();
                SunoVM.Clear();
                PlaylistsVM.Clear();
            }
        }

        private void AnnounceFinished(IReadOnlyList<SunoClip> clips)
        {
            var ready = clips.Where(c => !c.HasFailed).Select(c => c.DisplayTitle).Distinct().ToList();
            var failed = clips.Count(c => c.HasFailed);
            var parts = new List<string>();
            if (ready.Count > 0) parts.Add(T("Ready: {0}.", string.Join(", ", ready)));
            if (failed > 0) parts.Add(Plural(failed, "{0} song failed.", "{0} songs failed."));
            Announcer.Announce(string.Join(" ", parts), interrupt: false);
        }

        // ── Keys ────────────────────────────────────────────────────────────

        [RelayCommand]
        private void ContextHelp()
        {
            var text = SelectedTab switch
            {
                TrendingTab => HelpText.Trending,
                AccountTab => HelpText.Account,
                SettingsTab => HelpText.Settings,
                PlaylistsTab => HelpText.Playlists,
                _ => HelpText.Library
            };
            if (!_auth.IsAuthenticated && SelectedTab != SettingsTab) text += HelpText.NotSignedIn;
            var open = Views.ToolWindows.All.Where(w => w.IsVisible).Select(w => w.Title).ToList();
            var windows = open.Count == 0
                ? T("Ctrl+N opens Create; F6 then moves between it and the main window.")
                : T("F6 moves between the main window and {0}.", string.Join(", ", open));
            Announcer.Announce(windows + " " + text + " " + T("Ctrl+F1 for all commands."), always: true);
        }

        [RelayCommand]
        private Task ReadCreditsAsync() => ReadAccountAsync(() => SunoVM.Account.CreditsText);

        [RelayCommand]
        private Task ReadSubscriptionAsync() => ReadAccountAsync(() => SunoVM.Account.SubscriptionText);

        private async Task ReadAccountAsync(Func<string> text)
        {
            if (!_auth.IsAuthenticated)
            {
                Announcer.Announce(T("Not signed in."), always: true);
                return;
            }
            // Always fetch, so the numbers are current; fall back to the last known ones if that fails.
            try
            {
                await SunoVM.Account.RefreshAsync();
            }
            catch (SunoApiException ex)
            {
                if (SunoVM.Account.Billing == null)
                {
                    Announcer.Announce(ex.Message, always: true);
                    return;
                }
                Log.Warn($"Account refresh failed, reading last known values: {ex.Message}");
            }
            Announcer.Announce(text(), always: true);
        }

        /// <summary>Shift+F3: reading timed lyrics aloud as they're sung, on or off. Kept in sync with the Settings check box.</summary>
        [RelayCommand]
        private async Task ToggleLiveLyricsAsync()
        {
            SettingsVM.ReadLyricsLive = !SettingsVM.ReadLyricsLive;
            if (!SettingsVM.ReadLyricsLive)
            {
                Announcer.Announce(T("Lyrics won't be read as they're sung."), always: true);
                return;
            }
            Announcer.Announce(T("Lyrics will be read as they're sung."), always: true);
            if (await _liveLyrics.LoadAsync() == 0)
                Announcer.Announce(T("This song has no timed lyrics."), interrupt: false, always: true);
        }

        [RelayCommand]
        private void NowPlaying()
        {
            if (_audio.CurrentClipId == null)
            {
                Announcer.Announce(T("Nothing playing."), always: true);
                return;
            }
            var position = SunoClip.FormatTime(_audio.CurrentPosition);
            var where = _audio.TotalDuration > TimeSpan.Zero ? T("{0} of {1}", position, SunoClip.FormatTime(_audio.TotalDuration)) : position;
            Announcer.Announce((_audio.IsPaused ? T("Paused: {0}, {1}.", _audio.CurrentTitle, where) : T("Playing: {0}, {1}.", _audio.CurrentTitle, where)), always: true);
        }

        [RelayCommand]
        private Task RefreshAsync() => SelectedTab switch
        {
            LibraryTab => LibraryVM.RefreshCommand.ExecuteAsync(null),
            TrendingTab => SunoVM.RefreshTrendingCommand.ExecuteAsync(null),
            AccountTab => RefreshAccountPageAsync(),
            PlaylistsTab => PlaylistsVM.RefreshCommand.ExecuteAsync(null),
            _ => Task.CompletedTask
        };

        /// <summary>Trending loads the first time you open it, instead of waiting for F5.</summary>
        partial void OnSelectedTabChanged(int value)
        {
            if (value == SettingsTab) SettingsVM.RefreshOutputs();
            if (value == TrendingTab && _auth.IsAuthenticated && SunoVM.Trending.Count == 0 && SunoVM.RefreshTrendingCommand.CanExecute(null))
                _ = SunoVM.RefreshTrendingCommand.ExecuteAsync(null);
        }

        private async Task RefreshAccountPageAsync()
        {
            await SunoVM.RefreshAccountCommand.ExecuteAsync(null);
            await SunoVM.LoadNotificationsAsync(background: true);
        }

        [RelayCommand]
        private void ShowTab(string index) => SelectedTab = int.Parse(index);

        /// <summary>The list that started the current song, or the Library when nothing list-based is playing.</summary>
        private IPlaybackQueue Queue => _audio.CurrentOwner ?? SelectedTab switch
        {
            PlaylistsTab => PlaylistsVM,
            TrendingTab => SunoVM,
            _ => LibraryVM
        };

        [RelayCommand(AllowConcurrentExecutions = true)] private Task NextAsync() => Queue.NextAsync();
        [RelayCommand] private void PlayPause() => Queue.PlayPause();
        [RelayCommand(AllowConcurrentExecutions = true)] private Task PreviousAsync() => Queue.PreviousAsync();
    }
}
