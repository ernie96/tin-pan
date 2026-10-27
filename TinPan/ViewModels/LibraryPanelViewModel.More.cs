using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Threading.Tasks;
using CommunityToolkit.Mvvm.Input;
using TinPan.Models;
using TinPan.Services;
using TinPan.Views;
using static TinPan.Localization.Loc;

namespace TinPan.ViewModels
{
    // Song credits, stems already made, recommendations, the remix list, trash, cover pictures, video,
    // personas, a creator's playlists, custom models and lyrics mashups.
    public partial class LibraryPanelViewModel
    {
        private static readonly TimeSpan VideoCheckEvery = TimeSpan.FromSeconds(6);
        private static readonly TimeSpan VideoGiveUpAfter = TimeSpan.FromMinutes(10);

        // Suno doesn't say which songs you marked; this remembers the ones marked in this run.
        private readonly HashSet<string> _notInterested = new();

        public string NotInterestedLabel(SunoClip? song) =>
            song != null && _notInterested.Contains(song.Id) ? T("Suggest songs like this a_gain") : T("Not interes_ted: fewer like this");

        public static string ShowRemixesLabel(SunoClip? song) =>
            song?.Metadata?.ShowRemix == true ? T("Hide the list of re_mixes") : T("Show the list of re_mixes");

        public static string TrashLabel(SunoClip? song) => song?.IsTrashed == true ? T("Restore from tr_ash") : T("Move to tr_ash");

        // ── Song: recommendations, remix list, credits, stems ───────────────

        [RelayCommand]
        private async Task NotInterestedAsync(SunoClip? clip)
        {
            clip ??= SelectedClip;
            if (clip == null) return;
            var mark = !_notInterested.Contains(clip.Id);
            if (!await RunAsync(() => _api.SetNotInterestedAsync(clip.Id, mark))) return;
            if (mark) _notInterested.Add(clip.Id);
            else _notInterested.Remove(clip.Id);
            Announcer.Announce(mark ? T("Suno will suggest fewer songs like {0}.", clip.DisplayTitle) : T("Suno may suggest songs like {0} again.", clip.DisplayTitle));
        }

        [RelayCommand]
        private async Task ToggleShowRemixesAsync(SunoClip? clip)
        {
            clip ??= SelectedClip;
            if (clip == null) return;
            var show = clip.Metadata?.ShowRemix != true;
            if (!await RunAsync(() => _api.SetShowRemixesAsync(clip.Id, show))) return;
            (clip.Metadata ??= new ClipMetadata()).ShowRemix = show;
            Announcer.Announce(show ? T("The song's page now lists its remixes.") : T("The song's page no longer lists its remixes."));
        }

        [RelayCommand]
        private async Task CreditsAsync(SunoClip? clip)
        {
            clip ??= SelectedClip;
            if (clip == null) return;
            SongCredits? credits = null;
            if (!await RunAsync(async () => credits = await _api.GetCreditsAsync(clip.Id))) return;
            var lines = new List<string>
            {
                credits!.RemixCount is { } n
                    ? (credits.RemixCountCapped ? T("Remixes and covers: {0} or more.", n) : Plural(n, "{0} remix or cover.", "{0} remixes and covers."))
                    : T("Suno didn't say how many remixes it has.")
            };
            if (credits.Sources.Count == 0) lines.Add(T("It credits no other songs."));
            lines.AddRange(credits.Sources.Select(s =>
            {
                var parts = new List<string> { s.Title };
                if (s.Creator != null) parts.Add(T("by {0}", s.Creator));
                if (!string.IsNullOrEmpty(s.Relationship)) parts.Add(s.Relationship.Replace('_', ' '));
                if (s.Deleted) parts.Add(T("deleted"));
                return string.Join(", ", parts);
            }));
            _dialogs.ShowLines(T("Credits for {0}. Enter plays a credited song.", clip.DisplayTitle), lines, async index =>
            {
                var source = index > 0 ? credits.Sources[index - 1] : null;
                if (source?.ClipId == null || source.Deleted) return;
                IReadOnlyList<SunoClip> found = Array.Empty<SunoClip>();
                if (await RunAsync(async () => found = await _api.GetClipsAsync(new[] { source.ClipId })) && found.Count > 0)
                    ShowSongList(source.Title, found);
            });
        }

        [RelayCommand]
        private async Task ExistingStemsAsync(SunoClip? clip)
        {
            clip ??= SelectedClip;
            if (clip == null) return;
            IReadOnlyList<SunoClip> stems = Array.Empty<SunoClip>();
            if (!await RunAsync(async () => stems = await _api.GetExistingStemsAsync(clip.Id))) return;
            if (stems.Count == 0)
            {
                Announcer.Announce(T("No stems have been made from {0} yet. Stems, under Make from it, makes them.", clip.DisplayTitle));
                return;
            }
            ShowSongList(T("Stems of {0}", clip.DisplayTitle), stems, sayCreator: false);
        }

        // ── Song: trash ─────────────────────────────────────────────────────

        private async Task RestoreAsync(SunoClip clip)
        {
            if (!await RunAsync(() => _api.RestoreFromTrashAsync(clip.Id))) return;
            clip.IsTrashed = false;
            clip.NotifyChanged();
            Announcer.Announce(T("Restored {0} from the trash.", clip.DisplayTitle));
        }

        [RelayCommand]
        private async Task DeleteForeverAsync(SunoClip? clip)
        {
            clip ??= SelectedClip;
            if (clip == null) return;
            if (clip.IsTrashed != true)
            {
                Announcer.Announce(T("Only songs in the trash can be deleted for good. Move it to the trash first."));
                return;
            }
            if (!_dialogs.Confirm(T("Delete \"{0}\" for good? This can't be undone.", clip.DisplayTitle), T("Delete forever"))) return;
            if (!await RunAsync(() => _api.DeleteForeverAsync(new[] { clip.Id }))) return;
            if (clip.Id == _audio.CurrentClipId) _audio.Stop();
            var index = Songs.IndexOf(clip);
            if (index >= 0)
            {
                Songs.RemoveAt(index);
                if (Songs.Count > 0) SelectedClip = Songs[Math.Clamp(index, 0, Songs.Count - 1)];
            }
            Announcer.Announce(T("Deleted {0} for good.", clip.DisplayTitle));
        }

        // ── Song: cover picture and video ───────────────────────────────────

        private static (string Key, string Label)[] CoverSources => new[]
        {
            ("file", T("A picture from this computer")), ("prompt", T("A picture Suno draws from your description")), ("none", T("Suno's own cover again"))
        };

        [RelayCommand]
        private async Task CoverPictureAsync(SunoClip? clip)
        {
            clip ??= SelectedClip;
            if (clip == null) return;
            var source = new ChoiceField(T("_New cover"), CoverSources.Select(s => (object)s.Label).ToList(), CoverSources[0].Label);
            var prompt = new TextField(T("_Description, for a picture Suno draws"), "");
            if (!_dialogs.ShowForm(T("Cover picture of {0}", clip.DisplayTitle), T("Changes the picture Suno shows for this song."), T("Change"),
                    new FormField[] { source, prompt })) return;
            var key = CoverSources.First(s => s.Label == (string?)source.Selected).Key;
            string? uploaded = null, url = null;
            if (key == "file")
            {
                var path = _dialogs.PickPicture();
                if (path == null) return;
                Announcer.Announce(T("Sending the picture."));
                if (!await RunAsync(async () => uploaded = await _uploads.UploadPictureAsync(path))) return;
            }
            else if (key == "prompt")
            {
                if (prompt.Value.Length == 0)
                {
                    Announcer.Announce(T("Describe the picture first."));
                    return;
                }
                Announcer.Announce(T("Suno is drawing the picture."));
                if (!await RunAsync(async () => url = await _api.MakePictureAsync(prompt.Value))) return;
            }
            if (!await RunAsync(() => _api.SetSongCoverAsync(clip.Id, uploaded, url))) return;
            if (url != null) clip.ImageUrl = url;
            Announcer.Announce(key == "none" ? T("{0} has Suno's own cover again.", clip.DisplayTitle) : T("{0} has its new cover.", clip.DisplayTitle));
        }

        [RelayCommand]
        private async Task MakeVideoAsync(SunoClip? clip)
        {
            clip ??= SelectedClip;
            if (clip == null || !Finished(clip)) return;
            if (!await RunAsync(() => _api.StartVideoAsync(clip.Id))) return;
            Announcer.Announce(T("Making the video for {0}. You'll hear when it's ready.", clip.DisplayTitle));
            var until = DateTime.UtcNow + VideoGiveUpAfter;
            while (DateTime.UtcNow < until)
            {
                await Task.Delay(VideoCheckEvery);
                (string Status, string? Url) state;
                try { state = await _api.GetVideoStatusAsync(clip.Id); }
                catch (SunoApiException ex) { Log.Warn($"Video status for {clip.Id}: {ex.Message}"); continue; }
                if (state.Status is "failed" or "error")
                {
                    Announcer.Announce(T("Suno couldn't make the video for {0}.", clip.DisplayTitle), interrupt: false, always: true);
                    return;
                }
                if (state.Status == "complete" && !string.IsNullOrEmpty(state.Url))
                {
                    clip.VideoUrl = state.Url;
                    Announcer.Announce(T("The video for {0} is ready. Download video saves it.", clip.DisplayTitle), interrupt: false);
                    return;
                }
            }
            Announcer.Announce(T("The video for {0} is taking long. Check again later with Download video.", clip.DisplayTitle), interrupt: false);
        }

        // ── Personas ────────────────────────────────────────────────────────

        [RelayCommand]
        private async Task MakePersonaAsync(SunoClip? clip)
        {
            clip ??= SelectedClip;
            if (clip == null || !Finished(clip)) return;
            var name = new TextField(T("_Name"), clip.Title ?? "", maxLength: 80);
            var description = new TextField(T("_Description (the voice and style, for example)"), clip.Metadata?.Tags ?? "");
            var isPublic = new CheckField(T("_Public: others can use it too"), false);
            if (!_dialogs.ShowForm(T("Make a persona from {0}", clip.DisplayTitle),
                    T("A persona keeps this song's voice and style, so new songs, covers and extends can sound like it. Choose it in the Persona box when you make one."),
                    T("Make persona"), new FormField[] { name, description, isPublic })) return;
            if (name.Value.Length == 0)
            {
                Announcer.Announce(T("Give the persona a name."));
                return;
            }
            if (await RunAsync(() => _api.CreatePersonaAsync(clip.Id, name.Value, description.Value, isPublic.Value)))
                Announcer.Announce(T("Made the persona {0}.", name.Value));
        }

        private static (PersonaList List, string Label)[] PersonaLists => new[]
        {
            (PersonaList.Mine, T("My personas")), (PersonaList.Loved, T("Personas I love")), (PersonaList.Followed, T("Personas I follow"))
        };

        [RelayCommand]
        private void Personas() =>
            _dialogs.ShowLines(T("Personas. Enter opens a list."), PersonaLists.Select(l => l.Label).ToList(), index => _ = ShowPersonasAsync(PersonaLists[index].List));

        private async Task ShowPersonasAsync(PersonaList list)
        {
            IReadOnlyList<PersonaDetails> personas = Array.Empty<PersonaDetails>();
            if (!await RunAsync(async () => personas = await _api.GetPersonaListAsync(list))) return;
            if (personas.Count == 0)
            {
                Announcer.Announce(list == PersonaList.Mine ? T("You have no personas yet. Make a persona, under Make from it, makes one from a song.") : T("None yet."));
                return;
            }
            _dialogs.ShowLines(T("{0}. Enter for what you can do with one.", PersonaLists.First(l => l.List == list).Label),
                personas.Select(p => p.ToString()).ToList(), index => PersonaActions(personas[index], list));
        }

        private void PersonaActions(PersonaDetails p, PersonaList list)
        {
            var actions = new List<(string Label, Func<Task> Run)>
            {
                (T("Songs made with it"), async () =>
                {
                    IReadOnlyList<SunoClip> songs = Array.Empty<SunoClip>();
                    if (!await RunAsync(async () => songs = await _api.GetPersonaSongsAsync(p.Id))) return;
                    if (songs.Count == 0) Announcer.Announce(T("No songs made with {0} yet.", p.Name));
                    else ShowSongList(T("Songs made with {0}", p.Name), songs);
                })
            };
            if (p.RootClipId != null)
                actions.Add((T("Its source song"), async () =>
                {
                    IReadOnlyList<SunoClip> found = Array.Empty<SunoClip>();
                    if (await RunAsync(async () => found = await _api.GetClipsAsync(new[] { p.RootClipId })) && found.Count > 0)
                        ShowSongList(T("Source song of {0}", p.Name), found);
                }));
            actions.Add((p.IsLoved ? T("Stop loving it") : T("Love it"), async () =>
            {
                var loved = false;
                if (await RunAsync(async () => loved = await _api.TogglePersonaLoveAsync(p.Id)))
                    Announcer.Announce(loved ? T("You love {0}.", p.Name) : T("You no longer love {0}.", p.Name));
            }));
            if (p.IsOwned)
            {
                actions.Add((T("Rename and describe..."), () => EditPersonaAsync(p)));
                actions.Add((p.IsPublic ? T("Make it private") : T("Make it public"), async () =>
                {
                    if (await RunAsync(() => _api.EditPersonaAsync(p.Id, p.Name, p.Description, !p.IsPublic)))
                        Announcer.Announce(p.IsPublic ? T("{0} is private now.", p.Name) : T("{0} is public now.", p.Name));
                }));
                actions.Add((p.IsTrashed ? T("Restore from the trash") : T("Move to the trash"), async () =>
                {
                    if (!p.IsTrashed && !_dialogs.Confirm(T("Move the persona \"{0}\" to the trash?", p.Name), T("Move to trash"))) return;
                    if (await RunAsync(() => _api.TrashPersonaAsync(p.Id, undo: p.IsTrashed)))
                        Announcer.Announce(p.IsTrashed ? T("Restored {0} from the trash.", p.Name) : T("Moved {0} to trash.", p.Name));
                }));
            }
            _dialogs.ShowLines(p.Name, actions.Select(a => a.Label).ToList(), index => _ = actions[index].Run());
        }

        private async Task EditPersonaAsync(PersonaDetails p)
        {
            var name = new TextField(T("_Name"), p.Name, maxLength: 80);
            var description = new TextField(T("_Description (the voice and style, for example)"), p.Description);
            if (!_dialogs.ShowForm(T("Rename and describe {0}", p.Name), "", T("Save"), new FormField[] { name, description })) return;
            if (name.Value.Length == 0) return;
            if (await RunAsync(() => _api.EditPersonaAsync(p.Id, name.Value, description.Value, p.IsPublic)))
                Announcer.Announce(T("Saved {0}.", name.Value));
        }

        // ── A creator's playlists: like and save ────────────────────────────

        [RelayCommand]
        private async Task CreatorPlaylistsAsync(SunoClip? clip)
        {
            clip ??= SelectedClip;
            if (clip == null) return;
            if (string.IsNullOrWhiteSpace(clip.Handle))
            {
                Announcer.Announce(T("Suno didn't say who made this song."));
                return;
            }
            CreatorPage? page = null;
            if (!await RunAsync(async () => page = await _api.GetCreatorAsync(clip.Handle))) return;
            var lists = page!.Playlists ?? Array.Empty<Playlist>();
            if (lists.Count == 0)
            {
                Announcer.Announce(T("{0} has no public playlists.", page.Name));
                return;
            }
            _dialogs.ShowLines(T("Playlists of {0}. Enter for what you can do with one.", page.Name), lists.Select(l => l.SpokenText).ToList(),
                index => PlaylistActions(lists[index]));
        }

        private void PlaylistActions(Playlist list)
        {
            var actions = new (string Label, Func<Task> Run)[]
            {
                (T("Its songs"), async () =>
                {
                    IReadOnlyList<SunoClip> songs = Array.Empty<SunoClip>();
                    if (await RunAsync(async () => songs = await _api.GetPlaylistSongsAsync(list.Id)))
                        if (songs.Count == 0) Announcer.Announce(T("{0} is empty.", list.DisplayName));
                        else ShowSongList(list.DisplayName, songs);
                }),
                (T("Like it"), async () => { if (await RunAsync(() => _api.SetPlaylistLikedAsync(list.Id, true))) Announcer.Announce(T("Liked {0}.", list.DisplayName)); }),
                (T("Remove your like"), async () => { if (await RunAsync(() => _api.SetPlaylistLikedAsync(list.Id, false))) Announcer.Announce(T("Removed your like from {0}.", list.DisplayName)); }),
                (T("Save it to your library"), async () => { if (await RunAsync(() => _api.SetPlaylistSavedAsync(list.Id, true))) Announcer.Announce(T("Saved {0} to your library.", list.DisplayName)); }),
                (T("Remove it from your library"), async () => { if (await RunAsync(() => _api.SetPlaylistSavedAsync(list.Id, false))) Announcer.Announce(T("Removed {0} from your library.", list.DisplayName)); })
            };
            _dialogs.ShowLines(list.DisplayName, actions.Select(a => a.Label).ToList(), index => _ = actions[index].Run());
        }

        // ── Custom models ───────────────────────────────────────────────────

        [RelayCommand]
        private async Task CustomModelsAsync()
        {
            IReadOnlyList<PendingCustomModel> pending = Array.Empty<PendingCustomModel>();
            if (!await RunAsync(async () => pending = await _api.GetPendingCustomModelsAsync())) return;
            var ready = _account.CustomModels;
            var lines = new List<string> { T("Train a new custom model from a playlist...") };
            lines.AddRange(ready.Select(m => m.ToString()));
            lines.AddRange(pending.Select(m => T("{0}, still training", m.Name)));
            _dialogs.ShowLines(T("Custom models. Enter for what you can do with one."), lines, index =>
            {
                if (index == 0) _ = TrainCustomModelAsync();
                else if (index <= ready.Count) CustomModelActions(ready[index - 1]);
                else Announcer.Announce(T("Suno is still training it; it shows up in the model list when it's ready."));
            });
        }

        private void CustomModelActions(SunoModel model)
        {
            var actions = new (string Label, Func<Task> Run)[]
            {
                (T("Rename..."), async () =>
                {
                    var name = new TextField(T("_Name, up to 16 characters"), model.Name, maxLength: 16);
                    if (!_dialogs.ShowForm(T("Rename {0}", model.Name), "", T("Save"), new FormField[] { name }) || name.Value.Length == 0) return;
                    if (await RunAsync(() => _api.RenameCustomModelAsync(model.Id!, name.Value)))
                    {
                        Announcer.Announce(T("Saved {0}.", name.Value));
                        _ = RefreshAccountQuietlyAsync();
                    }
                }),
                (T("Archive it"), async () =>
                {
                    if (!_dialogs.Confirm(T("Archive the custom model \"{0}\"? It leaves your model list.", model.Name), T("Archive"))) return;
                    if (await RunAsync(() => _api.ArchiveCustomModelAsync(model.Id!)))
                    {
                        Announcer.Announce(T("Archived {0}.", model.Name));
                        _ = RefreshAccountQuietlyAsync();
                    }
                })
            };
            _dialogs.ShowLines(model.Name, actions.Select(a => a.Label).ToList(), index => _ = actions[index].Run());
        }

        private async Task TrainCustomModelAsync()
        {
            IReadOnlyList<Playlist> playlists = Array.Empty<Playlist>();
            IReadOnlyList<CustomModelBase> bases = Array.Empty<CustomModelBase>();
            if (!await RunAsync(async () =>
                {
                    playlists = await _api.GetPlaylistsAsync();
                    bases = await _api.GetCustomModelBasesAsync();
                })) return;
            if (playlists.Count == 0)
            {
                Announcer.Announce(T("Put 6 to 100 of your songs in a playlist first; the model learns from them."));
                return;
            }
            var playlist = new ChoiceField(T("_Playlist with the songs to learn from"), playlists.Cast<object>().ToList(), playlists[0]);
            var name = new TextField(T("_Name, up to 16 characters"), "", maxLength: 16);
            var fields = new List<FormField> { playlist, name };
            ChoiceField? baseModel = null;
            if (bases.Count > 0)
            {
                baseModel = new ChoiceField(T("_Base model"), bases.Cast<object>().ToList(), bases.FirstOrDefault(b => b.IsDefault) ?? bases[0]);
                fields.Add(baseModel);
            }
            if (!_dialogs.ShowForm(T("Train a custom model"),
                    T("Suno learns your sound from the finished songs you made in a playlist (6 to 100 of them), and the new model joins your model list when it's trained. Your plan has to include custom models."),
                    T("Train"), fields)) return;
            if (name.Value.Length == 0)
            {
                Announcer.Announce(T("Give the model a name."));
                return;
            }
            IReadOnlyList<SunoClip> songs = Array.Empty<SunoClip>();
            if (!await RunAsync(async () => songs = await _api.GetPlaylistSongsAsync(((Playlist)playlist.Selected!).Id))) return;
            var ids = songs.Where(s => s.IsComplete && s.IsTrashed != true)
                .Select(s => s.Id).Distinct().Take(100).ToList();
            if (ids.Count < 6)
            {
                Announcer.Announce(Plural(ids.Count, "That playlist has {0} finished song of yours; the model needs at least 6.",
                    "That playlist has {0} finished songs of yours; the model needs at least 6."));
                return;
            }
            if (await RunAsync(() => _api.CreateCustomModelAsync(ids, name.Value, (baseModel?.Selected as CustomModelBase)?.Key)))
                Announcer.Announce(T("Suno is training {0} on {1} songs. It joins your model list when it's ready.", name.Value, ids.Count));
        }

        // ── Lyrics mashup ───────────────────────────────────────────────────

        [RelayCommand]
        private void LyricsMashup()
        {
            var first = new TextField(T("_First lyrics"), "", multiline: true);
            var second = new TextField(T("_Second lyrics"), "", multiline: true);
            var result = new TextField(T("_Mashup (copy it from here)"), "", multiline: true);
            _dialogs.ShowToolForm(T("Lyrics mashup"), T("Suno writes one set of lyrics from two. Paste or type both, then Mash up; the result appears in the Mashup box."),
                T("Mash up"), new FormField[] { first, second, result }, async () =>
                {
                    if (first.Current.Trim().Length == 0 || second.Current.Trim().Length == 0)
                    {
                        Announcer.Announce(T("Put lyrics in both boxes first."));
                        return;
                    }
                    Announcer.Announce(T("Mashing up the lyrics."));
                    await RunAsync(async () =>
                    {
                        result.Current = await _api.MashupLyricsAsync(first.Current.Trim(), second.Current.Trim());
                        Announcer.Announce(T("The mashup is in the Mashup box."));
                    });
                });
        }
    }
}
