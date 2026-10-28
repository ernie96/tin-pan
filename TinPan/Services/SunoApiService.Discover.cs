using System;
using System.Collections.Generic;
using System.Linq;
using System.Net.Http;
using System.Threading;
using System.Threading.Tasks;
using TinPan.Models;
using static TinPan.Localization.Loc;

namespace TinPan.Services
{
    // Creator pages, the following feed, remixes and the original song, Explore, notifications read,
    // style suggestions, lyrics drafts and contests. Routes from Nsomnia/chadvis-projectm-qt's
    // ENDPOINT-INVENTORY and suno-api-local-manager-frontend; the request details they don't have
    // (profile sort fields, lyrics-projects paging) come from Suno's web code.
    public partial class SunoApiService
    {
        // ── Creators ────────────────────────────────────────────────────────

        /// <summary>
        /// GET /api/profiles/{handle}, 20 songs a page, newest first. Both sort fields are required
        /// (suno-api-local-manager-frontend: 422 without them).
        /// </summary>
        public async Task<CreatorPage> GetCreatorAsync(string handle, int page = 1, CancellationToken ct = default)
        {
            handle = handle.Trim().TrimStart('@');
            var json = await SendAsync(HttpMethod.Get,
                $"/api/profiles/{Uri.EscapeDataString(handle)}?clips_sort_by=created_at&playlists_sort_by=created_at&page={page}", null, ct);
            return SunoJson.ReadCreatorPage(json, handle);
        }

        /// <summary>
        /// POST /api/social/following-feed (chadvis: page_size, ranking_method and result_type; only the first page
        /// was captured, and not the values, so these are the newest-first ones Suno's sort lists use).
        /// </summary>
        public async Task<IReadOnlyList<SunoClip>> GetFollowingFeedAsync(CancellationToken ct = default) =>
            SunoJson.FindClips(await SendAsync(HttpMethod.Post, "/api/social/following-feed",
                new { page_size = 50, ranking_method = "most_recent", result_type = "all" }, ct));

        // ── Song family ─────────────────────────────────────────────────────

        /// <summary>GET /api/clips/remixes?clip_id= (chadvis, route only): covers and remixes others made of this song.</summary>
        public async Task<IReadOnlyList<SunoClip>> GetRemixesAsync(string clipId, CancellationToken ct = default)
        {
            var clips = SunoJson.FindClips(await SendAsync(HttpMethod.Get, $"/api/clips/remixes?clip_id={Uri.EscapeDataString(clipId)}", null, ct));
            return clips.Where(c => c.Id != clipId).ToList();
        }

        /// <summary>
        /// The song this one was made from: its own metadata says so for covers, extensions and whole songs;
        /// otherwise GET /api/clips/parent?clip_id= (clip_id required, per suno-api-local-manager-frontend).
        /// </summary>
        public async Task<SunoClip?> GetOriginalAsync(SunoClip clip, CancellationToken ct = default)
        {
            var id = clip.Metadata?.OriginalId;
            if (id == null)
            {
                try
                {
                    id = SunoJson.ReadParentId(await SendAsync(HttpMethod.Get, $"/api/clips/parent?clip_id={Uri.EscapeDataString(clip.Id)}", null, ct), clip.Id);
                }
                catch (SunoApiException ex) when (ex.StatusCode == 404)
                {
                    return null;
                }
            }
            if (id == null) return null;
            return (await GetClipsAsync(new[] { id }, ct)).FirstOrDefault();
        }

        // ── Explore ─────────────────────────────────────────────────────────

        /// <summary>POST /api/unified/explore with a cursor (chadvis: feeds[] and next_cursor).</summary>
        public async Task<IReadOnlyList<ExploreSection>> GetExploreAsync(CancellationToken ct = default)
        {
            var sections = new List<ExploreSection>();
            string? cursor = null;
            for (var page = 0; page < 3; page++)
            {
                var (more, next) = SunoJson.ReadExplore(await SendAsync(HttpMethod.Post, "/api/unified/explore", new { cursor }, ct));
                sections.AddRange(more);
                if (next == null || next == cursor) break;
                cursor = next;
            }
            return sections;
        }

        public async Task<IReadOnlyList<Contest>> GetContestsAsync(CancellationToken ct = default) =>
            SunoJson.ReadContests(await SendAsync(HttpMethod.Get, "/api/contests/", null, ct));

        // ── Notifications ───────────────────────────────────────────────────

        /// <summary>
        /// Marks every notification up to now as read (chadvis and Suno's web code: all and before_datetime_utc),
        /// then clears the unread count (POST /api/notification/v2/clear-badge).
        /// </summary>
        public async Task MarkNotificationsReadAsync(CancellationToken ct = default)
        {
            var now = DateTime.UtcNow.ToString("yyyy-MM-dd'T'HH:mm:ss.fff'Z'", System.Globalization.CultureInfo.InvariantCulture);
            await SendAsync(HttpMethod.Post, "/api/notification/v2/read", new { all = true, before_datetime_utc = now }, ct);
            try { await SendAsync(HttpMethod.Post, "/api/notification/v2/clear-badge", null, ct); }
            catch (SunoApiException ex) { Log.Warn($"Notification count not cleared: {ex.Message}"); }
        }

        // ── Writing help ────────────────────────────────────────────────────

        public async Task<IReadOnlyList<string>> GetStyleSuggestionsAsync(CancellationToken ct = default) =>
            SunoJson.ReadStyleSuggestions(await SendAsync(HttpMethod.Get, "/api/prompts/suggestions", null, ct));

        /// <summary>GET /api/lyrics-projects?limit=50&amp;sort=updated_at, following next_cursor (Suno's web code).</summary>
        public async Task<IReadOnlyList<LyricsDraft>> GetLyricsDraftsAsync(CancellationToken ct = default)
        {
            var drafts = new List<LyricsDraft>();
            string? cursor = null;
            for (var page = 0; page < 10; page++)
            {
                var path = "/api/lyrics-projects?limit=50&sort=updated_at" + (cursor == null ? "" : "&cursor=" + Uri.EscapeDataString(cursor));
                var (more, next) = SunoJson.ReadLyricsDrafts(await SendAsync(HttpMethod.Get, path, null, ct));
                drafts.AddRange(more);
                if (next == null || next == cursor) break;
                cursor = next;
            }
            return drafts;
        }

        public async Task<string?> GetLyricsDraftTextAsync(string draftId, CancellationToken ct = default) =>
            SunoJson.ReadLyricsDraftText(await SendAsync(HttpMethod.Get, DraftPath(draftId), null, ct));

        // Create, rename, flush and delete as the sunox reference records Suno's web client: titles are cut to 200
        // characters (Unicode code points), create and rename return the draft, flush takes the whole lyrics.

        private static string DraftPath(string draftId) => $"/api/lyrics-projects/{Uri.EscapeDataString(draftId)}";

        public static string DraftTitle(string title) =>
            string.Concat(title.Trim().EnumerateRunes().Take(200).Select(r => r.ToString()));

        public async Task<LyricsDraft> CreateLyricsDraftAsync(string title, CancellationToken ct = default)
        {
            var name = DraftTitle(title);
            var json = await SendAsync(HttpMethod.Post, "/api/lyrics-projects", new { title = name }, ct);
            var id = SunoJson.TryReadString(json, "id");
            if (string.IsNullOrEmpty(id)) throw new SunoApiException(T("Suno didn't make the draft."));
            return new LyricsDraft(id, SunoJson.TryReadString(json, "title") ?? name, DateTimeOffset.Now);
        }

        public async Task RenameLyricsDraftAsync(string draftId, string title, CancellationToken ct = default) =>
            await SendAsync(HttpMethod.Patch, DraftPath(draftId), new { title = DraftTitle(title) }, ct);

        public async Task SaveLyricsDraftAsync(string draftId, string lyrics, CancellationToken ct = default) =>
            await SendAsync(HttpMethod.Post, DraftPath(draftId) + "/flush", new { lyrics }, ct);

        public async Task DeleteLyricsDraftAsync(string draftId, CancellationToken ct = default) =>
            await SendAsync(HttpMethod.Delete, DraftPath(draftId), null, ct);
    }
}
