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
    // Reading, changing and sharing a single song.
    public partial class LibraryPanelViewModel
    {
        // ── Reading about a song ────────────────────────────────────────────

        [RelayCommand]
        private void Details(SunoClip? clip)
        {
            clip ??= SelectedClip;
            if (clip != null) _dialogs.ShowDocument(T("Details: {0}", clip.DisplayTitle), clip.DetailsText);
        }

        /// <summary>Song information one fact per line, like KeySoft's track information (Alt+Enter).</summary>
        [RelayCommand]
        private void SongInfo(SunoClip? clip)
        {
            clip ??= SelectedClip;
            if (clip != null) _dialogs.ShowLines(T("Information: {0}", clip.DisplayTitle), clip.InfoLines);
        }

        /// <summary>Plays a song from a saved position. Carries on through the library when the song is in it.</summary>
        public async Task ResumeAsync(string clipId, string title, double seconds)
        {
            var clip = Songs.FirstOrDefault(c => c.Id == clipId);
            _current = clip;
            if (!await RunAsync(() => _audio.PlayAsync(clipId, clip?.DisplayTitle ?? title, clip != null ? this : null))) return;
            if (seconds > 5) _audio.CurrentPosition = TimeSpan.FromSeconds(seconds);
            Announcer.Announce(seconds > 5
                ? T("Resuming {0} at {1}.", clip?.DisplayTitle ?? title, SunoClip.FormatTime(TimeSpan.FromSeconds(seconds)))
                : T("Playing: {0}.", clip?.DisplayTitle ?? title));
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
                // A failed fetch has already said why; "no lyrics" on top would be wrong.
                if (!await RunAsync(async () =>
                    {
                        var aligned = await _api.GetAlignedLyricsAsync(clip.Id);
                        lyrics = aligned == null ? null : JoinWords(aligned.Words);
                    })) return;
            }
            if (string.IsNullOrWhiteSpace(lyrics)) Announcer.Announce(T("This song has no lyrics."));
            else _dialogs.ShowDocument(T("Lyrics: {0}", clip.DisplayTitle), lyrics);
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
                Announcer.Announce(T("Link copied."));
            }
            catch (System.Runtime.InteropServices.ExternalException)
            {
                Announcer.Announce(T("Couldn't use the clipboard. Try again."));
            }
        }

        // ── Changing a song ─────────────────────────────────────────────────

        [RelayCommand]
        private async Task TrashAsync(SunoClip? clip)
        {
            clip ??= SelectedClip;
            if (clip == null) return;
            if (clip.IsTrashed == true)
            {
                await RestoreAsync(clip);
                return;
            }
            if (!_dialogs.Confirm(T("Move \"{0}\" to your Suno trash?", clip.DisplayTitle), T("Move to trash"))) return;

            if (await RunAsync(() => _api.TrashAsync(clip.Id)))
            {
                var index = Songs.IndexOf(clip);
                if (clip.Id == _audio.CurrentClipId) _audio.Stop();
                // A song trashed from a playlist or Trending may not be in the Library list at all.
                if (index >= 0)
                {
                    Songs.RemoveAt(index);
                    if (Songs.Count > 0) SelectedClip = Songs[Math.Clamp(index, 0, Songs.Count - 1)];
                }
                Announcer.Announce(T("Moved {0} to trash.", clip.DisplayTitle));
            }
        }

        [RelayCommand]
        private async Task EditAsync(SunoClip? clip)
        {
            clip ??= SelectedClip;
            if (clip == null) return;
            var title = new TextField(T("_Title"), clip.Title ?? "", maxLength: 100);
            var lyrics = new TextField(T("_Lyrics"), clip.Metadata?.Prompt ?? "", multiline: true);
            if (!_dialogs.ShowForm(T("Edit {0}", clip.DisplayTitle), T("Changes what Suno shows for this song. The audio stays the same."), T("Save"), new FormField[] { title, lyrics }))
                return;

            var newTitle = title.Value != (clip.Title ?? "") ? title.Value : null;
            var newLyrics = lyrics.Value != (clip.Metadata?.Prompt ?? "").Trim() ? lyrics.Value : null;
            if (newTitle == null && newLyrics == null) return;

            if (await RunAsync(() => _api.SetDetailsAsync(clip.Id, newTitle, newLyrics)))
            {
                if (newTitle != null) clip.Title = newTitle;
                if (newLyrics != null) (clip.Metadata ??= new ClipMetadata()).Prompt = newLyrics;
                clip.NotifyChanged();
                Announcer.Announce(T("Saved."));
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
                Announcer.Announce(makePublic ? T("{0} is public.", clip.DisplayTitle) : T("{0} is private.", clip.DisplayTitle));
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
                    Announcer.Announce(T("Share link copied."));
                }
                catch (System.Runtime.InteropServices.ExternalException)
                {
                    Announcer.Announce(T("Couldn't use the clipboard. Try again."));
                }
            });
        }

        /// <summary>Like (Ctrl+Shift+U), or remove the like. Suno uses likes for your Liked filter and its suggestions.</summary>
        [RelayCommand]
        private Task ToggleLikeAsync(SunoClip? clip) => ReactAsync(clip ?? SelectedClip, like: true);

        /// <summary>Dislike (Ctrl+Shift+N), or remove the dislike. Disliked songs are hidden unless the filters show them.</summary>
        [RelayCommand]
        private Task ToggleDislikeAsync(SunoClip? clip) => ReactAsync(clip ?? SelectedClip, like: false);

        public static string LikeLabel(SunoClip? song) => song?.IsLiked == true ? T("Remove li_ke") : T("Li_ke");
        public static string DislikeLabel(SunoClip? song) => song?.IsDisliked == true ? T("Remove d_islike") : T("D_islike");

        private async Task ReactAsync(SunoClip? clip, bool like)
        {
            if (clip == null) return;
            var undo = like ? clip.IsLiked : clip.IsDisliked;
            string? state = null;
            if (!await RunAsync(async () => state = await _api.SetReactionAsync(clip.Id, undo ? null : like ? "LIKE" : "DISLIKE"))) return;
            clip.IsLiked = state == "L";
            clip.IsDisliked = state == "D";
            clip.NotifyChanged();
            Announcer.Announce(undo
                ? (like ? T("Like removed from {0}.", clip.DisplayTitle) : T("Dislike removed from {0}.", clip.DisplayTitle))
                : (like ? T("Liked {0}.", clip.DisplayTitle) : T("Disliked {0}.", clip.DisplayTitle)));
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
                Announcer.Announce(allow ? T("Others may now cover and remix it.") : T("Others can no longer cover or remix it."));
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
                Announcer.Announce(T("Suno found no similar songs."));
                return;
            }
            ShowSongList(T("Songs similar to {0}", clip.DisplayTitle), similar);
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
                Announcer.Announce(T("No comments."));
                return;
            }
            var text = string.Join(Environment.NewLine + Environment.NewLine, comments.Select(c => c.ToString()));
            _dialogs.ShowDocument(T("Comments on {0} ({1})", clip.DisplayTitle, comments.Count), text);
        }

        [RelayCommand]
        private async Task WriteCommentAsync(SunoClip? clip)
        {
            clip ??= SelectedClip;
            if (clip == null) return;
            var text = new TextField(T("_Comment"), "", multiline: true, maxLength: 1000);
            if (!_dialogs.ShowForm(T("Comment on {0}", clip.DisplayTitle), T("Comments are public on Suno."), T("Post"), new FormField[] { text })
                || text.Value.Length == 0) return;
            if (await RunAsync(() => _api.PostCommentAsync(clip.Id, text.Value))) Announcer.Announce(T("Comment posted."));
        }

        [RelayCommand]
        private async Task AddToPlaylistAsync(SunoClip? clip)
        {
            clip ??= SelectedClip;
            if (clip == null) return;
            IReadOnlyList<Playlist> playlists = Array.Empty<Playlist>();
            if (!await RunAsync(async () => playlists = await _api.GetPlaylistsAsync())) return;

            var newOne = T("New playlist");
            var choices = new List<object> { newOne };
            choices.AddRange(playlists);
            var target = new ChoiceField(T("_Playlist"), choices, playlists.FirstOrDefault() ?? (object)newOne);
            var name = new TextField(T("Name for a _new playlist"), "", maxLength: 100);
            if (!_dialogs.ShowForm(T("Add {0} to a playlist", clip.DisplayTitle), T("Pick a playlist, or New playlist and give it a name."), T("Add"),
                    new FormField[] { target, name })) return;

            await RunAsync(async () =>
            {
                var playlist = target.Selected as Playlist;
                if (playlist == null)
                {
                    if (name.Value.Length == 0)
                    {
                        Announcer.Announce(T("Give the new playlist a name."));
                        return;
                    }
                    playlist = await _api.CreatePlaylistAsync(name.Value);
                }
                await _api.AddToPlaylistAsync(playlist.Id, new[] { clip.Id });
                Announcer.Announce(T("Added to {0}.", playlist.DisplayName));
            });
        }
    }
}
