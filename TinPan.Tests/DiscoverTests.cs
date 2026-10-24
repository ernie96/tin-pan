using System.Net.Http;
using TinPan.Models;
using TinPan.Services;
using TinPan.ViewModels;
using Xunit;

namespace TinPan.Tests
{
    public class DiscoverTests
    {
        private const string Clip = "{\"id\":\"c1\",\"title\":\"Rain\",\"audio_url\":\"https://cdn1.suno.ai/c1.mp3\",\"handle\":\"ana\",\"display_name\":\"Ana\"}";

        [Fact]
        public void A_creator_page_reads_who_they_are_and_their_songs()
        {
            var json = "{\"handle\":\"ana\",\"display_name\":\"Ana\",\"profile_description\":\"I sing.\",\"num_total_clips\":42,"
                       + "\"stats\":{\"followers_count\":7,\"following_count\":3,\"play_count__sum\":1200},\"clips\":[" + Clip + "],\"playlists\":[]}";
            var page = SunoJson.ReadCreatorPage(json, "ana");
            Assert.Equal("Ana (@ana)", page.Name);
            Assert.Equal(42, page.TotalSongs);
            Assert.Equal(7, page.Followers);
            Assert.Equal(1200, page.Plays);
            Assert.Equal("c1", Assert.Single(page.Songs).Id);
            Assert.Contains("I sing.", page.InfoLines);
        }

        [Theory]
        [InlineData("ana", "ana")]
        [InlineData("@ana", "ana")]
        [InlineData("https://suno.com/@ana?sh=x", "ana")]
        [InlineData("  suno.com/@ana/ ", "ana")]
        public void A_creator_can_be_found_by_handle_or_link(string typed, string handle) =>
            Assert.Equal(handle, LibraryPanelViewModel.CreatorHandle(typed));

        [Fact]
        public void Explore_keeps_each_shelf_with_its_songs_and_skips_empty_ones()
        {
            var json = "{\"feeds\":[{\"id\":\"top\",\"title\":\"Top songs\",\"items\":[{\"clip\":" + Clip + "}]},"
                       + "{\"id\":\"empty\",\"title\":\"Nothing\",\"items\":[]}],\"next_cursor\":\"n2\"}";
            var (sections, next) = SunoJson.ReadExplore(json);
            var section = Assert.Single(sections);
            Assert.Equal("Top songs", section.Title);
            Assert.Equal("c1", Assert.Single(section.Songs).Id);
            Assert.Equal("n2", next);
        }

        [Fact]
        public void Contests_read_their_title_end_entries_and_song()
        {
            var json = "{\"contests\":[{\"id\":\"k\",\"title\":\"Summer remix\",\"description\":\"Remix it.\",\"end_time\":\"2030-01-02T00:00:00Z\","
                       + "\"submission_count\":12,\"base_clip_id\":\"c1\"},{\"id\":\"blank\"}]}";
            var contest = Assert.Single(SunoJson.ReadContests(json));
            Assert.Equal("Summer remix", contest.Title);
            Assert.Equal(12, contest.Entries);
            Assert.Equal("c1", contest.BaseClipId);
            Assert.NotNull(contest.Ends);
        }

        [Fact]
        public void Style_ideas_come_as_text_or_objects_without_repeats()
        {
            var json = "{\"prompts\":[\"dreamy synthpop\",{\"prompt\":\"lofi jazz\"},\"Dreamy synthpop\"],\"tags\":[{\"name\":\"punk\"}]}";
            Assert.Equal(new[] { "dreamy synthpop", "lofi jazz", "punk" }, SunoJson.ReadStyleSuggestions(json));
        }

        [Fact]
        public void Lyrics_drafts_list_and_their_words_are_read()
        {
            var (drafts, next) = SunoJson.ReadLyricsDrafts("{\"projects\":[{\"id\":\"p1\",\"title\":\"Rain\",\"updated_at\":\"2026-10-01T10:00:00Z\"}],\"next_cursor\":null}");
            Assert.Equal("p1", Assert.Single(drafts).Id);
            Assert.Null(next);
            Assert.Equal("Rain on the roof\nLight in the hall",
                SunoJson.ReadLyricsDraftText("{\"id\":\"p1\",\"title\":\"Rain\",\"current_version\":{\"lyrics\":\"Rain on the roof\\nLight in the hall\"}}"));
            Assert.Null(SunoJson.ReadLyricsDraftText("{\"id\":\"p1\",\"title\":\"Rain\"}"));
        }

        [Fact]
        public void The_original_song_comes_from_the_songs_own_details_first()
        {
            var cover = SunoJson.FindClips("{\"clips\":[{\"id\":\"c2\",\"audio_url\":\"x\",\"metadata\":{\"cover_clip_id\":\"c1\"}}]}")[0];
            Assert.Equal("c1", cover.Metadata!.OriginalId);
            var extension = SunoJson.FindClips("{\"clips\":[{\"id\":\"c3\",\"audio_url\":\"x\",\"metadata\":{\"history\":[{\"id\":\"c1\",\"continue_at\":30}]}}]}")[0];
            Assert.Equal("c1", extension.Metadata!.OriginalId);
            Assert.Equal("c1", SunoJson.ReadParentId("{\"parent\":{\"id\":\"c1\"}}", "c3"));
            Assert.Null(SunoJson.ReadParentId("{}", "c3"));
        }

        [Fact]
        public void An_odd_history_never_loses_the_song()
        {
            var clips = SunoJson.FindClips("{\"clips\":[{\"id\":\"c4\",\"audio_url\":\"x\",\"metadata\":{\"history\":\"c1\",\"concat_history\":[\"c9\"]}}]}");
            Assert.Equal("c9", Assert.Single(clips).Metadata!.OriginalId);
        }

        [Fact]
        public void Other_peoples_songs_say_who_made_them()
        {
            var clip = SunoJson.FindClips("{\"clips\":[" + Clip + "]}")[0];
            Assert.EndsWith(", by Ana", clip.SpokenWithCreator);
            Assert.Contains("By Ana (@ana)", clip.InfoLines);
        }

        [Fact]
        public void Unread_notifications_say_new()
        {
            var list = SunoJson.ReadNotifications("{\"notifications\":[{\"title\":\"Like\",\"message\":\"Ana liked Rain\",\"is_read\":false},{\"title\":\"Old\",\"is_read\":true}]}");
            Assert.Equal("New: Like: Ana liked Rain", list[0].ToString());
            Assert.Equal("Old", list[1].ToString());
        }
    }

    public class ReportTests
    {
        [Fact]
        public void The_windows_user_name_is_taken_out_of_the_log()
        {
            var log = @"Saved C:\Users\Ana Maria\Music\Suno\x.mp3; ana said hi; banana stays";
            Assert.Equal(@"Saved C:\Users\<user>\Music\Suno\x.mp3; <user> said hi; banana stays", ReportService.Anonymize(log, "Ana"));
        }

        [Theory]
        [InlineData("{\"number\": 12}", 12)]
        [InlineData("{\"error\": \"x\"}", null)]
        [InlineData("<html>", null)]
        public void The_relay_answer_gives_the_reference_number(string body, int? number) =>
            Assert.Equal(number, ReportService.ParseNumber(body));

        [Fact]
        public async Task An_empty_report_is_never_sent()
        {
            var service = new ReportService(new HttpClient(new NoNetwork()));
            await Assert.ThrowsAsync<SunoApiException>(() => service.SendAsync(new Report { Description = "  " }));
        }

        private sealed class NoNetwork : HttpMessageHandler
        {
            protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken ct) =>
                throw new InvalidOperationException("No report should be sent.");
        }
    }
}
