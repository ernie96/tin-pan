using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.ComponentModel;
using System.IO;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using TinPan.Models;
using TinPan.Services;
using TinPan.Views;

namespace TinPan.ViewModels
{
    public partial class LibraryPanelViewModel : ViewModelBase, IPlaybackQueue
    {
        private const float VolumeStep = 0.1f;

        private readonly ISunoApiService _api;
        private readonly IAuthService _auth;
        private readonly IAudioService _audio;
        private readonly IConfigService _config;
        private readonly IDialogService _dialogs;
        private readonly SongWatcher _watcher;
        private readonly AccountState _account;

        private SunoClip? _current;
        private LibraryQuery _next = new();
        private bool _hasMore = true;
        private bool _loadingMore;
        private int _loadVersion;
        private string? _activeSearch;

        public ObservableCollection<SunoClip> Songs { get; } = new();

        [ObservableProperty]
        [NotifyPropertyChangedFor(nameof(VisibilityMenuText), nameof(RemixMenuText))]
        private SunoClip? selectedClip;

        [ObservableProperty] private string searchText = "";

        public LibraryPanelViewModel(ISunoApiService api, IAuthService auth, IAudioService audio, IConfigService config,
            IDialogService dialogs, SongWatcher watcher, AccountState account, IAnnouncer announcer) : base(announcer)
        {
            _api = api;
            _auth = auth;
            _audio = audio;
            _config = config;
            _dialogs = dialogs;
            _watcher = watcher;
            _account = account;

            _audio.TrackEnded += OnTrackEnded;
            _audio.PlaybackFailed += (_, message) => Announcer.Announce(message);
            _watcher.ClipsUpdated += (_, clips) => Upsert(clips);
            _account.PropertyChanged += OnAccountChanged;
        }

        // ── Menu labels that depend on the plan or the song ────────────────

        public string WavMenuText => _account.PaidLabel("Download _WAV");
        public string CoverMenuText => _account.PaidLabel("_Cover...");
        public string RemasterMenuText => _account.PaidLabel("Re_master...");
        public string VisibilityMenuText => SelectedClip?.IsPublic == true ? "Make p_rivate" : "Make p_ublic";
        public string RemixMenuText => SelectedClip?.Metadata?.CanRemix == false ? "Allow remi_xes" : "Stop remi_xes";

        /// <summary>Disabled only when Suno says this is a free plan; unknown plans may try.</summary>
        private bool CanUsePaidFeatures() => _account.IsPaid != false;

        private void OnAccountChanged(object? sender, PropertyChangedEventArgs e)
        {
            if (e.PropertyName != nameof(AccountState.Billing)) return;
            OnPropertyChanged(nameof(WavMenuText));
            OnPropertyChanged(nameof(CoverMenuText));
            OnPropertyChanged(nameof(RemasterMenuText));
            DownloadWavCommand.NotifyCanExecuteChanged();
            CoverCommand.NotifyCanExecuteChanged();
            RemasterCommand.NotifyCanExecuteChanged();
        }

        /// <summary>Slider value, 0 to 100.</summary>
        public double Volume
        {
            get => Math.Round(_audio.Volume * 100);
            set
            {
                _audio.Volume = (float)(value / 100);
                _config.Current.Audio.DefaultVolume = _audio.Volume;
                _config.SaveSoon();
                OnPropertyChanged();
            }
        }

        // ── Loading and search ──────────────────────────────────────────────

        [RelayCommand]
        private Task RefreshAsync() => LoadFirstPageAsync(background: false);

        [RelayCommand]
        private Task SearchAsync()
        {
            _activeSearch = string.IsNullOrWhiteSpace(SearchText) ? null : SearchText.Trim();
            return LoadFirstPageAsync(background: false);
        }

        [RelayCommand]
        private Task ClearSearchAsync()
        {
            if (_activeSearch == null && SearchText.Length == 0) return Task.CompletedTask;
            SearchText = "";
            _activeSearch = null;
            return LoadFirstPageAsync(background: false);
        }

        // ── Filters ─────────────────────────────────────────────────────────

        private static readonly (string Value, string Label)[] DislikedChoices =
        {
            ("False", "Hide songs I disliked"), ("Any", "Show songs I disliked too"), ("True", "Only songs I disliked")
        };

        private LibraryFilter CurrentFilter()
        {
            var f = _config.Current.Library;
            return new LibraryFilter(string.IsNullOrEmpty(f.WorkspaceId) ? null : f.WorkspaceId, f.Disliked, f.IncludeTrashed, f.HideStems, f.NewestFirst);
        }

        /// <summary>Short spoken description of the filters, for after they change.</summary>
        public string FilterSummary
        {
            get
            {
                var f = _config.Current.Library;
                var parts = new List<string> { string.IsNullOrEmpty(f.WorkspaceId) ? "All workspaces" : f.WorkspaceName };
                parts.Add(f.Disliked switch { "Any" => "disliked shown", "True" => "only disliked", _ => "disliked hidden" });
                if (f.IncludeTrashed) parts.Add("trash included");
                if (!f.HideStems) parts.Add("stems shown");
                parts.Add(f.NewestFirst ? "newest first" : "oldest first");
                return string.Join(", ", parts) + ".";
            }
        }

        [RelayCommand]
        private async Task FiltersAsync()
        {
            if (!_auth.IsAuthenticated)
            {
                Announcer.Announce("Sign in from Settings first.");
                return;
            }
            IReadOnlyList<Workspace> workspaces = Array.Empty<Workspace>();
            await RunAsync(async () => workspaces = await _api.GetWorkspacesAsync());

            var f = _config.Current.Library;
            var all = new Workspace("", "All workspaces", null);
            var workspaceItems = new List<object> { all };
            workspaceItems.AddRange(workspaces);
            var workspace = new ChoiceField("_Workspace", workspaceItems,
                workspaces.FirstOrDefault(w => w.Id == f.WorkspaceId) ?? (object)all);
            var dislikedLabels = DislikedChoices.Select(d => (object)d.Label).ToList();
            var disliked = new ChoiceField("_Disliked songs", dislikedLabels,
                DislikedChoices.FirstOrDefault(d => d.Value == f.Disliked).Label ?? DislikedChoices[0].Label);
            var trashed = new CheckField("Include songs in the _trash", f.IncludeTrashed);
            var stems = new CheckField("Hide _stems", f.HideStems);
            var order = new ChoiceField("_Order", new object[] { "Newest first", "Oldest first" }, f.NewestFirst ? "Newest first" : "Oldest first");

            if (!_dialogs.ShowForm("Library filters", "Suno applies these filters itself, so paging and search follow them.", "Apply",
                    new FormField[] { workspace, disliked, trashed, stems, order })) return;

            var chosen = (Workspace?)workspace.Selected ?? all;
            f.WorkspaceId = chosen.Id;
            f.WorkspaceName = chosen.Name;
            f.Disliked = DislikedChoices.FirstOrDefault(d => d.Label == (string?)disliked.Selected).Value ?? "False";
            f.IncludeTrashed = trashed.Value;
            f.HideStems = stems.Value;
            f.NewestFirst = (string?)order.Selected != "Oldest first";
            _config.SaveSoon();

            await LoadFirstPageAsync(background: false, prefix: FilterSummary + " ");
        }

        public async Task LoadFirstPageAsync(bool background, string prefix = "")
        {
            if (!_auth.IsAuthenticated) return;
            var version = ++_loadVersion;
            var search = _activeSearch;
            await RunAsync(async () =>
            {
                var page = await _api.GetLibraryPageAsync(new LibraryQuery(Search: search, Filter: CurrentFilter()));
                if (version != _loadVersion) return;

                var keep = SelectedClip?.Id;
                Songs.Clear();
                foreach (var clip in page.Clips) Songs.Add(clip);
                AdvancePaging(page, 0);
                _watcher.Watch(page.Clips);

                SelectedClip = Songs.FirstOrDefault(c => c.Id == keep) ?? Songs.FirstOrDefault();
                if (_current != null) _current = Songs.FirstOrDefault(c => c.Id == _current.Id) ?? _current;
                if (!background)
                {
                    var what = search == null ? "songs" : $"songs matching {search}";
                    Announcer.Announce(prefix + (Songs.Count == 0 ? (search == null ? "No songs." : $"No songs match {search}.") : $"{Songs.Count} {what}."));
                }
            }, background);
        }

        private void AdvancePaging(FeedPage page, int pageNumber)
        {
            _next = new LibraryQuery(page.NextCursor, pageNumber + 1, _activeSearch, CurrentFilter());
            _hasMore = page.HasMore ?? (page.Clips.Count > 0 && (page.NumTotal == null || Songs.Count < page.NumTotal));
        }

        /// <summary>Fetches the next page. Says how many arrived only when the user is waiting on the last item.</summary>
        private async Task<int> LoadMoreAsync(bool announce)
        {
            if (_loadingMore || !_hasMore || !_auth.IsAuthenticated) return 0;
            _loadingMore = true;
            var version = _loadVersion;
            var query = _next;
            var added = 0;
            try
            {
                await RunAsync(async () =>
                {
                    var page = await _api.GetLibraryPageAsync(query);
                    if (version != _loadVersion) return; // a refresh or search started meanwhile
                    var known = Songs.Select(s => s.Id).ToHashSet();
                    foreach (var clip in page.Clips.Where(c => known.Add(c.Id)))
                    {
                        Songs.Add(clip);
                        added++;
                    }
                    AdvancePaging(page, query.Page);
                    if (added == 0) _hasMore = false; // guards against a cursor that never ends
                    _watcher.Watch(page.Clips);
                    if (announce) Announcer.Announce(added > 0 ? $"{added} more." : "End of list.", interrupt: false);
                }, background: !announce);
            }
            finally
            {
                _loadingMore = false;
            }
            return added;
        }

        partial void OnSelectedClipChanged(SunoClip? value)
        {
            if (value != null && Songs.Count > 0 && ReferenceEquals(value, Songs[^1]))
                _ = LoadMoreAsync(announce: true);
        }

        public void Clear()
        {
            _loadVersion++;
            _watcher.Clear();
            _audio.Stop();
            _current = null;
            _activeSearch = null;
            SearchText = "";
            Songs.Clear();
            _hasMore = true;
        }

        /// <summary>New or changed songs from generation, extending or stems.</summary>
        public void Upsert(IEnumerable<SunoClip> clips)
        {
            foreach (var clip in clips)
            {
                var existing = Songs.FirstOrDefault(s => s.Id == clip.Id);
                if (existing != null) existing.UpdateFrom(clip);
                else Songs.Insert(0, clip);
            }
        }

        // ── Playback ────────────────────────────────────────────────────────

        [RelayCommand]
        private Task PlayAsync(SunoClip? clip) => StartAsync(clip ?? SelectedClip);

        private async Task StartAsync(SunoClip? clip)
        {
            if (clip == null) return;
            if (!clip.CanPlay)
            {
                Announcer.Announce(clip.HasFailed ? "That song failed to generate." : "That song isn't ready yet.");
                return;
            }

            _current = clip;
            if (await RunAsync(() => _audio.PlayAsync(clip.Id, clip.DisplayTitle, this)))
            {
                Announcer.Announce($"Playing: {clip.DisplayTitle}.");
            }
        }

        [RelayCommand]
        private void PlayPause()
        {
            if (_audio.IsPlaying)
            {
                _audio.Pause();
                Announcer.Announce("Paused.");
            }
            else if (_audio.IsPaused)
            {
                _audio.Resume();
                Announcer.Announce("Playing.");
            }
            else
            {
                _ = StartAsync(SelectedClip);
            }
        }

        [RelayCommand]
        private void Stop()
        {
            if (_audio.CurrentClipId == null) return;
            _audio.Stop();
            Announcer.Announce("Stopped.");
        }

        /// <summary>Seconds to move, as text from a key binding: "-10", "5".</summary>
        [RelayCommand]
        private void Seek(string seconds)
        {
            if (_audio.CurrentClipId == null || !double.TryParse(seconds, System.Globalization.NumberStyles.Float,
                    System.Globalization.CultureInfo.InvariantCulture, out var delta)) return;
            _audio.CurrentPosition += TimeSpan.FromSeconds(delta);
            Announcer.Announce(SunoClip.FormatTime(_audio.CurrentPosition));
        }

        Task IPlaybackQueue.PreviousAsync() => PreviousAsync();
        Task IPlaybackQueue.NextAsync() => NextAsync();
        void IPlaybackQueue.PlayPause() => PlayPause();

        [RelayCommand]
        private Task PreviousAsync()
        {
            var index = _current == null ? -1 : Songs.IndexOf(_current);
            // Skip songs still being made or failed, as Next does.
            var previous = index > 0 ? Songs.Take(index).Reverse().FirstOrDefault(c => c.CanPlay) : null;
            if (previous != null) return StartAsync(previous);
            Announcer.Announce("First song.");
            return Task.CompletedTask;
        }

        [RelayCommand]
        private async Task NextAsync()
        {
            var next = await FindNextAsync();
            if (next != null) await StartAsync(next);
            else Announcer.Announce("End of list.");
        }

        private async Task<SunoClip?> FindNextAsync()
        {
            var index = _current == null ? -1 : Songs.IndexOf(_current);
            if (index < 0) return null;
            if (index == Songs.Count - 1) await LoadMoreAsync(announce: false);
            return Songs.Skip(index + 1).FirstOrDefault(c => c.CanPlay);
        }

        private async void OnTrackEnded(object? sender, TrackEndedEventArgs e)
        {
            // Only continue through the library if the library started this track.
            if (!ReferenceEquals(e.Owner, this) || _current?.Id != e.ClipId) return;
            var next = await FindNextAsync();
            if (next != null) await StartAsync(next);
            else
            {
                _current = null;
                Announcer.Announce("End of library.", interrupt: false);
            }
        }

        [RelayCommand] private void VolumeUp() => ChangeVolume(+VolumeStep);
        [RelayCommand] private void VolumeDown() => ChangeVolume(-VolumeStep);

        private void ChangeVolume(float delta)
        {
            Volume = Math.Clamp(_audio.Volume + delta, 0f, 1f) * 100;
            Announcer.Announce($"Volume {Volume:0}.");
        }
    }
}
