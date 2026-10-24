using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Net;
using System.Net.Http;
using System.Text;
using System.Text.Json;
using System.Threading;
using System.Threading.Tasks;
using TinPan.Models;
using static TinPan.Localization.Loc;

namespace TinPan.Services
{
    // Playlists, sharing, remixes, similar songs, comments and writing help.
    // Routes from Suno's web code and Nsomnia/chadvis-projectm-qt docs/suno_api/ENDPOINT-INVENTORY.md.
    public partial class SunoApiService
    {
        // ── Playlists ───────────────────────────────────────────────────────

        public async Task<IReadOnlyList<Playlist>> GetPlaylistsAsync(CancellationToken ct = default)
        {
            var result = new List<Playlist>();
            for (var page = 1; page <= 20; page++)
            {
                using var doc = ParseDocument(await SendAsync(HttpMethod.Get, $"/api/playlist/me?page={page}&show_trashed=false&show_sharelist=false", null, ct));
                var root = doc.RootElement;
                if (root.ValueKind != JsonValueKind.Object || !root.TryGetProperty("playlists", out var list) || list.ValueKind != JsonValueKind.Array) break;
                var before = result.Count;
                foreach (var p in list.EnumerateArray().Where(p => p.ValueKind == JsonValueKind.Object)) result.Add(SunoJson.ReadPlaylist(p));
                var total = root.TryGetProperty("num_total_results", out var t) && SunoJson.AsInt(t) is { } n ? n : result.Count;
                if (result.Count == before || result.Count >= total) break;
            }
            return result;
        }

        public async Task<IReadOnlyList<SunoClip>> GetPlaylistSongsAsync(string playlistId, CancellationToken ct = default)
        {
            var id = Uri.EscapeDataString(playlistId);
            var result = new List<SunoClip>();
            for (var page = 1; page <= 50; page++)
            {
                var json = await SendAsync(HttpMethod.Get, $"/api/playlist/{id}/?page={page}", null, ct);
                var (clips, total) = SunoJson.ReadPlaylistSongs(json);
                var known = result.Select(c => c.Id).ToHashSet();
                var added = clips.Where(c => known.Add(c.Id)).ToList();
                result.AddRange(added);
                // Without a total, carry on until a page brings nothing new.
                if (added.Count == 0 || (total != null && result.Count >= total)) break;
            }
            return result;
        }

        public async Task<Playlist> CreatePlaylistAsync(string name, CancellationToken ct = default)
        {
            using var doc = ParseDocument(await SendAsync(HttpMethod.Post, "/api/playlist/create/", new { name }, ct));
            var p = SunoJson.ReadPlaylist(doc.RootElement);
            if (string.IsNullOrEmpty(p.Id)) throw new SunoApiException(T("Suno didn't say whether the playlist was made. Refresh playlists to check."));
            if (string.IsNullOrEmpty(p.Name)) p.Name = name;
            return p;
        }

        public async Task RenamePlaylistAsync(string playlistId, string name, string description, CancellationToken ct = default)
        {
            var json = await SendAsync(HttpMethod.Post, "/api/playlist/set_metadata", new { playlist_id = playlistId, name, description }, ct);
            var moderation = SunoJson.TryReadString(json, "moderation_error_message");
            if (!string.IsNullOrWhiteSpace(moderation)) throw new SunoApiException(T("Suno didn't accept that name: {0}", moderation));
        }

        public async Task TrashPlaylistAsync(string playlistId, CancellationToken ct = default) =>
            await SendAsync(HttpMethod.Post, "/api/playlist/trash/", new { playlist_id = playlistId, undo_trash = false, is_hidden = false }, ct);

        public async Task AddToPlaylistAsync(string playlistId, IEnumerable<string> clipIds, CancellationToken ct = default)
        {
            foreach (var chunk in clipIds.Distinct().Chunk(100))
                await SendAsync(HttpMethod.Post, $"/api/playlist/v2/{Uri.EscapeDataString(playlistId)}/tracks/add", new { clip_ids = chunk }, ct);
        }

        public async Task RemoveFromPlaylistAsync(string playlistId, IEnumerable<string> clipIds, CancellationToken ct = default)
        {
            foreach (var chunk in clipIds.Distinct().Chunk(100))
                await SendAsync(HttpMethod.Post, $"/api/playlist/v2/{Uri.EscapeDataString(playlistId)}/tracks/remove", new { clip_ids = chunk }, ct);
        }

        public async Task MoveInPlaylistAsync(string playlistId, string clipId, int index, CancellationToken ct = default) =>
            await SendAsync(HttpMethod.Post, $"/api/playlist/v2/{Uri.EscapeDataString(playlistId)}/tracks/reorder-by-index",
                new { positions = new[] { new { clip_id = clipId, index } } }, ct);

        // ── Song extras ─────────────────────────────────────────────────────

        public async Task<string> GetShareLinkAsync(string clipId, CancellationToken ct = default)
        {
            var json = await SendAsync(HttpMethod.Post, "/api/share/link", new { content_type = "clip", content_id = clipId, source = "web" }, ct);
            var link = SunoJson.TryReadString(json, "link");
            if (string.IsNullOrEmpty(link)) throw new SunoApiException(T("Suno gave no share link."));
            return link;
        }

        public async Task SetRemixAllowedAsync(string clipId, bool allowed, CancellationToken ct = default) =>
            await SendAsync(HttpMethod.Post, $"/api/clips/{Uri.EscapeDataString(clipId)}/toggle_remixes/", new { can_remix = allowed }, ct);

        public async Task<IReadOnlyList<SunoClip>> GetSimilarAsync(string clipId, CancellationToken ct = default)
        {
            var clips = SunoJson.FindClips(await SendAsync(HttpMethod.Get, $"/api/clips/get_similar/?id={Uri.EscapeDataString(clipId)}", null, ct));
            return clips.Where(c => c.Id != clipId).ToList();
        }

        public async Task<IReadOnlyList<SongComment>> GetCommentsAsync(string clipId, CancellationToken ct = default)
        {
            var id = Uri.EscapeDataString(clipId);
            var result = new List<SongComment>();
            string? cursor = null;
            for (var page = 0; page < 10; page++)
            {
                var path = $"/api/gen/{id}/comments" + (cursor == null ? "" : "?cursor=" + Uri.EscapeDataString(cursor));
                var json = await SendAsync(HttpMethod.Get, path, null, ct);
                var (comments, next) = SunoJson.ReadComments(json);
                result.AddRange(comments);
                if (string.IsNullOrEmpty(next) || next == cursor || comments.Count == 0) break;
                cursor = next;
            }
            return result;
        }

        public async Task PostCommentAsync(string clipId, string text, CancellationToken ct = default) =>
            await SendAsync(HttpMethod.Post, $"/api/gen/{Uri.EscapeDataString(clipId)}/comment",
                new { content = text, parent_id = (string?)null, entity_type = "clip", track_timestamp = (double?)null }, ct);

        // ── Writing help ────────────────────────────────────────────────────

        public async Task<string> BoostStyleAsync(string style, string? lyrics, bool instrumental, string? guidance, CancellationToken ct = default)
        {
            var body = new Dictionary<string, object> { ["original_tags"] = style, ["is_instrumental"] = instrumental };
            if (!string.IsNullOrWhiteSpace(lyrics)) body["lyrics"] = lyrics;
            if (!string.IsNullOrWhiteSpace(guidance)) body["user_guidance"] = guidance.Trim();
            var boosted = SunoJson.TryReadString(await SendAsync(HttpMethod.Post, "/api/prompts/upsample", body, ct), "upsampled");
            if (string.IsNullOrWhiteSpace(boosted)) throw new SunoApiException(T("Suno gave no boosted style."));
            return boosted.Trim();
        }

        public async Task<string> CowriteLyricsAsync(string lyrics, string instruction, string? title, string? style, CancellationToken ct = default)
        {
            // The same body Suno's lyrics editor sends when you ask it to change your lyrics.
            var body = new
            {
                selected = lyrics,
                context_before = "",
                context_after = "",
                instruction,
                title = (title ?? "").Length > 100 ? title![..100] : title ?? "",
                style = style ?? "",
                mode = "apply_user_request",
                references = Array.Empty<object>(),
                num_variants = (int?)null,
                lyricist_id = (string?)null,
                metadata = new { lyrics_model = "default", enable_thinking = false },
                create_session_token = (string?)null,
                lyrics_project_id = (string?)null
            };
            var edited = SunoJson.TryReadString(await SendAsync(HttpMethod.Post, "/api/generate/cowrite-lyrics/", body, ct), "edited_lyrics");
            if (string.IsNullOrWhiteSpace(edited)) throw new SunoApiException(T("Suno sent back no lyrics."));
            return edited.Trim();
        }
    }
}
