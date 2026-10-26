using System.Reflection;
using TinPan.Models;
using TinPan.Services;
using TinPan.ViewModels;
using Xunit;

namespace TinPan.Tests
{
    public class SongListQueueTests
    {
        private static readonly SunoClip[] Songs =
        {
            new() { Id = "a", Title = "A", Status = "complete" },
            new() { Id = "b", Title = "B", Status = "error" }, // can't play: skipped
            new() { Id = "c", Title = "C", Status = "complete" },
        };

        [Fact]
        public async Task Next_steps_through_the_windows_songs_skipping_ones_that_cant_play()
        {
            var (queue, audio) = Make();
            queue.Show(Songs);
            await queue.PlayAsync(Songs[0]);
            await queue.NextAsync();
            Assert.Equal(new[] { "a", "c" }, audio.Played);
        }

        [Fact]
        public async Task The_end_of_a_song_plays_the_next_one()
        {
            var (queue, audio) = Make();
            queue.Show(Songs);
            await queue.PlayAsync(Songs[0]);
            audio.End("a", queue);
            await Task.Delay(50);
            Assert.Equal(new[] { "a", "c" }, audio.Played);
        }

        [Fact]
        public async Task Space_pauses_whats_playing_instead_of_starting_over()
        {
            var (queue, audio) = Make();
            queue.Show(Songs);
            await queue.PlayAsync(Songs[0]);
            audio.Playing = true;
            await queue.PlayPauseAsync(Songs[2]);
            Assert.True(audio.PausedCalled);
            Assert.Equal(new[] { "a" }, audio.Played);
        }

        private static (SongListQueue, FakeAudio) Make()
        {
            var proxy = DispatchProxy.Create<IAudioService, FakeAudio>();
            var audio = (FakeAudio)(object)proxy;
            var queue = new SongListQueue(proxy, DispatchProxy.Create<IAnnouncer, Quiet>(), async work => { await work(); return true; });
            return (queue, audio);
        }

        public class FakeAudio : DispatchProxy
        {
            public List<string> Played { get; } = new();
            public bool Playing { get; set; }
            public bool PausedCalled { get; private set; }
            private EventHandler<TrackEndedEventArgs>? _ended;

            public void End(string id, IPlaybackQueue owner) => _ended?.Invoke(this, new TrackEndedEventArgs(id, owner));

            protected override object? Invoke(MethodInfo? method, object?[]? args)
            {
                switch (method!.Name)
                {
                    case "add_TrackEnded": _ended += (EventHandler<TrackEndedEventArgs>)args![0]!; return null;
                    case nameof(IAudioService.PlayAsync): Played.Add((string)args![0]!); return Task.CompletedTask;
                    case "get_" + nameof(IAudioService.IsPlaying): return Playing;
                    case "get_" + nameof(IAudioService.IsPaused): return false;
                    case nameof(IAudioService.Pause): PausedCalled = true; return null;
                    default: throw new InvalidOperationException("Unexpected call: " + method.Name);
                }
            }
        }

        public class Quiet : DispatchProxy
        {
            protected override object? Invoke(MethodInfo? method, object?[]? args) => null;
        }
    }
}
