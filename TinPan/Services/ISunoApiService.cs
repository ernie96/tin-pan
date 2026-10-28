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

        /// <summary>A new song at another speed; keepPitch keeps the key, otherwise it changes like a tape.</summary>
        Task<SunoClip> ChangeSpeedAsync(string clipId, double multiplier, bool keepPitch, string title, CancellationToken ct = default);

        /// <summary>A new song played backwards.</summary>
        Task<SunoClip> ReverseAsync(string clipId, string title, CancellationToken ct = default);

        /// <summary>A new song keeping only start..end, or with start..end taken out. Comes back unfinished.</summary>
        Task<SunoClip> CropAsync(string clipId, double start, double end, bool removeSection, string title, CancellationToken ct = default);

        /// <summary>A new song fading in and/or out over the given seconds. Comes back unfinished.</summary>
        Task<SunoClip> FadeAsync(string clipId, double? fadeIn, double? fadeOut, string title, CancellationToken ct = default);

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

        /// <summary>
        /// Unmetered direct download for a song without consuming user's downloads quota.
        /// Uses unencrypted progressive CDN file if available, or Suno's Mango DRM unwrap/decrypt pipeline.
        /// </summary>
        Task DownloadClipDirectAsync(SunoClip clip, string destinationPath, CancellationToken ct = default);

        // Songs
        Task SetNotInterestedAsync(string clipId, bool notInterested, CancellationToken ct = default);
        Task SetShowRemixesAsync(string clipId, bool show, CancellationToken ct = default);
        Task<SongCredits> GetCreditsAsync(string clipId, CancellationToken ct = default);
        Task<IReadOnlyList<SunoClip>> GetExistingStemsAsync(string clipId, CancellationToken ct = default);
        Task<AlignedLyrics?> AlignLyricsAsync(string clipId, string lyrics, CancellationToken ct = default);
        Task RestoreFromTrashAsync(string clipId, CancellationToken ct = default);
        Task DeleteForeverAsync(IReadOnlyList<string> clipIds, CancellationToken ct = default);

        // Pictures and video
        Task<AudioUploadTicket> StartImageUploadAsync(string extension, CancellationToken ct = default);
        Task FinishImageUploadAsync(string uploadId, CancellationToken ct = default);
        Task SetSongCoverAsync(string clipId, string? uploadedImageId, string? imageUrl, CancellationToken ct = default);
        Task<string> MakePictureAsync(string prompt, CancellationToken ct = default);
        Task SetPlaylistCoverAsync(string playlistId, string uploadedImageId, CancellationToken ct = default);
        Task StartVideoAsync(string clipId, CancellationToken ct = default);
        Task<(string Status, string? VideoUrl)> GetVideoStatusAsync(string clipId, CancellationToken ct = default);

        // Playlists and albums
        Task SetPlaylistLikedAsync(string playlistId, bool liked, CancellationToken ct = default);
        Task SetPlaylistSavedAsync(string playlistId, bool saved, CancellationToken ct = default);
        Task<string> CreateAlbumAsync(string name, string description, string albumType, string? fromPlaylistId, CancellationToken ct = default);
        Task PublishAlbumAsync(string albumId, CancellationToken ct = default);

        // Personas
        Task<IReadOnlyList<PersonaDetails>> GetPersonaListAsync(PersonaList list, CancellationToken ct = default);
        Task<string> CreatePersonaAsync(string rootClipId, string name, string description, bool isPublic, CancellationToken ct = default);
        Task EditPersonaAsync(string personaId, string name, string description, bool isPublic, CancellationToken ct = default);
        Task<bool> TogglePersonaLoveAsync(string personaId, CancellationToken ct = default);
        Task TrashPersonaAsync(string personaId, bool undo, CancellationToken ct = default);
        Task<IReadOnlyList<SunoClip>> GetPersonaSongsAsync(string personaId, CancellationToken ct = default);

        // Custom models
        Task<IReadOnlyList<CustomModelBase>> GetCustomModelBasesAsync(CancellationToken ct = default);
        Task<IReadOnlyList<PendingCustomModel>> GetPendingCustomModelsAsync(CancellationToken ct = default);
        Task CreateCustomModelAsync(IReadOnlyList<string> clipIds, string name, string? baseModel, CancellationToken ct = default);
        Task RenameCustomModelAsync(string modelId, string name, CancellationToken ct = default);
        Task ArchiveCustomModelAsync(string modelId, CancellationToken ct = default);

        // Lyrics mashup and personalization
        Task<string> MashupLyricsAsync(string first, string second, CancellationToken ct = default);
        Task<bool?> GetStylePersonalizationAsync(CancellationToken ct = default);

        /// <summary>The song's file, decrypted, for playing it in Tin Pan; uses none of the account's downloads.</summary>
        Task<byte[]> GetSongAudioAsync(string clipId, CancellationToken ct = default);

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

        /// <summary>Likes ("LIKE"), dislikes ("DISLIKE") or clears (null) a song. Returns Suno's new state: "L", "D" or null.</summary>
        Task<string?> SetReactionAsync(string clipId, string? reaction, CancellationToken ct = default);

        /// <summary>A creator's page: who they are and one page (20) of their public songs, newest first.</summary>
        Task<CreatorPage> GetCreatorAsync(string handle, int page = 1, CancellationToken ct = default);

        /// <summary>New songs from the people you follow.</summary>
        Task<IReadOnlyList<SunoClip>> GetFollowingFeedAsync(CancellationToken ct = default);

        /// <summary>Covers and remixes of a song.</summary>
        Task<IReadOnlyList<SunoClip>> GetRemixesAsync(string clipId, CancellationToken ct = default);

        /// <summary>The song a cover, extension or whole song was made from, or null when it wasn't made from one.</summary>
        Task<SunoClip?> GetOriginalAsync(SunoClip clip, CancellationToken ct = default);

        /// <summary>Explore's shelves of songs.</summary>
        Task<IReadOnlyList<ExploreSection>> GetExploreAsync(CancellationToken ct = default);

        Task<IReadOnlyList<Contest>> GetContestsAsync(CancellationToken ct = default);

        /// <summary>Marks every notification so far as read and clears the unread count.</summary>
        Task MarkNotificationsReadAsync(CancellationToken ct = default);

        /// <summary>Style ideas Suno suggests for you.</summary>
        Task<IReadOnlyList<string>> GetStyleSuggestionsAsync(CancellationToken ct = default);

        /// <summary>Your drafts in Suno's lyrics editor, most recently changed first.</summary>
        Task<IReadOnlyList<LyricsDraft>> GetLyricsDraftsAsync(CancellationToken ct = default);

        /// <summary>The words of one lyrics draft, or null when it has none.</summary>
        Task<string?> GetLyricsDraftTextAsync(string draftId, CancellationToken ct = default);

        /// <summary>Starts a new, empty lyrics draft and returns it.</summary>
        Task<LyricsDraft> CreateLyricsDraftAsync(string title, CancellationToken ct = default);

        Task RenameLyricsDraftAsync(string draftId, string title, CancellationToken ct = default);

        /// <summary>Saves these words as the draft's lyrics, replacing what it had.</summary>
        Task SaveLyricsDraftAsync(string draftId, string lyrics, CancellationToken ct = default);

        Task DeleteLyricsDraftAsync(string draftId, CancellationToken ct = default);

        Task<IReadOnlyList<SongComment>> GetCommentsAsync(string clipId, CancellationToken ct = default);
        Task PostCommentAsync(string clipId, string text, CancellationToken ct = default);

        // ── Writing help ──

        /// <summary>Style Boost: Suno rewrites the style text into a richer one.</summary>
        Task<string> BoostStyleAsync(string style, string? lyrics, bool instrumental, string? guidance, CancellationToken ct = default);

        /// <summary>Lyric co-writing: Suno changes the lyrics the way the instruction asks. Returns the new lyrics.</summary>
        Task<string> CowriteLyricsAsync(string lyrics, string instruction, string? title, string? style, CancellationToken ct = default);
        // Uploading audio (SunoApiService.Uploads.cs). The storage POST itself is UploadService's.
        Task<AudioUploadTicket> StartAudioUploadAsync(string extension, CancellationToken ct = default);
        Task FinishAudioUploadAsync(string uploadId, string fileName, CancellationToken ct = default);
        Task<AudioUploadState> GetAudioUploadAsync(string uploadId, CancellationToken ct = default);
        Task<string> CreateClipFromUploadAsync(string uploadId, CancellationToken ct = default);
        Task NameUploadedSongAsync(string clipId, string title, string? lyrics = null, string? imageUrl = null, CancellationToken ct = default);
    }
}
