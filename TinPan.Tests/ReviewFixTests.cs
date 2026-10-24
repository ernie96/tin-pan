using TinPan.Services;
using Xunit;

namespace TinPan.Tests
{
    /// <summary>Replies Suno sends now and then that used to drop songs or throw.</summary>
    public class ReviewFixTests
    {
        [Fact]
        public void A_song_with_null_counts_is_kept()
        {
            var page = SunoJson.ReadFeedPage("{\"clips\":[{\"id\":\"c1\",\"title\":\"Rain\",\"audio_url\":\"x\",\"play_count\":null,\"upvote_count\":\"7\",\"is_public\":null,\"is_liked\":1}],\"num_total\":null}");
            var clip = Assert.Single(page.Clips);
            Assert.Equal(0, clip.PlayCount);
            Assert.Equal(7, clip.UpvoteCount);
            Assert.False(clip.IsPublic);
            Assert.True(clip.IsLiked);
            Assert.Null(page.NumTotal);
        }

        [Fact]
        public void A_count_sent_as_text_or_null_reads_safely()
        {
            Assert.Equal(12, SunoJson.ReadInt("{\"n\":\"12\"}", "n"));
            Assert.Null(SunoJson.ReadInt("{\"n\":null}", "n"));
            Assert.Null(SunoJson.ReadInt("{\"n\":\"many\"}", "n"));
        }

        [Fact]
        public void A_playlist_with_a_null_total_still_reads()
        {
            var (clips, total) = SunoJson.ReadPlaylistSongs("{\"num_total_results\":null,\"playlist_clips\":[{\"clip\":{\"id\":\"c1\",\"audio_url\":\"x\"}}]}");
            Assert.Single(clips);
            Assert.Null(total);
        }

        [Theory]
        [InlineData("<html>502 Bad Gateway</html>")]
        [InlineData("[]")]
        [InlineData("")]
        public void A_rights_reply_that_isnt_one_gives_nothing_instead_of_throwing(string body) =>
            Assert.Null(SunoDrm.ParseRights(body, "c1"));

        [Fact]
        public void An_unreadable_token_has_no_expiry() =>
            Assert.Null(Jwt.ReadExpiry("aaa." + System.Convert.ToBase64String("[1,2]"u8.ToArray()) + ".sig"));
    }
}
