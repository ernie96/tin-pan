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

namespace TinPan.ViewModels
{
    public partial class SunoPanelViewModel : ViewModelBase
    {
        public static readonly IReadOnlyList<string> VocalChoices = new[] { "Any voice", "Male vocals", "Female vocals" };

        private readonly ISunoApiService _api;
        private readonly IAuthService _auth;
        private readonly IAudioService _audio;
        private readonly IConfigService _config;
        private readonly SongWatcher _watcher;

        public SunoPanelViewModel(ISunoApiService api, IAuthService auth, IAudioService audio, IConfigService config,
            SongWatcher watcher, AccountState account, IAnnouncer announcer) : base(announcer)
        {
            _api = api;
            _auth = auth;
            _audio = audio;
            _config = config;
            _watcher = watcher;
            Account = account;
            Account.PropertyChanged += OnAccountChanged;
        }

        public AccountState Account { get; }

        /// <summary>Songs that were just started from this tab, for the library to show.</summary>
        public event EventHandler<IReadOnlyList<SunoClip>>? SongsStarted;

        // ── Account ─────────────────────────────────────────────────────────

        [ObservableProperty] private string accountText = "Not signed in.";

        public ObservableCollection<Notification> Notifications { get; } = new();

        private void OnAccountChanged(object? sender, PropertyChangedEventArgs e)
        {
            if (e.PropertyName != nameof(AccountState.Billing)) return;
            AccountText = Account.Billing == null ? "Not signed in." : $"{Account.SubscriptionText} {BillingText.Features(Account.Billing)}".Trim();
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

        public async Task LoadNotificationsAsync(bool background)
        {
            if (!_auth.IsAuthenticated) return;
            await RunAsync(background, async () =>
            {
                var items = await _api.GetNotificationsAsync();
                Notifications.Clear();
                foreach (var n in items) Notifications.Add(n);
                if (!background) Announcer.Announce(items.Count == 0 ? "No notifications." : $"{items.Count} notifications.");
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

        public static readonly Persona NoPersona = new("", "No persona");

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
                Announcer.Announce("Describe the song first.");
                return Task.CompletedTask;
            }
            return StartSongsAsync(GenerateRequest.Modes.Inspiration);
        }

        [RelayCommand]
        private Task CreateCustomAsync()
        {
            if (string.IsNullOrWhiteSpace(Lyrics) && !IsInstrumental)
            {
                Announcer.Announce("Add lyrics, or tick Instrumental.");
                return Task.CompletedTask;
            }
            return StartSongsAsync(GenerateRequest.Modes.Custom);
        }

        /// <summary>Builds the request from the Create fields. Returns null and says why when something is off.</summary>
        public GenerateRequest? BuildRequest(string mode)
        {
            if (SelectedModel == null)
            {
                Announcer.Announce("Models haven't loaded yet. Press F5 to refresh, then try again.");
                return null;
            }

            var weird = ParsePercent(Weirdness, "Weirdness");
            var influence = ParsePercent(StyleInfluence, "Style influence");
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
                ("Title", r.Title, limits.Title),
                ("Style", r.Tags, limits.Tags),
                ("Exclude styles", r.NegativeTags, limits.NegativeTags),
                (inspiration ? "Description" : "Lyrics", r.Prompt, inspiration ? limits.Description : limits.Prompt)
            };
            foreach (var (name, value, limit) in checks)
                if (limit > 0 && value.Length > limit)
                    return $"{name} is {value.Length} characters. This model allows {limit}.";
            return null;
        }

        /// <summary>Empty means Suno chooses (null). NaN means the text was wrong; that has been announced.</summary>
        private double? ParsePercent(string text, string name)
        {
            if (string.IsNullOrWhiteSpace(text)) return null;
            var value = NumberField.Parse(text.Trim(), isTime: false);
            if (value is >= 0 and <= 100) return value;
            Announcer.Announce($"{name} must be a number from 0 to 100, or empty.");
            return double.NaN;
        }

        private async Task StartSongsAsync(string mode)
        {
            if (!_auth.IsAuthenticated)
            {
                Announcer.Announce("Sign in from Settings first.");
                return;
            }
            var request = BuildRequest(mode);
            if (request == null) return;

            Announcer.Announce("Sending to Suno.");
            await RunAsync(async () =>
            {
                var clips = await _api.GenerateAsync(request);
                SongsStarted?.Invoke(this, clips);
                _watcher.Watch(clips);
                Announcer.Announce(clips.Count == 1
                    ? "Making 1 song. You'll hear when it's ready."
                    : $"Making {clips.Count} songs. You'll hear when they're ready.");
                _ = LoadAccountAsync(background: true);
            });
        }

        [RelayCommand]
        private async Task WriteLyricsAsync()
        {
            if (string.IsNullOrWhiteSpace(LyricsTopic))
            {
                Announcer.Announce("Type a topic first.");
                return;
            }
            Announcer.Announce("Writing lyrics.");
            await RunAsync(async () =>
            {
                var result = await _api.GenerateLyricsAsync(LyricsTopic.Trim());
                GeneratedLyrics = result.Text;
                if (string.IsNullOrWhiteSpace(SongTitle) && !string.IsNullOrWhiteSpace(result.Title)) SongTitle = result.Title;
                Announcer.Announce("Lyrics ready.");
            });
        }

        /// <summary>Style Boost: Suno rewrites the Style field into a fuller description, using the lyrics as a hint.</summary>
        [RelayCommand]
        private async Task BoostStyleAsync()
        {
            if (string.IsNullOrWhiteSpace(Style))
            {
                Announcer.Announce("Type a style first, even a word or two.");
                return;
            }
            Announcer.Announce("Boosting the style.");
            await RunAsync(async () =>
            {
                Style = await _api.BoostStyleAsync(Style.Trim(), IsInstrumental ? null : Lyrics, IsInstrumental, null);
                Announcer.Announce("Style boosted: " + Style);
            });
        }

        /// <summary>Lyric co-writing: Suno changes the Lyrics the way the instruction asks; the result lands in Generated lyrics.</summary>
        [RelayCommand]
        private async Task CowriteAsync()
        {
            if (string.IsNullOrWhiteSpace(Lyrics))
            {
                Announcer.Announce("Put lyrics in the Lyrics box first. Write lyrics makes some from a topic.");
                return;
            }
            if (string.IsNullOrWhiteSpace(CowriteInstruction))
            {
                Announcer.Announce("Say what to change, for example: make the chorus rhyme.");
                return;
            }
            Announcer.Announce("Rewriting the lyrics.");
            await RunAsync(async () =>
            {
                GeneratedLyrics = await _api.CowriteLyricsAsync(Lyrics, CowriteInstruction.Trim(), SongTitle, Style);
                Announcer.Announce("New lyrics ready in Generated lyrics. Use these lyrics copies them in.");
            });
        }

        [RelayCommand]
        private void UseGeneratedLyrics()
        {
            if (string.IsNullOrWhiteSpace(GeneratedLyrics)) return;
            Lyrics = GeneratedLyrics;
            Announcer.Announce("Copied into Lyrics.");
        }

        // ── Trending ────────────────────────────────────────────────────────

        public ObservableCollection<SunoClip> Trending { get; } = new();

        [ObservableProperty] private SunoClip? selectedTrending;

        [RelayCommand]
        private async Task RefreshTrendingAsync()
        {
            if (!_auth.IsAuthenticated)
            {
                Announcer.Announce("Sign in from Settings first.");
                return;
            }
            await RunAsync(async () =>
            {
                var clips = await _api.GetTrendingAsync();
                Trending.Clear();
                foreach (var clip in clips) Trending.Add(clip);
                SelectedTrending = Trending.FirstOrDefault();
                Announcer.Announce(clips.Count == 0 ? "Nothing trending came back." : $"{clips.Count} trending songs.");
            });
        }

        [RelayCommand]
        private async Task PlayTrendingAsync(SunoClip? clip)
        {
            clip ??= SelectedTrending;
            if (clip == null) return;
            if (!clip.CanPlay)
            {
                Announcer.Announce("That song isn't ready yet.");
                return;
            }
            if (await RunAsync(() => _audio.PlayAsync(clip.Id, clip.DisplayTitle)))
                Announcer.Announce($"Playing: {clip.DisplayTitle}.");
        }
    }
}
