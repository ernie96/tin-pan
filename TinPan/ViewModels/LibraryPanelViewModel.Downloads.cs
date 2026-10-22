using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.ComponentModel;
using System.IO;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using TinPan.Models;
using TinPan.Services;
using TinPan.Views;
using static TinPan.Localization.Loc;

namespace TinPan.ViewModels
{
    // Downloading songs and the extra files that go with them.
    public partial class LibraryPanelViewModel
    {
        // ── Downloads ───────────────────────────────────────────────────────

        [RelayCommand]
        private Task DownloadAsync(SunoClip? clip)
        {
            var format = _config.Current.Audio.OutputFormat;
            if (format != "wav" || _account.IsPaid != false) return DownloadInFormatAsync(clip ?? SelectedClip, format);
            return DownloadInFormatAsync(clip ?? SelectedClip, "mp3", T("WAV needs Pro or Premier, so this is an MP3.") + " ");
        }

        [RelayCommand(CanExecute = nameof(CanUsePaidFeatures))]
        private Task DownloadWavAsync(SunoClip? clip) => DownloadInFormatAsync(clip ?? SelectedClip, "wav");

        [RelayCommand]
        private Task DownloadVideoAsync(SunoClip? clip) => DownloadInFormatAsync(clip ?? SelectedClip, "mp4");

        private async Task DownloadInFormatAsync(SunoClip? clip, string format, string note = "")
        {
            if (clip == null) return;
            if (!clip.IsComplete)
            {
                Announcer.Announce(clip.HasFailed ? T("That song failed to generate.") : T("That song isn't finished yet."));
                return;
            }
            string path;
            try
            {
                path = TargetPath(clip, format, out _);
            }
            catch (SunoApiException ex)
            {
                Announcer.Announce(ex.Message, always: true);
                return;
            }
            if (File.Exists(path))
            {
                Announcer.Announce(T("{0} is already downloaded.", clip.DisplayTitle));
                return;
            }

            Announcer.Announce(note + (format == "wav" ? T("Asking Suno for a WAV of {0}.", clip.DisplayTitle) : T("Downloading {0}.", clip.DisplayTitle)));
            try
            {
                await DownloadDirectAsync(clip, format, path);
                Announcer.Announce(T("Saved {0}.", clip.DisplayTitle), interrupt: false);
            }
            catch (SunoApiException ex)
            {
                // A sign-in or save problem would fail Suno's route too, so only offer it for a refused file.
                if (format == "wav" || ex is SunoSignInRequiredException || ex.InnerException is IOException or UnauthorizedAccessException
                    || !OfferSunoDownload(clip, ex.Message))
                {
                    Announcer.Announce(ex.Message, always: true);
                    return;
                }
                await DownloadThroughSunoAsync(clip, format, path);
            }
        }

        /// <summary>
        /// The song's own file: the unencrypted one in media_urls when there is one (free),
        /// otherwise audio_url as before. WAV uses Suno's WAV conversion, as before.
        /// The file name keeps the real extension, since the free file is often an M4A.
        /// </summary>
        private string TargetPath(SunoClip clip, string format, out string? directUrl)
        {
            directUrl = format switch
            {
                "wav" => null,
                "mp4" => clip.VideoUrl,
                _ => clip.ProgressiveAudioUrl ?? clip.AudioUrl
            };
            var extension = format == "mp3" ? FileNames.ExtensionOf(directUrl, "mp3") : format;
            return Path.Combine(DownloadFolder(), FileNames.ForSong(clip.Title, clip.Id, extension));
        }

        private async Task DownloadDirectAsync(SunoClip clip, string format, string path)
        {
            TargetPath(clip, format, out var url);
            if (format == "wav") url = await _api.GetWavUrlAsync(clip.Id);
            if (string.IsNullOrEmpty(url)) throw new SunoApiException(T("Suno gave no file address for this song."));
            await _api.DownloadAsync(url, path);
            await SaveExtrasAsync(clip, path);
        }

        /// <summary>The files chosen in Settings to sit next to each download: lyrics, timed lyrics, details, cover.</summary>
        private async Task SaveExtrasAsync(SunoClip clip, string audioPath)
        {
            var audio = _config.Current.Audio;
            var stem = Path.Combine(Path.GetDirectoryName(audioPath)!, Path.GetFileNameWithoutExtension(audioPath));
            try
            {
                var lyrics = clip.Metadata?.Prompt?.Trim();
                if (audio.SaveLyrics && !string.IsNullOrEmpty(lyrics))
                    await File.WriteAllTextAsync(stem + ".txt", lyrics);
                if (audio.SaveDetails)
                    await File.WriteAllTextAsync(stem + ".details.txt", clip.DetailsText);
                if (audio.SaveTimedLyrics)
                {
                    var aligned = await _api.GetAlignedLyricsAsync(clip.Id);
                    if (aligned is { Words.Count: > 0 })
                        await File.WriteAllTextAsync(stem + ".lrc", LrcWriter.Write(clip, aligned.Words));
                }
                var cover = clip.ImageLargeUrl ?? clip.ImageUrl;
                if (audio.SaveCover && !string.IsNullOrEmpty(cover))
                    await _api.DownloadAsync(cover, stem + "." + FileNames.ExtensionOf(cover, "jpg"));
            }
            catch (Exception ex) when (ex is SunoApiException or IOException or UnauthorizedAccessException)
            {
                // The song itself is saved; a missing extra file isn't worth interrupting for.
                Log.Warn($"Extra files for a download failed: {ex.Message}");
            }
        }

        /// <summary>
        /// Downloads every finished song in the list through the free direct route only.
        /// Songs already in the folder are skipped; the end says what couldn't be fetched directly.
        /// </summary>
        [RelayCommand]
        private async Task DownloadAllAsync()
        {
            var format = _config.Current.Audio.OutputFormat;
            if (format == "wav" && _account.IsPaid == false) format = "mp3";
            var todo = Songs.Where(c => c.IsComplete).ToList();
            List<SunoClip> missing;
            try
            {
                missing = todo.Where(c => !File.Exists(TargetPath(c, format, out _))).ToList();
            }
            catch (SunoApiException ex)
            {
                Announcer.Announce(ex.Message, always: true);
                return;
            }
            if (missing.Count == 0)
            {
                Announcer.Announce(todo.Count == 0 ? T("No finished songs in this list.") : T("Every song in this list is already downloaded."));
                return;
            }
            if (!_dialogs.Confirm(T("Download {0} as {1} into {2}? {3} already there will be skipped.",
                    Plural(missing.Count, "{0} song", "{0} songs"), format.ToUpperInvariant(), DownloadFolder(), todo.Count - missing.Count), T("Download all"))) return;

            Announcer.Announce(T("Downloading {0}.", Plural(missing.Count, "{0} song", "{0} songs")));
            var saved = 0;
            var failed = new List<string>();
            for (var i = 0; i < missing.Count; i++)
            {
                var clip = missing[i];
                try
                {
                    await DownloadDirectAsync(clip, format, TargetPath(clip, format, out _));
                    saved++;
                }
                catch (SunoApiException ex)
                {
                    failed.Add(clip.DisplayTitle);
                    Log.Warn($"Download all: one song failed: {ex.Message}");
                    // Signed out, or the folder can't be written: every other song would fail the same way.
                    if (ex is SunoSignInRequiredException || ex.InnerException is IOException or UnauthorizedAccessException)
                    {
                        Announcer.Announce(ex.Message, always: true);
                        break;
                    }
                }
                if (i + 1 == missing.Count / 2 && missing.Count >= 10)
                    Announcer.Announce(T("Halfway: {0} saved.", saved), interrupt: false);
            }

            var summary = T("Downloaded {0} of {1} into {2}.", saved, Plural(missing.Count, "{0} song", "{0} songs"), Path.GetFileName(DownloadFolder()));
            if (failed.Count > 0)
                summary += failed.Count <= 3
                    ? " " + T("Couldn't download directly: {0}. Ctrl+D on one offers Suno's own download.", string.Join(", ", failed))
                    : " " + T("{0} couldn't be downloaded directly. Ctrl+D on one offers Suno's own download.", failed.Count);
            Announcer.Announce(summary, interrupt: false);
        }

        /// <summary>
        /// Only when the direct download fails: asks whether to use Suno's own download instead,
        /// since Suno counts that one against the plan's downloads.
        /// </summary>
        private bool OfferSunoDownload(SunoClip clip, string problem)
        {
            var cost = clip.IsDownloadUnlocked == true
                ? T("You've downloaded this song before, so it won't count against your downloads.")
                : _account.Billing?.DownloadUsage?.Remaining is { } left
                    ? T("Suno counts it as 1 of your {0} downloads left.", left)
                    : T("Suno counts it against your downloads.");
            return _dialogs.Confirm(T("The direct download didn't work ({0}) Use Suno's own download for \"{1}\" instead? {2}", problem, clip.DisplayTitle, cost), T("Download"));
        }

        private async Task DownloadThroughSunoAsync(SunoClip clip, string format, string path)
        {
            var studio = _account.Billing?.HasStudio == true;
            Announcer.Announce(T("Downloading {0} through Suno.", clip.DisplayTitle));
            await RunAsync(async () =>
            {
                if (!studio && clip.IsDownloadUnlocked != true)
                {
                    await _api.AuthorizeDownloadAsync(clip.Id);
                    clip.IsDownloadUnlocked = true;
                    clip.NotifyChanged();
                }
                await _api.DownloadAsync(await _api.PrepareDownloadAsync(clip.Id, format, studio), path);
                await SaveExtrasAsync(clip, path);
                Announcer.Announce(T("Saved {0}.", clip.DisplayTitle), interrupt: false);
                _ = RefreshAccountQuietlyAsync();
            });
        }

        private async Task RefreshAccountQuietlyAsync()
        {
            try { await _account.RefreshAsync(); }
            catch (SunoApiException ex) { Log.Warn($"Account refresh after download failed: {ex.Message}"); }
        }

        private string DownloadFolder()
        {
            var dir = _config.Current.Audio.DownloadDir;
            if (string.IsNullOrWhiteSpace(dir))
                dir = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.MyMusic), "Suno");
            try
            {
                Directory.CreateDirectory(dir);
            }
            catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or ArgumentException or NotSupportedException)
            {
                throw new SunoApiException(T("Can't use the download folder. Check it in Settings."), null, ex);
            }
            return dir;
        }
    }
}
