using TinPan.Services;
using TinPan.Views;
using Xunit;

namespace TinPan.Tests
{
    public class PlayingCueTests
    {
        [Fact]
        public void The_playing_song_says_playing_only_when_asked_to()
        {
            Assert.Equal("Rain, 2:31, playing", PlayingCue.NameFor("Rain, 2:31", "a1", "a1", says: true));
            Assert.Equal("Rain, 2:31", PlayingCue.NameFor("Rain, 2:31", "a1", "a1", says: false));
            Assert.Equal("Rain, 2:31", PlayingCue.NameFor("Rain, 2:31", "a1", "b2", says: true));
            Assert.Equal("Rain, 2:31", PlayingCue.NameFor("Rain, 2:31", "a1", null, says: true));
        }

        [Fact]
        public void The_click_is_a_short_valid_wave_file()
        {
            using var wav = PlayingCue.MakeClick();
            var bytes = wav.ToArray();
            Assert.Equal("RIFF", System.Text.Encoding.ASCII.GetString(bytes, 0, 4));
            Assert.Equal("WAVE", System.Text.Encoding.ASCII.GetString(bytes, 8, 4));
            Assert.Equal(bytes.Length - 8, System.BitConverter.ToInt32(bytes, 4));
            Assert.Equal(bytes.Length - 44, System.BitConverter.ToInt32(bytes, 40));
        }
    }

    public class SafetyTests
    {
        [Theory]
        [InlineData("https://suno.com/song/abc", true)]
        [InlineData("https://auth.suno.com/v1/client", true)]
        [InlineData("http://suno.com/song/abc", false)]
        [InlineData("https://notsuno.com/", false)]
        [InlineData("https://suno.com.evil.example/", false)]
        [InlineData("https://evil.example/?suno.com", false)]
        public void Only_https_Suno_pages_count_as_Suno(string url, bool expected) =>
            Assert.Equal(expected, WebViewEnvironment.IsSuno(new System.Uri(url)));

        [Fact]
        public void One_oddly_typed_song_does_not_lose_the_page()
        {
            // An odd count is read as 0 and the song stays; a song that can't be read at all is the only one lost.
            var json = """{"clips":[{"id":"a1","title":"Good","status":"complete"},{"id":"b2","play_count":"lots"},{"id":"c3","metadata":"broken"}]}""";
            var page = SunoJson.ReadFeedPage(json);
            Assert.Equal(new[] { "a1", "b2" }, page.Clips.Select(c => c.Id));
            Assert.Equal(0, page.Clips[1].PlayCount);
        }

        [Fact]
        public void Replies_that_are_not_objects_read_as_empty()
        {
            Assert.Empty(SunoJson.ReadFeedPage("\"nope\"").Clips);
            Assert.Empty(SunoJson.FindClipsById("42"));
            Assert.Empty(SunoJson.ReadPlaylistSongs("[1,2]").Clips);
            Assert.Equal("", SunoJson.ReadPlaylist(System.Text.Json.JsonDocument.Parse("7").RootElement).Id);
        }

        [Fact]
        public void The_licenses_and_NVDA_helper_are_built_into_the_exe()
        {
            Assert.Contains("Jenni and Diana", EmbeddedFiles.ReadText("THIRD_PARTY_NOTICES.md"));
            Assert.Contains("GNU", EmbeddedFiles.ReadText("nvdaControllerClient-license.txt"));
            using var dll = typeof(EmbeddedFiles).Assembly.GetManifestResourceStream("TinPan." + EmbeddedFiles.NvdaDll);
            Assert.NotNull(dll);
            Assert.True(dll!.Length > 10_000);
        }

        [Fact]
        public void The_free_download_never_uses_Sunos_placeholder_address()
        {
            var encryptedOnly = SunoJson.ReadFeedPage(
                """{"clips":[{"id":"a1","audio_url":"https://studio-api-prod.suno.com/api/forbidden","media_urls":[{"url":"https://cdn1.suno.ai/a1.m4a","encoding":"m4a-opus"}]}]}""").Clips[0];
            Assert.Null(TinPan.ViewModels.LibraryPanelViewModel.DirectAudioUrl(encryptedOnly));

            var plain = SunoJson.ReadFeedPage(
                """{"clips":[{"id":"b2","audio_url":"https://studio-api-prod.suno.com/api/forbidden","media_urls":[{"url":"https://cdn1.suno.ai/b2.m4a","encoding":"m4a-opus"},{"url":"https://cdn1.suno.ai/b2-plain.m4a"}]}]}""").Clips[0];
            Assert.Equal("https://cdn1.suno.ai/b2-plain.m4a", TinPan.ViewModels.LibraryPanelViewModel.DirectAudioUrl(plain));

            var older = SunoJson.ReadFeedPage("""{"clips":[{"id":"c3","audio_url":"https://cdn1.suno.ai/c3.mp3"}]}""").Clips[0];
            Assert.Equal("https://cdn1.suno.ai/c3.mp3", TinPan.ViewModels.LibraryPanelViewModel.DirectAudioUrl(older));
        }

        [Fact]
        public void A_token_with_an_impossible_expiry_is_treated_as_unreadable()
        {
            var payload = System.Convert.ToBase64String(System.Text.Encoding.UTF8.GetBytes("{\"exp\":99999999999999}")).TrimEnd('=');
            Assert.Null(Jwt.ReadExpiry("x." + payload + ".y"));
        }
    }
}
