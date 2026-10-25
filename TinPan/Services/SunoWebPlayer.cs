using System;
using System.Text.Json;
using System.Threading.Tasks;
using System.Windows;
using Microsoft.Web.WebView2.Core;
using Microsoft.Web.WebView2.Wpf;
using static TinPan.Localization.Loc;

namespace TinPan.Services
{
    /// <summary>
    /// Plays songs with Suno's own web player, in a signed-in browser window kept off screen.
    /// Since September 2026 Suno encrypts song audio and only its player can unlock it; streaming
    /// this way is unlimited and uses none of the account's downloads. The app drives the page's
    /// audio element (play, pause, seek, volume) and listens for its progress.
    /// </summary>
    public sealed class SunoWebPlayer : IAudioService, IDisposable
    {
        private static readonly TimeSpan StartTimeout = TimeSpan.FromSeconds(30);

        // Runs at the start of every page. On a song page it presses Suno's Play button once,
        // then reports what the audio element does.
        private const string BootScript = @"
(() => {
  if (window.__companion || !location.pathname.startsWith('/song/')) return;
  window.__companion = true;
  const post = m => { try { window.chrome.webview.postMessage(JSON.stringify(m)); } catch (e) {} };
  let hooked = null, clicks = 0, clickedAt = 0, started = false, last = 0, tries = 0;
  const vol = parseFloat((location.hash.match(/cv=([0-9.]+)/) || [])[1]);
  // The audio output Tin Pan plays on, by its Windows name; empty is the Windows default.
  let sink = decodeURIComponent((location.hash.match(/[#&]co=([^&]*)/) || [])[1] || '');
  // Tin Pan only plays here: the page may see the names of the audio devices, but never records.
  try { if (navigator.mediaDevices) navigator.mediaDevices.getUserMedia = () => Promise.reject(new DOMException('Not allowed', 'NotAllowedError')); } catch (e) {}
  const applySink = a => {
    if (!a.setSinkId) { if (sink) post({ type: 'sinkfailed', why: 'unsupported' }); return; }
    if (!sink) { a.setSinkId('').catch(() => {}); return; }
    navigator.mediaDevices.enumerateDevices().then(list => {
      const outs = list.filter(x => x.kind === 'audiooutput');
      const d = outs.find(x => x.label === sink && x.deviceId !== 'default' && x.deviceId !== 'communications') || outs.find(x => x.label === sink);
      if (!d) { post({ type: 'sinkfailed', why: outs.some(x => x.label) ? 'notfound' : 'nolabels' }); return; }
      return a.setSinkId(d.deviceId).then(() => post({ type: 'sink' }));
    }).catch(e => post({ type: 'sinkfailed', why: String((e && e.name) || e) }));
  };
  window.__tinpanSink = label => { sink = label || ''; if (hooked) applySink(hooked); };
  const hook = a => {
    if (hooked === a) return;
    hooked = a;
    if (sink) applySink(a);
    if (!isNaN(vol)) { a.volume = vol; a.addEventListener('play', () => { if (!started) a.volume = vol; }); }
    a.addEventListener('playing', () => { started = true; post({ type: 'playing', d: a.duration || 0 }); });
    a.addEventListener('pause', () => post({ type: 'time', t: a.currentTime, d: a.duration || 0, paused: true }));
    a.addEventListener('timeupdate', () => {
      const now = Date.now();
      if (now - last > 900) { last = now; post({ type: 'time', t: a.currentTime, d: a.duration || 0, paused: a.paused }); }
    });
    a.addEventListener('ended', () => post({ type: 'ended' }));
    a.addEventListener('error', () => post({ type: 'error', code: a.error ? a.error.code : 0 }));
    // Already going by the time it was found: its 'playing' event has been and gone.
    if (!a.paused && a.currentTime > 0) { started = true; post({ type: 'playing', d: a.duration || 0 }); }
  };
  const timer = setInterval(() => {
    tries++;
    const a = document.getElementById('active-audio-play');
    if (a) hook(a);
    // On a cold page Suno's player may ignore the first press, so press again until it plays.
    if (!started && a && a.paused && clicks < 6 && Date.now() - clickedAt > 4000) {
      const b = document.querySelector('button[aria-label=""Play""]');
      if (b) { clicks++; clickedAt = Date.now(); b.click(); }
    }
    if (clicks === 0 && tries > 80) { clearInterval(timer); post({ type: 'nobutton' }); }
    if (tries > 2400) clearInterval(timer);
  }, 250);
})();";

        private Window? _window;
        private WebView2? _view;
        private Task? _ready;
        private TaskCompletionSource<string>? _start;
        private string? _expectedPath;
        private int _playVersion;
        private float _volume;
        private double _position;
        private double _duration;
        private bool _paused;

        public SunoWebPlayer(IConfigService config)
        {
            _volume = Math.Clamp(config.Current.Audio.DefaultVolume, 0f, 1f);
            _output = config.Current.Audio.OutputDevice ?? "";
        }

        private string _output;
        private string? _outputFailedFor;

        public event EventHandler<string>? OutputFailed;

        /// <summary>
        /// The Windows name of the output to play on, or "" for the Windows default. Suno's page plays the song,
        /// so this is applied there (setSinkId); a song already playing moves over at once.
        /// </summary>
        public string OutputDevice
        {
            get => _output;
            set
            {
                value ??= "";
                if (value == _output) return;
                _output = value;
                _outputFailedFor = null;
                _ = ApplyOutputAsync();
            }
        }

        private async Task ApplyOutputAsync()
        {
            if (_view?.CoreWebView2 is not { } core) return;
            // Back on the Windows default, the page no longer needs to see the devices, so that's taken back.
            await AllowDeviceNamesAsync(core, _output.Length > 0);
            if (CurrentClipId == null) return;
            try
            {
                await core.ExecuteScriptAsync($"window.__tinpanSink && window.__tinpanSink({JsonSerializer.Serialize(_output)});");
            }
            catch (Exception ex) when (ex is InvalidOperationException or System.Runtime.InteropServices.COMException)
            {
                Log.Warn($"Couldn't move the song to another output: {ex.Message}");
            }
        }

        /// <summary>
        /// A page only learns which audio outputs exist, by name, with Chromium's microphone permission, so the
        /// player's suno.com gets it once you pick an output that isn't the Windows default. The page is never
        /// allowed to record: the start-up script turns getUserMedia off.
        /// </summary>
        private static async Task AllowDeviceNamesAsync(CoreWebView2 core, bool allow = true)
        {
            try
            {
                await core.Profile.SetPermissionStateAsync(CoreWebView2PermissionKind.Microphone, "https://suno.com",
                    allow ? CoreWebView2PermissionState.Allow : CoreWebView2PermissionState.Default);
            }
            catch (Exception ex) when (ex is InvalidOperationException or NotImplementedException or System.Runtime.InteropServices.COMException)
            {
                Log.Warn($"Couldn't let the player see the audio outputs: {ex.Message}");
            }
        }

        public event EventHandler<TrackEndedEventArgs>? TrackEnded;
        public event EventHandler<string>? PlaybackFailed;
        public event EventHandler? CurrentChanged;

        private string? _currentClipId;

        public string? CurrentClipId
        {
            get => _currentClipId;
            private set
            {
                if (_currentClipId == value) return;
                _currentClipId = value;
                CurrentChanged?.Invoke(this, EventArgs.Empty);
            }
        }
        public string? CurrentTitle { get; private set; }
        public IPlaybackQueue? CurrentOwner { get; private set; }
        public bool IsPlaying => CurrentClipId != null && !_paused;
        public bool IsPaused => CurrentClipId != null && _paused;

        public float Volume
        {
            get => _volume;
            set
            {
                _volume = Math.Clamp(value, 0f, 1f);
                Run($"a.volume = {_volume.ToString(System.Globalization.CultureInfo.InvariantCulture)};");
            }
        }

        public TimeSpan CurrentPosition
        {
            get => TimeSpan.FromSeconds(_position);
            set
            {
                if (CurrentClipId == null) return;
                var max = _duration > 0 ? _duration - 0.5 : value.TotalSeconds;
                _position = Math.Clamp(value.TotalSeconds, 0, Math.Max(0, max));
                Run($"a.currentTime = {_position.ToString(System.Globalization.CultureInfo.InvariantCulture)};");
            }
        }

        public TimeSpan TotalDuration => TimeSpan.FromSeconds(_duration);

        public async Task PlayAsync(string clipId, string title, IPlaybackQueue? owner = null)
        {
            Stop();
            var version = ++_playVersion;
            await EnsureReadyAsync();
            if (version != _playVersion) return;

            // A cold first load can miss; one fresh load of the page nearly always works.
            var result = await LoadAndStartAsync(clipId, version);
            if (result == "timeout" && version == _playVersion)
            {
                Log.Warn($"Player didn't start {clipId} on the first load ({await PageStateAsync()}); trying once more");
                result = await LoadAndStartAsync(clipId, version);
            }
            if (version != _playVersion) return; // replaced by another song or stopped while starting
            if (result != "playing")
            {
                var state = result == "timeout" ? $" ({await PageStateAsync()})" : "";
                Stop();
                Log.Warn($"Player didn't start {clipId}: {result}{state}");
                throw new SunoApiException(result == "nobutton"
                    ? T("Suno's player didn't offer this song. It may be private to another account, or still being made.")
                    : T("Suno's player didn't start this song. Try again."));
            }

            CurrentClipId = clipId;
            CurrentTitle = title;
            CurrentOwner = owner;
            Volume = _volume;
        }

        private async Task<string> LoadAndStartAsync(string clipId, int version)
        {
            var start = new TaskCompletionSource<string>(TaskCreationOptions.RunContinuationsAsynchronously);
            _start = start;
            _expectedPath = "/song/" + clipId;
            _position = 0;
            _duration = 0;
            _paused = false;
            var volume = _volume.ToString("0.00", System.Globalization.CultureInfo.InvariantCulture);
            var output = _output.Length == 0 ? "" : "&co=" + Uri.EscapeDataString(_output);
            _view!.CoreWebView2.Navigate($"https://suno.com/song/{Uri.EscapeDataString(clipId)}#cv={volume}{output}");

            var finished = await Task.WhenAny(start.Task, Task.Delay(StartTimeout));
            if (version != _playVersion) return "stopped";
            return finished == start.Task ? start.Task.Result : "timeout";
        }

        public void Pause()
        {
            if (CurrentClipId == null) return;
            _paused = true;
            Run("a.pause();");
        }

        public void Resume()
        {
            if (CurrentClipId == null) return;
            _paused = false;
            Run("a.play();");
        }

        public void Stop()
        {
            _playVersion++;
            _start?.TrySetResult("stopped");
            _start = null;
            _expectedPath = null;
            CurrentClipId = null;
            CurrentTitle = null;
            CurrentOwner = null;
            _position = 0;
            _duration = 0;
            _paused = false;
            if (_view?.CoreWebView2 != null) _view.CoreWebView2.Navigate("about:blank");
        }

        public async Task WarmUpAsync()
        {
            try { await EnsureReadyAsync(); }
            catch (SunoApiException ex) { Log.Warn($"Player warm-up failed: {ex.Message}"); }
        }

        public async Task ForgetSignInAsync()
        {
            Stop();
            if (_view?.CoreWebView2 != null) await WebViewEnvironment.ForgetSunoAsync(_view.CoreWebView2.CookieManager);
        }

        private Task EnsureReadyAsync() => _ready ??= CreateAsync();

        private async Task CreateAsync()
        {
            // Off screen, never activated and never in Alt+Tab, so focus and the screen reader stay in Tin Pan.
            _window = new Window
            {
                Title = T("Suno player"),
                Width = 900,
                Height = 700,
                WindowStyle = WindowStyle.None,
                ResizeMode = ResizeMode.NoResize,
                Focusable = false,
                Owner = Application.Current.MainWindow
            };
            HiddenWindow.Keep(_window);
            _view = new WebView2 { Focusable = false };
            _window.Content = _view;
            _window.Show();

            try
            {
                await _view.EnsureCoreWebView2Async(await WebViewEnvironment.GetAsync());
            }
            catch (WebView2RuntimeNotFoundException ex)
            {
                _ready = null;
                throw new SunoApiException(T("Playing needs the Microsoft Edge WebView2 Runtime. Install it from Microsoft, then try again."), null, ex);
            }

            var core = _view.CoreWebView2;
            core.Settings.AreDefaultContextMenusEnabled = false;
            core.Settings.AreDevToolsEnabled = false;
            core.Settings.IsStatusBarEnabled = false;
            await core.AddScriptToExecuteOnDocumentCreatedAsync(BootScript);
            if (_output.Length > 0) await AllowDeviceNamesAsync(core);
            core.WebMessageReceived += OnMessage;
            WebViewEnvironment.KeepOnSuno(core, "player");
        }

        private void OnMessage(object? sender, CoreWebView2WebMessageReceivedEventArgs e)
        {
            // Pages being left can still report; only the song we asked for, on suno.com, counts.
            if (_expectedPath == null || !Uri.TryCreate(e.Source, UriKind.Absolute, out var source)
                || !WebViewEnvironment.IsSuno(source)
                || !source.AbsolutePath.StartsWith(_expectedPath, StringComparison.OrdinalIgnoreCase)) return;

            string? type;
            double t = 0, d = 0;
            bool paused = false;
            string? why = null;
            try
            {
                using var doc = JsonDocument.Parse(e.TryGetWebMessageAsString());
                var root = doc.RootElement;
                type = root.TryGetProperty("type", out var ty) ? ty.GetString() : null;
                if (root.TryGetProperty("t", out var tv) && tv.ValueKind == JsonValueKind.Number) t = tv.GetDouble();
                if (root.TryGetProperty("d", out var dv) && dv.ValueKind == JsonValueKind.Number) d = dv.GetDouble();
                if (root.TryGetProperty("paused", out var pv) && pv.ValueKind is JsonValueKind.True or JsonValueKind.False) paused = pv.GetBoolean();
                if (root.TryGetProperty("why", out var wv) && wv.ValueKind == JsonValueKind.String) why = wv.GetString();
            }
            catch (Exception ex) when (ex is JsonException or ArgumentException)
            {
                return;
            }

            if (d > 0 && !double.IsInfinity(d)) _duration = d;
            switch (type)
            {
                case "playing":
                    _paused = false;
                    _start?.TrySetResult("playing");
                    break;
                case "time":
                    _position = t;
                    _paused = paused;
                    // Time moving means it's playing, even if the 'playing' event was missed.
                    if (!paused && t > 0) _start?.TrySetResult("playing");
                    break;
                case "nobutton":
                    _start?.TrySetResult("nobutton");
                    break;
                case "sinkfailed":
                    // From a page still set up for an earlier choice: the Windows default can't fail.
                    if (_output.Length == 0) break;
                    // Once per output: every song after that would only repeat it.
                    Log.Warn($"Couldn't play on \"{_output}\": {why}");
                    if (_outputFailedFor == _output) break;
                    _outputFailedFor = _output;
                    OutputFailed?.Invoke(this, why == "notfound"
                        ? T("{0} isn't connected, so the song plays on the Windows default output.", _output)
                        : T("Couldn't switch to {0}, so the song plays on the Windows default output.", _output));
                    break;
                case "error":
                    if (_start != null && !_start.Task.IsCompleted) _start.TrySetResult("error");
                    else if (CurrentClipId != null)
                    {
                        Stop();
                        PlaybackFailed?.Invoke(this, T("Playback stopped because of an error."));
                    }
                    break;
                case "ended":
                    var ended = CurrentClipId;
                    var owner = CurrentOwner;
                    if (ended == null) return;
                    Stop(); // before Suno's own player moves on to something else
                    TrackEnded?.Invoke(this, new TrackEndedEventArgs(ended, owner));
                    break;
            }
        }

        /// <summary>Runs a statement against the page's audio element, if there is one.</summary>
        /// <summary>What the page showed when the song didn't start, for the log: address, audio element, Play buttons.</summary>
        private async Task<string> PageStateAsync()
        {
            if (_view?.CoreWebView2 == null) return "no page";
            try
            {
                var json = await _view.CoreWebView2.ExecuteScriptAsync(
                    "(() => { const a = document.getElementById('active-audio-play');" +
                    " const p = document.querySelectorAll('button[aria-label=\"Play\"]').length;" +
                    " return location.pathname + ', ' + document.readyState + ', ' + (a ? 'audio paused=' + a.paused" +
                    " + ' ready=' + a.readyState + ' error=' + (a.error ? a.error.code : 'none') + ' src=' + (a.src ? 'yes' : 'no')" +
                    " : 'no audio element') + ', play buttons=' + p; })()");
                return System.Text.Json.JsonSerializer.Deserialize<string>(json) ?? "unknown";
            }
            catch (Exception ex) when (ex is System.Text.Json.JsonException or InvalidOperationException or System.Runtime.InteropServices.COMException)
            {
                return "page state unreadable";
            }
        }

        private void Run(string statement)
        {
            if (CurrentClipId == null || _view?.CoreWebView2 == null) return;
            _ = _view.CoreWebView2.ExecuteScriptAsync(
                $"(() => {{ const a = document.getElementById('active-audio-play'); if (a) {{ {statement} }} }})();");
        }

        public void Dispose()
        {
            _view?.Dispose();
            if (_window != null) HiddenWindow.Close(_window);
        }
    }
}
