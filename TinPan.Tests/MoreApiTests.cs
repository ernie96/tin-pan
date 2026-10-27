using System.Collections.Generic;
using System.Linq;
using System.Net;
using System.Net.Http;
using System.Text.Json;
using TinPan.Services;
using Xunit;

namespace TinPan.Tests
{
    /// <summary>The requests behind credits, stems, trash, personas, albums, mashups and the rest.</summary>
    public class MoreApiTests
    {
        [Fact]
        public async Task Not_interested_posts_and_undoing_deletes()
        {
            var suno = new Fake();
            await Api(suno).SetNotInterestedAsync("c1", true);
            await Api(suno).SetNotInterestedAsync("c1", false);
            Assert.Equal("POST", suno.Sent[0].Method);
            Assert.EndsWith("/api/recommend/feedback/song/c1", suno.Sent[0].Path);
            Assert.Equal("DELETE", suno.Sent[1].Method);
            Assert.Equal("not_interested", suno.Sent[0].Body.GetProperty("feedback_type").GetString());
        }

        [Fact]
        public async Task The_remix_list_toggle_sends_show_remix()
        {
            var suno = new Fake();
            await Api(suno).SetShowRemixesAsync("c1", false);
            Assert.EndsWith("/api/clips/c1/toggle_show_remixes", suno.Sent[0].Path);
            Assert.False(suno.Sent[0].Body.GetProperty("show_remix").GetBoolean());
        }

        [Fact]
        public async Task Credits_read_the_sources_and_the_remix_count()
        {
            var suno = new Fake
            {
                ["/attribution"] = """{"source_clips":[{"clip_id":"s1","title":"Old one","relationship":"cover_of","user":{"user_handle":"ana"}},{"title":"Gone","is_deleted":true}]}""",
                ["/remixes/count"] = """{"count":12,"is_capped":true}"""
            };
            var credits = await Api(suno).GetCreditsAsync("c1");
            Assert.Equal(2, credits.Sources.Count);
            Assert.Equal(("s1", "Old one", "@ana", "cover_of"), (credits.Sources[0].ClipId, credits.Sources[0].Title, credits.Sources[0].Creator, credits.Sources[0].Relationship));
            Assert.True(credits.Sources[1].Deleted);
            Assert.Equal((12, true), (credits.RemixCount, credits.RemixCountCapped));
        }

        [Fact]
        public async Task Existing_stems_read_every_page_then_the_songs()
        {
            var suno = new Fake
            {
                ["/stems/pages"] = """{"pages":2}""",
                ["/stems?page=0"] = """{"stems":[{"id":"v1"},{"id":"d1"}]}""",
                ["/stems?page=1"] = """{"stems":[{"id":"d1"},{"id":"b1"}]}""",
                ["/api/feed/"] = """{"clips":[{"id":"v1","audio_url":"x"},{"id":"d1","audio_url":"x"},{"id":"b1","audio_url":"x"}]}"""
            };
            var stems = await Api(suno).GetExistingStemsAsync("c1");
            Assert.Equal(new[] { "v1", "d1", "b1" }, stems.Select(s => s.Id));
        }

        [Fact]
        public async Task Restore_and_delete_forever_use_the_trash_routes()
        {
            var suno = new Fake();
            await Api(suno).RestoreFromTrashAsync("c1");
            await Api(suno).DeleteForeverAsync(new[] { "c1", "c2" });
            Assert.False(suno.Sent[0].Body.GetProperty("trash").GetBoolean());
            Assert.EndsWith("/api/clips/delete/", suno.Sent[1].Path);
            Assert.Equal(2, suno.Sent[1].Body.GetProperty("ids").GetArrayLength());
        }

        [Fact]
        public async Task An_album_is_made_from_a_playlist_as_a_draft()
        {
            var suno = new Fake { ["/api/playlist/create/"] = """{"metadata":{"id":"al1"}}""" };
            Assert.Equal("al1", await Api(suno).CreateAlbumAsync("Best", "", "ep", "pl1"));
            var body = suno.Sent[0].Body;
            Assert.Equal(("album", "ep", "draft", "pl1", "playlist_make_album"),
                (body.GetProperty("playlist_type").GetString(), body.GetProperty("album_type").GetString(), body.GetProperty("album_status").GetString(),
                 body.GetProperty("source_playlist_id").GetString(), body.GetProperty("creation_source").GetString()));
        }

        [Fact]
        public async Task Persona_lists_read_ownership_love_trash_and_voice()
        {
            var suno = new Fake
            {
                ["/get-loved-personas/"] = """{"personas":[{"id":"p1","name":"Ana voice","persona_type":"vox","is_loved":true,"is_owned":false,"user_handle":"ana"},{"id":"p2","name":"Mine","is_trashed":true,"root_clip_id":"r1","is_public":true}],"total_results":2}"""
            };
            var list = await Api(suno).GetPersonaListAsync(PersonaList.Loved);
            Assert.Equal(2, list.Count);
            Assert.True(list[0].IsVoice && list[0].IsLoved && !list[0].IsOwned);
            Assert.Equal("@ana", list[0].Creator);
            Assert.True(list[1].IsTrashed && list[1].IsOwned && list[1].IsPublic);
            Assert.Equal("r1", list[1].RootClipId);
        }

        [Fact]
        public async Task A_persona_is_made_from_a_song()
        {
            var suno = new Fake { ["/api/persona/create/"] = """{"id":"p9"}""" };
            Assert.Equal("p9", await Api(suno).CreatePersonaAsync("c1", "Me", "warm", false));
            Assert.Equal("c1", suno.Sent[0].Body.GetProperty("root_clip_id").GetString());
        }

        [Fact]
        public async Task A_mashup_sends_both_lyrics_and_waits_for_the_text()
        {
            var suno = new Fake
            {
                ["/lyrics-mashup"] = """{"mashup_id":"m1"}""",
                ["/api/generate/lyrics/m1"] = """{"id":"m1","status":"complete","text":"la la"}"""
            };
            Assert.Equal("la la", await Api(suno).MashupLyricsAsync("a", "b"));
            var body = suno.Sent[0].Body;
            Assert.Equal(("a", "b", "create_ui"), (body.GetProperty("lyrics_a").GetString(), body.GetProperty("lyrics_b").GetString(), body.GetProperty("source").GetString()));
            Assert.True(System.Guid.TryParse(body.GetProperty("create_session_token").GetString(), out _));
        }

        [Fact]
        public async Task Songs_without_stored_timed_lyrics_get_them_timed()
        {
            var suno = new Fake
            {
                ["/aligned_lyrics/v2/"] = """{"aligned_words":[]}""",
                ["/api/feed/"] = """{"clips":[{"id":"c1","audio_url":"x","metadata":{"prompt":"hello world"}}]}""",
                ["/aligned_lyrics/v3"] = """{"alignment":[{"word":"hello","start_s":1.0,"end_s":1.5},{"word":"world","start_s":1.6,"end_s":2.0}]}"""
            };
            var timed = await Api(suno).GetAlignedLyricsAsync("c1");
            Assert.Equal(new[] { "hello", "world" }, timed!.Words.Select(w => w.Word));
            Assert.Equal("hello world", suno.Sent.Last().Body.GetProperty("lyrics").GetString());
        }

        [Fact]
        public void Custom_models_are_told_apart_from_Sunos_own()
        {
            Assert.True(new Models.SunoModel { ExternalKey = "chirp-custom-abc", Id = "m1", Badges = new() { "custom" } }.IsCustom);
            Assert.False(new Models.SunoModel { ExternalKey = "chirp-fenix", Badges = new() { "custom" } }.IsCustom);
        }

        private static SunoApiService Api(Fake suno) => new(new HttpClient(suno), RateLimitTests.FakeAuth.Create(), null!) { PollIntervalForTests = System.TimeSpan.Zero };

        /// <summary>Answers by the first key the request's path and query contain; "{}" otherwise.</summary>
        private sealed class Fake : HttpMessageHandler
        {
            private readonly Dictionary<string, string> _replies = new();
            public string this[string pathPart] { set => _replies[pathPart] = value; }
            public List<(string Method, string Path, JsonElement Body)> Sent { get; } = new();

            protected override async Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken ct)
            {
                var text = request.Content == null ? "{}" : await request.Content.ReadAsStringAsync(ct);
                var path = request.RequestUri!.PathAndQuery;
                Sent.Add((request.Method.Method, request.RequestUri.AbsolutePath, JsonDocument.Parse(text.Length == 0 ? "{}" : text).RootElement.Clone()));
                var reply = _replies.Where(r => path.Contains(r.Key)).OrderByDescending(r => r.Key.Length).Select(r => r.Value).FirstOrDefault() ?? "{}";
                return new HttpResponseMessage(HttpStatusCode.OK) { Content = new StringContent(reply) };
            }
        }
    }
}
