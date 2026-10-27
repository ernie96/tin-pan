using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading.Tasks;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using TinPan.Services;
using TinPan.Views;
using static TinPan.Localization.Loc;

namespace TinPan.ViewModels
{
    public partial class SettingsPanelViewModel : ViewModelBase
    {
        private readonly IConfigService _config;
        private readonly IAuthService _auth;
        private readonly IDialogService _dialogs;
        private readonly IAudioService _audio;
        private readonly LiveLyrics _liveLyrics;
        private readonly OutputSwitch _outputs;
        private bool _syncingOutputs;

        [ObservableProperty] private string signInText = T("Not signed in.");
        [ObservableProperty] private bool isSignedIn;
        [ObservableProperty] private string downloadDir;
        [ObservableProperty] private string outputFormat;
        [ObservableProperty] private bool saveLyrics;
        [ObservableProperty] private bool saveTimedLyrics;
        [ObservableProperty] private bool saveDetails;
        [ObservableProperty] private bool saveCover;
        [ObservableProperty] private bool playDirectly;
        [ObservableProperty] private bool checkUpdatesAtStartup;
        [ObservableProperty] private bool readLyricsLive;
        [ObservableProperty] private IReadOnlyList<string> outputChoices = Array.Empty<string>();
        [ObservableProperty] private IReadOnlyList<string> secondOutputChoices = Array.Empty<string>();
        [ObservableProperty] private string? mainOutput;
        [ObservableProperty] private string? secondOutput;

        // The Windows names behind the choices; "" is the Windows default and null is none.
        private List<string> _outputNames = new();
        private List<string?> _secondNames = new();

        public IReadOnlyList<string> OutputFormats { get; } = new[] { "mp3", "wav" };

        private static readonly (SpeechLevel Level, string Label)[] SpeechLevelChoices =
        {
            (Services.SpeechLevel.Everything, T("Everything")),
            (Services.SpeechLevel.AskedFor, T("What I ask for and errors, not background news")),
            (Services.SpeechLevel.ErrorsOnly, T("Only errors and what I ask to hear"))
        };

        public IReadOnlyList<string> SpeechLevels { get; } = SpeechLevelChoices.Select(c => c.Label).ToList();

        [ObservableProperty] private string speechLevel = SpeechLevelChoices[0].Label;

        public static string SpeechLevelLabel(SpeechLevel level) => SpeechLevelChoices.First(c => c.Level == level).Label;

        partial void OnSpeechLevelChanged(string value)
        {
            var level = SpeechLevelChoices.FirstOrDefault(c => c.Label == value).Level;
            Announcer.Level = level;
            _config.Current.SpeechLevel = level.ToString();
            _config.SaveSoon();
        }

        private static readonly (PlayingCueMode Mode, string Label)[] PlayingCueChoices =
        {
            (PlayingCueMode.Click, T("A short click")),
            (PlayingCueMode.Say, T("Say playing (also shown in braille)")),
            (PlayingCueMode.ClickAndSay, T("Click and say playing")),
            (PlayingCueMode.Off, T("Nothing"))
        };

        public IReadOnlyList<string> PlayingCues { get; } = PlayingCueChoices.Select(c => c.Label).ToList();

        [ObservableProperty] private string playingCue = PlayingCueChoices[0].Label;

        partial void OnPlayingCueChanged(string value)
        {
            var mode = PlayingCueChoices.FirstOrDefault(c => c.Label == value).Mode;
            Views.PlayingCue.Instance.Mode = mode;
            _config.Current.PlayingCue = mode.ToString();
            _config.SaveSoon();
        }

        private readonly IReadOnlyList<(string Code, string Label)> _languages = Localization.Loc.Choices();

        /// <summary>Each language in its own words, and in the current language's too.</summary>
        public IReadOnlyList<string> Languages => _languages.Select(l => l.Label).ToList();

        [ObservableProperty] private string language = "";

        partial void OnLanguageChanged(string value)
        {
            var code = _languages.FirstOrDefault(l => l.Label == value).Code ?? "";
            if (code == _config.Current.Language) return;
            _config.Current.Language = code;
            _config.SaveSoon();
            Announcer.Announce(T("The language changes the next time Tin Pan starts."));
        }

        public SettingsPanelViewModel(IConfigService config, IAuthService auth, IDialogService dialogs, IAudioService audio, LiveLyrics liveLyrics, OutputSwitch outputs, IAnnouncer announcer)
            : base(announcer)
        {
            _config = config;
            _auth = auth;
            _dialogs = dialogs;
            _audio = audio;
            _liveLyrics = liveLyrics;
            _outputs = outputs;
            RefreshOutputs();
            readLyricsLive = liveLyrics.Enabled;
            downloadDir = config.Current.Audio.DownloadDir;
            outputFormat = OutputFormats.Contains(config.Current.Audio.OutputFormat) ? config.Current.Audio.OutputFormat : "mp3";
            speechLevel = SpeechLevelLabel(Enum.TryParse<SpeechLevel>(config.Current.SpeechLevel, out var lvl) ? lvl : Services.SpeechLevel.Everything);
            var cue = Enum.TryParse<PlayingCueMode>(config.Current.PlayingCue, out var m) ? m : PlayingCueMode.Click;
            Views.PlayingCue.Instance.Mode = cue;
            playingCue = PlayingCueChoices.First(c => c.Mode == cue).Label;
            language = (_languages.FirstOrDefault(l => l.Code == config.Current.Language).Label) ?? _languages[0].Label;
            var downloads = config.Current.Audio;
            saveLyrics = downloads.SaveLyrics;
            saveTimedLyrics = downloads.SaveTimedLyrics;
            saveDetails = downloads.SaveDetails;
            saveCover = downloads.SaveCover;
            playDirectly = downloads.PlayDirectly;
            checkUpdatesAtStartup = config.Current.Update.CheckAtStartup;
            _auth.AuthStateChanged += (_, _) => UpdateSignInText();
        }

        private void UpdateSignInText()
        {
            IsSignedIn = _auth.IsAuthenticated;
            SignInText = !_auth.IsAuthenticated ? T("Not signed in.")
                : string.IsNullOrEmpty(_auth.UserName) ? T("Signed in.") : T("Signed in as {0}.", _auth.UserName);
        }

        [RelayCommand]
        private async Task SignInAsync()
        {
            if (await _auth.SignInAsync()) Announcer.Announce(SignInText);
            else Announcer.Announce(T("Not signed in."));
        }

        [RelayCommand]
        private async Task SignOutAsync()
        {
            if (!_auth.IsAuthenticated) return;
            _auth.SignOut();
            await _audio.ForgetSignInAsync();
            Announcer.Announce(T("Signed out."));
        }

        [RelayCommand]
        private void Browse()
        {
            var picked = _dialogs.PickFolder(string.IsNullOrEmpty(DownloadDir) ? null : DownloadDir);
            if (picked == null) return;
            DownloadDir = picked;
            Announcer.Announce(T("Download folder set."));
        }

        partial void OnDownloadDirChanged(string value)
        {
            _config.Current.Audio.DownloadDir = value.Trim();
            _config.SaveSoon();
        }

        partial void OnSaveLyricsChanged(bool value) { _config.Current.Audio.SaveLyrics = value; _config.SaveSoon(); }
        partial void OnSaveTimedLyricsChanged(bool value) { _config.Current.Audio.SaveTimedLyrics = value; _config.SaveSoon(); }
        partial void OnSaveDetailsChanged(bool value) { _config.Current.Audio.SaveDetails = value; _config.SaveSoon(); }
        partial void OnSaveCoverChanged(bool value) { _config.Current.Audio.SaveCover = value; _config.SaveSoon(); }
        partial void OnPlayDirectlyChanged(bool value) { _config.Current.Audio.PlayDirectly = value; _config.SaveSoon(); }
        partial void OnReadLyricsLiveChanged(bool value) => _liveLyrics.Enabled = value;

        /// <summary>Lists the outputs Windows has now, keeping a saved one that isn't plugged in.</summary>
        public void RefreshOutputs()
        {
            _syncingOutputs = true;
            try
            {
                var connected = AudioOutputs.List();
                _outputNames = new List<string> { "" };
                _outputNames.AddRange(connected);
                if (_outputs.Main.Length > 0 && !_outputNames.Contains(_outputs.Main)) _outputNames.Add(_outputs.Main);
                _secondNames = new List<string?> { null, "" };
                _secondNames.AddRange(connected);
                if (_outputs.Second is { Length: > 0 } second && !_secondNames.Contains(second)) _secondNames.Add(second);

                OutputChoices = _outputNames.Select(AudioOutputs.Spoken).ToList();
                SecondOutputChoices = _secondNames.Select(n => n == null ? T("None") : AudioOutputs.Spoken(n)).ToList();
                MainOutput = OutputChoices[Math.Max(0, _outputNames.IndexOf(_outputs.Main))];
                SecondOutput = SecondOutputChoices[Math.Max(0, _secondNames.IndexOf(_outputs.Second))];
            }
            finally
            {
                _syncingOutputs = false;
            }
        }

        partial void OnMainOutputChanged(string? value)
        {
            if (_syncingOutputs || value == null) return;
            var index = OutputChoices.ToList().IndexOf(value);
            if (index >= 0) _outputs.SetMain(_outputNames[index]);
        }

        partial void OnSecondOutputChanged(string? value)
        {
            if (_syncingOutputs || value == null) return;
            var index = SecondOutputChoices.ToList().IndexOf(value);
            if (index >= 0) _outputs.SetSecond(_secondNames[index]);
        }
        partial void OnCheckUpdatesAtStartupChanged(bool value) { _config.Current.Update.CheckAtStartup = value; _config.SaveSoon(); }

        partial void OnOutputFormatChanged(string value)
        {
            _config.Current.Audio.OutputFormat = value;
            _config.SaveSoon();
        }

    }
}
