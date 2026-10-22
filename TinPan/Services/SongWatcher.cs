using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading.Tasks;
using TinPan.Models;

namespace TinPan.Services
{
    /// <summary>
    /// Follows songs Suno is still making (new songs, extensions, stems) until they finish,
    /// so the library can update them and the user hears one line when they are ready.
    /// Events are raised on the thread that called Watch (the UI thread).
    /// </summary>
    public sealed class SongWatcher
    {
        private static readonly TimeSpan Interval = TimeSpan.FromSeconds(5);
        private static readonly TimeSpan GiveUpAfter = TimeSpan.FromMinutes(10);
        private const int MaxFailuresInARow = 5;

        private readonly ISunoApiService _api;
        private readonly Dictionary<string, DateTime> _pending = new();
        private bool _running;

        public SongWatcher(ISunoApiService api) => _api = api;

        /// <summary>Latest state of watched songs, finished or not.</summary>
        public event EventHandler<IReadOnlyList<SunoClip>>? ClipsUpdated;

        /// <summary>Songs that are done (ready or failed) since the last check.</summary>
        public event EventHandler<IReadOnlyList<SunoClip>>? ClipsFinished;

        public void Watch(IEnumerable<SunoClip> clips)
        {
            foreach (var clip in clips.Where(c => !IsDone(c)))
                _pending.TryAdd(clip.Id, DateTime.UtcNow);
            if (!_running && _pending.Count > 0) _ = RunAsync();
        }

        public void Clear() => _pending.Clear();

        // "streaming" can already be played, but only "complete" is the finished song.
        private static bool IsDone(SunoClip c) => c.Status == "complete" || c.HasFailed;

        private async Task RunAsync()
        {
            _running = true;
            var failures = 0;
            try
            {
                while (_pending.Count > 0)
                {
                    await Task.Delay(Interval);
                    if (_pending.Count == 0) break;

                    IReadOnlyList<SunoClip> clips;
                    try
                    {
                        clips = await _api.GetClipsAsync(_pending.Keys.ToList());
                        failures = 0;
                    }
                    catch (SunoApiException ex)
                    {
                        Log.Warn($"Checking on new songs failed: {ex.Message}");
                        if (ex is SunoSignInRequiredException || ++failures >= MaxFailuresInARow)
                        {
                            _pending.Clear();
                            break;
                        }
                        continue;
                    }

                    ClipsUpdated?.Invoke(this, clips);

                    var finished = clips.Where(IsDone).ToList();
                    foreach (var clip in finished) _pending.Remove(clip.Id);
                    foreach (var stale in _pending.Where(p => DateTime.UtcNow - p.Value > GiveUpAfter).Select(p => p.Key).ToList())
                        _pending.Remove(stale);

                    if (finished.Count > 0) ClipsFinished?.Invoke(this, finished);
                }
            }
            finally
            {
                _running = false;
            }
        }
    }
}
