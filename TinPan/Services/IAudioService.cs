using System;
using System.Threading.Tasks;

namespace TinPan.Services
{
    /// <summary>A list that can play its songs in order: the Library or a playlist.</summary>
    public interface IPlaybackQueue
    {
        Task PreviousAsync();
        Task NextAsync();

        /// <summary>Pause, resume, or start the list's selected song when nothing is playing.</summary>
        void PlayPause();
    }

    public sealed record TrackEndedEventArgs(string ClipId, IPlaybackQueue? Owner);

    public interface IAudioService
    {
        /// <summary>0 to 1. Kept across tracks.</summary>
        float Volume { get; set; }
        bool IsPlaying { get; }
        bool IsPaused { get; }

        /// <summary>Suno id of the song playing or paused, null when stopped.</summary>
        string? CurrentClipId { get; }
        string? CurrentTitle { get; }

        /// <summary>Updated about once a second while playing; setting it seeks.</summary>
        TimeSpan CurrentPosition { get; set; }
        TimeSpan TotalDuration { get; }

        /// <summary>
        /// Stops whatever is playing and starts this song. owner is the list it came from, if any;
        /// only that list carries on when it ends. Throws SunoApiException if it won't start.
        /// </summary>
        Task PlayAsync(string clipId, string title, IPlaybackQueue? owner = null);

        /// <summary>The list the current song came from, or null for one-off plays (Trending, Similar songs).</summary>
        IPlaybackQueue? CurrentOwner { get; }
        void Pause();
        void Resume();
        void Stop();

        /// <summary>Starts the hidden browser ahead of time, so the first song starts quickly.</summary>
        Task WarmUpAsync();

        /// <summary>Signs the player out (after Sign out).</summary>
        Task ForgetSignInAsync();

        /// <summary>CurrentClipId changed: a song started, or playback stopped. Raised on the UI thread.</summary>
        event EventHandler CurrentChanged;

        /// <summary>A song reached its end by itself. Raised on the UI thread.</summary>
        event EventHandler<TrackEndedEventArgs> TrackEnded;

        /// <summary>Playback broke off. Raised on the UI thread with a speakable message.</summary>
        event EventHandler<string> PlaybackFailed;

        /// <summary>The Windows name of the output to play on, or "" for the Windows default.</summary>
        string OutputDevice { get; set; }

        /// <summary>The chosen output couldn't be used (unplugged, say); the message says so, once per output.</summary>
        event EventHandler<string>? OutputFailed;
    }
}
