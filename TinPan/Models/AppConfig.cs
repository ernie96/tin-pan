using System.Text.Json.Serialization;

namespace TinPan.Models
{
    public class AppConfig
    {
        public AudioConfig Audio { get; set; } = new();
        public UserConfig User { get; set; } = new();
        public LibraryConfig Library { get; set; } = new();

        /// <summary>Everything, AskedFor or ErrorsOnly (see SpeechLevel).</summary>
        public string SpeechLevel { get; set; } = "Everything";

        /// <summary>Click, Say, ClickAndSay or Off: how a song list marks the playing song (see PlayingCueMode).</summary>
        public string PlayingCue { get; set; } = "Click";

        /// <summary>The last song played and how far in, for Resume (Ctrl+Shift+F8).</summary>
        public ResumeConfig Resume { get; set; } = new();

        /// <summary>The "mv" key of the model picked on the Create tab, reused for extending.</summary>
        public string LastModel { get; set; } = "";

        public UpdateConfig Update { get; set; } = new();
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
