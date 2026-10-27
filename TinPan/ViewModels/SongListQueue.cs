using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading.Tasks;
using TinPan.Models;
using TinPan.Services;
using static TinPan.Localization.Loc;

namespace TinPan.ViewModels
{
    /// <summary>
    /// The songs of the last song window (an Explore shelf, similar songs, a creator's songs), as a queue like the
    /// Library's: F9 and F7 step through them and the next one plays when a song ends. One instance, reused for each
    /// window, so nothing piles up.
    /// </summary>
    public sealed class SongListQueue : IPlaybackQueue
    {
        private readonly IAudioService _audio;
        private readonly IAnnouncer _announcer;
        private readonly Func<Func<Task>, Task<bool>> _run;
        private IReadOnlyList<SunoClip> _songs = Array.Empty<SunoClip>();
        private SunoClip? _current;

        /// <param name="run">Runs a step and says Suno's error if it fails (the view model's RunAsync).</param>
        public SongListQueue(IAudioService audio, IAnnouncer announcer, Func<Func<Task>, Task<bool>> run)
        {
            _audio = audio;
            _announcer = announcer;
            _run = run;
            _audio.TrackEnded += async (_, e) =>
            {
                if (!ReferenceEquals(e.Owner, this) || _current?.Id != e.ClipId) return;
                if (!await StepAsync(+1)) _announcer.Announce(T("End of list."), interrupt: false);
            };
        }

        /// <summary>The songs a new window shows; the song playing from an earlier one carries on through its own list until it ends.</summary>
        public void Show(IReadOnlyList<SunoClip> songs)
        {
            if (_current == null || !ReferenceEquals(_audio.CurrentOwner, this)) _songs = songs;
            _pending = songs;
        }

        private IReadOnlyList<SunoClip> _pending = Array.Empty<SunoClip>();

        public async Task PlayAsync(SunoClip clip)
        {
            if (!clip.CanPlay)
            {
                _announcer.Announce(clip.HasFailed ? T("That song failed to generate.") : T("That song isn't ready yet."));
                return;
            }
            _songs = _pending.Contains(clip) ? _pending : _songs;
            _current = clip;
            if (await _run(() => _audio.PlayAsync(clip.Id, clip.DisplayTitle, this)))
                _announcer.Announce(T("Playing: {0}.", clip.DisplayTitle));
        }

        /// <summary>Space in a song window: pause or resume what's playing, or play the selected song when nothing is.</summary>
        public Task PlayPauseAsync(SunoClip? selected)
        {
            if (_audio.IsPlaying) { _audio.Pause(); _announcer.Announce(T("Paused.")); }
            else if (_audio.IsPaused) { _audio.Resume(); _announcer.Announce(T("Playing.")); }
            else if (selected != null) return PlayAsync(selected);
            return Task.CompletedTask;
        }

        void IPlaybackQueue.PlayPause() => _ = PlayPauseAsync(null);

        public async Task NextAsync()
        {
            if (!await StepAsync(+1)) _announcer.Announce(T("End of list."));
        }

        public string? UpNextId => SongQueues.After(_songs.ToList(), _current);

        public async Task PreviousAsync()
        {
            if (!await StepAsync(-1)) _announcer.Announce(T("First song."));
        }

        private async Task<bool> StepAsync(int direction)
        {
            var list = _songs.ToList();
            var index = _current == null ? -1 : list.IndexOf(_current);
            if (index < 0) return false;
            var next = (direction > 0 ? list.Skip(index + 1) : list.Take(index).Reverse()).FirstOrDefault(c => c.CanPlay);
            if (next == null) return false;
            await PlayAsync(next);
            return true;
        }
    }
}
