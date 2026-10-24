using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.Text.RegularExpressions;
using System.Threading.Tasks;
using System.Windows.Threading;
using TinPan.Models;

namespace TinPan.Services
{
    /// <summary>
    /// Reads a song's timed lyrics aloud, line by line, as they're sung (Shift+F3, off to start with).
    /// Uses Suno's word timings, the same ones the LRC download is made from.
    /// </summary>
    public sealed class LiveLyrics
    {
        private readonly IAudioService _audio;
        private readonly ISunoApiService _api;
        private readonly IAnnouncer _announcer;
        private readonly IConfigService _config;
        private readonly DispatcherTimer _timer = new() { Interval = TimeSpan.FromMilliseconds(100) };
        private readonly Stopwatch _sinceReport = new();
        private LyricsCursor? _cursor;
        private string? _loadedFor;
        private int _version;
        private double _lastReported = -1;

        public LiveLyrics(IAudioService audio, ISunoApiService api, IAnnouncer announcer, IConfigService config)
        {
            _audio = audio;
            _api = api;
            _announcer = announcer;
            _config = config;
            _timer.Tick += (_, _) => Tick();
            _audio.CurrentChanged += (_, _) => { if (Enabled) _ = LoadAsync(); };
        }

        public bool Enabled
        {
            get => _config.Current.Audio.ReadLyricsLive;
            set
            {
                if (Enabled == value) return;
                _config.Current.Audio.ReadLyricsLive = value;
                _config.SaveSoon();
                if (value) _ = LoadAsync();
                else Clear();
            }
        }

        /// <summary>
        /// Gets the playing song's timed lyrics and starts reading them. Returns how many lines it has,
        /// or null when nothing is playing.
        /// </summary>
        public async Task<int?> LoadAsync()
        {
            var clipId = _audio.CurrentClipId;
            if (clipId == null)
            {
                Clear();
                return null;
            }
            if (clipId == _loadedFor && _cursor != null) return _cursor.Count;

            Clear();
            var version = ++_version;
            IReadOnlyList<LyricLine> lines;
            try
            {
                var aligned = await _api.GetAlignedLyricsAsync(clipId);
                lines = LyricLine.From(aligned?.Words ?? new());
            }
            catch (SunoApiException ex)
            {
                Log.Warn($"Timed lyrics for {clipId} not loaded: {ex.Message}");
                lines = Array.Empty<LyricLine>();
            }
            if (version != _version || !Enabled || _audio.CurrentClipId != clipId) return null;

            _loadedFor = clipId;
            _cursor = new LyricsCursor(lines);
            if (lines.Count > 0) _timer.Start();
            return lines.Count;
        }

        private void Clear()
        {
            _version++;
            _timer.Stop();
            _cursor = null;
            _loadedFor = null;
            _lastReported = -1;
        }

        private void Tick()
        {
            if (_cursor == null || _audio.CurrentClipId != _loadedFor) return;
            // The player reports its time about once a second; count on from the last report in between.
            var reported = _audio.CurrentPosition.TotalSeconds;
            if (reported != _lastReported)
            {
                _lastReported = reported;
                _sinceReport.Restart();
            }
            var position = _audio.IsPlaying ? reported + Math.Min(_sinceReport.Elapsed.TotalSeconds, 1.5) : reported;
            var line = _cursor.Advance(position);
            if (line != null) _announcer.Announce(line, interrupt: true, always: true);
        }
    }

    /// <summary>One sung line and when it starts, in seconds.</summary>
    public sealed partial record LyricLine(double Start, string Text)
    {
        private const int WordsPerLineWithoutBreaks = 8;

        /// <summary>Groups Suno's word timings into lines: at its line breaks, or every 8 words when it has none.</summary>
        public static IReadOnlyList<LyricLine> From(IReadOnlyList<AlignedWord> words)
        {
            var lines = new List<LyricLine>();
            var hasBreaks = words.Any(w => w.Word.Contains('\n'));
            var line = new System.Text.StringBuilder();
            double? start = null;
            var count = 0;

            void Flush()
            {
                var text = line.ToString().Trim();
                if (text.Length > 0 && start != null) lines.Add(new LyricLine(start.Value, text));
                line.Clear();
                start = null;
                count = 0;
            }

            foreach (var w in words)
            {
                var parts = w.Word.Replace("\r", "").Split('\n');
                for (var i = 0; i < parts.Length; i++)
                {
                    if (i > 0) Flush();
                    if (parts[i].Trim().Length == 0) continue;
                    start ??= w.Start;
                    line.Append(parts[i]);
                    if (!parts[i].EndsWith(' ')) line.Append(' ');
                    count++;
                }
                if (!hasBreaks && count >= WordsPerLineWithoutBreaks) Flush();
            }
            Flush();
            return lines;
        }

        /// <summary>The line without section tags such as [Verse 1], for speaking; empty when it was only a tag.</summary>
        public string Spoken => Whitespace().Replace(Tag().Replace(Text, " "), " ").Trim();

        [GeneratedRegex(@"\[[^\]]*\]")]
        private static partial Regex Tag();

        [GeneratedRegex(@"\s+")]
        private static partial Regex Whitespace();
    }

    /// <summary>Which line is being sung at a time in the song; says each one once, and only the newest after a jump.</summary>
    public sealed class LyricsCursor
    {
        /// <summary>Speech takes a moment to start, so lines are said this much early.</summary>
        public const double Lead = 0.15;

        private readonly IReadOnlyList<LyricLine> _lines;
        private int _index = -1;

        public LyricsCursor(IReadOnlyList<LyricLine> lines) => _lines = lines;

        public int Count => _lines.Count;

        /// <summary>The line to say now, or null when the line hasn't changed (or has nothing to say).</summary>
        public string? Advance(double position)
        {
            var index = -1;
            for (var i = 0; i < _lines.Count && _lines[i].Start <= position + Lead; i++) index = i;
            if (index == _index) return null;
            _index = index;
            if (index < 0) return null;
            var spoken = _lines[index].Spoken;
            return spoken.Length > 0 ? spoken : null;
        }
    }
}
