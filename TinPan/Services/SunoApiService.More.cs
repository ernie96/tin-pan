using System;
using System.Collections.Generic;
using System.Linq;
using System.Net.Http;
using System.Text.Json;
using System.Threading;
using System.Threading.Tasks;
using TinPan.Models;
using static TinPan.Localization.Loc;

namespace TinPan.Services
{
    public sealed record CreditedSong(string? ClipId, string Title, string? Creator, string? Relationship, bool Deleted);

    public sealed record SongCredits(IReadOnlyList<CreditedSong> Sources, int? RemixCount, bool RemixCountCapped);

    public sealed record PersonaDetails(string Id, string Name, string Description, bool IsPublic, bool IsLoved, bool IsOwned,
        bool IsTrashed, bool IsVoice, string? RootClipId, string? Creator)
    {
        public override string ToString()
        {
            var parts = new List<string> { string.IsNullOrWhiteSpace(Name) ? T("Unnamed persona") : Name.Trim() };
            if (IsVoice) parts.Add(T("voice"));
            if (!IsOwned && !string.IsNullOrEmpty(Creator)) parts.Add(T("by {0}", Creator));
            if (IsOwned) parts.Add(IsPublic ? T("public") : T("private"));
            if (IsLoved) parts.Add(T("loved"));
            if (IsTrashed) parts.Add(T("in the trash"));
            if (!string.IsNullOrWhiteSpace(Description)) parts.Add(Description.Trim());
            return string.Join(", ", parts);
        }
    }

    public enum PersonaList { Mine, Loved, Followed }

    public sealed record CustomModelBase(string Key, string Name, bool IsDefault)
    {
        public override string ToString() => Name;
    }

    public sealed record PendingCustomModel(string Id, string Name);

    // Routes and bodies from Nsomnia/sunox-chadvis-suno-frontend-projectm-qt-research (src/api: clip_info, delete,
    // stems, metadata, persona, playlist, album, visual, custom_model, lyrics_editor, prompts), and
    // Nsomnia/chadvis-projectm-qt ENDPOINT-INVENTORY for toggle_show_remixes. "Not interested" and the show_remix
    // field are from Suno's own web code (the clip menu), as no reference project has them yet.
    public partial class SunoApiService
    {
        // ── Songs ───────────────────────────────────────────────────────────

        /// <summary>Tells Suno's recommendations to stop (or start again) suggesting songs like this one.</summary>
        public async Task SetNotInterestedAsync(string clipId, bool notInterested, CancellationToken ct = default) =>
            await SendAsync(notInterested ? HttpMethod.Post : HttpMethod.Delete,
                $"/api/recommend/feedback/song/{Uri.EscapeDataString(clipId)}", new { feedback_type = "not_interested" }, ct);

        /// <summary>Shows or hides the list of remixes on the song's page (metadata.show_remix).</summary>
        public async Task SetShowRemixesAsync(string clipId, bool show, CancellationToken ct = default) =>
            await SendAsync(HttpMethod.Post, $"/api/clips/{Uri.EscapeDataString(clipId)}/toggle_show_remixes", new { show_remix = show }, ct);

        /// <summary>The songs this one credits (GET /api/clips/{id}/attribution) and how many remixes it has.</summary>
        public async Task<SongCredits> GetCreditsAsync(string clipId, CancellationToken ct = default)
        {
            var id = Uri.EscapeDataString(clipId);
            var sources = new List<CreditedSong>();
            using (var doc = ParseDocument(await SendAsync(HttpMethod.Get, $"/api/clips/{id}/attribution", null, ct)))
            {
                if (doc.RootElement.ValueKind == JsonValueKind.Object && doc.RootElement.TryGetProperty("source_clips", out var list) && list.ValueKind == JsonValueKind.Array)
                    foreach (var s in list.EnumerateArray().Where(s => s.ValueKind == JsonValueKind.Object))
                    {
                        var user = s.TryGetProperty("user", out var u) && u.ValueKind == JsonValueKind.Object ? u : default;
                        var creator = user.ValueKind == JsonValueKind.Object
                            ? Str(user, "user_display_name") ?? (Str(user, "user_handle") is { } h ? "@" + h : null)
                            : null;
                        sources.Add(new CreditedSong(Str(s, "clip_id"), Str(s, "title") ?? T("Untitled"), creator, Str(s, "relationship"),
                            s.TryGetProperty("is_deleted", out var d) && d.ValueKind == JsonValueKind.True));
                    }
            }
            int? count = null;
            var capped = false;
            try
            {
                var json = await SendAsync(HttpMethod.Get, $"/api/clips/remixes/count?clip_id={id}", null, ct);
                count = SunoJson.ReadInt(json, "count");
                capped = SunoJson.ReadBool(json, "is_capped") == true;
            }
            catch (SunoApiException ex) when (ex is not SunoSignInRequiredException) { Log.Warn($"Remix count didn't load: {ex.Message}"); }
            return new SongCredits(sources, count, capped);
        }

        /// <summary>Stems Suno already made from this song: GET /api/clip/{id}/stems/pages, then each zero-based page.</summary>
        public async Task<IReadOnlyList<SunoClip>> GetExistingStemsAsync(string clipId, CancellationToken ct = default)
        {
            var id = Uri.EscapeDataString(clipId);
            var pages = SunoJson.ReadInt(await SendAsync(HttpMethod.Get, $"/api/clip/{id}/stems/pages", null, ct), "pages") ?? 0;
            var ids = new List<string>();
            for (var page = 0; page < Math.Min(pages, 20); page++)
            {
                using var doc = ParseDocument(await SendAsync(HttpMethod.Get, $"/api/clip/{id}/stems?page={page}", null, ct));
                if (doc.RootElement.ValueKind == JsonValueKind.Object && doc.RootElement.TryGetProperty("stems", out var stems) && stems.ValueKind == JsonValueKind.Array)
                    ids.AddRange(stems.EnumerateArray().Select(s => s.ValueKind == JsonValueKind.Object ? Str(s, "id") : null)
                        .Where(s => !string.IsNullOrEmpty(s) && !ids.Contains(s!))!);
            }
            if (ids.Count == 0) return Array.Empty<SunoClip>();
            var clips = await GetClipsAsync(ids, ct);
            return ids.Select(i => clips.FirstOrDefault(c => c.Id == i)).OfType<SunoClip>().ToList();
        }

        /// <summary>
        /// Times the song's own words when Suno has no timed lyrics for it: POST /api/gen/{id}/aligned_lyrics/v3 with the
        /// lyrics, then the same path until it answers with "alignment" (or state "error").
        /// </summary>
        public async Task<AlignedLyrics?> AlignLyricsAsync(string clipId, string lyrics, CancellationToken ct = default)
        {
            var path = $"/api/gen/{Uri.EscapeDataString(clipId)}/aligned_lyrics/v3";
            var json = await SendAsync(HttpMethod.Post, path, new { lyrics, enable_augmentation = false }, ct);
            for (var attempt = 0; ; attempt++)
            {
                using (var doc = ParseDocument(json))
                {
                    var r = doc.RootElement;
                    if (r.ValueKind == JsonValueKind.Object && r.TryGetProperty("alignment", out var a) && a.ValueKind == JsonValueKind.Array)
                        return new AlignedLyrics
                        {
                            Words = a.EnumerateArray().Where(w => w.ValueKind == JsonValueKind.Object)
                                .Select(w => new AlignedWord { Word = Str(w, "word") ?? "", Start = Num(w, "start_s"), End = Num(w, "end_s") })
                                .Where(w => w.End < 6000).ToList()
                        };
                    if (r.ValueKind == JsonValueKind.Object && Str(r, "state") == "error") return null;
                }
                if (attempt >= 40) return null;
                await Task.Delay(PollInterval, ct);
                json = await SendAsync(HttpMethod.Get, path, null, ct);
            }
        }

        // ── Trash ───────────────────────────────────────────────────────────

        public async Task RestoreFromTrashAsync(string clipId, CancellationToken ct = default) =>
            await SendAsync(HttpMethod.Post, "/api/gen/trash", new { clip_ids = new[] { clipId }, trash = false }, ct);

        /// <summary>Deletes songs for good (POST /api/clips/delete/); there's no undo.</summary>
        public async Task DeleteForeverAsync(IReadOnlyList<string> clipIds, CancellationToken ct = default)
        {
            foreach (var chunk in clipIds.Chunk(20))
                await SendAsync(HttpMethod.Post, "/api/clips/delete/", new { ids = chunk }, ct);
        }

        // ── Pictures and video ──────────────────────────────────────────────

        public static readonly string[] ImageExtensions = { "jpg", "jpeg", "png", "webp" };

        public async Task<AudioUploadTicket> StartImageUploadAsync(string extension, CancellationToken ct = default) =>
            ParseUploadTicket(await SendAsync(HttpMethod.Post, "/api/uploads/image/", new { extension }, ct));

        /// <summary>Finishes an image upload; Suno checks the picture and says whether it's approved.</summary>
        public async Task FinishImageUploadAsync(string uploadId, CancellationToken ct = default)
        {
            var json = await SendAsync(HttpMethod.Post, $"/api/uploads/image/{Uri.EscapeDataString(uploadId)}/upload-finish/", new { }, ct);
            var status = SunoJson.TryReadString(json, "moderation_status");
            if (status != null && status != "approved")
                throw new SunoApiException(T("Suno didn't accept that picture."));
        }

        /// <summary>Sets a song's cover: a picture uploaded to Suno, one at an address, or none (Suno's own again).</summary>
        public async Task SetSongCoverAsync(string clipId, string? uploadedImageId, string? imageUrl, CancellationToken ct = default)
        {
            var body = new Dictionary<string, object>();
            if (uploadedImageId != null) body["image_s3_id"] = "image_" + uploadedImageId;
            else if (imageUrl != null) body["image_url"] = imageUrl;
            else body["remove_image_cover"] = true;
            await SendAsync(HttpMethod.Post, $"/api/gen/{Uri.EscapeDataString(clipId)}/set_metadata/", body, ct);
        }

        /// <summary>Has Suno draw a picture from a description (POST /api/gen/prompt_image/); returns its address.</summary>
        public async Task<string> MakePictureAsync(string prompt, CancellationToken ct = default) =>
            SunoJson.TryReadString(await SendAsync(HttpMethod.Post, "/api/gen/prompt_image/", new { prompt }, ct), "image_url") is { Length: > 0 } url
                ? url
                : throw new SunoApiException(T("Suno didn't make a picture. Try another description."));

        public async Task SetPlaylistCoverAsync(string playlistId, string uploadedImageId, CancellationToken ct = default) =>
            await SendAsync(HttpMethod.Patch, $"/api/playlist/v2/{Uri.EscapeDataString(playlistId)}",
                new { metadata = new { cover_image_s3_id = "image_" + uploadedImageId } }, ct);

        /// <summary>Asks Suno to make the song's video (POST /api/video/generate/{id}/, no body).</summary>
        public async Task StartVideoAsync(string clipId, CancellationToken ct = default) =>
            await SendAsync(HttpMethod.Post, $"/api/video/generate/{Uri.EscapeDataString(clipId)}/", null, ct);

        public async Task<(string Status, string? VideoUrl)> GetVideoStatusAsync(string clipId, CancellationToken ct = default)
        {
            var json = await SendAsync(HttpMethod.Get, $"/api/video/generate/{Uri.EscapeDataString(clipId)}/status/", null, ct);
            return (SunoJson.TryReadString(json, "status") ?? "", SunoJson.TryReadString(json, "video_url"));
        }

        // ── Playlists and albums ────────────────────────────────────────────

        public async Task SetPlaylistLikedAsync(string playlistId, bool liked, CancellationToken ct = default) =>
            await SendAsync(HttpMethod.Post, $"/api/playlist_reaction/{Uri.EscapeDataString(playlistId)}/update_reaction_type/",
                new Dictionary<string, object?> { ["reaction"] = liked ? "LIKE" : null }, ct);

        public async Task SetPlaylistSavedAsync(string playlistId, bool saved, CancellationToken ct = default) =>
            await SendAsync(saved ? HttpMethod.Post : HttpMethod.Delete, $"/api/playlist/v2/{Uri.EscapeDataString(playlistId)}/save", null, ct);

        /// <summary>A draft album (album_type "lp" or "ep"), from a playlist's songs when one is given. Returns its id.</summary>
        public async Task<string> CreateAlbumAsync(string name, string description, string albumType, string? fromPlaylistId, CancellationToken ct = default)
        {
            var body = new Dictionary<string, object>
            {
                ["name"] = name, ["description"] = description, ["playlist_type"] = "album", ["album_type"] = albumType, ["album_status"] = "draft",
                ["creation_source"] = fromPlaylistId != null ? "playlist_make_album" : "library_albums_tab"
            };
            if (fromPlaylistId != null) body["source_playlist_id"] = fromPlaylistId;
            using var doc = ParseDocument(await SendAsync(HttpMethod.Post, "/api/playlist/create/", body, ct));
            var r = doc.RootElement;
            var id = r.ValueKind != JsonValueKind.Object ? null
                : Str(r, "id") ?? (r.TryGetProperty("metadata", out var m) && m.ValueKind == JsonValueKind.Object ? Str(m, "id") : null)
                  ?? (r.TryGetProperty("playlist", out var p) && p.ValueKind == JsonValueKind.Object ? Str(p, "id") : null);
            return id ?? throw new SunoApiException(T("Suno didn't say whether the album was made. Refresh playlists to check."));
        }

        public async Task PublishAlbumAsync(string albumId, CancellationToken ct = default) =>
            await SendAsync(HttpMethod.Post, $"/api/playlist/v2/{Uri.EscapeDataString(albumId)}/album/publish", new { use_album_cover = true }, ct);

        // ── Personas ────────────────────────────────────────────────────────

        public async Task<IReadOnlyList<PersonaDetails>> GetPersonaListAsync(PersonaList list, CancellationToken ct = default)
        {
            var path = list switch
            {
                PersonaList.Loved => "/api/persona/get-loved-personas/",
                PersonaList.Followed => "/api/persona/get-followed-personas/",
                _ => "/api/persona/get-personas/"
            };
            var result = new List<PersonaDetails>();
            string? token = null;
            for (var page = 1; page <= 20; page++)
            {
                var query = $"?page={page}" + (token != null ? "&continuation_token=" + Uri.EscapeDataString(token) : "");
                using var doc = ParseDocument(await SendAsync(HttpMethod.Get, path + query, null, ct));
                var r = doc.RootElement;
                if (r.ValueKind != JsonValueKind.Object || !r.TryGetProperty("personas", out var personas) || personas.ValueKind != JsonValueKind.Array) break;
                var before = result.Count;
                result.AddRange(personas.EnumerateArray().Where(p => p.ValueKind == JsonValueKind.Object).Select(ReadPersonaDetails)
                    .Where(p => p.Id.Length > 0 && result.All(x => x.Id != p.Id)));
                token = Str(r, "continuation_token");
                var total = r.TryGetProperty("total_results", out var t) ? SunoJson.AsInt(t) : null;
                if (result.Count == before || (total != null && result.Count >= total) || (token == null && total == null)) break;
            }
            return result;
        }

        internal static PersonaDetails ReadPersonaDetails(JsonElement p)
        {
            var handle = Str(p, "user_handle");
            var info = ReadPersona(p.GetRawText());
            return new PersonaDetails(Str(p, "id") ?? "", Str(p, "name") ?? "", Str(p, "description") ?? "",
                p.TryGetProperty("is_public", out var pub) && pub.ValueKind == JsonValueKind.True,
                p.TryGetProperty("is_loved", out var l) && l.ValueKind == JsonValueKind.True,
                !p.TryGetProperty("is_owned", out var o) || o.ValueKind != JsonValueKind.False,
                info.IsTrashed, info.IsVoice, info.RootClipId,
                Str(p, "user_display_name") ?? (handle != null ? "@" + handle : null));
        }

        /// <summary>A persona from a song of yours (POST /api/persona/create/); returns its id.</summary>
        public async Task<string> CreatePersonaAsync(string rootClipId, string name, string description, bool isPublic, CancellationToken ct = default)
        {
            var json = await SendAsync(HttpMethod.Post, "/api/persona/create/",
                new { root_clip_id = rootClipId, name, description, is_public = isPublic }, ct);
            return SunoJson.TryReadString(json, "id") is { Length: > 0 } id
                ? id
                : throw new SunoApiException(T("Suno didn't say whether the persona was made. Check your personas."));
        }

        public async Task EditPersonaAsync(string personaId, string name, string description, bool isPublic, CancellationToken ct = default) =>
            await SendAsync(HttpMethod.Put, $"/api/persona/edit-persona/{Uri.EscapeDataString(personaId)}/",
                new { persona_id = personaId, name, description, is_public = isPublic }, ct);

        /// <summary>Loves or un-loves a persona; returns whether it's loved now.</summary>
        public async Task<bool> TogglePersonaLoveAsync(string personaId, CancellationToken ct = default) =>
            SunoJson.ReadBool(await SendAsync(HttpMethod.Post, $"/api/persona/{Uri.EscapeDataString(personaId)}/toggle_love/", null, ct), "loved") == true;

        /// <summary>Moves a persona to the trash, or back out with undo.</summary>
        public async Task TrashPersonaAsync(string personaId, bool undo, CancellationToken ct = default) =>
            await SendAsync(HttpMethod.Put, $"/api/persona/trash-persona/{Uri.EscapeDataString(personaId)}/?undo={(undo ? "true" : "false")}&hide=false", null, ct);

        /// <summary>Songs made with a persona (GET /api/persona/get-persona-paginated/{id}/), first pages.</summary>
        public async Task<IReadOnlyList<SunoClip>> GetPersonaSongsAsync(string personaId, CancellationToken ct = default)
        {
            var result = new List<SunoClip>();
            for (var page = 1; page <= 5; page++)
            {
                var clips = SunoJson.FindClips(await SendAsync(HttpMethod.Get,
                    $"/api/persona/get-persona-paginated/{Uri.EscapeDataString(personaId)}/?page={page}", null, ct));
                var added = clips.Where(c => result.All(x => x.Id != c.Id)).ToList();
                if (added.Count == 0) break;
                result.AddRange(added);
            }
            return result;
        }

        // ── Custom models ───────────────────────────────────────────────────

        public async Task<IReadOnlyList<CustomModelBase>> GetCustomModelBasesAsync(CancellationToken ct = default)
        {
            using var doc = ParseDocument(await SendAsync(HttpMethod.Get, "/api/custom-model/bases/", null, ct));
            var r = doc.RootElement;
            if (r.ValueKind != JsonValueKind.Object || !r.TryGetProperty("options", out var o) || o.ValueKind != JsonValueKind.Array) return Array.Empty<CustomModelBase>();
            return o.EnumerateArray().Where(b => b.ValueKind == JsonValueKind.Object && Str(b, "base_model") != null)
                .Select(b => new CustomModelBase(Str(b, "base_model")!, Str(b, "name") ?? Str(b, "base_model")!,
                    b.TryGetProperty("is_default", out var d) && d.ValueKind == JsonValueKind.True)).ToList();
        }

        /// <summary>Custom models Suno is still training.</summary>
        public async Task<IReadOnlyList<PendingCustomModel>> GetPendingCustomModelsAsync(CancellationToken ct = default)
        {
            using var doc = ParseDocument(await SendAsync(HttpMethod.Get, "/api/custom-model/pending/", null, ct));
            var r = doc.RootElement;
            if (r.ValueKind != JsonValueKind.Object || !r.TryGetProperty("pending_models", out var list) || list.ValueKind != JsonValueKind.Array) return Array.Empty<PendingCustomModel>();
            return list.EnumerateArray().Where(m => m.ValueKind == JsonValueKind.Object && Str(m, "id") != null)
                .Select(m => new PendingCustomModel(Str(m, "id")!, Str(m, "name") ?? "")).ToList();
        }

        /// <summary>Starts training a custom model on your songs (POST /api/custom-model/create/). Names are up to 16 characters.</summary>
        public async Task CreateCustomModelAsync(IReadOnlyList<string> clipIds, string name, string? baseModel, CancellationToken ct = default)
        {
            var body = new Dictionary<string, object> { ["clip_ids"] = clipIds, ["name"] = name };
            if (baseModel != null) body["base_model"] = baseModel;
            await SendAsync(HttpMethod.Post, "/api/custom-model/create/", body, ct);
        }

        public async Task RenameCustomModelAsync(string modelId, string name, CancellationToken ct = default) =>
            await SendAsync(HttpMethod.Put, "/api/custom-model/edit/", new { id = modelId, name }, ct);

        public async Task ArchiveCustomModelAsync(string modelId, CancellationToken ct = default) =>
            await SendAsync(HttpMethod.Post, "/api/custom-model/archive/", new { id = modelId }, ct);

        // ── Lyrics mashup and personalization ───────────────────────────────

        /// <summary>
        /// Suno's lyrics mashup: two sets of lyrics in, one out. POST /api/generate/lyrics-mashup with a fresh
        /// create_session_token (sunox makes a new UUID each time), then the lyrics status route until complete.
        /// </summary>
        public async Task<string> MashupLyricsAsync(string first, string second, CancellationToken ct = default)
        {
            var json = await SendAsync(HttpMethod.Post, "/api/generate/lyrics-mashup",
                new { lyrics_a = first, lyrics_b = second, create_session_token = Guid.NewGuid().ToString(), source = "create_ui" }, ct);
            var id = SunoJson.TryReadString(json, "mashup_id");
            if (string.IsNullOrEmpty(id)) throw new SunoApiException(T("Suno didn't start the mashup."));
            for (var attempt = 0; attempt < 30; attempt++)
            {
                await Task.Delay(PollInterval, ct);
                var result = Deserialize<LyricsResult>(await SendAsync(HttpMethod.Get, $"/api/generate/lyrics/{Uri.EscapeDataString(id)}", null, ct));
                if (result?.Status == "complete" && !string.IsNullOrWhiteSpace(result.Text)) return result.Text;
                if (result?.Status is "error" or "failed") throw new SunoApiException(T("Suno couldn't mash those lyrics up."));
            }
            throw new SunoApiException(T("Suno took too long writing the lyrics. Try again."));
        }

        /// <summary>Whether Suno shapes style ideas and boosts to your taste (GET /api/personalization/settings); null if unknown.</summary>
        public async Task<bool?> GetStylePersonalizationAsync(CancellationToken ct = default) =>
            SunoJson.ReadBool(await SendAsync(HttpMethod.Get, "/api/personalization/settings", null, ct), "styles_augmentation");

        private static string? Str(JsonElement e, string name) =>
            e.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.String && v.GetString() is { Length: > 0 } s ? s : null;

        private static double Num(JsonElement e, string name) =>
            e.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.Number ? v.GetDouble() : 0;
    }
}
