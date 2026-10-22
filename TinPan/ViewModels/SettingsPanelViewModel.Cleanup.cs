using System.Runtime.CompilerServices;
using CommunityToolkit.Mvvm.Input;
using TinPan.Models;
using TinPan.Services;
using static TinPan.Localization.Loc;

namespace TinPan.ViewModels
{
    // Clean-up with sunscreen: whether Download MP3, Download WAV and Download all clean up by
    // themselves (Download asks every time), and sunscreen's own steps and values.
    public partial class SettingsPanelViewModel
    {
        private CleanupConfig Cleanup => _config.Current.Cleanup;

        private void Saved([CallerMemberName] string? name = null)
        {
            _config.SaveSoon();
            OnPropertyChanged(name);
        }

        public string CleanupAbout =>
            T("sunscreen {0}, by Alan Bradburne (MIT license), cleans up on this computer; nothing is uploaded. It reads MP3 and WAV, so Suno's M4A files are converted to WAV first. The result is always a 24-bit WAV, named like the download with (clean) added.", CleanupService.Version);

        public bool CleanupAfterDownload { get => Cleanup.AfterDownload; set { Cleanup.AfterDownload = value; Saved(); } }
        public bool CleanupKeepOriginal { get => Cleanup.KeepOriginal; set { Cleanup.KeepOriginal = value; Saved(); } }
        public bool CleanupDebuzz { get => Cleanup.Debuzz; set { Cleanup.Debuzz = value; Saved(); } }
        public bool CleanupGate { get => Cleanup.Gate; set { Cleanup.Gate = value; Saved(); } }
        public bool CleanupSmooth { get => Cleanup.Smooth; set { Cleanup.Smooth = value; Saved(); } }
        public double CleanupSmoothMs { get => Cleanup.SmoothMs; set { Cleanup.SmoothMs = value; Saved(); } }
        public bool CleanupWiden { get => Cleanup.Widen; set { Cleanup.Widen = value; Saved(); } }
        public double CleanupWidenAmount { get => Cleanup.WidenAmount; set { Cleanup.WidenAmount = System.Math.Round(value, 1); Saved(); } }
        public bool CleanupExcite { get => Cleanup.Excite; set { Cleanup.Excite = value; Saved(); } }
        public double CleanupExciteDb { get => Cleanup.ExciteDb; set { Cleanup.ExciteDb = value; Saved(); } }
        public bool CleanupMatch { get => Cleanup.Match; set { Cleanup.Match = value; Saved(); } }
        public string CleanupMatchReference { get => Cleanup.MatchReference; set { Cleanup.MatchReference = value; Saved(); } }
        public bool CleanupMaster { get => Cleanup.Master; set { Cleanup.Master = value; Saved(); } }
        public double CleanupMasterLufs { get => Cleanup.MasterLufs; set { Cleanup.MasterLufs = value; Saved(); } }

        [RelayCommand]
        private void ChooseMatchReference()
        {
            var picked = _dialogs.PickAudioFile(T("Choose a song to match the tone of"));
            if (picked == null) return;
            CleanupMatchReference = picked;
            Announcer.Announce(T("Clean-up matches the tone of {0}.", System.IO.Path.GetFileNameWithoutExtension(picked)));
        }

        [RelayCommand]
        private void ClearMatchReference()
        {
            if (CleanupMatchReference.Length == 0) return;
            CleanupMatchReference = "";
            Announcer.Announce(T("Clean-up matches the average of commercial pop masters."));
        }

        /// <summary>sunscreen's recommended chain with its own values. Leaves when and what to keep alone.</summary>
        [RelayCommand]
        private void RecommendedCleanup()
        {
            var fresh = new CleanupConfig { AfterDownload = Cleanup.AfterDownload, KeepOriginal = Cleanup.KeepOriginal };
            _config.Current.Cleanup = fresh;
            _config.SaveSoon();
            OnPropertyChanged(string.Empty);
            Announcer.Announce(T("Clean-up steps set to sunscreen's recommended ones."));
        }
    }
}
