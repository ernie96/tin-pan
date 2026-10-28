using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Linq;
using System.Threading.Tasks;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using TinPan.Models;
using TinPan.Services;
using TinPan.Views;
using static TinPan.Localization.Loc;

namespace TinPan.ViewModels
{
    public partial class SunoPanelViewModel : ViewModelBase, IPlaybackQueue
    {
        public static IReadOnlyList<string> VocalChoices { get; } = new[] { T("Any voice"), T("Male vocals"), T("Female vocals") };

        private readonly ISunoApiService _api;
        private readonly IAuthService _auth;
        private readonly IAudioService _audio;
        private readonly IConfigService _config;
        private readonly SongWatcher _watcher;
        private readonly IDialogService _dialogs;

        public SunoPanelViewModel(ISunoApiService api, IAuthService auth, IAudioService audio, IConfigService config,
            SongWatcher watcher, AccountState account, IDialogService dialogs, IAnnouncer announcer) : base(announcer)
        {
            _dialogs = dialogs;
            _api = api;
            _auth = auth;
            _audio = audio;
            _config = config;
            _watcher = watcher;
            Account = account;
            Account.PropertyChanged += OnAccountChanged;
            _audio.TrackEnded += OnTrackEnded;
        }

        public AccountState Account { get; }

        /// <summary>Songs that were just started from this tab, for the library to show.</summary>
        public event EventHandler<IReadOnlyList<SunoClip>>? SongsStarted;

        // ── Account ─────────────────────────────────────────────────────────

        [ObservableProperty] private string accountText = T("Not signed in.");

        public ObservableCollection<Notification> Notifications { get; } = new();

        private void OnAccountChanged(object? sender, PropertyChangedEventArgs e)
        {
            if (e.PropertyName != nameof(AccountState.Billing)) return;
            AccountText = Account.Billing == null ? T("Not signed in.") : $"{Account.SubscriptionText} {BillingText.Features(Account.Billing)}".Trim();
            SetModels(Account.Models);
        }

        [RelayCommand]
        private Task RefreshAccountAsync() => LoadAccountAsync(background: false);

        /// <summary>background=true is the load after sign-in: failures are only logged, the library load speaks for both.</summary>
        public async Task LoadAccountAsync(bool background)
        {
            if (!_auth.IsAuthenticated) return;
            await RunAsync(background, async () =>
            {
                await Account.RefreshAsync();
                if (!background) Announcer.Announce(Account.CreditsText);
                await LoadPersonasAsync();
            });
        }

        [RelayCommand]
        private Task RefreshNotificationsAsync() => LoadNotificationsAsync(background: false);

        [RelayCommand]
        private async Task MarkNotificationsReadAsync()
        {
            if (!await RunAsync(() => _api.MarkNotificationsReadAsync())) return;
            foreach (var n in Notifications) n.IsRead = true;
            var items = Notifications.ToList();
            Notifications.Clear();
            foreach (var n in items) Notifications.Add(n);
            Announcer.Announce(T("Notifications marked as read."));
        }

        public async Task LoadNotificationsAsync(bool background)
        {
            if (!_auth.IsAuthenticated) return;
            await RunAsync(background, async () =>
            {
                var items = await _api.GetNotificationsAsync();
                Notifications.Clear();
                foreach (var n in items) Notifications.Add(n);
                if (!background) Announcer.Announce(items.Count == 0 ? T("No notifications.") : Plural(items.Count, "{0} notification.", "{0} notifications."));
            });
        }

        private async Task LoadPersonasAsync()
        {
            try
            {
                var list = await _api.GetPersonasAsync();
                var keep = SelectedPersona?.Id;
                Personas.Clear();
                Personas.Add(NoPersona);
                foreach (var p in list) Personas.Add(p);
                SelectedPersona = Personas.FirstOrDefault(p => p.Id == keep) ?? NoPersona;
            }
            catch (SunoApiException ex)
            {
                Log.Warn($"Personas didn't load: {ex.Message}");
            }
        }

        private async Task RunAsync(bool background, Func<Task> work)
        {
            if (!background)
            {
                await RunAsync(work);
                return;
            }
            try { await work(); }
            catch (SunoApiException ex) { Log.Warn($"Background load failed: {ex.Message}"); }
        }

        public void Clear()
        {
            Account.Clear();
            Notifications.Clear();
            Trending.Clear();
        }

        // ── Create ──────────────────────────────────────────────────────────

        public ObservableCollection<SunoModel> Models { get; } = new();

        [ObservableProperty] private SunoModel? selectedModel;
        [ObservableProperty] private string description = "";
        [ObservableProperty] private string lyrics = "";
        [ObservableProperty] private string style = "";
        [ObservableProperty] private string excludeStyles = "";
        [ObservableProperty] private string songTitle = "";
        [ObservableProperty] private bool isInstrumental;
        [ObservableProperty] private bool isMaxMode;
        [ObservableProperty] private string vocal = VocalChoices[0];
        [ObservableProperty] private string weirdness = "";
        [ObservableProperty] private string styleInfluence = "";
        [ObservableProperty] private string personaId = "";
        [ObservableProperty] private Persona? selectedPersona;

        public static Persona NoPersona { get; } = new("", T("No persona"));

        /// <summary>Your personas from Suno, with "No persona" first.</summary>
        public ObservableCollection<Persona> Personas { get; } = new() { NoPersona };
        [ObservableProperty] private string lyricsTopic = "";
        [ObservableProperty] private string generatedLyrics = "";
        [ObservableProperty] private string cowriteInstruction = "";

        public IReadOnlyList<string> Vocals => VocalChoices;

        partial void OnSelectedModelChanged(SunoModel? value)
        {
            if (value == null || value.ExternalKey == _config.Current.LastModel) return;
            _config.Current.LastModel = value.ExternalKey;
            _config.SaveSoon();
        }

        private void SetModels(IReadOnlyList<SunoModel> usable)
        {
            if (usable.Count == 0) return;
            var keep = SelectedModel?.ExternalKey ?? _config.Current.LastModel;
            Models.Clear();
            foreach (var m in usable) Models.Add(m);
            SelectedModel = usable.FirstOrDefault(m => m.ExternalKey == keep) ?? Account.DefaultModel;
        }

        [RelayCommand]
        private Task CreateFromDescriptionAsync()
        {
            if (string.IsNullOrWhiteSpace(Description))
            {
                Announcer.Announce(T("Describe the song first."));
                return Task.CompletedTask;
            }
            return StartSongsAsync(GenerateRequest.Modes.Inspiration);
        }

        [RelayCommand]
        private Task CreateCustomAsync()
        {
            if (string.IsNullOrWhiteSpace(Lyrics) && !IsInstrumental)
            {
                Announcer.Announce(T("Add lyrics, or tick Instrumental."));
                return Task.CompletedTask;
            }
            return StartSongsAsync(GenerateRequest.Modes.Custom);
        }

        /// <summary>Builds the request from the Create fields. Returns null and says why when something is off.</summary>
        public GenerateRequest? BuildRequest(string mode)
        {
            if (SelectedModel == null)
            {
                Announcer.Announce(T("Models haven't loaded yet. Press F5 to refresh, then try again."));
                return null;
            }

            var weird = ParsePercent(Weirdness, T("Weirdness"));
            var influence = ParsePercent(StyleInfluence, T("Style influence"));
            if (double.IsNaN(weird ?? 0) || double.IsNaN(influence ?? 0)) return null;

            var inspiration = mode == GenerateRequest.Modes.Inspiration;
            var tags = string.Join(", ", new[] { Style.Trim(), Vocal == VocalChoices[1] ? "male vocals" : Vocal == VocalChoices[2] ? "female vocals" : "" }
                .Where(s => s.Length > 0));
            var request = new GenerateRequest(SelectedModel.ExternalKey, mode)
            {
                Prompt = inspiration ? Description.Trim() : IsInstrumental ? "" : Lyrics.Trim(),
                Title = inspiration ? "" : SongTitle.Trim(),
                Tags = tags,
                NegativeTags = ExcludeStyles.Trim(),
                MakeInstrumental = IsInstrumental,
                PersonaId = !string.IsNullOrWhiteSpace(PersonaId) ? PersonaId.Trim()
                    : string.IsNullOrEmpty(SelectedPersona?.Id) ? null : SelectedPersona.Id
            };
            request.Metadata.ControlSliders = ControlSliders.FromPercent(weird, influence, null);
            request.Metadata.IsMaxMode = IsMaxMode;

            var problem = CheckLengths(request, SelectedModel.MaxLengths ?? MaxLengths.Fallback, inspiration);
            if (problem != null)
            {
                Announcer.Announce(problem);
                return null;
            }
            return request;
        }

        /// <summary>Same limits the Suno web form applies, counted the same way (UTF-16 units).</summary>
        public static string? CheckLengths(GenerateRequest r, MaxLengths limits, bool inspiration)
        {
            var checks = new (string Name, string Value, int Limit)[]
            {
                (T("Title"), r.Title, limits.Title),
                (T("Style"), r.Tags, limits.Tags),
                (T("Exclude styles"), r.NegativeTags, limits.NegativeTags),
                (inspiration ? T("Description") : T("Lyrics"), r.Prompt, inspiration ? limits.Description : limits.Prompt)
            };
            foreach (var (name, value, limit) in checks)
                if (limit > 0 && value.Length > limit)
                    return T("{0} is {1} characters. This model allows {2}.", name, value.Length, limit);
            return null;
        }

        /// <summary>Empty means Suno chooses (null). NaN means the text was wrong; that has been announced.</summary>
        private double? ParsePercent(string text, string name)
        {
            if (string.IsNullOrWhiteSpace(text)) return null;
            var value = NumberField.Parse(text.Trim(), isTime: false);
            if (value is >= 0 and <= 100) return value;
            Announcer.Announce(T("{0} must be a number from 0 to 100, or empty.", name));
            return double.NaN;
        }

        private async Task StartSongsAsync(string mode)
        {
            if (!_auth.IsAuthenticated)
            {
                Announcer.Announce(T("Sign in from Settings first."));
                return;
            }
            var request = BuildRequest(mode);
            if (request == null) return;

            Announcer.Announce(T("Sending to Suno."));
            await RunAsync(async () =>
            {
                var clips = await _api.GenerateAsync(request);
                SongsStarted?.Invoke(this, clips);
                _watcher.Watch(clips);
                Announcer.Announce(clips.Count == 1
                    ? T("Making 1 song. You'll hear when it's ready.")
                    : T("Making {0} songs. You'll hear when they're ready.", clips.Count));
                _ = LoadAccountAsync(background: true);
            });
        }

        [RelayCommand]
        private async Task WriteLyricsAsync()
        {
            if (string.IsNullOrWhiteSpace(LyricsTopic))
            {
                Announcer.Announce(T("Type a topic first."));
                return;
            }
            Announcer.Announce(T("Writing lyrics."));
            await RunAsync(async () =>
            {
                var result = await _api.GenerateLyricsAsync(LyricsTopic.Trim());
                GeneratedLyrics = result.Text;
                if (string.IsNullOrWhiteSpace(SongTitle) && !string.IsNullOrWhiteSpace(result.Title)) SongTitle = result.Title;
                Announcer.Announce(T("Lyrics ready."));
            });
        }

        /// <summary>Style Boost: Suno rewrites the Style field into a fuller description, using the lyrics as a hint.</summary>
        [RelayCommand]
        private async Task BoostStyleAsync()
        {
            if (string.IsNullOrWhiteSpace(Style))
            {
                Announcer.Announce(T("Type a style first, even a word or two."));
                return;
            }
            Announcer.Announce(T("Boosting the style."));
            await RunAsync(async () =>
            {
                Style = await _api.BoostStyleAsync(Style.Trim(), IsInstrumental ? null : Lyrics, IsInstrumental, null);
                Announcer.Announce(T("Style boosted: {0}", Style));
            });
        }

        /// <summary>Style ideas from Suno; Enter puts one in the Style box.</summary>
        [RelayCommand]
        private async Task StyleIdeasAsync()
        {
            IReadOnlyList<string> ideas = Array.Empty<string>();
            if (!await RunAsync(async () => ideas = await _api.GetStyleSuggestionsAsync())) return;
            if (ideas.Count == 0)
            {
                Announcer.Announce(T("Suno has no style ideas right now."));
                return;
            }
            _dialogs.ShowLines(T("Style ideas. Enter puts one in the Style box."), ideas, index =>
            {
                Style = ideas[index];
                Announcer.Announce(T("Style: {0}", Style));
            });
        }

        /// <summary>
        /// Drafts from Suno's lyrics editor. The first line saves the Lyrics box as a new draft; Enter on a draft
        /// offers putting it in the Lyrics box, saving the Lyrics box into it, renaming and deleting.
        /// </summary>
        [RelayCommand]
        private async Task LyricsDraftsAsync()
        {
            IReadOnlyList<LyricsDraft> drafts = Array.Empty<LyricsDraft>();
            if (!await RunAsync(async () => drafts = await _api.GetLyricsDraftsAsync())) return;
            var lines = new List<string> { T("Save the Lyrics box as a new draft...") };
            lines.AddRange(drafts.Select(d => d.ToString()));
            var title = drafts.Count == 0
                ? T("Lyrics drafts. You have none on Suno yet.")
                : Plural(drafts.Count, "Lyrics drafts. {0} draft; Enter on one for what to do with it.", "Lyrics drafts. {0} drafts; Enter on one for what to do with it.");
            _dialogs.ShowLines(title, lines, index =>
            {
                if (index == 0) _ = NewLyricsDraftAsync();
                else DraftActions(drafts[index - 1]);
            });
        }

        private void DraftActions(LyricsDraft draft)
        {
            var actions = new List<(string Label, Func<Task> Run)>
            {
                (T("Put it in the Lyrics box"), () => OpenDraftAsync(draft)),
                (T("Save the Lyrics box into it"), () => SaveIntoDraftAsync(draft)),
                (T("Rename..."), () => RenameDraftAsync(draft)),
                (T("Delete..."), () => DeleteDraftAsync(draft))
            };
            _dialogs.ShowLines(draft.ToString(), actions.Select(a => a.Label).ToList(), index => _ = actions[index].Run());
        }

        private async Task OpenDraftAsync(LyricsDraft draft)
        {
            string? text = null;
            if (!await RunAsync(async () => text = await _api.GetLyricsDraftTextAsync(draft.Id))) return;
            if (string.IsNullOrWhiteSpace(text))
            {
                Announcer.Announce(T("That draft has no lyrics yet."));
                return;
            }
            if (!string.IsNullOrWhiteSpace(Lyrics) && !_dialogs.Confirm(T("Replace the lyrics in the Lyrics box?"), T("Lyrics drafts"))) return;
            Lyrics = text;
            if (string.IsNullOrWhiteSpace(SongTitle) && !string.IsNullOrWhiteSpace(draft.Title)) SongTitle = draft.Title;
            Announcer.Announce(T("Lyrics from {0} are in the Lyrics box.", draft.ToString()));
        }

        private bool LyricsToSave()
        {
            if (!string.IsNullOrWhiteSpace(Lyrics)) return true;
            Announcer.Announce(T("The Lyrics box is empty, so there's nothing to save."));
            return false;
        }

        private async Task NewLyricsDraftAsync()
        {
            if (!LyricsToSave()) return;
            var name = new TextField(T("_Draft name"), SongTitle.Trim(), maxLength: 200);
            if (!_dialogs.ShowForm(T("Save as a new draft"), T("Saves the Lyrics box to Suno as a lyrics draft."), T("Save"), new FormField[] { name })) return;
            var words = Lyrics;
            if (await RunAsync(async () =>
            {
                var draft = await _api.CreateLyricsDraftAsync(name.Value);
                await _api.SaveLyricsDraftAsync(draft.Id, words);
            }))
                Announcer.Announce(T("Saved as the draft {0}.", name.Value.Length > 0 ? name.Value : T("Untitled")));
        }

        private async Task SaveIntoDraftAsync(LyricsDraft draft)
        {
            if (!LyricsToSave()) return;
            if (!_dialogs.Confirm(T("Replace the words in the draft {0} with the Lyrics box?", draft.Name), T("Lyrics drafts"))) return;
            if (await RunAsync(() => _api.SaveLyricsDraftAsync(draft.Id, Lyrics)))
                Announcer.Announce(T("Saved into {0}.", draft.Name));
        }

        private async Task RenameDraftAsync(LyricsDraft draft)
        {
            var name = new TextField(T("_Name"), draft.Title, maxLength: 200);
            if (!_dialogs.ShowForm(T("Rename {0}", draft.Name), "", T("Rename"), new FormField[] { name }) || name.Value.Length == 0) return;
            if (await RunAsync(() => _api.RenameLyricsDraftAsync(draft.Id, name.Value)))
                Announcer.Announce(T("Renamed to {0}.", name.Value));
        }

        private async Task DeleteDraftAsync(LyricsDraft draft)
        {
            if (!_dialogs.Confirm(T("Delete the draft {0} from Suno? This can't be undone.", draft.Name), T("Delete draft"))) return;
            if (await RunAsync(() => _api.DeleteLyricsDraftAsync(draft.Id)))
                Announcer.Announce(T("Deleted the draft {0}.", draft.Name));
        }

        /// <summary>Lyric co-writing: Suno changes the Lyrics the way the instruction asks; the result lands in Generated lyrics.</summary>
        [RelayCommand]
        private async Task CowriteAsync()
        {
            if (string.IsNullOrWhiteSpace(Lyrics))
            {
                Announcer.Announce(T("Put lyrics in the Lyrics box first. Write lyrics makes some from a topic."));
                return;
            }
            if (string.IsNullOrWhiteSpace(CowriteInstruction))
            {
                Announcer.Announce(T("Say what to change, for example: make the chorus rhyme."));
                return;
            }
            Announcer.Announce(T("Rewriting the lyrics."));
            await RunAsync(async () =>
            {
                GeneratedLyrics = await _api.CowriteLyricsAsync(Lyrics, CowriteInstruction.Trim(), SongTitle, Style);
                Announcer.Announce(T("New lyrics ready in Generated lyrics. Use these lyrics copies them in."));
            });
        }

        [RelayCommand]
        private void UseGeneratedLyrics()
        {
            if (string.IsNullOrWhiteSpace(GeneratedLyrics)) return;
            Lyrics = GeneratedLyrics;
            Announcer.Announce(T("Copied into Lyrics."));
        }

        // ── Trending ────────────────────────────────────────────────────────

        public ObservableCollection<SunoClip> Trending { get; } = new();

        [ObservableProperty] private SunoClip? selectedTrending;

        [RelayCommand]
        private async Task RefreshTrendingAsync()
        {
            if (!_auth.IsAuthenticated)
            {
                Announcer.Announce(T("Sign in from Settings first."));
                return;
            }
            await RunAsync(async () =>
            {
                var clips = await _api.GetTrendingAsync();
                Trending.Clear();
                foreach (var clip in clips) Trending.Add(clip);
                SelectedTrending = Trending.FirstOrDefault();
                // A song playing from the old list carries on through the new one when it's still there.
                if (_currentTrending != null) _currentTrending = Trending.FirstOrDefault(c => c.Id == _currentTrending.Id) ?? _currentTrending;
                Announcer.Announce(clips.Count == 0 ? T("Nothing trending came back.") : Plural(clips.Count, "{0} trending song.", "{0} trending songs."));
            });
        }

        private SunoClip? _currentTrending;

        [RelayCommand(AllowConcurrentExecutions = true)]
        private async Task PlayTrendingAsync(SunoClip? clip)
        {
            clip ??= SelectedTrending;
            if (clip == null) return;
            if (!clip.CanPlay)
            {
                Announcer.Announce(T("That song isn't ready yet."));
                return;
            }
            _currentTrending = clip;
            if (await RunAsync(() => _audio.PlayAsync(clip.Id, clip.DisplayTitle, this)))
                Announcer.Announce(T("Playing: {0}.", clip.DisplayTitle));
        }

        [RelayCommand]
        private void PlayPauseTrending()
        {
            if (_audio.IsPlaying) { _audio.Pause(); Announcer.Announce(T("Paused.")); }
            else if (_audio.IsPaused) { _audio.Resume(); Announcer.Announce(T("Playing.")); }
            else _ = PlayTrendingAsync(SelectedTrending);
        }

        void IPlaybackQueue.PlayPause() => PlayPauseTrending();

        Task IPlaybackQueue.NextAsync() => StepTrendingAsync(+1, T("End of list."));
        string? IPlaybackQueue.UpNextId => SongQueues.After(Trending, _currentTrending);

        Task IPlaybackQueue.PreviousAsync() => StepTrendingAsync(-1, T("First song."));

        private async Task StepTrendingAsync(int direction, string atEnd, bool quietEnd = false)
        {
            var index = _currentTrending == null ? -1 : Trending.IndexOf(_currentTrending);
            var candidates = index < 0 ? Enumerable.Empty<SunoClip>() : direction > 0 ? Trending.Skip(index + 1) : Trending.Take(index).Reverse();
            var next = candidates.FirstOrDefault(c => c.CanPlay);
            if (next == null)
            {
                Announcer.Announce(atEnd, interrupt: !quietEnd);
                return;
            }
            SelectedTrending = next;
            await PlayTrendingAsync(next);
        }

        private async void OnTrackEnded(object? sender, TrackEndedEventArgs e)
        {
            // Carry on through Trending only when Trending started this song.
            if (!ReferenceEquals(e.Owner, this) || _currentTrending?.Id != e.ClipId) return;
            await StepTrendingAsync(+1, T("End of list."), quietEnd: true);
        }
    }
}
