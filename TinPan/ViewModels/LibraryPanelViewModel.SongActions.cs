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

namespace TinPan.ViewModels
{
    // Reading, changing and sharing a single song.
    public partial class LibraryPanelViewModel
    {
        // ── Reading about a song ────────────────────────────────────────────

        [RelayCommand]
        private void Details(SunoClip? clip)
        {
            clip ??= SelectedClip;
            if (clip != null) _dialogs.ShowDocument($"Details: {clip.DisplayTitle}", clip.DetailsText);
        }

        /// <summary>Song information one fact per line, like KeySoft's track information (Alt+Enter).</summary>
        [RelayCommand]
        private void SongInfo(SunoClip? clip)
        {
            clip ??= SelectedClip;
            if (clip != null) _dialogs.ShowLines($"Information: {clip.DisplayTitle}", clip.InfoLines);
        }

        /// <summary>Plays a song from a saved position. Carries on through the library when the song is in it.</summary>
        public async Task ResumeAsync(string clipId, string title, double seconds)
        {
            var clip = Songs.FirstOrDefault(c => c.Id == clipId);
            _current = clip;
            if (!await RunAsync(() => _audio.PlayAsync(clipId, clip?.DisplayTitle ?? title, clip != null ? this : null))) return;
            if (seconds > 5) _audio.CurrentPosition = TimeSpan.FromSeconds(seconds);
            Announcer.Announce(seconds > 5
                ? $"Resuming {clip?.DisplayTitle ?? title} at {SunoClip.FormatTime(TimeSpan.FromSeconds(seconds))}."
                : $"Playing: {clip?.DisplayTitle ?? title}.");
        }

        [RelayCommand]
        private async Task LyricsAsync(SunoClip? clip)
        {
            clip ??= SelectedClip;
            if (clip == null) return;
            var lyrics = clip.Metadata?.Prompt?.Trim();
            if (string.IsNullOrEmpty(lyrics))
            {
                // Description-mode songs keep their words only in the timed lyrics.
                await RunAsync(async () =>
                {
                    var aligned = await _api.GetAlignedLyricsAsync(clip.Id);
                    lyrics = aligned == null ? null : JoinWords(aligned.Words);
                });
            }
            if (string.IsNullOrWhiteSpace(lyrics)) Announcer.Announce("This song has no lyrics.");
            else _dialogs.ShowDocument($"Lyrics: {clip.DisplayTitle}", lyrics);
        }

        private static string JoinWords(IEnumerable<AlignedWord> words)
        {
            var sb = new StringBuilder();
            foreach (var w in words)
            {
                // Some words carry their own trailing space or line break; add a space only when missing.
                sb.Append(w.Word);
                if (w.Word.Length > 0 && !char.IsWhiteSpace(w.Word[^1])) sb.Append(' ');
            }
            return sb.ToString().Trim();
        }

        [RelayCommand]
        private void CopyLink(SunoClip? clip)
        {
            clip ??= SelectedClip;
            if (clip == null) return;
            try
            {
                Clipboard.SetText(clip.WebLink);
                Announcer.Announce("Link copied.");
            }
            catch (System.Runtime.InteropServices.ExternalException)
            {
                Announcer.Announce("Couldn't use the clipboard. Try again.");
            }
        }

        // ── Changing a song ─────────────────────────────────────────────────

        [RelayCommand]
        private async Task TrashAsync(SunoClip? clip)
        {
            clip ??= SelectedClip;
            if (clip == null) return;
            if (!_dialogs.Confirm($"Move \"{clip.DisplayTitle}\" to your Suno trash?", "Move to trash")) return;

            if (await RunAsync(() => _api.TrashAsync(clip.Id)))
            {
                var index = Songs.IndexOf(clip);
                if (clip.Id == _audio.CurrentClipId) _audio.Stop();
                Songs.Remove(clip);
                if (Songs.Count > 0) SelectedClip = Songs[Math.Clamp(index, 0, Songs.Count - 1)];
                Announcer.Announce($"Moved {clip.DisplayTitle} to trash.");
            }
        }

        [RelayCommand]
        private async Task EditAsync(SunoClip? clip)
        {
            clip ??= SelectedClip;
            if (clip == null) return;
            var title = new TextField("_Title", clip.Title ?? "", maxLength: 100);
            var lyrics = new TextField("_Lyrics", clip.Metadata?.Prompt ?? "", multiline: true);
            if (!_dialogs.ShowForm($"Edit {clip.DisplayTitle}", "Changes what Suno shows for this song. The audio stays the same.", "Save", new FormField[] { title, lyrics }))
                return;

            var newTitle = title.Value != (clip.Title ?? "") ? title.Value : null;
            var newLyrics = lyrics.Value != (clip.Metadata?.Prompt ?? "").Trim() ? lyrics.Value : null;
            if (newTitle == null && newLyrics == null) return;

            if (await RunAsync(() => _api.SetDetailsAsync(clip.Id, newTitle, newLyrics)))
            {
                if (newTitle != null) clip.Title = newTitle;
                if (newLyrics != null) (clip.Metadata ??= new ClipMetadata()).Prompt = newLyrics;
                clip.NotifyChanged();
                Announcer.Announce("Saved.");
            }
        }

        [RelayCommand]
        private async Task ToggleVisibilityAsync(SunoClip? clip)
        {
            clip ??= SelectedClip;
            if (clip == null) return;
            var makePublic = !clip.IsPublic;
            if (await RunAsync(() => _api.SetPublicAsync(clip.Id, makePublic)))
            {
                clip.IsPublic = makePublic;
                clip.NotifyChanged();
                OnPropertyChanged(nameof(VisibilityMenuText));
                Announcer.Announce(makePublic ? $"{clip.DisplayTitle} is public." : $"{clip.DisplayTitle} is private.");
            }
        }

        // ── Sharing, remixes, similar songs, comments, playlists ───────────

        [RelayCommand]
        private async Task ShareLinkAsync(SunoClip? clip)
        {
            clip ??= SelectedClip;
            if (clip == null) return;
            await RunAsync(async () =>
            {
                var link = await _api.GetShareLinkAsync(clip.Id);
                try
                {
                    Clipboard.SetText(link);
                    Announcer.Announce("Share link copied.");
                }
                catch (System.Runtime.InteropServices.ExternalException)
                {
                    Announcer.Announce("Couldn't use the clipboard. Try again.");
                }
            });
        }

        [RelayCommand]
        private async Task ToggleRemixAsync(SunoClip? clip)
        {
            clip ??= SelectedClip;
            if (clip == null) return;
            var allow = clip.Metadata?.CanRemix == false;
            if (await RunAsync(() => _api.SetRemixAllowedAsync(clip.Id, allow)))
            {
                (clip.Metadata ??= new ClipMetadata()).CanRemix = allow;
                OnPropertyChanged(nameof(RemixMenuText));
                Announcer.Announce(allow ? "Others may now cover and remix it." : "Others can no longer cover or remix it.");
            }
        }

        [RelayCommand]
        private async Task SimilarAsync(SunoClip? clip)
        {
            clip ??= SelectedClip;
            if (clip == null) return;
            IReadOnlyList<SunoClip> similar = Array.Empty<SunoClip>();
            if (!await RunAsync(async () => similar = await _api.GetSimilarAsync(clip.Id))) return;
            if (similar.Count == 0)
            {
                Announcer.Announce("Suno found no similar songs.");
                return;
            }
            _dialogs.ShowSongs($"Songs similar to {clip.DisplayTitle}", similar, async s =>
            {
                if (await RunAsync(() => _audio.PlayAsync(s.Id, s.DisplayTitle))) Announcer.Announce($"Playing: {s.DisplayTitle}.");
            });
        }

        [RelayCommand]
        private async Task CommentsAsync(SunoClip? clip)
        {
            clip ??= SelectedClip;
            if (clip == null) return;
            IReadOnlyList<SongComment> comments = Array.Empty<SongComment>();
            if (!await RunAsync(async () => comments = await _api.GetCommentsAsync(clip.Id))) return;
            if (comments.Count == 0)
            {
                Announcer.Announce("No comments.");
                return;
            }
            var text = string.Join(Environment.NewLine + Environment.NewLine, comments.Select(c => c.ToString()));
            _dialogs.ShowDocument($"Comments on {clip.DisplayTitle} ({comments.Count})", text);
        }

        [RelayCommand]
        private async Task WriteCommentAsync(SunoClip? clip)
        {
            clip ??= SelectedClip;
            if (clip == null) return;
            var text = new TextField("_Comment", "", multiline: true, maxLength: 1000);
            if (!_dialogs.ShowForm($"Comment on {clip.DisplayTitle}", "Comments are public on Suno.", "Post", new FormField[] { text })
                || text.Value.Length == 0) return;
            if (await RunAsync(() => _api.PostCommentAsync(clip.Id, text.Value))) Announcer.Announce("Comment posted.");
        }

        [RelayCommand]
        private async Task AddToPlaylistAsync(SunoClip? clip)
        {
            clip ??= SelectedClip;
            if (clip == null) return;
            IReadOnlyList<Playlist> playlists = Array.Empty<Playlist>();
            if (!await RunAsync(async () => playlists = await _api.GetPlaylistsAsync())) return;

            const string newOne = "New playlist";
            var choices = new List<object> { newOne };
            choices.AddRange(playlists);
            var target = new ChoiceField("_Playlist", choices, playlists.FirstOrDefault() ?? (object)newOne);
            var name = new TextField("Name for a _new playlist", "", maxLength: 100);
            if (!_dialogs.ShowForm($"Add {clip.DisplayTitle} to a playlist", "Pick a playlist, or New playlist and give it a name.", "Add",
                    new FormField[] { target, name })) return;

            await RunAsync(async () =>
            {
                var playlist = target.Selected as Playlist;
                if (playlist == null)
                {
                    if (name.Value.Length == 0)
                    {
                        Announcer.Announce("Give the new playlist a name.");
                        return;
                    }
                    playlist = await _api.CreatePlaylistAsync(name.Value);
                }
                await _api.AddToPlaylistAsync(playlist.Id, new[] { clip.Id });
                Announcer.Announce($"Added to {playlist.DisplayName}.");
            });
        }
    }
}
