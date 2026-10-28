using System.Net;
using System.Net.Http;
using System.Text.Json;
using TinPan.Models;
using TinPan.Services;
using Xunit;

namespace TinPan.Tests
{
    /// <summary>Which models may cover or inspire, why a song failed, Use as inspiration, and saving lyrics drafts.</summary>
    public class InspirationTests
    {
        private static SunoModel Model(string json) => JsonSerializer.Deserialize<SunoModel>(json)!;

        [Fact]
        public void A_model_covers_only_when_Suno_lists_cover_among_its_combinations()
        {
            var both = Model("{\"external_key\":\"chirp-fenix\",\"capabilities\":[\"all\"],\"allowed_condition_combinations\":[[\"cover\"],[\"persona\",\"cover\"],[\"playlist\"]]}");
            Assert.True(both.Supports("cover", "cover"));
            Assert.True(both.Supports("cover", "persona", "cover"));
            Assert.True(both.Supports("playlist_condition", "playlist"));
            Assert.False(both.Supports("cover", "vox", "cover"));

            var noCover = Model("{\"external_key\":\"chirp-x\",\"capabilities\":[\"generate\",\"extend\"],\"allowed_condition_combinations\":[[\"cover\"]]}");
            Assert.False(noCover.Supports("cover", "cover"));
        }

        [Theory]
        [InlineData("chirp-v3-0", false, false)]
        [InlineData("chirp-v3-5", true, false)]
        [InlineData("chirp-v4", true, false)]
        [InlineData("chirp-auk", true, true)]
        [InlineData("chirp-bluejay", true, true)]
        public void Without_combinations_the_older_table_decides(string key, bool cover, bool personaCover)
        {
            var m = new SunoModel { ExternalKey = key };
            Assert.Equal(cover, m.Supports("cover", "cover"));
            Assert.Equal(personaCover, m.Supports("cover", "persona", "cover"));
            Assert.Equal(key.Contains("bluejay"), m.Supports("playlist_condition", "playlist"));
        }

        [Theory]
        [InlineData("chirp-v3-5", "chirp-v3-5-tau")]
        [InlineData("chirp-v4", "chirp-v4-tau")]
        [InlineData("chirp-fenix", "chirp-fenix")]
        public void Covers_go_out_on_Sunos_cover_keys(string key, string sent) =>
            Assert.Equal(sent, new SunoModel { ExternalKey = key }.CoverKey);

        [Fact]
        public void Sliders_need_the_feature_unless_Suno_lists_none()
        {
            Assert.True(Model("{\"external_key\":\"a\",\"features\":[\"create_control_sliders\"]}").TakesSliders);
            Assert.False(Model("{\"external_key\":\"a\",\"features\":[\"reuse_styles_lyrics\"]}").TakesSliders);
            Assert.True(Model("{\"external_key\":\"a\",\"features\":[\"x\"],\"badges\":[\"custom\"]}").TakesSliders);
            Assert.Null(new SunoModel { ExternalKey = "a" }.TakesSliders);
        }

        [Theory]
        [InlineData("\"error_message\":\"Song description contains artist name\"", "Song description contains artist name")]
        [InlineData("\"error_type\":\"moderation_error\"", "Suno's content rules turned it down.")]
        [InlineData("\"error_type\":\"audio_too_short\"", "audio too short")]
        [InlineData("\"error_message\":\"\"", null)]
        public void A_failed_song_says_why(string meta, string? reason)
        {
            var clip = SunoJson.FindClips("{\"clips\":[{\"id\":\"c1\",\"audio_url\":\"\",\"status\":\"error\",\"metadata\":{" + meta + "}}]}")[0];
            Assert.True(clip.HasFailed);
            Assert.Equal(reason, clip.FailureReason);
        }

        [Fact]
        public void Use_as_inspiration_sends_Sunos_playlist_condition_body()
        {
            var request = new GenerateRequest("chirp-fenix", GenerateRequest.Modes.Custom)
            {
                Task = "playlist_condition",
                PlaylistId = "inspiration",
                PlaylistClipIds = new() { "c1" }
            };
            using var body = JsonDocument.Parse(JsonSerializer.Serialize(request));
            var r = body.RootElement;
            Assert.Equal("playlist_condition", r.GetProperty("task").GetString());
            Assert.Equal("inspiration", r.GetProperty("playlist_id").GetString());
            Assert.Equal("c1", r.GetProperty("playlist_clip_ids")[0].GetString());
            Assert.Equal("custom", r.GetProperty("metadata").GetProperty("create_mode").GetString());
            Assert.False(r.GetProperty("metadata").TryGetProperty("is_remix", out _));
        }

        [Fact]
        public void An_ordinary_song_sends_no_playlist_fields()
        {
            using var body = JsonDocument.Parse(JsonSerializer.Serialize(new GenerateRequest("chirp-fenix", GenerateRequest.Modes.Custom)));
            Assert.False(body.RootElement.TryGetProperty("playlist_id", out _));
            Assert.False(body.RootElement.TryGetProperty("playlist_clip_ids", out _));
        }

        [Fact]
        public async Task A_new_draft_is_created_with_its_title_and_returned()
        {
            var suno = new Recorder("{\"id\":\"d1\",\"title\":\"Rain\",\"lyrics\":\"\"}");
            var draft = await Api(suno).CreateLyricsDraftAsync("  Rain  ");
            Assert.Equal("d1", draft.Id);
            Assert.Equal("Rain", draft.Title);
            Assert.Equal(HttpMethod.Post, suno.Method);
            Assert.EndsWith("/api/lyrics-projects", suno.Url);
            Assert.Equal("{\"title\":\"Rain\"}", suno.Body);
        }

        [Fact]
        public async Task Saving_flushes_the_whole_lyrics()
        {
            var suno = new Recorder("{\"updated_at\":\"x\"}");
            await Api(suno).SaveLyricsDraftAsync("d1", "la la\nla");
            Assert.EndsWith("/api/lyrics-projects/d1/flush", suno.Url);
            using var body = JsonDocument.Parse(suno.Body!);
            Assert.Equal("la la\nla", body.RootElement.GetProperty("lyrics").GetString());
        }

        [Fact]
        public async Task Rename_patches_and_delete_deletes()
        {
            var suno = new Recorder("{\"id\":\"d1\",\"title\":\"Sun\"}");
            await Api(suno).RenameLyricsDraftAsync("d1", "Sun");
            Assert.Equal(HttpMethod.Patch, suno.Method);
            Assert.EndsWith("/api/lyrics-projects/d1", suno.Url);
            Assert.Equal("{\"title\":\"Sun\"}", suno.Body);

            var gone = new Recorder("", HttpStatusCode.NoContent);
            await Api(gone).DeleteLyricsDraftAsync("d1");
            Assert.Equal(HttpMethod.Delete, gone.Method);
            Assert.EndsWith("/api/lyrics-projects/d1", gone.Url);
        }

        [Fact]
        public void Draft_titles_are_cut_at_200_characters_without_splitting_one()
        {
            Assert.Equal(200, SunoApiService.DraftTitle(new string('a', 250)).Length);
            var emoji = string.Concat(Enumerable.Repeat("🎵", 210));
            var cut = SunoApiService.DraftTitle(emoji);
            Assert.Equal(200, cut.EnumerateRunes().Count());
            Assert.False(char.IsHighSurrogate(cut[^1]));
        }

        [Fact]
        public void A_draft_reads_its_lyrics_field()
        {
            Assert.Equal("words", SunoJson.ReadLyricsDraftText("{\"id\":\"d1\",\"title\":\"A much longer title than the words\",\"lyrics\":\"words\"}"));
        }

        private static SunoApiService Api(Recorder suno) => new(new HttpClient(suno), RateLimitTests.FakeAuth.Create(), null!);

        private sealed class Recorder : HttpMessageHandler
        {
            private readonly string _reply;
            private readonly HttpStatusCode _status;
            public Recorder(string reply, HttpStatusCode status = HttpStatusCode.OK) { _reply = reply; _status = status; }
            public string? Url { get; private set; }
            public string? Body { get; private set; }
            public HttpMethod? Method { get; private set; }

            protected override async Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken ct)
            {
                Url = request.RequestUri!.ToString();
                Method = request.Method;
                Body = request.Content == null ? null : await request.Content.ReadAsStringAsync(ct);
                return new HttpResponseMessage(_status) { Content = new StringContent(_reply) };
            }
        }
    }
}
