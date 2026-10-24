using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading.Tasks;
using CommunityToolkit.Mvvm.Input;
using TinPan.Models;
using TinPan.Views;
using static TinPan.Localization.Loc;

namespace TinPan.ViewModels
{
    // Song menu: the creator's songs, covers and remixes, the original song.
    // Discover menu: Explore, the following feed, contests and finding a creator.
    public partial class LibraryPanelViewModel
    {
        private const int CreatorPagesToLoad = 5;

        [RelayCommand]
        private async Task CreatorAsync(SunoClip? clip)
        {
            clip ??= SelectedClip;
            if (clip == null) return;
            if (string.IsNullOrWhiteSpace(clip.Handle))
            {
                Announcer.Announce(T("Suno didn't say who made this song."));
                return;
            }
            await ShowCreatorAsync(clip.Handle);
        }

        [RelayCommand]
        private async Task FindCreatorAsync()
        {
            var handle = new TextField(T("_Handle: the name after suno.com/@, or the whole link"));
            if (!_dialogs.ShowForm(T("Find a creator"), T("Shows a Suno creator's songs, newest first."), T("Find"), new FormField[] { handle })) return;
            var name = CreatorHandle(handle.Value);
            if (name.Length == 0) return;
            await ShowCreatorAsync(name);
        }

        /// <summary>"https://suno.com/@ana?x=1", "@ana" and "ana" are all "ana".</summary>
        public static string CreatorHandle(string text)
        {
            var t = text.Trim();
            var at = t.LastIndexOf('@');
            if (at >= 0) t = t[(at + 1)..];
            return t.Split('?', '#', '/')[0].Trim();
        }

        private async Task ShowCreatorAsync(string handle)
        {
            CreatorPage? first = null;
            var songs = new List<SunoClip>();
            if (!await RunAsync(async () =>
                {
                    for (var page = 1; page <= CreatorPagesToLoad; page++)
                    {
                        var p = await _api.GetCreatorAsync(handle, page);
                        first ??= p;
                        var before = songs.Count;
                        songs.AddRange(p.Songs.Where(s => songs.All(o => o.Id != s.Id)));
                        if (p.Songs.Count < 20 || songs.Count == before || songs.Count >= p.TotalSongs) break;
                    }
                })) return;
            if (first == null) return;
            if (songs.Count == 0)
            {
                Announcer.Announce(T("{0} has no public songs.", first.Name));
                return;
            }
            ShowSongList(first.Summary, songs, sayCreator: false);
        }

        [RelayCommand]
        private async Task RemixesAsync(SunoClip? clip)
        {
            clip ??= SelectedClip;
            if (clip == null) return;
            IReadOnlyList<SunoClip> remixes = Array.Empty<SunoClip>();
            if (!await RunAsync(async () => remixes = await _api.GetRemixesAsync(clip.Id))) return;
            if (remixes.Count == 0)
            {
                Announcer.Announce(T("No one has covered or remixed {0} yet.", clip.DisplayTitle));
                return;
            }
            ShowSongList(T("Covers and remixes of {0}", clip.DisplayTitle), remixes);
        }

        [RelayCommand]
        private async Task OriginalAsync(SunoClip? clip)
        {
            clip ??= SelectedClip;
            if (clip == null) return;
            SunoClip? original = null;
            if (!await RunAsync(async () => original = await _api.GetOriginalAsync(clip))) return;
            if (original == null)
            {
                Announcer.Announce(T("{0} wasn't made from another song.", clip.DisplayTitle));
                return;
            }
            ShowSongList(T("{0} was made from", clip.DisplayTitle), new[] { original });
        }

        [RelayCommand]
        private async Task ExploreAsync()
        {
            IReadOnlyList<ExploreSection> sections = Array.Empty<ExploreSection>();
            if (!await RunAsync(async () => sections = await _api.GetExploreAsync())) return;
            if (sections.Count == 0)
            {
                Announcer.Announce(T("Explore has nothing to show right now."));
                return;
            }
            _dialogs.ShowLines(T("Explore. Enter opens a section."), sections.Select(s => s.ToString()).ToList(),
                index => ShowSongList(sections[index].Title, sections[index].Songs));
        }

        [RelayCommand]
        private async Task FollowingAsync()
        {
            IReadOnlyList<SunoClip> songs = Array.Empty<SunoClip>();
            if (!await RunAsync(async () => songs = await _api.GetFollowingFeedAsync())) return;
            if (songs.Count == 0)
            {
                Announcer.Announce(T("Nothing new from the people you follow."));
                return;
            }
            ShowSongList(T("From people you follow"), songs);
        }

        [RelayCommand]
        private async Task ContestsAsync()
        {
            IReadOnlyList<Contest> contests = Array.Empty<Contest>();
            if (!await RunAsync(async () => contests = await _api.GetContestsAsync())) return;
            if (contests.Count == 0)
            {
                Announcer.Announce(T("Suno has no contests right now."));
                return;
            }
            _dialogs.ShowLines(T("Suno contests"), contests.Select(c => c.ToString()).ToList(), async index =>
            {
                var contest = contests[index];
                if (contest.BaseClipId == null)
                {
                    Announcer.Announce(T("{0} has no song to play.", contest.Title));
                    return;
                }
                if (await RunAsync(() => _audio.PlayAsync(contest.BaseClipId, contest.Title)))
                    Announcer.Announce(T("Playing: {0}.", contest.Title));
            });
        }

        /// <summary>Songs in their own window; Enter plays one. Each says who made it unless sayCreator is false.</summary>
        private void ShowSongList(string title, IReadOnlyList<SunoClip> songs, bool sayCreator = true) =>
            _dialogs.ShowSongs(title, songs, async s =>
            {
                if (await RunAsync(() => _audio.PlayAsync(s.Id, s.DisplayTitle))) Announcer.Announce(T("Playing: {0}.", s.DisplayTitle));
            }, sayCreator);
    }
}
