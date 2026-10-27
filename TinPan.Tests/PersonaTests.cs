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
            SunoApiService.PersonaTask(request, new SunoApiService.PersonaInfo("root1", type == "vox", false, false, 95.5));
            Assert.Equal(expected, request.Task);
            Assert.Equal("root1", request.ArtistClipId);
            Assert.Equal(0, request.ArtistStart);
            Assert.Equal(95.5, request.ArtistEnd);
        }

        [Theory]
        [InlineData("{\"persona_type\":\"VOX\",\"root_clip_id\":\"00000000-0000-0000-0000-000000000000\"}", true, null)]
        [InlineData("{\"persona_type\":null,\"is_vox_persona\":true,\"root_clip_id\":\"\"}", true, null)]
        [InlineData("{\"persona_type\":\"legacy\",\"root_clip_id\":\" r1 \"}", false, "r1")]
        public void Voice_personas_and_empty_source_songs_are_read_like_suno_does(string json, bool voice, string? root)
        {
            var info = SunoApiService.ReadPersona(json);
            Assert.Equal(voice, info.IsVoice);
            Assert.Equal(root, info.RootClipId);
        }

        [Fact]
        public void The_source_songs_length_comes_from_the_personas_own_clip()
        {
            var info = SunoApiService.ReadPersona("{\"root_clip_id\":\"r1\",\"clip\":{\"id\":\"r1\",\"metadata\":{\"duration\":120.25}}}");
            Assert.Equal(120.25, info.RootDuration);
            Assert.Null(SunoApiService.ReadPersona("{\"root_clip_id\":\"r1\",\"clip\":{\"id\":\"other\",\"metadata\":{\"duration\":9}}}").RootDuration);
        }

        [Theory]
        [InlineData(true, false)]
        [InlineData(false, true)]
        public void Trashed_or_hidden_personas_are_refused(bool trashed, bool hidden)
        {
            var request = new GenerateRequest("chirp", GenerateRequest.Modes.Custom) { PersonaId = "p1" };
            var ex = Assert.Throws<SunoApiException>(() => SunoApiService.PersonaTask(request, new SunoApiService.PersonaInfo("r1", false, trashed, hidden, 60)));
            Assert.Contains("trash or hidden", ex.Message);
        }

        [Fact]
        public void An_ordinary_persona_without_its_source_song_is_refused()
        {
            var request = new GenerateRequest("chirp", GenerateRequest.Modes.Custom) { Task = "cover", PersonaId = "p1" };
            Assert.Throws<SunoApiException>(() => SunoApiService.PersonaTask(request, new SunoApiService.PersonaInfo(null, false, false, false, null)));
            Assert.Equal("cover", request.Task);
        }

        [Fact]
        public void A_voice_persona_without_a_source_song_sends_only_the_start()
        {
            var request = new GenerateRequest("chirp", GenerateRequest.Modes.Custom) { PersonaId = "p1" };
            SunoApiService.PersonaTask(request, new SunoApiService.PersonaInfo(null, true, false, false, null), modelTakesVoice: true);
            Assert.Equal("vox", request.Task);
            Assert.Null(request.ArtistClipId);
            Assert.Equal(0, request.ArtistStart);
            Assert.Null(request.ArtistEnd);
        }

        [Fact]
        public void On_an_older_model_a_voice_persona_with_a_source_song_works_as_an_ordinary_one()
        {
            var request = new GenerateRequest("chirp", GenerateRequest.Modes.Custom) { Task = "cover", PersonaId = "p1" };
            SunoApiService.PersonaTask(request, new SunoApiService.PersonaInfo("r1", true, false, false, 30), modelTakesVoice: false);
            Assert.Equal("artist_cover", request.Task);

            var rootless = new GenerateRequest("chirp", GenerateRequest.Modes.Custom) { PersonaId = "p1" };
            Assert.Throws<SunoApiException>(() => SunoApiService.PersonaTask(rootless, new SunoApiService.PersonaInfo(null, true, false, false, null), modelTakesVoice: false));
        }

        [Fact]
        public void A_model_says_whether_it_takes_voice_personas()
        {
            Assert.True(new SunoModel { Capabilities = new() { "all" } }.TakesVoicePersonas);
            Assert.True(new SunoModel { Capabilities = new() { "generate", "vox" } }.TakesVoicePersonas);
            Assert.False(new SunoModel { Capabilities = new() { "generate" } }.TakesVoicePersonas);
            Assert.Null(new SunoModel().TakesVoicePersonas);
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
            Assert.Equal(0, sent.RootElement.GetProperty("artist_start_s").GetDouble());
            Assert.Equal(88.5, sent.RootElement.GetProperty("artist_end_s").GetDouble()); // read from the source song
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
                else if (request.Method == HttpMethod.Get) reply = "{\"clips\":[{\"id\":\"root1\",\"audio_url\":\"x\",\"metadata\":{\"duration\":88.5}}]}";
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
