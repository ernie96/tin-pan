using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using System.Text.Json.Serialization;

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
        private static readonly CultureInfo Culture = CultureInfo.CurrentCulture;

        /// <summary>F2: total credits, then where they come from.</summary>
        public static string Credits(BillingInfo info)
        {
            var text = $"{N(info.TotalCreditsLeft)} credits left.";
            if (info.MonthlyLimit is > 0 && info.MonthlyUsage is { } used)
            {
                var monthlyLeft = Math.Max(0, info.MonthlyLimit.Value - used);
                text += $" {N(monthlyLeft)} of this month's {N(info.MonthlyLimit.Value)}";
                text += info.ExtraCredits is > 0 ? $", plus {N(info.ExtraCredits.Value)} extra." : ".";
            }
            else if (info.ExtraCredits is > 0)
            {
                text += $" Including {N(info.ExtraCredits.Value)} extra.";
            }
            return text;
        }

        /// <summary>Shift+F2: plan, renewal, credits and downloads. The full feature list is on the Dashboard.</summary>
        public static string Subscription(BillingInfo info)
        {
            var parts = new List<string>
            {
                $"{PlanName(info)}, {(info.IsActive ? "active" : "not active")}" + (Renewal(info) is { } renews ? $", renews {renews}." : "."),
                Credits(info)
            };
            if (info.DownloadUsage?.Remaining is { } left)
            {
                var u = info.DownloadUsage;
                var detail = u.Limit is { } limit ? $" {N(limit)} a month, {N(u.Used ?? 0)} used" + (u.Extra is > 0 ? $", plus {N(u.Extra.Value)} extra" : "") : "";
                parts.Add($"{N(left)} downloads left{(detail.Length > 0 ? ":" + detail : "")}.");
            }
            if (info.IsPaid == false) parts.Add("WAV downloads, covers and remasters need Pro or Premier.");
            return string.Join(" ", parts);
        }

        /// <summary>Everything the plan includes, in plain words, for the Dashboard.</summary>
        public static string Features(BillingInfo info)
        {
            var names = (info.Plan?.Features.Select(f => f.Name) ?? Enumerable.Empty<string>())
                .Concat(info.AccessibleFeatures.Select(f => f.Name))
                .Where(n => !string.IsNullOrWhiteSpace(n))
                .Select(FeatureName)
                .Distinct(StringComparer.OrdinalIgnoreCase)
                .ToList();
            return names.Count == 0 ? "" : "Your plan includes: " + string.Join(", ", names) + ".";
        }

        /// <summary>"Pro Plan" becomes "Pro plan"; no "Pro Plan plan".</summary>
        public static string PlanName(BillingInfo info)
        {
            var name = info.Plan?.Name?.Trim();
            if (string.IsNullOrEmpty(name)) return "Plan unknown";
            if (name.EndsWith(" plan", StringComparison.OrdinalIgnoreCase)) name = name[..^5].TrimEnd();
            return name + " plan";
        }

        /// <summary>"November 9, 2026" from Suno's ISO date, in the user's own date format.</summary>
        public static string? Renewal(BillingInfo info)
        {
            if (string.IsNullOrWhiteSpace(info.RenewsOn)) return null;
            return DateTimeOffset.TryParse(info.RenewsOn, CultureInfo.InvariantCulture, DateTimeStyles.AssumeUniversal, out var when)
                ? when.ToLocalTime().ToString("D", Culture)
                : info.RenewsOn.Trim();
        }

        private static readonly Dictionary<string, string> FeatureNames = new(StringComparer.OrdinalIgnoreCase)
        {
            ["v4"] = "v4 model",
            ["auk"] = "v4.5 models",
            ["cover"] = "covers",
            ["edit_mode"] = "editing songs",
            ["persona"] = "personas",
            ["can_buy_credit_top_ups"] = "buying extra credits",
            ["commercial_rights"] = "commercial use",
            ["get_stems"] = "stems",
            ["generate_song_image"] = "song cover images",
            ["negative_tags"] = "styles to exclude",
            ["remaster"] = "remasters",
            ["generate_song_video"] = "song videos",
            ["long_uploads"] = "long uploads",
            ["convert_audio"] = "WAV conversion",
            ["create_control_sliders"] = "weirdness and style sliders",
            ["playlist_condition"] = "playlist inspiration",
            ["tag_upsample"] = "style boost",
            ["custom_models"] = "custom models",
            ["max_mode"] = "max mode",
            ["studio"] = "Suno Studio"
        };

        private static string FeatureName(string raw)
        {
            var key = raw.Trim().Replace(' ', '_');
            return FeatureNames.TryGetValue(key, out var name) ? name : raw.Trim().Replace('_', ' ');
        }

        private static string N(int value) => value.ToString("N0", Culture);
    }
}
