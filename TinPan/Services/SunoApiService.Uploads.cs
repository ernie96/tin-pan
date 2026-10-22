using System;
using System.Collections.Generic;
using System.Net.Http;
using System.Text.Json;
using System.Threading;
using System.Threading.Tasks;
using static TinPan.Localization.Loc;

namespace TinPan.Services
{
    /// <summary>Where and how to send an upload's bytes: a single-use signed storage POST.</summary>
    public sealed record AudioUploadTicket(string Id, Uri Url, IReadOnlyDictionary<string, string> Fields);

    /// <summary>An upload's processing state: "complete", "error", or still working.</summary>
    public sealed record AudioUploadState(string Status, string? Title, string? ErrorMessage);

    // Uploading audio. The transport (initialize, storage POST, upload-finish) follows
    // Nsomnia/chadvis-projectm-qt (ENDPOINT-INVENTORY 5.6, SunoAudioUploadService). The steps after it
    // (processing status, initialize-clip, set_metadata with is_audio_upload_tos_accepted) aren't in
    // chadvis or the local frontend yet, so they come from Suno's web app.
    public partial class SunoApiService
    {
        /// <summary>The only storage host an upload may go to (chadvis inventory, captured hosts).</summary>
        public const string UploadStorageHost = "suno-uploads.s3.amazonaws.com";

        private static readonly string[] RequiredUploadFields = { "AWSAccessKeyId", "Content-Type", "key", "policy", "signature" };

        public async Task<AudioUploadTicket> StartAudioUploadAsync(CancellationToken ct = default)
        {
            string json;
            try
            {
                // chadvis captured only M4A, so every upload is sent as M4A (UploadService converts the rest).
                json = await SendAsync(HttpMethod.Post, "/api/uploads/audio/", new { extension = "m4a", upload_type = "file_upload" }, ct);
            }
            catch (SunoApiException ex) when (ex.StatusCode == 423)
            {
                throw new SunoApiException(T("Suno doesn't allow uploads from this account right now."), 423, ex);
            }
            return ParseUploadTicket(json);
        }

        /// <summary>Checks the initializer's reply the way chadvis does: id, an allowed storage URL, and every signed field.</summary>
        public static AudioUploadTicket ParseUploadTicket(string json)
        {
            try
            {
                using var doc = JsonDocument.Parse(json);
                var root = doc.RootElement;
                if (root.ValueKind != JsonValueKind.Object
                    || !root.TryGetProperty("id", out var id) || id.ValueKind != JsonValueKind.String || id.GetString()!.Length == 0
                    || !root.TryGetProperty("url", out var url) || url.ValueKind != JsonValueKind.String
                    || !root.TryGetProperty("fields", out var fields) || fields.ValueKind != JsonValueKind.Object)
                    throw new SunoApiException(T("Suno didn't start the upload properly. Try again."));

                if (!Uri.TryCreate(url.GetString(), UriKind.Absolute, out var target) || !IsUploadStorage(target))
                    throw new SunoApiException(T("Suno gave an upload address this app won't use."));

                var values = new Dictionary<string, string>();
                foreach (var field in fields.EnumerateObject())
                {
                    if (field.Value.ValueKind != JsonValueKind.String) throw new SunoApiException(T("Suno didn't start the upload properly. Try again."));
                    values[field.Name] = field.Value.GetString()!;
                }
                foreach (var name in RequiredUploadFields)
                    if (!values.TryGetValue(name, out var v) || v.Length == 0) throw new SunoApiException(T("Suno didn't start the upload properly. Try again."));
                return new AudioUploadTicket(id.GetString()!, target, values);
            }
            catch (JsonException ex)
            {
                throw new SunoApiException(T("Suno sent a reply this app doesn't understand."), null, ex);
            }
        }

        public static bool IsUploadStorage(Uri url) =>
            url.Scheme == Uri.UriSchemeHttps && url.Host.Equals(UploadStorageHost, StringComparison.OrdinalIgnoreCase)
            && url.IsDefaultPort && string.IsNullOrEmpty(url.UserInfo) && string.IsNullOrEmpty(url.Fragment);

        /// <summary>chadvis's finish body: the file name, the upload type, and agreed_to_vip_upload_terms false.</summary>
        public async Task FinishAudioUploadAsync(string uploadId, string fileName, CancellationToken ct = default) =>
            await SendAsync(HttpMethod.Post, $"/api/uploads/audio/{Uri.EscapeDataString(uploadId)}/upload-finish/",
                new { agreed_to_vip_upload_terms = false, upload_filename = fileName, upload_type = "file_upload" }, ct);

        public async Task<AudioUploadState> GetAudioUploadAsync(string uploadId, CancellationToken ct = default)
        {
            var json = await SendAsync(HttpMethod.Get, $"/api/uploads/audio/{Uri.EscapeDataString(uploadId)}/", null, ct);
            return new AudioUploadState(SunoJson.TryReadString(json, "status") ?? "", SunoJson.TryReadString(json, "title"), SunoJson.TryReadString(json, "error_message"));
        }

        /// <summary>Turns a processed upload into a song in the library. Returns its clip id.</summary>
        public async Task<string> CreateClipFromUploadAsync(string uploadId, CancellationToken ct = default)
        {
            var json = await SendAsync(HttpMethod.Post, $"/api/uploads/audio/{Uri.EscapeDataString(uploadId)}/initialize-clip/", new { }, ct);
            return SunoJson.TryReadString(json, "clip_id") is { Length: > 0 } clipId
                ? clipId
                : throw new SunoApiException(T("Suno processed the upload but didn't make a song from it. Refresh the library to check."));
        }

        /// <summary>Names the uploaded song and records that the user accepted Suno's upload terms.</summary>
        public async Task NameUploadedSongAsync(string clipId, string title, CancellationToken ct = default) =>
            await SendAsync(HttpMethod.Post, $"/api/gen/{Uri.EscapeDataString(clipId)}/set_metadata/",
                new { title, is_audio_upload_tos_accepted = true }, ct);
    }
}
