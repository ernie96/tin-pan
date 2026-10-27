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
    /// <summary>
    /// Endpoints and headers follow paperfoot/suno-cli (checked against the live API in 2026),
    /// gcui-art/suno-api and Nsomnia/suno-api-local-manager-frontend.
    /// </summary>
    public partial class SunoApiService : ISunoApiService
    {

        private static readonly JsonSerializerOptions JsonOpts = LenientJson.Options(new() { PropertyNameCaseInsensitive = true });
        internal TimeSpan PollIntervalForTests { get; init; } = TimeSpan.FromSeconds(2);
        private TimeSpan PollInterval => PollIntervalForTests;

        private readonly HttpClient _http;
        private readonly IAuthService _auth;
        private readonly ICaptchaSolver _captcha;
        private bool _feedV3Missing;

        public SunoApiService(HttpClient http, IAuthService auth, ICaptchaSolver captcha)
        {
            _http = http;
            _auth = auth;
            _captcha = captcha;
        }

        // ── Account ─────────────────────────────────────────────────────────

        public async Task<BillingInfo> GetBillingInfoAsync(CancellationToken ct = default)
        {
            var info = Deserialize<BillingInfo>(await SendAsync(HttpMethod.Get, "/api/billing/info/", null, ct)) ?? new BillingInfo();
            _models = info.Models; // what each model can do, for voice personas
            return info;
        }

        private IReadOnlyList<SunoModel> _models = Array.Empty<SunoModel>();

        /// <summary>
        /// GET /api/notification/v3?include_hooks=true, what Suno's site reads since 2026-10-24 (chadvis); its
        /// entries are text segments. v2, the older list, if v3 isn't there.
        /// </summary>
        public async Task<IReadOnlyList<Notification>> GetNotificationsAsync(CancellationToken ct = default)
        {
            try
            {
                return SunoJson.ReadNotifications(await SendAsync(HttpMethod.Get, "/api/notification/v3?include_hooks=true", null, ct));
            }
            catch (SunoApiException ex) when (ex.StatusCode is 404 or 405)
            {
                Log.Warn("notification/v3 not available; using v2");
                return SunoJson.ReadNotifications(await SendAsync(HttpMethod.Get, "/api/notification/v2", null, ct));
            }
        }

        // ── Songs ───────────────────────────────────────────────────────────

        public async Task<FeedPage> GetLibraryPageAsync(LibraryQuery query, CancellationToken ct = default)
        {
            if (!_feedV3Missing)
            {
                var body = new Dictionary<string, object> { ["limit"] = 100, ["filters"] = BuildFeedFilters(query) };
                if (query.Cursor != null) body["cursor"] = query.Cursor;
                try
                {
                    return SunoJson.ReadFeedPage(await SendAsync(HttpMethod.Post, "/api/feed/v3", body, ct));
                }
                catch (SunoApiException ex) when (ex.StatusCode is 404 or 405)
                {
                    Log.Warn("feed/v3 not available; using feed/v2 pages");
                    _feedV3Missing = true;
                }
            }

            if (!string.IsNullOrWhiteSpace(query.Search))
                throw new SunoApiException(T("Searching isn't available from Suno right now."));
            return SunoJson.ReadFeedPage(await SendAsync(HttpMethod.Get, $"/api/feed/v2?page={query.Page}", null, ct));
        }

        /// <summary>
        /// The filter object Suno's web library sends: every filter with an explicit value, as in its
        /// bundle's default filter object. Hiding stems also hides their complements, as the web app does.
        /// </summary>
        public static Dictionary<string, object> BuildFeedFilters(LibraryQuery query)
        {
            var f = query.Filter ?? new LibraryFilter();
            var filters = new Dictionary<string, object> { ["disliked"] = FeedFilters.Valid(f.Disliked, FeedFilters.Hide) };
            foreach (var filter in FeedFilters.All)
            {
                var value = FeedFilters.Valid(f.Show.TryGetValue(filter.Field, out var v) ? v : null, filter.Default);
                filters[filter.Field] = filter.Presence ? new { presence = value } : value;
            }
            var stems = FeedFilters.Valid(f.Show.TryGetValue("stem", out var s) ? s : null, FeedFilters.Hide);
            filters["stemComplement"] = stems == FeedFilters.Hide ? FeedFilters.Hide : FeedFilters.Any;
            var (_, sortBy, direction) = FeedFilters.Sorts.FirstOrDefault(s => s.Id == f.Sort);
            filters["sort"] = new { sortBy = sortBy ?? "created_at", sortDirection = direction ?? "desc" };
            if (!string.IsNullOrEmpty(f.WorkspaceId)) filters["workspace"] = new { presence = "True", workspaceId = f.WorkspaceId };
            if (!string.IsNullOrWhiteSpace(query.Search)) filters["searchText"] = query.Search.Trim();
            return filters;
        }

        public async Task<IReadOnlyList<Workspace>> GetWorkspacesAsync(CancellationToken ct = default)
        {
            using var doc = ParseDocument(await SendAsync(HttpMethod.Get, "/api/project/me", null, ct));
            var list = new List<Workspace>();
            if (doc.RootElement.ValueKind == JsonValueKind.Object && doc.RootElement.TryGetProperty("projects", out var projects) && projects.ValueKind == JsonValueKind.Array)
            {
                foreach (var p in projects.EnumerateArray().Where(p => p.ValueKind == JsonValueKind.Object))
                {
                    var id = p.TryGetProperty("id", out var i) ? i.ToString() : "";
                    if (id.Length == 0) continue;
                    var name = p.TryGetProperty("name", out var n) && n.ValueKind == JsonValueKind.String ? n.GetString()! : "";
                    int? count = p.TryGetProperty("clip_count", out var c) ? SunoJson.AsInt(c) : null;
                    list.Add(new Workspace(id, name.Length > 0 ? name : id == "default" ? T("My Workspace") : T("Untitled workspace"), count));
                }
            }
            if (list.All(w => w.Id != "default")) list.Insert(0, new Workspace("default", T("My Workspace"), null));
            return list;
        }

        public async Task<IReadOnlyList<Persona>> GetPersonasAsync(CancellationToken ct = default)
        {
            using var doc = ParseDocument(await SendAsync(HttpMethod.Get, "/api/persona/get-personas/", null, ct));
            var list = new List<Persona>();
            if (doc.RootElement.ValueKind == JsonValueKind.Object && doc.RootElement.TryGetProperty("personas", out var personas) && personas.ValueKind == JsonValueKind.Array)
            {
                foreach (var p in personas.EnumerateArray().Where(p => p.ValueKind == JsonValueKind.Object))
                {
                    var id = p.TryGetProperty("id", out var i) ? i.ToString() : "";
                    var name = p.TryGetProperty("name", out var n) && n.ValueKind == JsonValueKind.String ? n.GetString()! : "";
                    if (id.Length > 0) list.Add(new Persona(id, name.Length > 0 ? name : T("Unnamed persona")));
                }
            }
            return list;
        }

        public async Task<IReadOnlyList<SunoClip>> GetClipsAsync(IEnumerable<string> ids, CancellationToken ct = default)
        {
            // Suno only answers reliably for two ids at a time.
            var result = new List<SunoClip>();
            foreach (var chunk in ids.Distinct().Chunk(2))
            {
                var list = string.Join(",", chunk.Select(Uri.EscapeDataString));
                result.AddRange(SunoJson.ReadFeedPage(await SendAsync(HttpMethod.Get, $"/api/feed/?ids={list}", null, ct)).Clips);
            }
            return result;
        }

        public async Task<IReadOnlyList<SunoClip>> GetTrendingAsync(CancellationToken ct = default)
        {
            // The same body Suno's Explore page sends.
            var body = new { feed_id = "trending", cursor = (string?)null, page_size = 50 };
            return SunoJson.FindClips(await SendAsync(HttpMethod.Post, "/api/unified/feed", body, ct));
        }

        public async Task SetDetailsAsync(string clipId, string? title, string? lyrics, CancellationToken ct = default)
        {
            var body = new Dictionary<string, object>();
            if (title != null) body["title"] = title;
            if (lyrics != null) body["lyrics"] = lyrics;
            if (body.Count == 0) return;
            await SendAsync(HttpMethod.Post, $"/api/gen/{Uri.EscapeDataString(clipId)}/set_metadata/", body, ct);
        }

        public async Task SetPublicAsync(string clipId, bool isPublic, CancellationToken ct = default) =>
            await SendAsync(HttpMethod.Post, $"/api/gen/{Uri.EscapeDataString(clipId)}/set_visibility/", new { is_public = isPublic }, ct);

        public async Task TrashAsync(string clipId, CancellationToken ct = default) =>
            await SendAsync(HttpMethod.Post, "/api/gen/trash", new { clip_ids = new[] { clipId }, trash = true }, ct);

        // ── Making songs ────────────────────────────────────────────────────

        public async Task<IReadOnlyList<SunoClip>> GenerateAsync(GenerateRequest request, CancellationToken ct = default)
        {
            if (!string.IsNullOrEmpty(request.PersonaId)) await ApplyPersonaAsync(request, ct);
            var provider = await CaptchaProviderNeededAsync(ct);
            if (provider != null)
            {
                Log.Info($"Captcha required (provider {provider})");
                (request.Token, var used) = await _captcha.SolveAsync(provider.Value);
                request.TokenProvider = used;
            }

            var json = await SendAsync(HttpMethod.Post, "/api/generate/v2-web/", request, ct);
            if (SunoJson.ReadString(json, "status")?.Equals("error", StringComparison.OrdinalIgnoreCase) == true)
                throw new SunoApiException(T("Suno turned this request down. Check the style and lyrics, then try again."));

            var clips = SunoJson.ReadFeedPage(json).Clips;
            if (clips.Count == 0)
                throw new SunoApiException(T("Suno didn't say whether it started. Refresh the library before trying again, so you don't pay twice."));
            return clips;
        }

        /// <summary>What a persona needs from get-persona: its source song and whether it's a voice persona.</summary>
        public sealed record PersonaInfo(string? RootClipId, bool IsVoice, bool IsTrashed, bool IsHidden, double? RootDuration);

        private const string EmptyClipId = "00000000-0000-0000-0000-000000000000";

        /// <summary>Reads get-persona the way sunox does: an empty or all-zero root is no source song; "vox" in any case,
        /// or is_vox_persona, is a voice persona; the root's length comes from the persona's own clip when it is the root.</summary>
        public static PersonaInfo ReadPersona(string json)
        {
            using var doc = JsonDocument.Parse(json);
            var r = doc.RootElement;
            var root = SunoJson.TryReadString(json, "root_clip_id")?.Trim();
            if (string.IsNullOrEmpty(root) || root == EmptyClipId) root = null;
            var voice = string.Equals(SunoJson.TryReadString(json, "persona_type"), "vox", StringComparison.OrdinalIgnoreCase)
                || SunoJson.ReadBool(json, "is_vox_persona") == true;
            double? duration = null;
            if (root != null && r.ValueKind == JsonValueKind.Object && r.TryGetProperty("clip", out var clip) && clip.ValueKind == JsonValueKind.Object
                && clip.TryGetProperty("id", out var id) && id.ValueKind == JsonValueKind.String && id.GetString() == root
                && clip.TryGetProperty("metadata", out var meta) && meta.ValueKind == JsonValueKind.Object
                && meta.TryGetProperty("duration", out var d) && d.ValueKind == JsonValueKind.Number && d.GetDouble() is >= 0 and var seconds
                && double.IsFinite(seconds))
                duration = seconds;
            return new PersonaInfo(root, voice, SunoJson.ReadBool(json, "is_trashed") == true, SunoJson.ReadBool(json, "is_hidden") == true, duration);
        }

        /// <summary>
        /// A persona only takes effect with its task and its source song, as Suno's Create page sends them: it reads
        /// the persona (GET /api/persona/get-persona/{id}/), then sends task "artist_consistency" ("vox" for a voice
        /// persona) with artist_clip_id and the source song's range, artist_start_s 0 to artist_end_s its length; a
        /// cover or extend becomes "artist_cover"/"vox_cover" or "artist_extend"/"vox_extend" (Suno's own task list).
        /// persona_id alone is ignored, and the song comes out as if no persona had been chosen. The checks (trashed or
        /// hidden, no source song, voice personas on older models) follow sunox's src/commands/create/submit.rs.
        /// </summary>
        private async Task ApplyPersonaAsync(GenerateRequest request, CancellationToken ct)
        {
            PersonaInfo persona;
            try
            {
                persona = ReadPersona(await SendAsync(HttpMethod.Get, $"/api/persona/get-persona/{Uri.EscapeDataString(request.PersonaId!)}/", null, ct));
            }
            catch (Exception ex) when (ex is JsonException || ex is SunoApiException and not SunoSignInRequiredException)
            {
                Log.Warn($"Persona {request.PersonaId} details didn't load ({ex.Message}); sending it without its source song");
                return;
            }
            if (persona is { RootClipId: { } root, RootDuration: null })
            {
                try { persona = persona with { RootDuration = (await GetClipsAsync(new[] { root }, ct)).FirstOrDefault(c => c.Id == root)?.Duration }; }
                catch (SunoApiException ex) when (ex is not SunoSignInRequiredException) { Log.Warn($"Persona source song length didn't load: {ex.Message}"); }
            }
            PersonaTask(request, persona, _models.FirstOrDefault(m => m.ExternalKey == request.Model)?.TakesVoicePersonas);
            Log.Info($"Persona {request.PersonaId}: task {request.Task}, source song {(request.ArtistClipId ?? "none")} 0 to {request.ArtistEnd?.ToString() ?? "?"} s");
        }

        /// <summary>Sets the persona's task, source song and range on a request (see ApplyPersonaAsync).</summary>
        /// <param name="modelTakesVoice">Whether the chosen model does voice personas; null when Suno didn't say.</param>
        public static void PersonaTask(GenerateRequest request, PersonaInfo persona, bool? modelTakesVoice = null)
        {
            if (persona.IsTrashed || persona.IsHidden)
                throw new SunoApiException(T("That persona is in the trash or hidden, so Suno can't use it. Choose another persona."));
            var voice = persona.IsVoice;
            if (voice && modelTakesVoice == false)
            {
                // An older model can't do voice personas; one with a source song still works as an ordinary persona.
                if (persona.RootClipId == null)
                    throw new SunoApiException(T("The model you chose can't use voice personas. Choose a newer model."));
                voice = false;
            }
            if (!voice && persona.RootClipId == null)
                throw new SunoApiException(T("That persona has no source song, so Suno can't use it. Choose another persona."));
            request.ArtistClipId = persona.RootClipId;
            request.ArtistStart = 0;
            request.ArtistEnd = persona.RootClipId == null ? null : persona.RootDuration;
            request.Task = request.Task switch
            {
                "cover" => voice ? "vox_cover" : "artist_cover",
                "extend" => voice ? "vox_extend" : "artist_extend",
                null or "" => voice ? "vox" : "artist_consistency",
                var other => other
            };
        }

        /// <summary>Null when Suno needs no captcha; otherwise 1 (hCaptcha) or 2 (Turnstile).</summary>
        private async Task<int?> CaptchaProviderNeededAsync(CancellationToken ct)
        {
            string check;
            try
            {
                check = await SendAsync(HttpMethod.Post, "/api/c/check", new { ctype = "generation" }, ct);
            }
            catch (SunoApiException ex) when (ex.StatusCode is 404 or 405)
            {
                return null; // No captcha gate on this API version; just try.
            }
            if (SunoJson.ReadBool(check, "required") != true) return null;
            return SunoJson.ReadInt(check, "captcha_version") is 2 ? 2 : 1;
        }

        public async Task<IReadOnlyList<SunoClip>> RemasterAsync(RemasterRequest request, CancellationToken ct = default)
        {
            var clips = SunoJson.FindClipsById(await SendAsync(HttpMethod.Post, "/api/generate/upsample", BuildRemasterBody(request), ct));
            if (clips.Count == 0)
                throw new SunoApiException(T("Suno didn't say whether the remaster started. Refresh the library before trying again."));
            return clips;
        }

        /// <summary>Same body Suno's web Remaster builds: defaults are left out, sliders clamped to 0..1.</summary>
        public static Dictionary<string, object> BuildRemasterBody(RemasterRequest r)
        {
            var body = new Dictionary<string, object> { ["clip_id"] = r.ClipId };
            if (!string.IsNullOrEmpty(r.Model)) body["model_name"] = r.Model;
            if (!string.IsNullOrWhiteSpace(r.Style)) body["tags"] = r.Style.Trim();
            if (!string.IsNullOrEmpty(r.Strength)) body["variation_category"] = r.Strength;
            if (!string.IsNullOrEmpty(r.Profile)) body["style_profile"] = r.Profile;
            foreach (var (name, value, fallback) in new[]
                     {
                         ("freedom", r.Freedom, 0.0), ("tone", r.Tone, 0.5), ("clarity", r.Clarity, 0.5),
                         ("strength", r.FineStrength, 0.5), ("stereo_width", r.StereoWidth, 0.5)
                     })
            {
                if (value is not { } v) continue;
                var clamped = Math.Clamp(v, 0, 1);
                if (Math.Abs(clamped - fallback) > 1e-9) body[name] = clamped;
            }
            return body;
        }

        public async Task<SunoClip?> ConcatAsync(string clipId, CancellationToken ct = default) =>
            Deserialize<SunoClip>(await SendAsync(HttpMethod.Post, "/api/generate/concat/v2/", new { clip_id = clipId }, ct));

        public async Task<IReadOnlyList<SunoClip>> GenerateStemsAsync(string clipId, CancellationToken ct = default)
        {
            var json = await SendAsync(HttpMethod.Post, $"/api/edit/stems/{Uri.EscapeDataString(clipId)}", new { }, ct);
            var clips = SunoJson.FindClipsById(json);
            return clips;
        }

        public async Task<LyricsResult> GenerateLyricsAsync(string topic, CancellationToken ct = default)
        {
            var submitted = Deserialize<LyricsResult>(await SendAsync(HttpMethod.Post, "/api/generate/lyrics/", new { prompt = topic }, ct));
            if (string.IsNullOrEmpty(submitted?.Id)) throw new SunoApiException(T("Suno didn't start writing the lyrics."));

            for (var attempt = 0; attempt < 30; attempt++)
            {
                await Task.Delay(PollInterval, ct);
                var result = Deserialize<LyricsResult>(await SendAsync(HttpMethod.Get, $"/api/generate/lyrics/{Uri.EscapeDataString(submitted.Id)}", null, ct));
                if (!string.IsNullOrWhiteSpace(result?.ErrorMessage)) throw new SunoApiException(T("Suno couldn't write those lyrics: {0}", result.ErrorMessage));
                if (result?.Status == "complete") return result;
                if (result?.Status == "error") throw new SunoApiException(T("Suno couldn't write lyrics for that topic."));
            }
            throw new SunoApiException(T("Suno took too long writing the lyrics. Try again."));
        }

        /// <summary>
        /// The song's timed words: Suno's stored ones (v2), or, when it has none but the song has lyrics, a fresh
        /// timing of those lyrics with the newer route (v3, as sunox's aligned_lyrics does).
        /// </summary>
        public async Task<AlignedLyrics?> GetAlignedLyricsAsync(string clipId, CancellationToken ct = default)
        {
            AlignedLyrics? stored = null;
            try { stored = Deserialize<AlignedLyrics>(await SendAsync(HttpMethod.Get, $"/api/gen/{Uri.EscapeDataString(clipId)}/aligned_lyrics/v2/", null, ct)); }
            catch (SunoApiException ex) when (ex.StatusCode == 404) { }
            if (stored?.Words.Count > 0) return stored;
            var lyrics = (await GetClipsAsync(new[] { clipId }, ct)).FirstOrDefault(c => c.Id == clipId)?.Metadata?.Prompt?.Trim();
            if (string.IsNullOrEmpty(lyrics)) return stored;
            try { return await AlignLyricsAsync(clipId, lyrics, ct) ?? stored; }
            catch (SunoApiException ex) when (ex is not SunoSignInRequiredException)
            {
                Log.Warn($"Timing the lyrics of {clipId} (v3) didn't work: {ex.Message}");
                return stored;
            }
        }

        // ── Files ───────────────────────────────────────────────────────────

        public async Task<string> GetWavUrlAsync(string clipId, CancellationToken ct = default)
        {
            var id = Uri.EscapeDataString(clipId);
            var requested = false;
            for (var attempt = 0; attempt < 90; attempt++)
            {
                var json = await SendAsync(HttpMethod.Get, $"/api/gen/{id}/wav_file/", null, ct);
                var url = SunoJson.ReadString(json, "wav_file_url");
                if (!string.IsNullOrEmpty(url)) return url;
                if (!requested)
                {
                    await SendAsync(HttpMethod.Post, $"/api/gen/{id}/convert_wav/", new { }, ct);
                    requested = true;
                }
                await Task.Delay(PollInterval, ct);
            }
            throw new SunoApiException(T("Suno took too long making the WAV file. Try again later."));
        }

        public async Task AuthorizeDownloadAsync(string clipId, CancellationToken ct = default)
        {
            var json = await SendAsync(HttpMethod.Post, "/api/download/authorize", new { item_id = clipId, item_type = "clip" }, ct);
            if (SunoJson.ReadBool(json, "ok") != true)
                throw new SunoApiException(T("Suno didn't confirm the download. Check your downloads left with Shift+F2."));
        }

        public async Task<string> PrepareDownloadAsync(string clipId, string format, bool studio, CancellationToken ct = default)
        {
            var id = Uri.EscapeDataString(clipId);
            var path = studio
                ? $"/api/studio/clip/{id}/download?format={Uri.EscapeDataString(format)}"
                : $"/api/download/clip/{id}?format={Uri.EscapeDataString(format)}";

            // Suno answers "processing" until the file is ready, for up to a few minutes.
            for (var attempt = 0; attempt < 90; attempt++)
            {
                var json = await SendAsync(HttpMethod.Get, path, null, ct);
                var status = SunoJson.ReadString(json, "status");
                var url = SunoJson.ReadString(json, "download_url");
                if (status == "ready" && !string.IsNullOrEmpty(url)) return url;
                var reason = SunoJson.ReadString(json, "reason");
                if (status != "processing" && status != "rate_limited" && reason != "rate_limited")
                {
                    var detail = SunoJson.ReadString(json, "detail") ?? reason;
                    throw new SunoApiException(string.IsNullOrWhiteSpace(detail)
                        ? T("Suno couldn't prepare that download.")
                        : T("Suno couldn't prepare that download: {0}", detail));
                }
                await Task.Delay(PollInterval, ct);
            }
            throw new SunoApiException(T("Suno took too long preparing the download. Try again later."));
        }

        public async Task DownloadAsync(string url, string destinationPath, CancellationToken ct = default)
        {
            if (!url.StartsWith("https://", StringComparison.OrdinalIgnoreCase))
                throw new SunoApiException(T("Suno gave a download address this app won't use."));

            var temp = destinationPath + ".part";
            try
            {
                // CDN files need no sign-in, so no token is sent here.
                using var response = await _http.GetAsync(url, HttpCompletionOption.ResponseHeadersRead, ct);
                if (!response.IsSuccessStatusCode)
                    throw new SunoApiException(T("The download failed ({0}).", (int)response.StatusCode), (int)response.StatusCode);

                await using (var file = File.Create(temp))
                    await response.Content.CopyToAsync(file, ct);
                File.Move(temp, destinationPath, overwrite: true);
            }
            catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
            {
                throw new SunoApiException(T("Couldn't save the file. Check the download folder in Settings."), null, ex);
            }
            catch (HttpRequestException ex)
            {
                throw new SunoApiException(T("Can't reach Suno. Check your internet connection."), null, ex);
            }
            catch (TaskCanceledException ex) when (!ct.IsCancellationRequested)
            {
                throw new SunoApiException(T("The download didn't start in time. Try again."), null, ex);
            }
            finally
            {
                try { if (File.Exists(temp)) File.Delete(temp); }
                catch (Exception ex) when (ex is IOException or UnauthorizedAccessException) { Log.Warn($"Couldn't remove a partial download: {ex.Message}"); }
            }
        }

        public async Task DownloadClipDirectAsync(SunoClip clip, string destinationPath, CancellationToken ct = default)
        {
            var (mediaUrl, isEncrypted) = SunoDrm.PickMediaUrl(clip, preferUnencrypted: true);
            if (!isEncrypted && !string.IsNullOrEmpty(mediaUrl) && !SunoDrm.IsForbiddenDecoy(mediaUrl))
            {
                try
                {
                    await DownloadAsync(mediaUrl, destinationPath, ct);
                    return;
                }
                catch (SunoApiException ex) when (ex.StatusCode == 403)
                {
                    Log.Warn($"Unencrypted direct URL was refused (403) for clip {clip.Id}; falling back to Mango DRM.");
                }
            }

            await DownloadMangoAsync(clip, mediaUrl, destinationPath, ct);
        }

        private async Task DownloadMangoAsync(SunoClip clip, string? mediaUrl, string destinationPath, CancellationToken ct)
        {
            var token = await _auth.GetTokenAsync(forceRefresh: false, ct);
            var rights = await FetchRightsAsync(clip.Id, token, ct);

            var targetUrl = !string.IsNullOrEmpty(mediaUrl) && !SunoDrm.IsForbiddenDecoy(mediaUrl)
                ? mediaUrl
                : (!string.IsNullOrEmpty(rights.MediaUrl) && !SunoDrm.IsForbiddenDecoy(rights.MediaUrl)
                    ? rights.MediaUrl
                    : SunoDrm.DefaultMediaUrl(clip.Id));

            var (contentKey, contentIv) = SunoDrm.UnwrapRights(rights, clip.Id, token);

            var temp = destinationPath + ".part";
            try
            {
                using var response = await _http.GetAsync(targetUrl, HttpCompletionOption.ResponseHeadersRead, ct);
                if (!response.IsSuccessStatusCode)
                    throw new SunoApiException(T("The download failed ({0}).", (int)response.StatusCode), (int)response.StatusCode);

                await using (var networkStream = await response.Content.ReadAsStreamAsync(ct))
                await using (var fileStream = File.Create(temp))
                {
                    await SunoDrm.DecryptAesCtrAsync(networkStream, contentKey, contentIv, fileStream, ct);
                }

                await using (var checkStream = File.OpenRead(temp))
                {
                    var header = new byte[32];
                    var read = await checkStream.ReadAsync(header, ct);
                    var container = SunoDrm.DetectContainer(header.AsSpan(0, read));
                    if (container == "unknown")
                    {
                        throw new SunoApiException(T("The free file didn't download: it isn't a song file Tin Pan recognizes."));
                    }
                }

                File.Move(temp, destinationPath, overwrite: true);
            }
            catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
            {
                throw new SunoApiException(T("Couldn't save the file. Check the download folder in Settings."), null, ex);
            }
            catch (HttpRequestException ex)
            {
                throw new SunoApiException(T("Can't reach Suno. Check your internet connection."), null, ex);
            }
            catch (TaskCanceledException ex) when (!ct.IsCancellationRequested)
            {
                throw new SunoApiException(T("The download didn't start in time. Try again."), null, ex);
            }
            finally
            {
                try { if (File.Exists(temp)) File.Delete(temp); }
                catch (Exception ex) when (ex is IOException or UnauthorizedAccessException) { Log.Warn($"Couldn't remove a partial download: {ex.Message}"); }
            }
        }

        private async Task<SunoRights> FetchRightsAsync(string clipId, string token, CancellationToken ct)
        {
            var shapes = new object[]
            {
                new { content_params = new { content_id = clipId, content_type = "clip" } },
                new { content_id = clipId, content_type = "clip" },
                new { content_id = clipId }
            };

            var hosts = HostOrder;
            int lastStatus = 0;

            foreach (var host in hosts)
            {
                foreach (var body in shapes)
                {
                    using var req = new HttpRequestMessage(HttpMethod.Post, host + "/api/mango/rights");
                    AddSunoHeaders(req, token);
                    if (host == Hosts[0]) req.Headers.TryAddWithoutValidation("Cookie", "__session=" + token);
                    req.Content = new StringContent(JsonSerializer.Serialize(body), Encoding.UTF8, "application/json");

                    HttpResponseMessage resp;
                    try
                    {
                        resp = await _http.SendAsync(req, ct);
                    }
                    catch (Exception ex) when (ex is HttpRequestException or TaskCanceledException)
                    {
                        continue;
                    }

                    using (resp)
                    {
                        lastStatus = (int)resp.StatusCode;
                        if (!resp.IsSuccessStatusCode) continue;

                        var json = await resp.Content.ReadAsStringAsync(ct);
                        var rights = SunoDrm.ParseRights(json, clipId);
                        if (rights != null && !string.IsNullOrEmpty(rights.Key) && !string.IsNullOrEmpty(rights.Iv))
                        {
                            return rights;
                        }
                    }
                }
            }

            throw new SunoApiException(T("The download failed ({0}).", lastStatus != 0 ? lastStatus : 403), lastStatus != 0 ? lastStatus : 403);
        }

        // ── Plumbing ────────────────────────────────────────────────────────

        /// <summary>How long to wait before retry n (1 or 2) after "too many requests", when Suno doesn't say.</summary>
        internal Func<int, TimeSpan> RateLimitWait { get; init; } = attempt => TimeSpan.FromSeconds(attempt == 1 ? 2 : 5);

        private const int RateLimitRetries = 2;

        /// <summary>
        /// Where the API answers, in order. Suno's site moved to suno.com/a/api on 2026-10-24 (chadvis
        /// ENDPOINT-INVENTORY: feed v3, unified/library and notification v3 there); the old host still
        /// answered on 2026-10-21 and is kept as the fallback. Paths keep their "/api/..." spelling.
        /// </summary>
        public static readonly string[] Hosts = { "https://suno.com/a", "https://studio-api-prod.suno.com" };

        /// <summary>The old host, used for the paths' own fallback and the rights call.</summary>
        public const string BaseUrl = "https://studio-api-prod.suno.com";

        // After the new host fails and the old one answers, the old one goes first for a while instead of
        // every request paying for a failed try; then the new one is tried again.
        private static readonly TimeSpan PreferOldFor = TimeSpan.FromMinutes(10);
        private DateTime _oldFirstUntil = DateTime.MinValue;
        private string? _lastHost;

        /// <summary>The hosts in the order to try them now.</summary>
        internal IReadOnlyList<string> HostOrder => DateTime.UtcNow < _oldFirstUntil ? new[] { Hosts[1], Hosts[0] } : Hosts;

        private async Task<string> SendAsync(HttpMethod method, string path, object? body, CancellationToken ct, bool retryAuth = true, int rateLimited = 0)
        {
            var token = await _auth.GetTokenAsync(forceRefresh: !retryAuth, ct);
            var hosts = HostOrder;
            for (var i = 0; i < hosts.Count; i++)
            {
                var host = hosts[i];
                var last = i == hosts.Count - 1;
                using var request = new HttpRequestMessage(method, host + path);
                AddSunoHeaders(request, token);
                // The site host reads the signed-in session as the browser sends it, a __session cookie.
                if (host == Hosts[0]) request.Headers.TryAddWithoutValidation("Cookie", "__session=" + token);
                if (body != null)
                    request.Content = new StringContent(JsonSerializer.Serialize(body), Encoding.UTF8, "application/json");

                HttpResponseMessage response;
                try
                {
                    response = await _http.SendAsync(request, ct);
                }
                catch (HttpRequestException ex)
                {
                    // No connection to this host. Only reads move on: a POST may already have arrived.
                    if (!last && method == HttpMethod.Get)
                    {
                        Log.Warn($"{method} {Queryless(path)} couldn't reach {host}; trying {hosts[i + 1]}");
                        continue;
                    }
                    Log.Error($"{method} {Queryless(path)} failed", ex);
                    throw new SunoApiException(T("Can't reach Suno. Check your internet connection."), null, ex);
                }
                catch (TaskCanceledException ex) when (!ct.IsCancellationRequested)
                {
                    Log.Error($"{method} {Queryless(path)} timed out", ex);
                    // A request that changes something may still have gone through; making songs costs credits.
                    throw new SunoApiException(method == HttpMethod.Get
                        ? T("Suno didn't answer in time. Try again.")
                        : T("Suno didn't answer in time. It may still have done it, so refresh to check before trying again."), null, ex);
                }

                using (response)
                {
                    var text = await response.Content.ReadAsStringAsync(ct);
                    var status = (int)response.StatusCode;
                    if (!last && NotTakenHere(method, status, text, host == Hosts[0]))
                    {
                        Log.Warn($"{method} {Queryless(path)} on {host} answered {status}; trying {hosts[i + 1]}");
                        continue;
                    }
                    if (i > 0 && host == Hosts[1] && response.IsSuccessStatusCode) _oldFirstUntil = DateTime.UtcNow + PreferOldFor;
                    if (host != _lastHost)
                    {
                        _lastHost = host;
                        Log.Info($"Suno API answering on {host}");
                    }
                    if (response.IsSuccessStatusCode) return text;

                    Log.Warn($"{method} {Queryless(path)} returned {status}: {Truncate(text.ReplaceLineEndings(" "), 300)}");

                    // Suno rejects tokens it considers stale even before they expire; one fresh token fixes it.
                    if (retryAuth && (status is 401 or 403 || LooksLikeStaleToken(text)))
                        return await SendAsync(method, path, body, ct, retryAuth: false, rateLimited);

                    // Too many requests (a burst at startup, say): Suno turned it away without doing it, so when it
                    // says it may be retried, wait and try again rather than failing the whole load.
                    if (status == 429 && rateLimited < RateLimitRetries && (RetryableRateLimit(text) || response.Headers.RetryAfter != null))
                    {
                        var wait = response.Headers.RetryAfter?.Delta is { } delta && delta <= TimeSpan.FromSeconds(10) ? delta : RateLimitWait(rateLimited + 1);
                        Log.Info($"{method} {Queryless(path)}: too many requests, trying again in {wait.TotalSeconds:0.#} s");
                        await Task.Delay(wait, ct);
                        return await SendAsync(method, path, body, ct, retryAuth, rateLimited + 1);
                    }

                    throw new SunoApiException(DescribeFailure(response.StatusCode, text), status);
                }
            }
            throw new SunoApiException(T("Can't reach Suno. Check your internet connection."));
        }

        /// <summary>
        /// The host didn't take the request, so the other may: the route isn't there (404, 405: nothing was done,
        /// so even a song request is safe to send on), the site host doesn't accept this sign-in (401), or it was
        /// turned away before reaching Suno (a web page instead of JSON, as a firewall or gateway sends). Reads
        /// also move on from a server error page. Any other JSON answer is Suno's own and is never sent elsewhere.
        /// </summary>
        public static bool NotTakenHere(HttpMethod method, int status, string body, bool siteHost)
        {
            if (status is >= 200 and < 300) return false;
            if (status is 404 or 405) return true;
            if (status == 401 && siteHost) return true;
            var looksLikeJson = body.TrimStart() is { Length: > 0 } t && (t[0] == '{' || t[0] == '[');
            if (looksLikeJson) return false;
            return status is 403 or 421 or 502 or 503 or 520 or 521 or 522 or 523 or 524 || (method == HttpMethod.Get && status >= 500);
        }

        private void AddSunoHeaders(HttpRequestMessage request, string token)
        {
            var h = request.Headers;
            h.TryAddWithoutValidation("Authorization", "Bearer " + token);
            h.TryAddWithoutValidation("Device-Id", _auth.DeviceId);
            h.TryAddWithoutValidation("Browser-Token", BrowserToken(DateTimeOffset.UtcNow));
            h.TryAddWithoutValidation("Origin", "https://suno.com");
            h.TryAddWithoutValidation("Referer", "https://suno.com/");
            h.TryAddWithoutValidation("Accept", "application/json");
        }

        /// <summary>{"token":"base64 of {"timestamp":ms}"}, as the Suno web app sends.</summary>
        public static string BrowserToken(DateTimeOffset now)
        {
            var payload = $"{{\"timestamp\":{now.ToUnixTimeMilliseconds()}}}";
            return $"{{\"token\":\"{Convert.ToBase64String(Encoding.UTF8.GetBytes(payload))}\"}}";
        }

        /// <summary>Suno's 429 body says {"retryable": true} when the request may simply be sent again.</summary>
        public static bool RetryableRateLimit(string body)
        {
            try
            {
                using var doc = JsonDocument.Parse(body);
                return doc.RootElement.ValueKind == JsonValueKind.Object
                       && doc.RootElement.TryGetProperty("retryable", out var r) && r.ValueKind == JsonValueKind.True;
            }
            catch (JsonException)
            {
                return false;
            }
        }

        public static bool LooksLikeStaleToken(string body)
        {
            var lower = body.ToLowerInvariant();
            return lower.Contains("token validation failed") || lower.Contains("jwt expired") || lower.Contains("jwt is expired")
                   || lower.Contains("invalid jwt") || lower.Contains("not authenticated") || lower.Contains("unauthenticated");
        }

        private static string DescribeFailure(HttpStatusCode code, string body)
        {
            var lower = body.ToLowerInvariant();
            return (int)code switch
            {
                401 => T("Suno turned the request down. If it keeps happening, sign in again from Settings."),
                403 when System.Text.RegularExpressions.Regex.IsMatch(lower, @"\b(pro|premier|plan)\b|subscri") =>
                    T("That needs a Pro or Premier plan."),
                403 => T("Suno says this account can't do that."),
                402 => T("Not enough credits."),
                404 => T("Suno couldn't find that song, or this feature moved."),
                400 or 422 when lower.Contains("credit") => T("Not enough credits."),
                400 or 422 when lower.Contains("moderat") || lower.Contains("inappropriate") || lower.Contains("policy") =>
                    T("Suno's content rules blocked this. Try different words."),
                400 or 422 when lower.Contains("captcha") => T("Suno didn't accept the captcha. Try again."),
                400 or 422 when lower.Contains("length") || lower.Contains("too long") => T("Something is too long for this model."),
                400 or 422 => T("Suno didn't accept those settings.") + DetailFrom(body),
                429 => T("Suno says too many requests. Wait a minute and try again."),
                >= 500 => T("Suno had a problem on its side ({0}). Try again later.", (int)code),
                _ => T("Suno refused the request ({0}).", (int)code)
            };
        }

        /// <summary>Suno's own "detail" text, when it is short enough to read out.</summary>
        private static string DetailFrom(string body)
        {
            var detail = SunoJson.TryReadString(body, "detail");
            return string.IsNullOrWhiteSpace(detail) || detail.Length > 160 ? "" : " " + T("Suno says: {0}", detail.Trim());
        }

        private static TResult? Deserialize<TResult>(string json) where TResult : class
        {
            try { return JsonSerializer.Deserialize<TResult>(json, JsonOpts); }
            catch (JsonException ex)
            {
                Log.Error($"Unexpected reply for {typeof(TResult).Name}", ex);
                throw new SunoApiException(T("Suno sent a reply this app doesn't understand."), null, ex);
            }
        }

        private static JsonDocument ParseDocument(string json)
        {
            try { return JsonDocument.Parse(json); }
            catch (JsonException ex) { throw new SunoApiException(T("Suno sent a reply this app doesn't understand."), null, ex); }
        }

        private static string Queryless(string path) => path.Split('?')[0];

        private static string Truncate(string s, int max) => s.Length <= max ? s : s[..max];
    }
}
