using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using System.Text.Json.Serialization;
using TinPan.Localization;
using static TinPan.Localization.Loc;

namespace TinPan.Models
{
    /// <summary>GET /api/billing/info/ — credits, plan and the models this account may use.</summary>
    public class BillingInfo
    {
        [JsonPropertyName("total_credits_left")]
        public int TotalCreditsLeft { get; set; }

        /// <summary>Credits bought on top of the plan.</summary>
        [JsonPropertyName("credits")]
        public int? ExtraCredits { get; set; }

        [JsonPropertyName("monthly_limit")]
        public int? MonthlyLimit { get; set; }

        [JsonPropertyName("monthly_usage")]
        public int? MonthlyUsage { get; set; }

        [JsonPropertyName("period")]
        public string? Period { get; set; }

        [JsonPropertyName("renews_on")]
        public string? RenewsOn { get; set; }

        [JsonPropertyName("is_active")]
        public bool IsActive { get; set; }

        [JsonPropertyName("plan")]
        public SunoPlan? Plan { get; set; }

        [JsonPropertyName("models")]
        public List<SunoModel> Models { get; set; } = new();

        [JsonPropertyName("remaster_model_types")]
        public List<SunoModel> RemasterModels { get; set; } = new();

        [JsonPropertyName("accessible_features")]
        public List<SunoFeature> AccessibleFeatures { get; set; } = new();

        [JsonPropertyName("download_usage")]
        public DownloadUsage? DownloadUsage { get; set; }

        /// <summary>Shortest and longest audio upload this account may make (chadvis inventory: numeric min and max).</summary>
        [JsonPropertyName("audio_upload_limits")]
        public AudioUploadLimits? AudioUploadLimits { get; set; }

        /// <summary>Suno Studio accounts may use Studio's download route. Suno counts it too since 2026-09-09.</summary>
        [JsonIgnore]
        public bool HasStudio => AccessibleFeatures.Any(f => f.Name == "studio");

        /// <summary>
        /// True for paid plans, false for the free plan, null when the reply doesn't say.
        /// WAV downloads, covers and remasters need a paid plan.
        /// </summary>
        [JsonIgnore]
        public bool? IsPaid
        {
            get
            {
                var key = Plan?.PlanKey?.ToLowerInvariant() ?? "";
                var name = Plan?.Name?.ToLowerInvariant() ?? "";
                if (key.Length == 0 && name.Length == 0) return null;
                return !(key.Contains("free") || name.Contains("free") || key == "basic" || name == "basic");
            }
        }
    }

    /// <summary>
    /// Upload length limits in seconds. The inventory records only that both are numbers, so values
    /// that can't be seconds (zero, negative, or over 10 hours) are treated as unknown.
    /// </summary>
    public class AudioUploadLimits
    {
        [JsonPropertyName("min")] public double? Min { get; set; }
        [JsonPropertyName("max")] public double? Max { get; set; }

        [JsonIgnore] public double? MinSeconds => Min is > 0 and < 36000 ? Min : null;
        [JsonIgnore] public double? MaxSeconds => Max is > 0 and < 36000 ? Max : null;
    }

    /// <summary>Since September 2026: free 7 in total, Pro 20 a month, Premier 60 a month.</summary>
    public class DownloadUsage
    {
        [JsonPropertyName("current_period_downloads_limit")] public int? Limit { get; set; }
        [JsonPropertyName("current_period_downloads_used")] public int? Used { get; set; }
        [JsonPropertyName("additional_download_remaining")] public int? Extra { get; set; }

        /// <summary>Downloads left this period, including bought ones. Null when Suno doesn't say.</summary>
        [JsonIgnore]
        public int? Remaining => Limit == null ? Extra : Math.Max(0, Limit.Value - (Used ?? 0)) + (Extra ?? 0);
    }

    public class SunoPlan
    {
        [JsonPropertyName("name")]
        public string? Name { get; set; }

        [JsonPropertyName("plan_key")]
        public string? PlanKey { get; set; }

        [JsonPropertyName("usage_plan_features")]
        public List<SunoFeature> Features { get; set; } = new();
    }

    public class SunoFeature
    {
        [JsonPropertyName("name")]
        public string Name { get; set; } = "";
    }

    public class SunoModel
    {
        /// <summary>Value sent as "mv", for example chirp-fenix.</summary>
        [JsonPropertyName("external_key")]
        public string ExternalKey { get; set; } = "";

        [JsonPropertyName("name")]
        public string Name { get; set; } = "";

        [JsonPropertyName("description")]
        public string? Description { get; set; }

        [JsonPropertyName("is_default_model")]
        public bool IsDefault { get; set; }

        [JsonPropertyName("can_use")]
        public bool CanUse { get; set; } = true;

        [JsonPropertyName("max_lengths")]
        public MaxLengths? MaxLengths { get; set; }

        public override string ToString() => string.IsNullOrEmpty(Name) ? ExternalKey : Name;
    }

    /// <summary>Limits in UTF-16 units, as the Suno web form counts them. 0 means no limit given.</summary>
    public class MaxLengths
    {
        [JsonPropertyName("title")] public int Title { get; set; }
        [JsonPropertyName("prompt")] public int Prompt { get; set; }
        [JsonPropertyName("tags")] public int Tags { get; set; }
        [JsonPropertyName("negative_tags")] public int NegativeTags { get; set; }
        [JsonPropertyName("gpt_description_prompt")] public int Description { get; set; }

        public static MaxLengths Fallback { get; } = new() { Title = 100, Prompt = 5000, Tags = 1000, NegativeTags = 1000, Description = 3000 };
    }

    public static class BillingText
    {
        private static CultureInfo Culture => Loc.Culture;

        /// <summary>F2: total credits, then where they come from.</summary>
        public static string Credits(BillingInfo info)
        {
            var text = Loc.Plural(info.TotalCreditsLeft, "{0:N0} credit left.", "{0:N0} credits left.");
            if (info.MonthlyLimit is > 0 && info.MonthlyUsage is { } used)
            {
                var monthlyLeft = Math.Max(0, info.MonthlyLimit.Value - used);
                text += " " + (info.ExtraCredits is > 0
                    ? T("{0} of this month's {1}, plus {2} extra.", N(monthlyLeft), N(info.MonthlyLimit.Value), N(info.ExtraCredits.Value))
                    : T("{0} of this month's {1}.", N(monthlyLeft), N(info.MonthlyLimit.Value)));
            }
            else if (info.ExtraCredits is > 0)
            {
                text += " " + T("Including {0} extra.", N(info.ExtraCredits.Value));
            }
            return text;
        }

        /// <summary>Shift+F2: plan, renewal, credits and downloads. The full feature list is on the Account tab.</summary>
        public static string Subscription(BillingInfo info)
        {
            var state = info.IsActive ? T("active") : T("not active");
            var parts = new List<string>
            {
                Renewal(info) is { } renews ? T("{0}, {1}, renews {2}.", PlanName(info), state, renews) : T("{0}, {1}.", PlanName(info), state),
                Credits(info)
            };
            if (info.DownloadUsage?.Remaining is { } left)
            {
                var u = info.DownloadUsage;
                var count = Loc.Plural(left, "{0:N0} download left", "{0:N0} downloads left");
                parts.Add(u.Limit is not { } limit ? count + "."
                    : u.Extra is > 0 ? T("{0}: {1} a month, {2} used, plus {3} extra.", count, N(limit), N(u.Used ?? 0), N(u.Extra.Value))
                    : T("{0}: {1} a month, {2} used.", count, N(limit), N(u.Used ?? 0)));
            }
            if (info.IsPaid == false) parts.Add(T("WAV downloads, covers and remasters need Pro or Premier."));
            return string.Join(" ", parts);
        }

        /// <summary>Everything the plan includes, in plain words, for the Account tab.</summary>
        public static string Features(BillingInfo info)
        {
            var names = (info.Plan?.Features.Select(f => f.Name) ?? Enumerable.Empty<string>())
                .Concat(info.AccessibleFeatures.Select(f => f.Name))
                .Where(n => !string.IsNullOrWhiteSpace(n))
                .Select(FeatureName)
                .Distinct(StringComparer.OrdinalIgnoreCase)
                .ToList();
            return names.Count == 0 ? "" : T("Your plan includes: {0}.", string.Join(", ", names));
        }

        /// <summary>"Pro Plan" becomes "Pro plan"; no "Pro Plan plan". Suno's plan names (Pro, Premier) stay as they are.</summary>
        public static string PlanName(BillingInfo info)
        {
            var name = info.Plan?.Name?.Trim();
            if (string.IsNullOrEmpty(name)) return T("Plan unknown");
            if (name.EndsWith(" plan", StringComparison.OrdinalIgnoreCase)) name = name[..^5].TrimEnd();
            return T("{0} plan", name);
        }

        /// <summary>"November 9, 2026" from Suno's ISO date, in the chosen language's date format.</summary>
        public static string? Renewal(BillingInfo info)
        {
            if (string.IsNullOrWhiteSpace(info.RenewsOn)) return null;
            return DateTimeOffset.TryParse(info.RenewsOn, CultureInfo.InvariantCulture, DateTimeStyles.AssumeUniversal, out var when)
                ? when.ToLocalTime().ToString("D", Culture)
                : info.RenewsOn.Trim();
        }

        private static string FeatureName(string raw) => raw.Trim().Replace(' ', '_').ToLowerInvariant() switch
        {
            "v4" => T("v4 model"),
            "auk" => T("v4.5 models"),
            "cover" => T("covers"),
            "edit_mode" => T("editing songs"),
            "persona" => T("personas"),
            "can_buy_credit_top_ups" => T("buying extra credits"),
            "commercial_rights" => T("commercial use"),
            "get_stems" => T("stems"),
            "generate_song_image" => T("song cover images"),
            "negative_tags" => T("styles to exclude"),
            "remaster" => T("remasters"),
            "generate_song_video" => T("song videos"),
            "long_uploads" => T("long uploads"),
            "convert_audio" => T("WAV conversion"),
            "create_control_sliders" => T("weirdness and style sliders"),
            "playlist_condition" => T("playlist inspiration"),
            "tag_upsample" => T("style boost"),
            "custom_models" => T("custom models"),
            "max_mode" => T("max mode"),
            "studio" => "Suno Studio",
            _ => raw.Trim().Replace('_', ' ')
        };

        private static string N(int value) => value.ToString("N0", Culture);
    }
}
