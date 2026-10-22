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

        /// <summary>When to clean up a download with sunscreen.</summary>
        private enum Cleanup { Ask, FromSettings, Never }

        /// <summary>Download (Ctrl+D): the format from Settings, and asks every time whether to clean it up.</summary>
        [RelayCommand]
        private Task DownloadAsync(SunoClip? clip)
        {
            var format = _config.Current.Audio.OutputFormat;
            if (format != "wav" || _account.IsPaid != false) return DownloadInFormatAsync(clip ?? SelectedClip, format, Cleanup.Ask);
            return DownloadInFormatAsync(clip ?? SelectedClip, "mp3", Cleanup.Ask, T("WAV needs Pro or Premier, so this is an MP3.") + " ");
        }

        /// <summary>Download MP3 and Download WAV don't ask; Settings decides whether to clean up.</summary>
        [RelayCommand]
        private Task DownloadMp3Async(SunoClip? clip) => DownloadInFormatAsync(clip ?? SelectedClip, "mp3", Cleanup.FromSettings);

        [RelayCommand(CanExecute = nameof(CanUsePaidFeatures))]
        private Task DownloadWavAsync(SunoClip? clip) => DownloadInFormatAsync(clip ?? SelectedClip, "wav", Cleanup.FromSettings);

        [RelayCommand]
        private Task DownloadVideoAsync(SunoClip? clip) => DownloadInFormatAsync(clip ?? SelectedClip, "mp4", Cleanup.Never);

        /// <summary>Cleans up a song that is already downloaded (its MP3, M4A or WAV in the download folder).</summary>
        [RelayCommand]
        private async Task CleanUpDownloadAsync(SunoClip? clip)
        {
            clip ??= SelectedClip;
            if (clip == null) return;
            string? path;
            try
            {
                path = DownloadedFile(clip);
            }
            catch (SunoApiException ex)
            {
                Announcer.Announce(ex.Message, always: true);
                return;
            }
            if (path == null)
            {
                Announcer.Announce(T("{0} isn't downloaded yet. Download it first, or use Download, which offers to clean it up.", clip.DisplayTitle), always: true);
                return;
            }
            await CleanUpAsync(clip, path);
        }

        /// <summary>The song's downloaded audio file, if there is one: its MP3, M4A or WAV, not a cleaned-up copy.</summary>
        private string? DownloadedFile(SunoClip clip)
        {
            var folder = DownloadFolder();
            var stem = Path.GetFileNameWithoutExtension(FileNames.ForSong(clip.Title, clip.Id, "x"));
            return new[] { ".m4a", ".mp3", ".wav" }.Select(e => Path.Combine(folder, stem + e)).FirstOrDefault(File.Exists);
        }

        /// <summary>The question Download asks every time: what clean-up does and what it costs in space.</summary>
        private bool AskToCleanUp(SunoClip clip)
        {
            var keep = _config.Current.Cleanup.KeepOriginal
                ? T("The download is kept too.")
                : T("It replaces the download (Settings can keep both).");
            return _dialogs.Ask(T("Clean up {0} too? sunscreen removes the haze and buzz in Suno songs and saves the result as a 24-bit WAV file, often 10 to 20 times bigger than the download. {1} It takes about 10 seconds for a 5-minute song. The clean-up steps are in Settings.", clip.DisplayTitle, keep),
                T("Clean up"));
        }

        /// <summary>Runs sunscreen on a saved download and says how it went. False when it failed.</summary>
        private async Task<bool> CleanUpAsync(SunoClip clip, string path, bool quiet = false)
        {
            if (!quiet) Announcer.Announce(T("Cleaning up {0}.", clip.DisplayTitle), interrupt: false);
            try
            {
                await _cleanup.CleanAsync(path, _config.Current.Cleanup);
                if (!quiet) Announcer.Announce(T("Cleaned up {0}.", clip.DisplayTitle), interrupt: false);
                return true;
            }
            catch (SunoApiException ex)
            {
                Log.Warn($"Clean-up of {clip.Id} failed: {ex.Message}");
                if (!quiet) Announcer.Announce(ex.Message, always: true);
                return false;
            }
        }

        private async Task DownloadInFormatAsync(SunoClip? clip, string format, Cleanup cleanup, string note = "")
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
                if (cleanup == Cleanup.Ask && format != "mp4" && !File.Exists(CleanupService.CleanedPath(path)) && AskToCleanUp(clip))
                    await CleanUpAsync(clip, path);
                else
                    Announcer.Announce(T("{0} is already downloaded.", clip.DisplayTitle));
                return;
            }

            var clean = format != "mp4" && cleanup switch
            {
                Cleanup.Ask => AskToCleanUp(clip),
                Cleanup.FromSettings => _config.Current.Cleanup.AfterDownload,
                _ => false
            };

            Announcer.Announce(note + (format == "wav" ? T("Asking Suno for a WAV of {0}.", clip.DisplayTitle) : T("Downloading {0}.", clip.DisplayTitle)));
            try
            {
                await DownloadDirectAsync(clip, format, path);
                Announcer.Announce(T("Saved {0}.", clip.DisplayTitle), interrupt: false);
                if (clean) await CleanUpAsync(clip, path);
            }
            catch (SunoApiException ex)
            {
                Log.Warn($"Download {clip.Id} ({format}) failed directly: {ex.Message}");
                // A sign-in or save problem would fail Suno's route too, so only offer it for a refused file.
                if (format == "wav" || ex is SunoSignInRequiredException || ex.InnerException is IOException or UnauthorizedAccessException
                    || !OfferSunoDownload(clip, T("The free file didn't download: {0}", ex.Message)))
                {
                    Announcer.Announce(ex.Message, always: true);
                    return;
                }
                if (await DownloadThroughSunoAsync(clip, format, path) && clean) await CleanUpAsync(clip, path);
            }
        }

        /// <summary>
        /// The free audio file: the unencrypted entry in media_urls, or else audio_url when it is a
        /// real file address. Suno now sets audio_url to a placeholder that always answers 403
        /// for songs it only keeps encrypted; that one is never used.
        /// </summary>
        internal static string? DirectAudioUrl(SunoClip clip)
        {
            if (clip.ProgressiveAudioUrl is { } plain) return plain;
            var url = clip.AudioUrl;
            if (string.IsNullOrEmpty(url) || !Uri.TryCreate(url, UriKind.Absolute, out var uri) || uri.Scheme != Uri.UriSchemeHttps) return null;
            return uri.AbsolutePath.Contains("forbidden", StringComparison.OrdinalIgnoreCase) || uri.Host.StartsWith("studio-api", StringComparison.OrdinalIgnoreCase)
                ? null
                : url;
        }

        /// <summary>
        /// The song's own file: unmetered direct stream (M4A) via progressive or Mango DRM,
        /// or official WAV conversion. Avoids the decoy /api/forbidden address.
        /// </summary>
        private string TargetPath(SunoClip clip, string format, out string? directUrl)
        {
            if (format == "wav")
            {
                directUrl = null;
                return Path.Combine(DownloadFolder(), FileNames.ForSong(clip.Title, clip.Id, "wav"));
            }
            if (format == "mp4")
            {
                directUrl = clip.VideoUrl;
                return Path.Combine(DownloadFolder(), FileNames.ForSong(clip.Title, clip.Id, "mp4"));
            }

            var (pickedUrl, _) = SunoDrm.PickMediaUrl(clip, preferUnencrypted: true);
            directUrl = pickedUrl;
            var ext = FileNames.ExtensionOf(directUrl, "m4a");

            var folder = DownloadFolder();
            var primaryPath = Path.Combine(folder, FileNames.ForSong(clip.Title, clip.Id, ext));
            if (File.Exists(primaryPath)) return primaryPath;

            var altExt = ext == "m4a" ? "mp3" : "m4a";
            var altPath = Path.Combine(folder, FileNames.ForSong(clip.Title, clip.Id, altExt));
            if (File.Exists(altPath)) return altPath;

            return primaryPath;
        }

        private async Task DownloadDirectAsync(SunoClip clip, string format, string path)
        {
            if (format == "wav")
            {
                var url = await _api.GetWavUrlAsync(clip.Id);
                if (string.IsNullOrEmpty(url)) throw new SunoApiException(T("Suno gave no file address for this song."));
                await _api.DownloadAsync(url, path);
            }
            else if (format == "mp4")
            {
                var url = clip.VideoUrl;
                if (string.IsNullOrEmpty(url)) throw new SunoApiException(T("Suno gave no file address for this song."));
                await _api.DownloadAsync(url, path);
            }
            else
            {
                await _api.DownloadClipDirectAsync(clip, path);
            }
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

            var clean = _config.Current.Cleanup.AfterDownload;
            Announcer.Announce(T("Downloading {0}.", Plural(missing.Count, "{0} song", "{0} songs")) + (clean ? " " + T("Each one is cleaned up after it downloads.") : ""));
            var saved = 0;
            var cleaned = 0;
            var failed = new List<string>();
            for (var i = 0; i < missing.Count; i++)
            {
                var clip = missing[i];
                try
                {
                    var path = TargetPath(clip, format, out _);
                    await DownloadDirectAsync(clip, format, path);
                    saved++;
                    if (clean && await CleanUpAsync(clip, path, quiet: true)) cleaned++;
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
            if (clean && saved > 0)
                summary += " " + (cleaned == saved ? T("All of them are cleaned up.") : T("{0} of them cleaned up; the log says why the others weren't.", cleaned));
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
            return _dialogs.Confirm(T("{0} Use Suno's own download for \"{1}\" instead? {2}", problem, clip.DisplayTitle, cost), T("Download"));
        }

        /// <returns>True when the file was saved.</returns>
        private async Task<bool> DownloadThroughSunoAsync(SunoClip clip, string format, string path)
        {
            var studio = _account.Billing?.HasStudio == true;
            Announcer.Announce(T("Downloading {0} through Suno.", clip.DisplayTitle));
            return await RunAsync(async () =>
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
