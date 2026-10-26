using System.Net;
using System.Net.Http;
using System.Text.Json;
using TinPan.Models;
using TinPan.Services;
using Xunit;

namespace TinPan.Tests
{
    /// <summary>A persona only works with its task and source song, as Suno's Create page sends them.</summary>
    public class PersonaTests
    {
        [Theory]
        [InlineData(null, null, "artist_consistency")]
        [InlineData(null, "vox", "vox")]
        [InlineData("cover", null, "artist_cover")]
        [InlineData("cover", "vox", "vox_cover")]
        [InlineData("extend", null, "artist_extend")]
        [InlineData("extend", "vox", "vox_extend")]
        public void Each_kind_of_song_gets_its_persona_task(string? task, string? type, string expected)
        {
            var request = new GenerateRequest("chirp", GenerateRequest.Modes.Custom) { Task = task, PersonaId = "p1" };
            SunoApiService.PersonaTask(request, "root1", type);
            Assert.Equal(expected, request.Task);
            Assert.Equal("root1", request.ArtistClipId);
        }

        [Fact]
        public void An_ordinary_persona_without_its_source_song_is_sent_as_before()
        {
            var request = new GenerateRequest("chirp", GenerateRequest.Modes.Custom) { Task = "cover", PersonaId = "p1" };
            SunoApiService.PersonaTask(request, null, null);
            Assert.Equal("cover", request.Task);
            Assert.Null(request.ArtistClipId);
        }

        [Fact]
        public async Task Making_a_song_with_a_persona_sends_its_task_and_source_song()
        {
            var suno = new Fake();
            var request = new GenerateRequest("chirp", GenerateRequest.Modes.Custom) { PersonaId = "p1", Prompt = "la la", Title = "T" };
            await new SunoApiService(new HttpClient(suno), RateLimitTests.FakeAuth.Create(), null!).GenerateAsync(request);
            using var sent = JsonDocument.Parse(suno.GenerateBody!);
            Assert.Equal("artist_consistency", sent.RootElement.GetProperty("task").GetString());
            Assert.Equal("root1", sent.RootElement.GetProperty("artist_clip_id").GetString());
            Assert.Equal("p1", sent.RootElement.GetProperty("persona_id").GetString());
        }

        [Fact]
        public void Song_information_says_when_a_persona_was_used()
        {
            var clip = SunoJson.FindClips("{\"clips\":[{\"id\":\"c1\",\"audio_url\":\"x\",\"metadata\":{\"task\":\"artist_consistency\",\"persona_id\":\"p1\"}}]}")[0];
            Assert.Contains("Made with a persona", clip.InfoLines);
        }

        private sealed class Fake : HttpMessageHandler
        {
            public string? GenerateBody { get; private set; }

            protected override async Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken ct)
            {
                var path = request.RequestUri!.AbsolutePath;
                string reply;
                if (path.Contains("/persona/get-persona/")) reply = "{\"id\":\"p1\",\"root_clip_id\":\"root1\",\"persona_type\":null}";
                else if (path.EndsWith("/c/check")) reply = "{\"required\":false}";
                else
                {
                    GenerateBody = await request.Content!.ReadAsStringAsync(ct);
                    reply = "{\"clips\":[{\"id\":\"new1\",\"audio_url\":\"\"}]}";
                }
                return new HttpResponseMessage(HttpStatusCode.OK) { Content = new StringContent(reply) };
            }
        }
    }
}
