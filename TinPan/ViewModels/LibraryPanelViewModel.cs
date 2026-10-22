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
using static TinPan.Localization.Loc;

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
        private readonly ICleanupService _cleanup;
        private readonly IUploadService _uploads;

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
            IDialogService dialogs, SongWatcher watcher, AccountState account, ICleanupService cleanup, IUploadService uploads, IAnnouncer announcer) : base(announcer)
        {
            _api = api;
            _auth = auth;
            _audio = audio;
            _config = config;
            _dialogs = dialogs;
            _watcher = watcher;
            _account = account;
            _cleanup = cleanup;
            _uploads = uploads;

            _audio.TrackEnded += OnTrackEnded;
            _audio.PlaybackFailed += (_, message) => Announcer.Announce(message);
            _watcher.ClipsUpdated += (_, clips) => Upsert(clips);
            _account.PropertyChanged += OnAccountChanged;
        }

        // ── Menu labels that depend on the plan or the song ────────────────

        public string WavMenuText => _account.PaidLabel(T("Download _WAV"));
        public string CoverMenuText => _account.PaidLabel(T("_Cover..."));
        public string RemasterMenuText => _account.PaidLabel(T("Re_master..."));
        public string VisibilityMenuText => SelectedClip?.IsPublic == true ? T("Make p_rivate") : T("Make p_ublic");
        public string RemixMenuText => SelectedClip?.Metadata?.CanRemix == false ? T("Allow remi_xes") : T("Stop remi_xes");

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

        private static (string Value, string Label)[] DislikedChoices => new[]
        {
            ("False", T("Hide songs I disliked")), ("Any", T("Show songs I disliked too")), ("True", T("Only songs I disliked"))
        };

        /// <summary>The form label for each of Suno's filters (FeedFilters.All), with its access key.</summary>
        private static string FilterLabel(string field) => field switch
        {
            "trashed" => T("Songs in the _trash"),
            "stem" => T("_Stems"),
            "fromStudioProject" => T("Suno Stud_io songs"),
            "liked" => T("_Liked songs"),
            "public" => T("Pu_blic or private"),
            "upload" => T("_Uploaded songs"),
            "cover" => T("_Covers"),
            "extend" => T("_Extended songs"),
            "remaster" => T("_Remasters"),
            "concat" => T("W_hole songs (joined parts)"),
            "persona" => T("Songs with a _persona"),
            "playlist" => T("Songs in a pla_ylist"),
            "unlocked" => T("Downloaded be_fore"),
            _ => field
        };

        /// <summary>The three choices of a filter, in Suno's values: show both, only these, hide these.</summary>
        private static (string Value, string Label)[] FilterChoices(string field) => field == "public"
            ? new[] { (FeedFilters.Any, T("Both")), (FeedFilters.Only, T("Only public")), (FeedFilters.Hide, T("Only private")) }
            : new[] { (FeedFilters.Any, T("Show")), (FeedFilters.Only, T("Only these")), (FeedFilters.Hide, T("Hide")) };

        private static (string Id, string Label)[] SortChoices => new[]
        {
            ("newest", T("Newest first")), ("oldest", T("Oldest first")), ("most-liked", T("Most liked first")), ("least-liked", T("Least liked first"))
        };

        private static string FilterValue(LibraryConfig f, FeedFilter filter) =>
            FeedFilters.Valid(f.Show.TryGetValue(filter.Field, out var v) ? v : null, filter.Default);

        private LibraryFilter CurrentFilter()
        {
            var f = _config.Current.Library;
            return new LibraryFilter
            {
                WorkspaceId = string.IsNullOrEmpty(f.WorkspaceId) ? null : f.WorkspaceId,
                Disliked = f.Disliked,
                Show = FeedFilters.All.ToDictionary(x => x.Field, x => FilterValue(f, x)),
                Sort = f.Sort ?? "newest"
            };
        }

        /// <summary>Short spoken description of the filters, for after they change: only what differs from Suno's defaults.</summary>
        public string FilterSummary
        {
            get
            {
                var f = _config.Current.Library;
                var parts = new List<string> { string.IsNullOrEmpty(f.WorkspaceId) ? T("All workspaces") : f.WorkspaceName };
                parts.Add(f.Disliked switch { "Any" => T("disliked shown"), "True" => T("only disliked"), _ => T("disliked hidden") });
                foreach (var filter in FeedFilters.All)
                {
                    var value = FilterValue(f, filter);
                    if (value == filter.Default) continue;
                    parts.Add(T("{0}: {1}", FilterLabel(filter.Field).Replace("_", ""), FilterChoices(filter.Field).First(c => c.Value == value).Label));
                }
                parts.Add(SortChoices.FirstOrDefault(c => c.Id == f.Sort).Label ?? SortChoices[0].Label);
                return string.Join(", ", parts) + ".";
            }
        }

        [RelayCommand]
        private async Task FiltersAsync()
        {
            if (!_auth.IsAuthenticated)
            {
                Announcer.Announce(T("Sign in from Settings first."));
                return;
            }
            IReadOnlyList<Workspace> workspaces = Array.Empty<Workspace>();
            await RunAsync(async () => workspaces = await _api.GetWorkspacesAsync());

            var f = _config.Current.Library;
            var all = new Workspace("", T("All workspaces"), null);
            var workspaceItems = new List<object> { all };
            workspaceItems.AddRange(workspaces);
            var workspace = new ChoiceField(T("_Workspace"), workspaceItems,
                workspaces.FirstOrDefault(w => w.Id == f.WorkspaceId) ?? (object)all);
            var dislikedLabels = DislikedChoices.Select(d => (object)d.Label).ToList();
            var disliked = new ChoiceField(T("_Disliked songs"), dislikedLabels,
                DislikedChoices.FirstOrDefault(d => d.Value == f.Disliked).Label ?? DislikedChoices[0].Label);
            var flags = FeedFilters.All.Select(filter =>
            {
                var choices = FilterChoices(filter.Field);
                return (filter, choices, field: new ChoiceField(FilterLabel(filter.Field), choices.Select(c => (object)c.Label).ToList(),
                    choices.First(c => c.Value == FilterValue(f, filter)).Label));
            }).ToList();
            var order = new ChoiceField(T("_Order"), SortChoices.Select(c => (object)c.Label).ToList(),
                SortChoices.FirstOrDefault(c => c.Id == f.Sort).Label ?? SortChoices[0].Label);

            var fields = new List<FormField> { workspace, disliked };
            fields.AddRange(flags.Select(x => x.field));
            fields.Add(order);
            if (!_dialogs.ShowForm(T("Library filters"), T("Suno applies these filters itself, so paging and search follow them. Each one shows those songs, shows only them, or hides them."), T("Apply"), fields))
                return;

            var chosen = (Workspace?)workspace.Selected ?? all;
            f.WorkspaceId = chosen.Id;
            f.WorkspaceName = chosen.Name;
            f.Disliked = DislikedChoices.FirstOrDefault(d => d.Label == (string?)disliked.Selected).Value ?? "False";
            foreach (var (filter, choices, field) in flags)
                f.Show[filter.Field] = choices.FirstOrDefault(c => c.Label == (string?)field.Selected).Value ?? filter.Default;
            f.Sort = SortChoices.FirstOrDefault(c => c.Label == (string?)order.Selected).Id ?? "newest";
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
                    Announcer.Announce(prefix + (Songs.Count == 0
                        ? search == null ? T("No songs.") : T("No songs match {0}.", search)
                        : search == null ? Plural(Songs.Count, "{0} song.", "{0} songs.") : T("{0} matching {1}.", Plural(Songs.Count, "{0} song", "{0} songs"), search)));
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
                    if (announce) Announcer.Announce(added > 0 ? T("{0} more.", added) : T("End of list."), interrupt: false);
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
                Announcer.Announce(clip.HasFailed ? T("That song failed to generate.") : T("That song isn't ready yet."));
                return;
            }

            _current = clip;
            if (await RunAsync(() => _audio.PlayAsync(clip.Id, clip.DisplayTitle, this)))
            {
                Announcer.Announce(T("Playing: {0}.", clip.DisplayTitle));
            }
        }

        [RelayCommand]
        private void PlayPause()
        {
            if (_audio.IsPlaying)
            {
                _audio.Pause();
                Announcer.Announce(T("Paused."));
            }
            else if (_audio.IsPaused)
            {
                _audio.Resume();
                Announcer.Announce(T("Playing."));
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
            Announcer.Announce(T("Stopped."));
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
            Announcer.Announce(T("First song."));
            return Task.CompletedTask;
        }

        [RelayCommand]
        private async Task NextAsync()
        {
            var next = await FindNextAsync();
            if (next != null) await StartAsync(next);
            else Announcer.Announce(T("End of list."));
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
                Announcer.Announce(T("End of library."), interrupt: false);
            }
        }

        [RelayCommand] private void VolumeUp() => ChangeVolume(+VolumeStep);
        [RelayCommand] private void VolumeDown() => ChangeVolume(-VolumeStep);

        private void ChangeVolume(float delta)
        {
            Volume = Math.Clamp(_audio.Volume + delta, 0f, 1f) * 100;
            Announcer.Announce(T("Volume {0:0}.", Volume));
        }
    }
}
