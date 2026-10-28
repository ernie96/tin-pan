using System;
using System.Collections.Generic;
using System.Text.Json.Serialization;

namespace TinPan.Models
{
    /// <summary>
    /// Body for POST /api/generate/v2-web/, the endpoint the Suno website uses since 2026.
    /// The server wants every field present, even as null. Schema from paperfoot/suno-cli.
    /// </summary>
    public class GenerateRequest
    {
        public static class Modes
        {
            public const string Custom = "custom";
            public const string Inspiration = "inspiration";
            public const string Cover = "cover";
        }

        public GenerateRequest(string model, string createMode)
        {
            Model = model;
            Metadata = new GenerateMetadata { CreateMode = createMode };
        }

        /// <summary>
        /// "cover" or "extend", as Suno's Create page sets it. Without it Suno ignores the
        /// cover song and makes a brand-new one. Left out for plain new songs.
        /// </summary>
        [JsonPropertyName("task")]
        [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
        public string? Task { get; set; }

        /// <summary>Captcha token, only when /api/c/check says one is needed.</summary>
        [JsonPropertyName("token")] public string? Token { get; set; }

        /// <summary>1 for hCaptcha, 2 for Turnstile; sent only with a token.</summary>
        [JsonPropertyName("token_provider")]
        [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
        public int? TokenProvider { get; set; }
        [JsonPropertyName("generation_type")] public string GenerationType { get; set; } = "TEXT";
        [JsonPropertyName("title")] public string Title { get; set; } = "";
        [JsonPropertyName("tags")] public string Tags { get; set; } = "";
        [JsonPropertyName("negative_tags")] public string NegativeTags { get; set; } = "";
        [JsonPropertyName("mv")] public string Model { get; set; }

        /// <summary>Lyrics in custom mode, the description in inspiration mode.</summary>
        [JsonPropertyName("prompt")] public string Prompt { get; set; } = "";

        [JsonPropertyName("make_instrumental")] public bool MakeInstrumental { get; set; }
        [JsonPropertyName("user_uploaded_images_b64")] public string? UploadedImages { get; set; }
        [JsonPropertyName("metadata")] public GenerateMetadata Metadata { get; set; }
        [JsonPropertyName("override_fields")] public List<object> OverrideFields { get; set; } = new();
        [JsonPropertyName("cover_clip_id")] public string? CoverClipId { get; set; }
        [JsonPropertyName("cover_start_s")] public double? CoverStart { get; set; }
        [JsonPropertyName("cover_end_s")] public double? CoverEnd { get; set; }
        [JsonPropertyName("persona_id")] public string? PersonaId { get; set; }
        [JsonPropertyName("artist_clip_id")] public string? ArtistClipId { get; set; }
        [JsonPropertyName("artist_start_s")] public double? ArtistStart { get; set; }
        [JsonPropertyName("artist_end_s")] public double? ArtistEnd { get; set; }
        [JsonPropertyName("continue_clip_id")] public string? ContinueClipId { get; set; }
        [JsonPropertyName("continued_aligned_prompt")] public string? ContinuedAlignedPrompt { get; set; }
        [JsonPropertyName("continue_at")] public double? ContinueAt { get; set; }

        /// <summary>"inspiration" with task playlist_condition: Use as inspiration, built on PlaylistClipIds.</summary>
        [JsonPropertyName("playlist_id")]
        [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
        public string? PlaylistId { get; set; }

        [JsonPropertyName("playlist_clip_ids")]
        [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
        public List<string>? PlaylistClipIds { get; set; }

        [JsonPropertyName("transaction_uuid")] public string TransactionId { get; set; } = Guid.NewGuid().ToString();
    }

    public class GenerateMetadata
    {
        [JsonPropertyName("web_client_pathname")] public string WebClientPath { get; set; } = "/create";
        [JsonPropertyName("is_max_mode")] public bool IsMaxMode { get; set; }
        [JsonPropertyName("is_mumble")] public bool IsMumble { get; set; }
        [JsonPropertyName("create_mode")] public string CreateMode { get; set; } = GenerateRequest.Modes.Custom;
        [JsonPropertyName("user_tier")] public string UserTier { get; set; } = "";
        [JsonPropertyName("create_session_token")] public string CreateSessionToken { get; set; } = Guid.NewGuid().ToString();
        [JsonPropertyName("disable_volume_normalization")] public bool DisableVolumeNormalization { get; set; }

        /// <summary>True when the song is made from another one (cover, extend), as Suno's web client and sunox send it.</summary>
        [JsonPropertyName("is_remix")]
        [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
        public bool? IsRemix { get; set; }

        [JsonPropertyName("control_sliders")]
        [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
        public ControlSliders? ControlSliders { get; set; }
    }

    /// <summary>All 0 to 1. Leave a value null to let Suno choose.</summary>
    public class ControlSliders
    {
        [JsonPropertyName("weirdness_constraint")]
        [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
        public double? Weirdness { get; set; }

        [JsonPropertyName("style_weight")]
        [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
        public double? StyleInfluence { get; set; }

        /// <summary>How strongly the original shapes a cover.</summary>
        [JsonPropertyName("audio_weight")]
        [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
        public double? AudioInfluence { get; set; }

        public static ControlSliders? FromPercent(double? weirdness, double? style, double? audio) =>
            weirdness == null && style == null && audio == null
                ? null
                : new ControlSliders
                {
                    Weirdness = weirdness / 100,
                    StyleInfluence = style / 100,
                    AudioInfluence = audio / 100
                };
    }
}
