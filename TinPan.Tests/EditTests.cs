using System.Collections.Generic;
using System.Net;
using System.Net.Http;
using System.Text.Json;
using TinPan.Services;
using Xunit;

namespace TinPan.Tests
{
    /// <summary>Speed, reverse, crop and fade send what Suno's edit routes expect.</summary>
    public class EditTests
    {
        [Fact]
        public async Task Speed_sends_the_multiplier_and_pitch_choice_and_returns_the_new_song()
        {
            var suno = new Fake("{\"id\":\"new1\",\"title\":\"Song (0.94x)\",\"status\":\"submitted\"}");
            var clip = await Api(suno).ChangeSpeedAsync("c1", 0.9439, true, "Song (0.94x)");
            Assert.Equal("new1", clip.Id);
            var (path, body) = suno.Sent[^1];
            Assert.EndsWith("/api/clips/adjust-speed/", path);
            Assert.Equal("c1", body.GetProperty("clip_id").GetString());
            Assert.Equal(0.9439, body.GetProperty("speed_multiplier").GetDouble());
            Assert.True(body.GetProperty("keep_pitch").GetBoolean());
        }

        [Fact]
        public async Task Reverse_sends_the_song_and_title()
        {
            var suno = new Fake("{\"id\":\"new2\"}");
            Assert.Equal("new2", (await Api(suno).ReverseAsync("c1", "Song (Reversed)")).Id);
            var (path, body) = suno.Sent[^1];
            Assert.EndsWith("/api/clips/reverse-clip/", path);
            Assert.Equal("Song (Reversed)", body.GetProperty("title").GetString());
        }

        [Fact]
        public async Task Crop_follows_the_action_clip_as_an_unfinished_song()
        {
            var suno = new Fake("{\"action_clip_id\":\"crop-1\"}");
            var clip = await Api(suno).CropAsync("c1", 10, 70.5, true, "Song (Section removed)");
            Assert.Equal(("crop-1", "submitted", "Song (Section removed)"), (clip.Id, clip.Status, clip.Title));
            var (path, body) = suno.Sent[^1];
            Assert.EndsWith("/api/edit/crop/c1/", path);
            Assert.Equal(10, body.GetProperty("crop_start_s").GetDouble());
            Assert.Equal(70.5, body.GetProperty("crop_end_s").GetDouble());
            Assert.True(body.GetProperty("is_crop_remove").GetBoolean());
            Assert.Equal("song_actions", body.GetProperty("ui_surface").GetString());
        }

        [Fact]
        public async Task Fade_leaves_out_the_side_not_asked_for()
        {
            var suno = new Fake("{\"action_clip_id\":\"fade-1\"}");
            Assert.Equal("fade-1", (await Api(suno).FadeAsync("c1", null, 4, "Song (Fade out)")).Id);
            var (path, body) = suno.Sent[^1];
            Assert.EndsWith("/api/edit/fade/c1/", path);
            Assert.False(body.TryGetProperty("fade_in_time", out _));
            Assert.Equal(4, body.GetProperty("fade_out_time").GetDouble());
        }

        [Fact]
        public async Task A_blank_action_id_is_an_error_not_a_ghost_song()
        {
            await Assert.ThrowsAsync<SunoApiException>(() => Api(new Fake("{\"action_clip_id\":\"  \"}")).FadeAsync("c1", 2, null, "x"));
        }

        private static SunoApiService Api(Fake suno) => new(new HttpClient(suno), RateLimitTests.FakeAuth.Create(), null!);

        private sealed class Fake : HttpMessageHandler
        {
            private readonly string _reply;
            public Fake(string reply) => _reply = reply;
            public List<(string Path, JsonElement Body)> Sent { get; } = new();

            protected override async Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken ct)
            {
                var text = request.Content == null ? "{}" : await request.Content.ReadAsStringAsync(ct);
                Sent.Add((request.RequestUri!.AbsolutePath, JsonDocument.Parse(text).RootElement.Clone()));
                return new HttpResponseMessage(HttpStatusCode.OK) { Content = new StringContent(_reply) };
            }
        }
    }
}
