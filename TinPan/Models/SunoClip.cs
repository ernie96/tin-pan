using System;
using System.Collections.Generic;
using System.Linq;
using System.Text.Json;
using System.Text.Json.Serialization;
using CommunityToolkit.Mvvm.ComponentModel;
using static TinPan.Localization.Loc;

namespace TinPan.Models
{
    public class SunoClip : ObservableObject
    {
        /// <summary>
        /// Takes newer data for the same clip without replacing the list item, so a screen reader
        /// keeps its place when a song finishes generating.
        /// </summary>
        public void UpdateFrom(SunoClip newer)
        {
            if (newer.Id != Id) return;
            Title = newer.Title;
            AudioUrl = newer.AudioUrl;
            VideoUrl = newer.VideoUrl;
            ImageUrl = newer.ImageUrl;
            Status = newer.Status;
            ModelName = newer.ModelName;
            CreatedAt = newer.CreatedAt;
            IsPublic = newer.IsPublic;
            PlayCount = newer.PlayCount;
            UpvoteCount = newer.UpvoteCount;
            IsDownloadUnlocked = newer.IsDownloadUnlocked;
            IsLiked = newer.IsLiked;
            MediaUrls = newer.MediaUrls;
            ImageLargeUrl = newer.ImageLargeUrl;
            Metadata = newer.Metadata;
            OnPropertyChanged(string.Empty);
        }

        /// <summary>Lets the app refresh the spoken text after changing a field itself.</summary>
        public void NotifyChanged() => OnPropertyChanged(string.Empty);

        [JsonPropertyName("id")]
        public string Id { get; set; } = string.Empty;

        [JsonPropertyName("title")]
        public string? Title { get; set; }

        [JsonPropertyName("audio_url")]
        public string? AudioUrl { get; set; }

        [JsonPropertyName("video_url")]
        public string? VideoUrl { get; set; }

        [JsonPropertyName("image_url")]
        public string? ImageUrl { get; set; }

        /// <summary>submitted, queued, streaming, complete or error.</summary>
        [JsonPropertyName("status")]
        public string? Status { get; set; }

        [JsonPropertyName("model_name")]
        public string? ModelName { get; set; }

        [JsonPropertyName("created_at")]
        public DateTimeOffset? CreatedAt { get; set; }

        [JsonPropertyName("is_public")]
        public bool IsPublic { get; set; }

        [JsonPropertyName("play_count")]
        public int PlayCount { get; set; }

        [JsonPropertyName("upvote_count")]
        public int UpvoteCount { get; set; }

        [JsonPropertyName("metadata")]
        public ClipMetadata? Metadata { get; set; }

        [JsonPropertyName("image_large_url")]
        public string? ImageLargeUrl { get; set; }

        /// <summary>Your own like on this song.</summary>
        [JsonPropertyName("is_liked")]
        public bool IsLiked { get; set; }

        /// <summary>
        /// The real files. audio_url is a decoy (/api/forbidden); an entry here without an
        /// "encoding" field is a plain, unencrypted file that downloads for free.
        /// </summary>
        [JsonPropertyName("media_urls")]
        public List<MediaUrl>? MediaUrls { get; set; }

        /// <summary>The first unencrypted audio file, if Suno offers one.</summary>
        public string? ProgressiveAudioUrl =>
            MediaUrls?.FirstOrDefault(m => !m.IsEncrypted && !m.IsVideo && m.Url?.StartsWith("https://", StringComparison.OrdinalIgnoreCase) == true)?.Url;

        /// <summary>True once this song has used a download; downloading it again is free.</summary>
        [JsonPropertyName("is_download_unlocked")]
        public bool? IsDownloadUnlocked { get; set; }

        public string DisplayTitle => string.IsNullOrWhiteSpace(Title) ? T("Untitled song") : Title.Trim();

        public double? Duration => Metadata?.Duration;

        public bool IsReady => Status is "complete" or "streaming";

        public bool IsComplete => Status == "complete";

        public bool HasFailed => Status == "error";

        /// <summary>Suno's player can start a song once it is streaming; audio_url itself is no longer playable.</summary>
        public bool CanPlay => IsReady;

        public string WebLink => $"https://suno.com/song/{Id}";

        /// <summary>What a screen reader says for this item in a list: title, length, then anything unusual.</summary>
        public string SpokenText
        {
            get
            {
                var text = DisplayTitle;
                if (Duration is > 0) text += ", " + FormatTime(TimeSpan.FromSeconds(Duration.Value));
                if (IsLiked) text += ", " + T("liked");
                if (HasFailed) text += ", " + T("failed");
                else if (!IsReady && !string.IsNullOrEmpty(Status)) text += ", " + T("still being made");
                if (Metadata?.Type is "concat") text += ", " + T("whole song");
                else if (Metadata?.HasStem == true || Metadata?.StemFromId != null) text += ", " + T("stem");
                return text;
            }
        }

        /// <summary>Everything worth knowing, then the lyrics, for the details window (Ctrl+I).</summary>
        public string DetailsText
        {
            get
            {
                var lines = new List<string>(InfoLines);
                if (!string.IsNullOrWhiteSpace(Metadata?.Prompt))
                {
                    lines.Add("");
                    lines.Add(T("Lyrics:"));
                    lines.Add(Metadata!.Prompt!.Trim());
                }
                return string.Join(Environment.NewLine, lines);
            }
        }

        /// <summary>One fact per item, to step through with the arrow keys (Alt+Enter).</summary>
        public IReadOnlyList<string> InfoLines
        {
            get
            {
                var lines = new List<string> { DisplayTitle };
                if (Duration is > 0) lines.Add(T("Length: {0}", FormatTime(TimeSpan.FromSeconds(Duration.Value))));
                lines.Add(T("Status: {0}", StatusName));
                if (!string.IsNullOrWhiteSpace(Metadata?.ErrorMessage)) lines.Add(T("Error: {0}", Metadata!.ErrorMessage));
                if (!string.IsNullOrWhiteSpace(Metadata?.Tags)) lines.Add(T("Style: {0}", Metadata!.Tags));
                if (Metadata?.MakeInstrumental == true) lines.Add(T("Instrumental"));
                if (!string.IsNullOrWhiteSpace(Metadata?.GptDescriptionPrompt)) lines.Add(T("Description: {0}", Metadata!.GptDescriptionPrompt));
                if (Metadata?.AvgBpm is > 0) lines.Add(T("Tempo: about {0:0} beats per minute", Metadata!.AvgBpm));
                if (!string.IsNullOrEmpty(ModelName)) lines.Add(T("Model: {0}", ModelName));
                if (CreatedAt != null) lines.Add(T("Made: {0}", CreatedAt.Value.ToLocalTime().ToString("f", TinPan.Localization.Loc.Culture)));
                lines.Add(IsPublic ? T("Public") : T("Private"));
                lines.Add(T("Plays: {0}. Likes: {1}.", PlayCount, UpvoteCount));
                if (IsDownloadUnlocked == true) lines.Add(T("Downloaded before, so downloading again is free."));
                lines.Add(T("Link: {0}", WebLink));
                return lines;
            }
        }

        /// <summary>Suno's status word, in the chosen language.</summary>
        private string StatusName => Status switch
        {
            "complete" => T("complete"),
            "streaming" => T("streaming"),
            "submitted" => T("submitted"),
            "queued" => T("queued"),
            "error" => T("error"),
            null or "" => T("unknown"),
            _ => Status
        };

        public override string ToString() => SpokenText;

        public static string FormatTime(TimeSpan t) =>
            t.TotalHours >= 1 ? t.ToString(@"h\:mm\:ss") : t.ToString(@"m\:ss");
    }

    public class MediaUrl
    {
        [JsonPropertyName("url")]
        public string? Url { get; set; }

        /// <summary>Present (for example "m4a-opus") only on encrypted files.</summary>
        [JsonPropertyName("encoding")]
        public JsonElement? Encoding { get; set; }

        public bool IsEncrypted => Encoding is { ValueKind: not JsonValueKind.Null and not JsonValueKind.Undefined };

        public bool IsVideo => Url?.Split('?')[0].EndsWith(".mp4", StringComparison.OrdinalIgnoreCase) == true;
    }

    public class ClipMetadata
    {
        /// <summary>Whether other people may cover or remix this song.</summary>
        [JsonPropertyName("can_remix")]
        public bool? CanRemix { get; set; }

        [JsonPropertyName("duration")]
        public double? Duration { get; set; }

        [JsonPropertyName("tags")]
        public string? Tags { get; set; }

        /// <summary>The lyrics in custom mode.</summary>
        [JsonPropertyName("prompt")]
        public string? Prompt { get; set; }

        [JsonPropertyName("gpt_description_prompt")]
        public string? GptDescriptionPrompt { get; set; }

        [JsonPropertyName("make_instrumental")]
        public bool? MakeInstrumental { get; set; }

        [JsonPropertyName("avg_bpm")]
        public double? AvgBpm { get; set; }

        /// <summary>For example "gen", "concat" or "upsample".</summary>
        [JsonPropertyName("type")]
        public string? Type { get; set; }

        [JsonPropertyName("has_stem")]
        public bool? HasStem { get; set; }

        [JsonPropertyName("stem_from_id")]
        public string? StemFromId { get; set; }

        [JsonPropertyName("error_message")]
        public string? ErrorMessage { get; set; }
    }
}
