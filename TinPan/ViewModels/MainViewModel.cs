using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading.Tasks;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using TinPan.Models;
using TinPan.Services;

namespace TinPan.ViewModels
{
    public partial class MainViewModel : ViewModelBase
    {
        private const string AppTitle = "Tin Pan, by Ernesto Melendez";
        // Tab order, which is also Ctrl+1 to Ctrl+6.
        public const int LibraryTab = 0, PlaylistsTab = 1, CreateTab = 2, TrendingTab = 3, AccountTab = 4, SettingsTab = 5;

        private readonly IAuthService _auth;
        private readonly IAudioService _audio;
        private readonly IDialogService _dialogs;
        private readonly IConfigService _config;
        private readonly IUpdateService _updates;

        [ObservableProperty] private string title = AppTitle;
        [ObservableProperty] private int selectedTab;

        public SunoPanelViewModel SunoVM { get; }
        public LibraryPanelViewModel LibraryVM { get; }
        public SettingsPanelViewModel SettingsVM { get; }
        public PlaylistsViewModel PlaylistsVM { get; }

        public MainViewModel(SunoPanelViewModel sunoVM, LibraryPanelViewModel libraryVM, SettingsPanelViewModel settingsVM, PlaylistsViewModel playlistsVM,
            IAuthService auth, IAudioService audio, IDialogService dialogs, IConfigService config, IUpdateService updates, SongWatcher watcher, IAnnouncer announcer)
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

            _auth.AuthStateChanged += (_, _) => OnAuthChanged();
            SunoVM.SongsStarted += (_, clips) => LibraryVM.Upsert(clips);
            watcher.ClipsFinished += (_, clips) => AnnounceFinished(clips);
        }

        /// <summary>Called once the window is showing.</summary>
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
                Announcer.Announce($"Tin Pan ready. {ex.Message} Press F1 for help.", always: true);
                _ = AfterStartupUpdatesAsync();
                return;
            }
            Announcer.Announce(signedIn
                ? "Tin Pan ready. Press F1 for help."
                : "Tin Pan ready. Not signed in. Press F1 for help.", interrupt: false, always: true);
            _ = AfterStartupUpdatesAsync();
        }

        private void OnAuthChanged()
        {
            if (_auth.IsAuthenticated)
            {
                Title = string.IsNullOrEmpty(_auth.UserName) ? "Tin Pan - signed in" : $"Tin Pan - {_auth.UserName}";
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
            if (ready.Count > 0) parts.Add($"Ready: {string.Join(", ", ready)}.");
            if (failed > 0) parts.Add(failed == 1 ? "1 song failed." : $"{failed} songs failed.");
            Announcer.Announce(string.Join(" ", parts), interrupt: false);
        }

        // ── Keys ────────────────────────────────────────────────────────────

        [RelayCommand]
        private void ContextHelp()
        {
            var text = SelectedTab switch
            {
                CreateTab => HelpText.Create,
                TrendingTab => HelpText.Trending,
                AccountTab => HelpText.Account,
                SettingsTab => HelpText.Settings,
                PlaylistsTab => HelpText.Playlists,
                _ => HelpText.Library
            };
            if (!_auth.IsAuthenticated && SelectedTab != SettingsTab) text += HelpText.NotSignedIn;
            Announcer.Announce(text + " Ctrl+F1 for all commands.", always: true);
        }

        [RelayCommand]
        private Task ReadCreditsAsync() => ReadAccountAsync(() => SunoVM.Account.CreditsText);

        [RelayCommand]
        private Task ReadSubscriptionAsync() => ReadAccountAsync(() => SunoVM.Account.SubscriptionText);

        private async Task ReadAccountAsync(Func<string> text)
        {
            if (!_auth.IsAuthenticated)
            {
                Announcer.Announce("Not signed in.", always: true);
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

        [RelayCommand]
        private void NowPlaying()
        {
            if (_audio.CurrentClipId == null)
            {
                Announcer.Announce("Nothing playing.", always: true);
                return;
            }
            var total = _audio.TotalDuration > TimeSpan.Zero ? " of " + SunoClip.FormatTime(_audio.TotalDuration) : "";
            Announcer.Announce($"{(_audio.IsPaused ? "Paused" : "Playing")}: {_audio.CurrentTitle}, {SunoClip.FormatTime(_audio.CurrentPosition)}{total}.", always: true);
        }

        [RelayCommand]
        private Task RefreshAsync() => SelectedTab switch
        {
            LibraryTab => LibraryVM.RefreshCommand.ExecuteAsync(null),
            CreateTab => SunoVM.RefreshAccountCommand.ExecuteAsync(null),
            TrendingTab => SunoVM.RefreshTrendingCommand.ExecuteAsync(null),
            AccountTab => RefreshAccountPageAsync(),
            PlaylistsTab => PlaylistsVM.RefreshCommand.ExecuteAsync(null),
            _ => Task.CompletedTask
        };

        /// <summary>Trending loads the first time you open it, instead of waiting for F5.</summary>
        partial void OnSelectedTabChanged(int value)
        {
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
        private IPlaybackQueue Queue => _audio.CurrentOwner ?? (SelectedTab == PlaylistsTab ? PlaylistsVM : LibraryVM);

        [RelayCommand] private Task NextAsync() => Queue.NextAsync();
        [RelayCommand] private void PlayPause() => Queue.PlayPause();
        [RelayCommand] private Task PreviousAsync() => Queue.PreviousAsync();
    }
}
