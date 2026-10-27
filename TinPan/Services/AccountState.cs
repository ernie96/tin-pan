using System.Collections.Generic;
using System.Linq;
using System.Threading.Tasks;
using CommunityToolkit.Mvvm.ComponentModel;
using TinPan.Models;
using static TinPan.Localization.Loc;

namespace TinPan.Services
{
    /// <summary>
    /// The latest billing reply, shared by every tab: credits, plan, and the models this
    /// account may use. Plan-only features (WAV, cover, remaster) read IsPaid from here.
    /// </summary>
    public sealed partial class AccountState : ObservableObject
    {
        private readonly ISunoApiService _api;

        public AccountState(ISunoApiService api) => _api = api;

        [ObservableProperty]
        [NotifyPropertyChangedFor(nameof(IsPaid), nameof(CreditsText), nameof(SubscriptionText), nameof(Models), nameof(RemasterModels))]
        private BillingInfo? billing;

        /// <summary>Null until billing has loaded or when Suno doesn't say.</summary>
        public bool? IsPaid => Billing?.IsPaid;

        public string CreditsText => Billing == null ? T("Credits not loaded yet.") : BillingText.Credits(Billing);

        public string SubscriptionText => Billing == null ? T("Plan not loaded yet.")
            : BillingText.Subscription(Billing) + StylePersonalization switch
            {
                true => " " + T("Style ideas and boosts are shaped to your taste."),
                false => " " + T("Style ideas and boosts aren't shaped to your taste."),
                null => ""
            };

        /// <summary>Suno's "personalize styles" setting, read with the plan; null when it didn't say.</summary>
        public bool? StylePersonalization { get; private set; }

        public IReadOnlyList<SunoModel> Models =>
            Billing?.Models.Where(m => m.CanUse && !string.IsNullOrEmpty(m.ExternalKey)).ToList() ?? new List<SunoModel>();

        /// <summary>Older remaster models paperfoot/suno-cli knows. Suno may have retired some; it says so if picked.</summary>
        private static readonly SunoModel[] OlderRemasterModels =
        {
            new() { ExternalKey = "chirp-flounder", Name = "v5.5 (older)" },
            new() { ExternalKey = "chirp-carp", Name = "v5 (older)" },
            new() { ExternalKey = "chirp-bass", Name = "v4.5+ (older)" }
        };

        // can_use is false on remaster models even for subscribers; being listed is what counts.
        // The plan's own list comes first, then the older ones it doesn't mention.
        public IReadOnlyList<SunoModel> RemasterModels
        {
            get
            {
                var listed = Billing?.RemasterModels.Where(m => !string.IsNullOrEmpty(m.ExternalKey)).ToList() ?? new List<SunoModel>();
                if (listed.Count == 0) return listed;
                listed.AddRange(OlderRemasterModels.Where(o => listed.All(m => m.ExternalKey != o.ExternalKey)));
                return listed;
            }
        }

        public SunoModel? DefaultModel => Models.FirstOrDefault(m => m.IsDefault) ?? Models.FirstOrDefault();

        public SunoModel? FindModel(string? key) => Models.FirstOrDefault(m => m.ExternalKey == key);

        /// <summary>The account's own trained custom models.</summary>
        public IReadOnlyList<SunoModel> CustomModels => Billing?.Models.Where(m => m.IsCustom).ToList() ?? new List<SunoModel>();

        public async Task<BillingInfo> RefreshAsync()
        {
            try { StylePersonalization = await _api.GetStylePersonalizationAsync(); }
            catch (SunoApiException ex) when (ex is not SunoSignInRequiredException) { Log.Warn($"Personalization setting didn't load: {ex.Message}"); }
            Billing = await _api.GetBillingInfoAsync();
            return Billing;
        }

        public void Clear() => Billing = null;

        /// <summary>Label for plan-only actions: plain when allowed, with the requirement otherwise.</summary>
        public string PaidLabel(string label) => IsPaid == true ? label : T("{0} (needs Pro or Premier)", label);
    }
}
