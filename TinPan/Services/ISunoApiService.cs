using System.Collections.Generic;
using System.Threading;
using System.Threading.Tasks;
using TinPan.Models;
using static TinPan.Localization.Loc;

namespace TinPan.Services
{
    /// <summary>Where to continue the library listing. Suno's newer feed uses a cursor, the older one a page number.</summary>
    public record LibraryQuery(string? Cursor = null, int Page = 0, string? Search = null, LibraryFilter? Filter = null);

    /// <summary>
    /// Suno's own library filters. Each value is Suno's: "Any" shows both, "True" only the matching
    /// songs, "False" hides them. Show is keyed by the field name in FeedFilters.All; a missing key
    /// takes that filter's default. A null WorkspaceId means every workspace.
    /// </summary>
    public sealed record LibraryFilter
    {
        public string? WorkspaceId { get; init; }
        public string Disliked { get; init; } = "False";
        public IReadOnlyDictionary<string, string> Show { get; init; } = new Dictionary<string, string>();

        /// <summary>One of FeedFilters.Sorts: newest, oldest, most-liked or least-liked.</summary>
        public string Sort { get; init; } = "newest";
    }

    /// <summary>A yes/no/either filter of Suno's library feed (feed/v3), as Suno's web app sends it.</summary>
    /// <param name="Field">Suno's field name.</param>
    /// <param name="Presence">True when Suno wants {"presence": value} rather than the value itself.</param>
    /// <param name="Default">What Suno's own library uses: trash, stems and Studio projects hidden, the rest shown.</param>
    public sealed record FeedFilter(string Field, bool Presence, string Default);

    public static class FeedFilters
    {
        public const string Any = "Any", Only = "True", Hide = "False";

        /// <summary>Every on/off filter Suno's web library has (its bundle's default filter object), except bpm and key.</summary>
        public static IReadOnlyList<FeedFilter> All { get; } = new FeedFilter[]
        {
            new("trashed", false, Hide),
            new("stem", true, Hide),
            new("fromStudioProject", true, Hide),
            new("liked", false, Any),
            new("public", false, Any),
            new("upload", false, Any),
            new("cover", true, Any),
            new("extend", true, Any),
            new("remaster", true, Any),
            new("concat", true, Any),
            new("persona", true, Any),
            new("playlist", true, Any),
            new("unlocked", false, Any),
        };

        /// <summary>Suno's sort choices: id, field and direction.</summary>
        public static IReadOnlyList<(string Id, string SortBy, string Direction)> Sorts { get; } = new[]
        {
            ("newest", "created_at", "desc"),
            ("oldest", "created_at", "asc"),
            ("most-liked", "upvote_count", "desc"),
            ("least-liked", "upvote_count", "asc"),
        };

        /// <summary>A value Suno accepts, or the fallback.</summary>
        public static string Valid(string? value, string fallback) => value is Any or Only or Hide ? value : fallback;
    }

    public record Workspace(string Id, string Name, int? ClipCount)
    {
        public override string ToString() => ClipCount is { } n ? $"{Name}, {Plural(n, "{0} song", "{0} songs")}" : Name;
    }

    /// <summary>
    /// Suno's remaster options. Strength is "subtle", "normal" or "high"; profile is "natural",
    /// "boost" or "clarity". The five fine controls run 0 to 1; null keeps Suno's default
    /// (freedom 0, the others 0.5).
    /// </summary>
    public record RemasterRequest(string ClipId, string? Model, string? Style = null, string? Strength = null, string? Profile = null,
        double? Freedom = null, double? Tone = null, double? Clarity = null, double? FineStrength = null, double? StereoWidth = null);

    public record Persona(string Id, string Name)
    {
        public override string ToString() => Name;
    }

    public record FeedPage(IReadOnlyList<SunoClip> Clips, string? NextCursor = null, bool? HasMore = null, int? NumTotal = null);

    /// <summary>
    /// Suno's private studio API. Every method throws SunoApiException with a short,
    /// speakable message when something goes wrong.
    /// </summary>
    public interface ISunoApiService
    {
        Task<BillingInfo> GetBillingInfoAsync(CancellationToken ct = default);

        /// <summary>One page of the signed-in user's songs, newest first, optionally searched.</summary>
        Task<FeedPage> GetLibraryPageAsync(LibraryQuery query, CancellationToken ct = default);

        Task<IReadOnlyList<SunoClip>> GetClipsAsync(IEnumerable<string> ids, CancellationToken ct = default);

        Task<IReadOnlyList<SunoClip>> GetTrendingAsync(CancellationToken ct = default);

        /// <summary>Your workspaces (Suno calls them projects). The unassigned one has id "default".</summary>
        Task<IReadOnlyList<Workspace>> GetWorkspacesAsync(CancellationToken ct = default);

        /// <summary>Your own voice personas, for the Create tab.</summary>
        Task<IReadOnlyList<Persona>> GetPersonasAsync(CancellationToken ct = default);

        Task<IReadOnlyList<Notification>> GetNotificationsAsync(CancellationToken ct = default);

        /// <summary>
        /// Every kind of creation (describe, custom lyrics, extend, cover, remaster) goes through here.
        /// Songs come back unfinished; follow them with GetClipsAsync.
        /// </summary>
        Task<IReadOnlyList<SunoClip>> GenerateAsync(GenerateRequest request, CancellationToken ct = default);

        /// <summary>Remaster: POST /api/generate/upsample, the request Suno's own Remaster button sends.</summary>
        Task<IReadOnlyList<SunoClip>> RemasterAsync(RemasterRequest request, CancellationToken ct = default);

        /// <summary>Joins an extended clip and the parts before it into one whole song.</summary>
        Task<SunoClip?> ConcatAsync(string clipId, CancellationToken ct = default);

        Task<IReadOnlyList<SunoClip>> GenerateStemsAsync(string clipId, CancellationToken ct = default);

        /// <summary>Writes lyrics from a topic. Waits until Suno has finished them.</summary>
        Task<LyricsResult> GenerateLyricsAsync(string topic, CancellationToken ct = default);

        Task<AlignedLyrics?> GetAlignedLyricsAsync(string clipId, CancellationToken ct = default);

        /// <summary>Changes the title and/or lyrics shown for a song. Null leaves a field as it is.</summary>
        Task SetDetailsAsync(string clipId, string? title, string? lyrics, CancellationToken ct = default);

        Task SetPublicAsync(string clipId, bool isPublic, CancellationToken ct = default);

        Task TrashAsync(string clipId, CancellationToken ct = default);

        /// <summary>Asks Suno for a WAV copy and waits until it is ready. Returns its address. Counts as a download.</summary>
        Task<string> GetWavUrlAsync(string clipId, CancellationToken ct = default);

        /// <summary>
        /// Spends one of the account's downloads on this song (Suno's Download confirmation).
        /// Later downloads of the same song, in any format, are free.
        /// </summary>
        Task AuthorizeDownloadAsync(string clipId, CancellationToken ct = default);

        /// <summary>
        /// Asks Suno to prepare a file ("mp3", "m4a", "wav" or "mp4") and waits for its short-lived address.
        /// studio=true uses Suno Studio's route. Both are counted against the allowance.
        /// </summary>
        Task<string> PrepareDownloadAsync(string clipId, string format, bool studio, CancellationToken ct = default);

        /// <summary>Downloads a file from Suno's CDN. Writes to a temporary name first.</summary>
        Task DownloadAsync(string url, string destinationPath, CancellationToken ct = default);

        // ── Playlists (routes from Suno's web code and chadvis ENDPOINT-INVENTORY.md) ──

        Task<IReadOnlyList<Playlist>> GetPlaylistsAsync(CancellationToken ct = default);
        Task<IReadOnlyList<SunoClip>> GetPlaylistSongsAsync(string playlistId, CancellationToken ct = default);
        Task<Playlist> CreatePlaylistAsync(string name, CancellationToken ct = default);
        Task RenamePlaylistAsync(string playlistId, string name, string description, CancellationToken ct = default);
        Task TrashPlaylistAsync(string playlistId, CancellationToken ct = default);
        Task AddToPlaylistAsync(string playlistId, IEnumerable<string> clipIds, CancellationToken ct = default);
        Task RemoveFromPlaylistAsync(string playlistId, IEnumerable<string> clipIds, CancellationToken ct = default);

        /// <summary>Moves one song to a 0-based position in the playlist.</summary>
        Task MoveInPlaylistAsync(string playlistId, string clipId, int index, CancellationToken ct = default);

        // ── Song extras ──

        /// <summary>Suno's own share link for a song (the one its Share button copies).</summary>
        Task<string> GetShareLinkAsync(string clipId, CancellationToken ct = default);

        /// <summary>Allows or stops other people covering and remixing this song.</summary>
        Task SetRemixAllowedAsync(string clipId, bool allowed, CancellationToken ct = default);

        Task<IReadOnlyList<SunoClip>> GetSimilarAsync(string clipId, CancellationToken ct = default);
        Task<IReadOnlyList<SongComment>> GetCommentsAsync(string clipId, CancellationToken ct = default);
        Task PostCommentAsync(string clipId, string text, CancellationToken ct = default);

        // ── Writing help ──

        /// <summary>Style Boost: Suno rewrites the style text into a richer one.</summary>
        Task<string> BoostStyleAsync(string style, string? lyrics, bool instrumental, string? guidance, CancellationToken ct = default);

        /// <summary>Lyric co-writing: Suno changes the lyrics the way the instruction asks. Returns the new lyrics.</summary>
        Task<string> CowriteLyricsAsync(string lyrics, string instruction, string? title, string? style, CancellationToken ct = default);
    }
}
