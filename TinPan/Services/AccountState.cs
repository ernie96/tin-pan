using System.Collections.Generic;
using System.Linq;
using System.Threading.Tasks;
using CommunityToolkit.Mvvm.ComponentModel;
using TinPan.Models;

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

        public string CreditsText => Billing == null ? "Credits not loaded yet." : BillingText.Credits(Billing);

        public string SubscriptionText => Billing == null ? "Plan not loaded yet." : BillingText.Subscription(Billing);

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

        public async Task<BillingInfo> RefreshAsync()
        {
            Billing = await _api.GetBillingInfoAsync();
            return Billing;
        }

        public void Clear() => Billing = null;

        /// <summary>Label for plan-only actions: plain when allowed, with the requirement otherwise.</summary>
        public string PaidLabel(string label) => IsPaid == true ? label : label + " (needs Pro or Premier)";
    }
}
