using System.Net;
using System.Net.Http;
using System.Text.Json;
using TinPan.Models;
using TinPan.Services;
using Xunit;

namespace TinPan.Tests
{
    /// <summary>Like and dislike, Explore's nested shelves, playlist results, 100 a page, free allowances.</summary>
    public class FiveTests
    {
        private const string Song = "{\"id\":\"c1\",\"title\":\"Rain\",\"audio_url\":\"x\"}";

        [Fact]
        public async Task Like_sends_Sunos_body_and_reads_the_letter_back()
        {
            var suno = new Recorder("{\"play_count\":3,\"skip_count\":0,\"flagged\":false,\"reaction_type\":\"L\",\"clip\":\"c1\",\"updated_at\":\"x\"}");
            var state = await Api(suno).SetReactionAsync("c1", "LIKE");
            Assert.Equal("L", state);
            Assert.EndsWith("/api/gen/c1/update_reaction_type/", suno.Url);
            using var body = JsonDocument.Parse(suno.Body!);
            Assert.Equal("LIKE", body.RootElement.GetProperty("reaction").GetString());
            Assert.Equal(JsonValueKind.Object, body.RootElement.GetProperty("recommendation_metadata").ValueKind);
        }

        [Fact]
        public async Task Clearing_sends_null_and_the_reply_has_no_letter()
        {
            var suno = new Recorder("{\"play_count\":3,\"skip_count\":0,\"flagged\":false,\"clip\":\"c1\",\"updated_at\":\"x\"}");
            Assert.Null(await Api(suno).SetReactionAsync("c1", null));
            using var body = JsonDocument.Parse(suno.Body!);
            Assert.Equal(JsonValueKind.Null, body.RootElement.GetProperty("reaction").ValueKind);
        }

        [Theory]
        [InlineData("\"reaction\":{\"reaction_type\":\"D\"}", true)]
        [InlineData("\"reaction\":\"D\"", true)]
        [InlineData("\"reaction\":{\"reaction_type\":\"L\"}", false)]
        [InlineData("\"reaction\":null", false)]
        public void A_disliked_song_says_so(string reaction, bool disliked)
        {
            var clip = SunoJson.FindClips("{\"clips\":[{\"id\":\"c1\",\"audio_url\":\"x\"," + reaction + "}]}")[0];
            Assert.Equal(disliked, clip.IsDisliked);
            Assert.Equal(disliked, clip.SpokenText.EndsWith(", disliked"));
        }

        [Fact]
        public void Explore_keeps_nested_shelves_under_their_own_names()
        {
            var json = "{\"feeds\":[" +
                       "{\"feed_id\":\"shortcuts\",\"feed_title\":\"Shortcuts\",\"items\":[{\"content_type\":\"shortcut\",\"content_item\":{\"entity_type\":\"shortcut_schema\",\"name\":\"Rap\",\"image_url\":\"i\"}}]}," +
                       "{\"feed_id\":\"home\",\"feed_title\":\"For you\",\"items\":[" +
                       "  {\"content_type\":\"generic_feed\",\"content_item\":{\"feed_id\":\"staff\",\"feed_title\":\"Staff Picks\",\"items\":[{\"content_type\":\"song\",\"content_item\":" + Song + "}]}}," +
                       "  {\"content_type\":\"song\",\"content_item\":{\"id\":\"c2\",\"title\":\"Sun\",\"audio_url\":\"y\"}}]}" +
                       "],\"next_cursor\":\"n\"}";
            var (sections, next) = SunoJson.ReadExplore(json);
            Assert.Equal(new[] { "For you", "Staff Picks" }, sections.Select(s => s.Title));
            Assert.Equal("c2", Assert.Single(sections[0].Songs).Id);
            Assert.Equal("c1", Assert.Single(sections[1].Songs).Id);
            Assert.Equal("n", next);
        }

        [Theory]
        [InlineData("{\"successes\":[],\"failures\":[\"c1\"]}", true)]
        [InlineData("{\"successes\":0,\"failures\":2}", true)]
        [InlineData("{\"successes\":[\"c1\"],\"failures\":[]}", false)]
        [InlineData("{\"successes\":[\"c1\"],\"failures\":[\"c2\"]}", false)]
        [InlineData("", false)]
        [InlineData("{}", false)]
        public void A_playlist_change_fails_only_when_nothing_went_in(string reply, bool failed) =>
            Assert.Equal(failed, SunoJson.PlaylistChangeFailed(reply));

        [Fact]
        public async Task Adding_to_a_playlist_that_Suno_refuses_is_an_error()
        {
            var suno = new Recorder("{\"successes\":[],\"failures\":[\"c1\"]}");
            await Assert.ThrowsAsync<SunoApiException>(() => Api(suno).AddToPlaylistAsync("p1", new[] { "c1" }));
        }

        [Fact]
        public async Task Deleting_a_playlist_sends_only_its_id()
        {
            var suno = new Recorder("", HttpStatusCode.NoContent);
            await Api(suno).TrashPlaylistAsync("p1");
            Assert.Equal("{\"playlist_id\":\"p1\"}", suno.Body);
        }

        [Fact]
        public async Task The_library_asks_for_100_songs_a_page()
        {
            var suno = new Recorder("{\"clips\":[],\"next_cursor\":null}");
            await Api(suno).GetLibraryPageAsync(new LibraryQuery());
            using var body = JsonDocument.Parse(suno.Body!);
            Assert.Equal(100, body.RootElement.GetProperty("limit").GetInt32());
        }

        [Fact]
        public void The_free_plan_hears_its_covers_and_remasters_left()
        {
            var info = JsonSerializer.Deserialize<BillingInfo>("{\"total_credits_left\":50,\"is_active\":true,\"plan\":{\"plan_key\":\"free\",\"name\":\"Free\"},\"free_cover_clips_remaining\":3,\"free_remasters_remaining\":1}")!;
            Assert.Equal(3, info.FreeCoversLeft);
            Assert.Equal(1, info.FreeRemastersLeft);
            Assert.False(info.IsPaid);
            var text = BillingText.Subscription(info);
            Assert.Contains("3 free covers left.", text);
            Assert.Contains("1 free remaster left.", text);
        }

        private static SunoApiService Api(Recorder suno) => new(new HttpClient(suno), RateLimitTests.FakeAuth.Create(), null!);

        private sealed class Recorder : HttpMessageHandler
        {
            private readonly string _reply;
            private readonly HttpStatusCode _status;
            public Recorder(string reply, HttpStatusCode status = HttpStatusCode.OK) { _reply = reply; _status = status; }
            public string? Url { get; private set; }
            public string? Body { get; private set; }

            protected override async Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken ct)
            {
                Url = request.RequestUri!.ToString();
                Body = request.Content == null ? null : await request.Content.ReadAsStringAsync(ct);
                return new HttpResponseMessage(_status) { Content = new StringContent(_reply) };
            }
        }
    }
}
