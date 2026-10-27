using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading;
using System.Threading.Tasks;
using static TinPan.Localization.Loc;

namespace TinPan.Services
{
    /// <summary>
    /// Plays songs in Tin Pan itself when it can (the song file is fetched, decrypted and played with Windows'
    /// audio, which starts far sooner than Suno's page), and with Suno's own hidden player when that fails: a
    /// file Windows can't decode, Suno refusing the key, no connection to its file host. The next song in the
    /// list is fetched ahead, and the last few are kept, so going back and forth is instant.
    /// </summary>
    public sealed class HybridPlayer : IAudioService, IDisposable
    {
        private const int Kept = 4;
        private static readonly TimeSpan FetchLimit = TimeSpan.FromSeconds(15);

        private readonly SunoWebPlayer _web;
        private readonly ISunoApiService _api;
        private readonly IConfigService _config;
        private readonly DirectPlayer _direct = new();
        private readonly LinkedList<(string Id, byte[] Song)> _kept = new();
        private readonly Dictionary<string, Task<byte[]>> _fetching = new();

        private bool _usingDirect;
        private string? _clipId;
        private string? _title;
        private IPlaybackQueue? _owner;
        private int _version;
        private int _account; // goes up on signing out, so fetches from before don't land
        private float _volume;
        private string _output;

        public HybridPlayer(SunoWebPlayer web, ISunoApiService api, IConfigService config)
        {
            _web = web;
            _api = api;
            _config = config;
            _volume = web.Volume;
            _output = web.OutputDevice;
            _direct.Volume = _volume;
            _web.CurrentChanged += (s, e) => { if (!_usingDirect) CurrentChanged?.Invoke(this, e); };
            _web.TrackEnded += (s, e) => { if (!_usingDirect) TrackEnded?.Invoke(this, e); };
            _web.PlaybackFailed += (s, e) => { if (!_usingDirect) PlaybackFailed?.Invoke(this, e); };
            _web.OutputFailed += (s, e) => { if (!_usingDirect) OutputFailed?.Invoke(this, e); };
            _direct.Ended += (_, _) =>
            {
                var (ended, owner) = (_clipId, _owner);
                Clear();
                if (ended != null) TrackEnded?.Invoke(this, new TrackEndedEventArgs(ended, owner));
            };
            _direct.Failed += (_, _) =>
            {
                Clear();
                PlaybackFailed?.Invoke(this, T("Playback stopped because of an error."));
            };
        }

        public event EventHandler? CurrentChanged;
        public event EventHandler<TrackEndedEventArgs>? TrackEnded;
        public event EventHandler<string>? PlaybackFailed;
        public event EventHandler<string>? OutputFailed;

        public string? CurrentClipId => _usingDirect ? _clipId : _web.CurrentClipId;
        public string? CurrentTitle => _usingDirect ? _title : _web.CurrentTitle;
        public IPlaybackQueue? CurrentOwner => _usingDirect ? _owner : _web.CurrentOwner;
        public bool IsPlaying => _usingDirect ? !_direct.IsPaused : _web.IsPlaying;
        public bool IsPaused => _usingDirect ? _direct.IsPaused : _web.IsPaused;
        public TimeSpan TotalDuration => _usingDirect ? _direct.Duration : _web.TotalDuration;

        public TimeSpan CurrentPosition
        {
            get => _usingDirect ? _direct.Position : _web.CurrentPosition;
            set
            {
                if (_usingDirect) _direct.Position = value;
                else _web.CurrentPosition = value;
            }
        }

        public float Volume
        {
            get => _volume;
            set
            {
                _volume = Math.Clamp(value, 0f, 1f);
                _direct.Volume = _volume;
                _web.Volume = _volume;
            }
        }

        public string OutputDevice
        {
            get => _output;
            set
            {
                value ??= "";
                if (value == _output) return;
                _output = value;
                _web.OutputDevice = value;
                if (!_usingDirect) return;
                try { _direct.MoveTo(value); }
                catch (Exception ex)
                {
                    Log.Warn($"Couldn't move the song to {value}: {ex.Message}");
                    if (!_direct.IsLoaded) Clear();
                    OutputFailed?.Invoke(this, T("{0} isn't connected.", AudioOutputs.Spoken(value)));
                }
            }
        }

        public async Task PlayAsync(string clipId, string title, IPlaybackQueue? owner = null)
        {
            Stop();
            var version = _version;
            if (_config.Current.Audio.PlayDirectly)
            {
                try
                {
                    var song = await FetchAsync(clipId);
                    if (version != _version) return;
                    _direct.Volume = _volume;
                    _direct.Play(song, _output);
                    (_usingDirect, _clipId, _title, _owner) = (true, clipId, title, owner);
                    CurrentChanged?.Invoke(this, EventArgs.Empty);
                    Prefetch(owner?.UpNextId);
                    return;
                }
                catch (Exception ex) when (ex is not SunoSignInRequiredException)
                {
                    if (version != _version) return;
                    Log.Warn($"Playing {clipId} directly didn't work ({ex.GetType().Name}: {ex.Message}); using Suno's player");
                }
            }
            await _web.PlayAsync(clipId, title, owner);
            if (version == _version) Prefetch(owner?.UpNextId);
        }

        public void Pause()
        {
            if (_usingDirect) _direct.Pause();
            else _web.Pause();
        }

        public void Resume()
        {
            if (_usingDirect) _direct.Resume();
            else _web.Resume();
        }

        public void Stop()
        {
            _version++;
            if (_usingDirect)
            {
                _direct.Stop();
                Clear();
            }
            else _web.Stop();
        }

        public Task WarmUpAsync() => _web.WarmUpAsync();

        public Task ForgetSignInAsync()
        {
            // The account's songs stop and leave memory with its sign-in, fetches still under way included.
            Stop();
            _kept.Clear();
            _fetching.Clear();
            _account++;
            return _web.ForgetSignInAsync();
        }

        /// <summary>Fetches a song ahead of time so it starts at once; failures only go to the log.</summary>
        public void Prefetch(string? clipId)
        {
            if (string.IsNullOrEmpty(clipId) || !_config.Current.Audio.PlayDirectly) return;
            _ = PrefetchAsync(clipId);
        }

        private async Task PrefetchAsync(string clipId)
        {
            try { await FetchAsync(clipId); }
            catch (Exception ex) { Log.Warn($"Fetching {clipId} ahead didn't work: {ex.Message}"); }
        }

        private void Clear()
        {
            var had = _clipId != null;
            (_usingDirect, _clipId, _title, _owner) = (false, null, null, null);
            if (had) CurrentChanged?.Invoke(this, EventArgs.Empty);
        }

        private Task<byte[]> FetchAsync(string clipId)
        {
            var kept = _kept.FirstOrDefault(k => k.Id == clipId);
            if (kept.Song != null)
            {
                _kept.Remove(kept);
                _kept.AddFirst(kept);
                return Task.FromResult(kept.Song);
            }
            if (_fetching.TryGetValue(clipId, out var running)) return running;
            var task = FetchNewAsync(clipId);
            _fetching[clipId] = task;
            return task;
        }

        private async Task<byte[]> FetchNewAsync(string clipId)
        {
            var account = _account;
            try
            {
                using var limit = new CancellationTokenSource(FetchLimit);
                var song = await _api.GetSongAudioAsync(clipId, limit.Token);
                if (account != _account) throw new OperationCanceledException("Signed out while fetching");
                _kept.AddFirst((clipId, song));
                while (_kept.Count > Kept) _kept.RemoveLast();
                return song;
            }
            finally
            {
                if (account == _account) _fetching.Remove(clipId);
            }
        }

        public void Dispose() => _direct.Dispose();
    }
}
