using System;
using System.IO;
using System.Threading.Tasks;
using CommunityToolkit.Mvvm.Input;
using TinPan.Models;
using TinPan.Services;
using TinPan.Views;
using static TinPan.Localization.Loc;

namespace TinPan.ViewModels
{
    // Uploading a song to Suno: choose a file, name it, confirm the rights, then it appears in the library.
    public partial class LibraryPanelViewModel
    {
        [RelayCommand]
        private async Task UploadAsync()
        {
            if (!_auth.IsAuthenticated)
            {
                Announcer.Announce(T("Sign in from Settings first."));
                return;
            }
            var path = _dialogs.PickSongToUpload();
            if (path == null) return;

            var length = _uploads.Duration(path);
            if (length == null)
            {
                Announcer.Announce(T("Windows can't read that file as audio."), always: true);
                return;
            }
            if (_account.Billing?.AudioUploadLimits is { } limits)
            {
                if (limits.MinSeconds is { } min && length.Value.TotalSeconds < min)
                {
                    Announcer.Announce(T("That song is {0}. Suno takes uploads of at least {1}.", SunoClip.FormatTime(length.Value), SunoClip.FormatTime(TimeSpan.FromSeconds(min))), always: true);
                    return;
                }
                if (limits.MaxSeconds is { } max && length.Value.TotalSeconds > max)
                {
                    Announcer.Announce(T("That song is {0}. Suno takes uploads of up to {1} on this account.", SunoClip.FormatTime(length.Value), SunoClip.FormatTime(TimeSpan.FromSeconds(max))), always: true);
                    return;
                }
            }

            var name = Path.GetFileNameWithoutExtension(path);
            var title = new TextField(T("_Title"), name, maxLength: 100);
            var rights = new CheckField(T("I _own this audio or have the rights to upload it, and I accept Suno's terms for uploads (suno.com/terms)"), false);
            var intro = T("Uploads {0} ({1}) to your Suno account, where it becomes a song you can play, extend, cover and remaster. Files that aren't M4A are converted to M4A first. Suno checks each upload and may refuse music it finds is copyrighted.",
                Path.GetFileName(path), SunoClip.FormatTime(length.Value));
            if (!_dialogs.ShowForm(T("Upload a song"), intro, T("Upload"), new FormField[] { title, rights })) return;
            if (!rights.Value)
            {
                Announcer.Announce(T("Nothing was uploaded. Suno needs you to confirm the rights first."), always: true);
                return;
            }
            var songTitle = title.Value.Length > 0 ? title.Value : name;

            Announcer.Announce(T("Uploading {0}.", songTitle));
            var progress = new Progress<UploadStage>(stage =>
            {
                if (stage == UploadStage.Processing) Announcer.Announce(T("Suno is checking {0}. This can take a minute or two.", songTitle), interrupt: false);
            });
            await RunAsync(async () =>
            {
                var clipId = await _uploads.UploadAsync(path, songTitle, progress);
                var clips = await _api.GetClipsAsync(new[] { clipId });
                Upsert(clips);
                _watcher.Watch(clips);
                Announcer.Announce(T("Uploaded {0}. It's at the top of your library.", songTitle), interrupt: false);
            });
        }
    }
}
