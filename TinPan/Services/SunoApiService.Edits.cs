using System;
using System.Collections.Generic;
using System.Net.Http;
using System.Threading;
using System.Threading.Tasks;
using TinPan.Models;
using static TinPan.Localization.Loc;

namespace TinPan.Services
{
    // Editing a song into a new one: speed, reverse, crop and fade. Routes and bodies follow
    // Nsomnia/sunox-chadvis-suno-frontend-projectm-qt-research (src/api/speed.rs, src/api/edit.rs).
    // Speed and reverse answer with the new clip; crop and fade answer with action_clip_id, which is
    // the new clip's id, so the song watcher follows it like any other song Suno is making.
    public partial class SunoApiService
    {
        public async Task<SunoClip> ChangeSpeedAsync(string clipId, double multiplier, bool keepPitch, string title, CancellationToken ct = default) =>
            NewClip(await SendAsync(HttpMethod.Post, "/api/clips/adjust-speed/",
                new { clip_id = clipId, speed_multiplier = multiplier, keep_pitch = keepPitch, title }, ct));

        public async Task<SunoClip> ReverseAsync(string clipId, string title, CancellationToken ct = default) =>
            NewClip(await SendAsync(HttpMethod.Post, "/api/clips/reverse-clip/", new { clip_id = clipId, title }, ct));

        public async Task<SunoClip> CropAsync(string clipId, double start, double end, bool removeSection, string title, CancellationToken ct = default) =>
            ActionClip(await SendAsync(HttpMethod.Post, $"/api/edit/crop/{Uri.EscapeDataString(clipId)}/",
                new { crop_start_s = start, crop_end_s = end, is_crop_remove = removeSection, title, ui_surface = "song_actions" }, ct), title);

        public async Task<SunoClip> FadeAsync(string clipId, double? fadeIn, double? fadeOut, string title, CancellationToken ct = default)
        {
            var body = new Dictionary<string, object> { ["title"] = title };
            if (fadeIn is > 0) body["fade_in_time"] = fadeIn.Value;
            if (fadeOut is > 0) body["fade_out_time"] = fadeOut.Value;
            return ActionClip(await SendAsync(HttpMethod.Post, $"/api/edit/fade/{Uri.EscapeDataString(clipId)}/", body, ct), title);
        }

        private static SunoClip NewClip(string json) =>
            Deserialize<SunoClip>(json) is { Id.Length: > 0 } clip
                ? clip
                : throw new SunoApiException(T("Suno didn't say which song it made. Refresh the library to check."));

        /// <summary>A stand-in for the clip a crop or fade is making, until the watcher reads the real one.</summary>
        internal static SunoClip ActionClip(string json, string title) =>
            SunoJson.TryReadString(json, "action_clip_id") is { } id && !string.IsNullOrWhiteSpace(id)
                ? new SunoClip { Id = id.Trim(), Title = title, Status = "submitted" }
                : throw new SunoApiException(T("Suno didn't say which song it made. Refresh the library to check."));
    }
}
