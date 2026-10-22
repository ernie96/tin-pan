using System;
using System.Collections.ObjectModel;
using System.Linq;
using System.Threading.Tasks;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using TinPan.Models;
using TinPan.Services;
using TinPan.Views;

namespace TinPan.ViewModels
{
    /// <summary>Your Suno playlists: open one, play through it, add, remove, move and rename.</summary>
    public partial class PlaylistsViewModel : ViewModelBase, IPlaybackQueue
    {
        private readonly ISunoApiService _api;
        private readonly IAuthService _auth;
        private readonly IAudioService _audio;
        private readonly IDialogService _dialogs;
        private SunoClip? _current;
        private int _songsVersion;

        public ObservableCollection<Playlist> Playlists { get; } = new();
        public ObservableCollection<SunoClip> Songs { get; } = new();

        [ObservableProperty] private Playlist? selectedPlaylist;
        [ObservableProperty] private SunoClip? selectedSong;

        public PlaylistsViewModel(ISunoApiService api, IAuthService auth, IAudioService audio, IDialogService dialogs, IAnnouncer announcer)
            : base(announcer)
        {
            _api = api;
            _auth = auth;
            _audio = audio;
            _dialogs = dialogs;
            _audio.TrackEnded += OnTrackEnded;
        }

        public void Clear()
        {
            _songsVersion++;
            _current = null;
            Playlists.Clear();
            Songs.Clear();
        }

        [RelayCommand]
        private Task RefreshAsync() => LoadAsync(background: false);

        public async Task LoadAsync(bool background)
        {
            if (!_auth.IsAuthenticated) return;
            await RunAsync(async () =>
            {
                var keep = SelectedPlaylist?.Id;
                var list = await _api.GetPlaylistsAsync();
                Playlists.Clear();
                foreach (var p in list) Playlists.Add(p);
                SelectedPlaylist = Playlists.FirstOrDefault(p => p.Id == keep) ?? Playlists.FirstOrDefault();
                if (!background) Announcer.Announce(list.Count == 0 ? "No playlists." : $"{list.Count} playlists.");
            }, background);
        }

        partial void OnSelectedPlaylistChanged(Playlist? value) => _ = LoadSongsAsync(value, announce: false);

        [RelayCommand]
        private Task OpenAsync() => LoadSongsAsync(SelectedPlaylist, announce: true);

        private async Task LoadSongsAsync(Playlist? playlist, bool announce)
        {
            var version = ++_songsVersion;
            Songs.Clear();
            if (playlist == null) return;
            if (!announce)
            {
                // Arrowing past playlists shouldn't fetch every one; load the one the user stops on.
                await Task.Delay(400);
                if (version != _songsVersion) return;
            }
            await RunAsync(async () =>
            {
                var songs = await _api.GetPlaylistSongsAsync(playlist.Id);
                if (version != _songsVersion) return;
                foreach (var s in songs) Songs.Add(s);
                SelectedSong = Songs.FirstOrDefault();
                playlist.SongCount = songs.Count;
                if (announce) Announcer.Announce(songs.Count == 0 ? $"{playlist.DisplayName} is empty." : $"{playlist.DisplayName}, {songs.Count} songs.");
            }, background: !announce);
        }

        // ── Managing playlists ──────────────────────────────────────────────

        [RelayCommand]
        private async Task NewAsync()
        {
            var name = new TextField("_Name", "", maxLength: 100);
            if (!_dialogs.ShowForm("New playlist", "", "Create", new FormField[] { name }) || name.Value.Length == 0) return;
            await RunAsync(async () =>
            {
                var created = await _api.CreatePlaylistAsync(name.Value);
                created.SongCount ??= 0;
                Playlists.Insert(0, created);
                SelectedPlaylist = created;
                Announcer.Announce($"Made playlist {created.DisplayName}.");
            });
        }

        [RelayCommand]
        private async Task RenameAsync()
        {
            var p = SelectedPlaylist;
            if (p == null) return;
            var name = new TextField("_Name", p.Name, maxLength: 100);
            var description = new TextField("_Description", p.Description, multiline: true);
            if (!_dialogs.ShowForm($"Rename {p.DisplayName}", "", "Save", new FormField[] { name, description }) || name.Value.Length == 0) return;
            if (await RunAsync(() => _api.RenamePlaylistAsync(p.Id, name.Value, description.Value)))
            {
                p.Name = name.Value;
                p.Description = description.Value;
                Announcer.Announce("Saved.");
            }
        }

        [RelayCommand]
        private async Task DeleteAsync()
        {
            var p = SelectedPlaylist;
            if (p == null || !_dialogs.Confirm($"Move the playlist \"{p.DisplayName}\" to the trash? The songs in it stay in your library.", "Delete playlist")) return;
            if (await RunAsync(() => _api.TrashPlaylistAsync(p.Id)))
            {
                var index = Playlists.IndexOf(p);
                Playlists.Remove(p);
                SelectedPlaylist = Playlists.Count == 0 ? null : Playlists[Math.Clamp(index, 0, Playlists.Count - 1)];
                Announcer.Announce($"Moved {p.DisplayName} to trash.");
            }
        }

        // ── Songs in the playlist ───────────────────────────────────────────

        [RelayCommand]
        private Task PlayAsync() => StartAsync(SelectedSong);

        private async Task StartAsync(SunoClip? clip)
        {
            if (clip == null) return;
            if (!clip.CanPlay)
            {
                Announcer.Announce(clip.HasFailed ? "That song failed to generate." : "That song isn't ready yet.");
                return;
            }
            _current = clip;
            if (await RunAsync(() => _audio.PlayAsync(clip.Id, clip.DisplayTitle, this)))
                Announcer.Announce($"Playing: {clip.DisplayTitle}.");
        }

        void IPlaybackQueue.PlayPause() => PlayPause();

        [RelayCommand]
        private void PlayPause()
        {
            if (_audio.IsPlaying) { _audio.Pause(); Announcer.Announce("Paused."); }
            else if (_audio.IsPaused) { _audio.Resume(); Announcer.Announce("Playing."); }
            else _ = StartAsync(SelectedSong);
        }

        private async void OnTrackEnded(object? sender, TrackEndedEventArgs e)
        {
            // Carry on through the playlist only when the playlist started this song.
            if (!ReferenceEquals(e.Owner, this) || _current?.Id != e.ClipId) return;
            if (!await StepAsync(+1))
            {
                _current = null;
                Announcer.Announce("End of playlist.", interrupt: false);
            }
        }

        public async Task NextAsync()
        {
            if (!await StepAsync(+1)) Announcer.Announce("End of playlist.");
        }

        public async Task PreviousAsync()
        {
            if (!await StepAsync(-1)) Announcer.Announce("First song.");
        }

        /// <summary>Plays the next or previous playable song after the current one. False when there is none.</summary>
        private async Task<bool> StepAsync(int direction)
        {
            var index = _current == null ? -1 : Songs.IndexOf(_current);
            if (index < 0) return false;
            var candidates = direction > 0 ? Songs.Skip(index + 1) : Songs.Take(index).Reverse();
            var next = candidates.FirstOrDefault(c => c.CanPlay);
            if (next == null) return false;
            SelectedSong = next;
            await StartAsync(next);
            return true;
        }

        [RelayCommand]
        private async Task RemoveSongAsync()
        {
            var p = SelectedPlaylist;
            var song = SelectedSong;
            if (p == null || song == null) return;
            if (await RunAsync(() => _api.RemoveFromPlaylistAsync(p.Id, new[] { song.Id })))
            {
                var index = Songs.IndexOf(song);
                Songs.Remove(song);
                p.SongCount = Songs.Count;
                if (Songs.Count > 0) SelectedSong = Songs[Math.Clamp(index, 0, Songs.Count - 1)];
                Announcer.Announce($"Removed {song.DisplayTitle} from {p.DisplayName}.");
            }
        }

        [RelayCommand] private Task MoveUpAsync() => MoveAsync(-1);
        [RelayCommand] private Task MoveDownAsync() => MoveAsync(+1);

        private async Task MoveAsync(int delta)
        {
            var p = SelectedPlaylist;
            var song = SelectedSong;
            if (p == null || song == null) return;
            var from = Songs.IndexOf(song);
            var to = from + delta;
            if (from < 0 || to < 0 || to >= Songs.Count)
            {
                Announcer.Announce(delta < 0 ? "Already first." : "Already last.");
                return;
            }
            if (await RunAsync(() => _api.MoveInPlaylistAsync(p.Id, song.Id, to)))
            {
                Songs.Move(from, to);
                SelectedSong = song;
                Announcer.Announce($"{to + 1} of {Songs.Count}.");
            }
        }
    }
}
