using System.Collections.Generic;
using System.Text.Json.Serialization;

namespace TinPan.Models
{
    public class AppConfig
    {
        public AudioConfig Audio { get; set; } = new();
        public UserConfig User { get; set; } = new();
        public LibraryConfig Library { get; set; } = new();

        /// <summary>"" follows Windows; otherwise "en", "ro", "uk" or "es". Takes effect at the next start.</summary>
        public string Language { get; set; } = "";

        /// <summary>Everything, AskedFor or ErrorsOnly (see SpeechLevel).</summary>
        public string SpeechLevel { get; set; } = "Everything";

        /// <summary>Click, Say, ClickAndSay or Off: how a song list marks the playing song (see PlayingCueMode).</summary>
        public string PlayingCue { get; set; } = "Click";

        /// <summary>The last song played and how far in, for Resume (Ctrl+Shift+F8).</summary>
        public ResumeConfig Resume { get; set; } = new();

        /// <summary>The "mv" key of the model picked on the Create tab, reused for extending.</summary>
        public string LastModel { get; set; } = "";

        public UpdateConfig Update { get; set; } = new();

        public CleanupConfig Cleanup { get; set; } = new();
    }

    /// <summary>
    /// Clean-up with sunscreen (abradburne/sunscreen). The stage switches and values are sunscreen's
    /// own options; the defaults are its recommended chain and its built-in values.
    /// </summary>
    public class CleanupConfig
    {
        /// <summary>Clean up after Download MP3, Download WAV and Download all. Plain Download asks instead.</summary>
        public bool AfterDownload { get; set; }

        /// <summary>Keep the downloaded file next to the cleaned-up WAV.</summary>
        public bool KeepOriginal { get; set; } = true;

        /// <summary>Remove the 5 ms decoder buzz (sunscreen does it only when it finds it). Off = --no-debuzz.</summary>
        public bool Debuzz { get; set; } = true;

        /// <summary>Only remove haze you could hear (hearing model). Off = --no-gate.</summary>
        public bool Gate { get; set; } = true;

        /// <summary>--smooth=ms: steady the flutter of the remaining haze.</summary>
        public bool Smooth { get; set; } = true;
        public double SmoothMs { get; set; } = 85;

        /// <summary>--widen=amount: give the highs the track's own stereo width (1 = the same, 0.5 = half).</summary>
        public bool Widen { get; set; } = true;
        public double WidenAmount { get; set; } = 1.0;

        /// <summary>--excite=dB: rebuild the top octave above the MP3 cut-off, this many dB from the slope.</summary>
        public bool Excite { get; set; } = true;
        public double ExciteDb { get; set; } = -3;

        /// <summary>--match[=file]: nudge the tone toward commercial masters, or toward this MP3 or WAV.</summary>
        public bool Match { get; set; } = true;
        public string MatchReference { get; set; } = "";

        /// <summary>--master=LUFS: set the loudness, with a true-peak limiter at -1 dBTP.</summary>
        public bool Master { get; set; } = true;
        public double MasterLufs { get; set; } = -14;
    }

    public class UpdateConfig
    {
        /// <summary>Look for a new version a few seconds after starting; quiet unless there is one.</summary>
        public bool CheckAtStartup { get; set; } = true;

        /// <summary>A version the user said No to when offered at startup; not offered again by itself.</summary>
        public string SkippedVersion { get; set; } = "";
    }

    public class AudioConfig
    {
        /// <summary>"mp3" or "wav". WAV asks Suno to convert first, so it is slower.</summary>
        public string OutputFormat { get; set; } = "mp3";
        public string DownloadDir { get; set; } = "";
        public float DefaultVolume { get; set; } = 0.8f;

        /// <summary>Play songs in Tin Pan itself (fast); Suno's own hidden player is the fallback, and the only player when off.</summary>
        public bool PlayDirectly { get; set; } = true;

        /// <summary>The output songs play on, by its Windows name; empty is the Windows default.</summary>
        public string OutputDevice { get; set; } = "";

        /// <summary>A second output Ctrl+Shift+F12 switches to and back (external speakers, say); empty is none.</summary>
        public string SecondOutputDevice { get; set; } = "";

        /// <summary>Read timed lyrics aloud as they're sung (Shift+F3). Off to start with.</summary>
        public bool ReadLyricsLive { get; set; }

        // Extra files written next to each download.
        public bool SaveLyrics { get; set; }
        public bool SaveTimedLyrics { get; set; }
        public bool SaveDetails { get; set; }
        public bool SaveCover { get; set; }
    }

    /// <summary>The Library's filters, kept between runs.</summary>
    public class LibraryConfig
    {
        /// <summary>Empty means every workspace.</summary>
        public string WorkspaceId { get; set; } = "";
        public string WorkspaceName { get; set; } = "";

        /// <summary>"False" hides disliked songs, "Any" shows them, "True" shows only them (Suno's own values).</summary>
        public string Disliked { get; set; } = "False";

        /// <summary>The other filters, by Suno field name (see FeedFilters.All): "Any", "True" or "False".</summary>
        public Dictionary<string, string> Show { get; set; } = new();

        /// <summary>newest, oldest, most-liked or least-liked. Null in settings from before 1.5.</summary>
        public string? Sort { get; set; }

        // Settings from before 1.5, read once into Show and Sort.
        public bool IncludeTrashed { get; set; }
        public bool HideStems { get; set; } = true;
        public bool NewestFirst { get; set; } = true;
    }

    public class ResumeConfig
    {
        public string ClipId { get; set; } = "";
        public string Title { get; set; } = "";
        public double Seconds { get; set; }
    }

    public class UserConfig
    {
        /// <summary>Suno and Clerk cookies, encrypted with Windows DPAPI for this Windows account.</summary>
        public string ProtectedCookies { get; set; } = "";

        public string DeviceId { get; set; } = "";

        /// <summary>Set after Sign out, so the next sign-in window starts with no saved browser session.</summary>
        public bool ClearBrowserOnNextSignIn { get; set; }

        // Plain-text fields written by earlier versions. Read once, migrated, then dropped.
        [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
        public string? SessionToken { get; set; }

        [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
        public string? CookieUat { get; set; }
    }
}
