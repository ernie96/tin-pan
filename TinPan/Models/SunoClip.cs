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
            Reaction = newer.Reaction;
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

        /// <summary>The creator's handle (suno.com/@handle) and name.</summary>
        [JsonPropertyName("handle")]
        public string? Handle { get; set; }

        [JsonPropertyName("display_name")]
        public string? CreatorName { get; set; }

        [JsonPropertyName("metadata")]
        public ClipMetadata? Metadata { get; set; }

        [JsonPropertyName("image_large_url")]
        public string? ImageLargeUrl { get; set; }

        /// <summary>Your own like on this song.</summary>
        [JsonPropertyName("is_liked")]
        public bool IsLiked { get; set; }

        /// <summary>Suno's reaction to a song, read loosely ({"reaction_type": "D"} or a plain "D"); "D" is disliked.</summary>
        [JsonPropertyName("reaction")]
        public JsonElement? Reaction { get; set; }

        /// <summary>Set by Tin Pan after Dislike, or read from Suno's reaction.</summary>
        [JsonIgnore]
        public bool IsDisliked
        {
            get => _disliked ?? ReactionLetter(Reaction) == "D";
            set => _disliked = value;
        }

        private bool? _disliked;

        private static string? ReactionLetter(JsonElement? reaction) => reaction switch
        {
            { ValueKind: JsonValueKind.String } r => r.GetString(),
            { ValueKind: JsonValueKind.Object } r when r.TryGetProperty("reaction_type", out var t) && t.ValueKind == JsonValueKind.String => t.GetString(),
            _ => null
        };

        /// <summary>
        /// The real files. audio_url is a decoy (/api/forbidden); an entry here without an
        /// "encoding" field is a plain, unencrypted file that downloads for free.
        /// </summary>
        [JsonPropertyName("media_urls")]
        public List<MediaUrl>? MediaUrls { get; set; }

        /// <summary>The first unencrypted audio file, if Suno offers one.</summary>
        public string? ProgressiveAudioUrl =>
            MediaUrls?.FirstOrDefault(m => !m.IsEncrypted && !m.IsVideo && m.Url?.StartsWith("https://", StringComparison.OrdinalIgnoreCase) == true)?.Url;

        /// <summary>True if audio_url points to Suno's decoy /api/forbidden.</summary>
        public bool AudioUrlIsDecoy => !string.IsNullOrEmpty(AudioUrl) && AudioUrl.Contains("forbidden", StringComparison.OrdinalIgnoreCase);

        /// <summary>The best media URL: progressive unencrypted first, otherwise encrypted stream.</summary>
        public string? BestMediaUrl =>
            ProgressiveAudioUrl ?? MediaUrls?.FirstOrDefault(m => !m.IsVideo && m.Url?.StartsWith("https://", StringComparison.OrdinalIgnoreCase) == true && !m.Url.Contains("forbidden", StringComparison.OrdinalIgnoreCase))?.Url;

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
                else if (IsDisliked) text += ", " + T("disliked");
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
                if (!string.IsNullOrWhiteSpace(Handle))
                    lines.Add(T("By {0}", string.IsNullOrWhiteSpace(CreatorName) ? "@" + Handle : $"{CreatorName} (@{Handle})"));
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

        /// <summary>SpokenText, then who made it, for lists of other people's songs.</summary>
        public string SpokenWithCreator =>
            string.IsNullOrWhiteSpace(CreatorName) && string.IsNullOrWhiteSpace(Handle) ? SpokenText
            : SpokenText + ", " + T("by {0}", string.IsNullOrWhiteSpace(CreatorName) ? "@" + Handle : CreatorName);

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

        /// <summary>For a cover, the song it covers.</summary>
        [JsonPropertyName("cover_clip_id")]
        public string? CoverClipId { get; set; }

        /// <summary>For an extension, the songs it continues, oldest first.</summary>
        /// <summary>Read loosely, so an unexpected shape never loses the song.</summary>
        [JsonPropertyName("history")]
        public JsonElement? History { get; set; }

        /// <summary>For a whole song, the parts it joins.</summary>
        [JsonPropertyName("concat_history")]
        public JsonElement? ConcatHistory { get; set; }

        /// <summary>The song this one was made from, when its own details say.</summary>
        [JsonIgnore]
        public string? OriginalId =>
            !string.IsNullOrEmpty(CoverClipId) ? CoverClipId : FirstId(History) ?? FirstId(ConcatHistory);

        private static string? FirstId(JsonElement? list)
        {
            if (list is not { ValueKind: JsonValueKind.Array } array) return null;
            foreach (var item in array.EnumerateArray())
            {
                if (item.ValueKind == JsonValueKind.String && !string.IsNullOrEmpty(item.GetString())) return item.GetString();
                if (item.ValueKind == JsonValueKind.Object && item.TryGetProperty("id", out var id)
                    && id.ValueKind == JsonValueKind.String && !string.IsNullOrEmpty(id.GetString())) return id.GetString();
            }
            return null;
        }
    }
}
