using System;
using System.Collections.Generic;
using static TinPan.Localization.Loc;

namespace TinPan.Models
{
    /// <summary>A Suno creator's page: GET /api/profiles/{handle}, one page of their public songs.</summary>
    public sealed record CreatorPage(string Handle, string DisplayName, string Description, int TotalSongs, int Followers,
        int Following, long Plays, IReadOnlyList<SunoClip> Songs, IReadOnlyList<Playlist>? Playlists = null)
    {
        public string Name => string.IsNullOrWhiteSpace(DisplayName) ? "@" + Handle : $"{DisplayName} (@{Handle})";

        /// <summary>The window title: who it is and the numbers that matter.</summary>
        public string Summary => T("{0}: {1} songs, {2} followers", Name, TotalSongs, Followers);

        public IReadOnlyList<string> InfoLines
        {
            get
            {
                var lines = new List<string> { Name };
                if (!string.IsNullOrWhiteSpace(Description)) lines.Add(Description.Trim());
                lines.Add(T("Songs: {0}", TotalSongs));
                lines.Add(T("Followers: {0}. Following: {1}.", Followers, Following));
                if (Plays > 0) lines.Add(T("Plays of all their songs: {0}", Plays));
                lines.Add(T("Link: {0}", "https://suno.com/@" + Handle));
                return lines;
            }
        }
    }

    /// <summary>One shelf of Explore (POST /api/unified/explore): a title and its songs.</summary>
    public sealed record ExploreSection(string Title, IReadOnlyList<SunoClip> Songs)
    {
        public override string ToString() => T("{0}, {1} songs", Title, Songs.Count);
    }

    /// <summary>A Suno contest (GET /api/contests/).</summary>
    public sealed record Contest(string Title, string Description, DateTimeOffset? Ends, int Entries, string? BaseClipId)
    {
        public override string ToString()
        {
            var text = Title;
            if (Ends != null)
                text += ", " + (Ends < DateTimeOffset.Now
                    ? T("ended {0}", Ends.Value.ToLocalTime().ToString("d", Culture))
                    : T("ends {0}", Ends.Value.ToLocalTime().ToString("d", Culture)));
            if (Entries > 0) text += ", " + Plural(Entries, "{0} entry", "{0} entries");
            if (BaseClipId != null) text += ", " + T("Enter plays its song");
            if (!string.IsNullOrWhiteSpace(Description)) text += ". " + Description.Trim();
            return text;
        }
    }

    /// <summary>A lyrics draft from Suno's lyrics editor (GET /api/lyrics-projects).</summary>
    public sealed record LyricsDraft(string Id, string Title, DateTimeOffset? Updated)
    {
        public string Name => string.IsNullOrWhiteSpace(Title) ? T("Untitled") : Title.Trim();

        public override string ToString() =>
            Name
            + (Updated == null ? "" : ", " + T("changed {0}", Updated.Value.ToLocalTime().ToString("g", Culture)));
    }
}
