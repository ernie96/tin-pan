using TinPan.Models;
using TinPan.Services;
using Xunit;

namespace TinPan.Tests
{
    public class LiveLyricsTests
    {
        private static AlignedWord W(string word, double start) => new() { Word = word, Start = start, End = start + 0.4 };

        private static readonly AlignedWord[] Song =
        {
            W("[Verse 1]\n", 0), W("Rain ", 10), W("on ", 10.5), W("the ", 11), W("roof\n", 11.5),
            W("Light ", 14), W("in ", 14.5), W("the ", 15), W("hall\n", 15.5), W("[Chorus]\n", 18), W("Oh ", 19),
        };

        [Fact]
        public void Words_are_grouped_into_sung_lines()
        {
            var lines = LyricLine.From(Song);
            Assert.Equal(new[] { "[Verse 1]", "Rain on the roof", "Light in the hall", "[Chorus]", "Oh" }, lines.Select(l => l.Text));
            Assert.Equal(10, lines[1].Start);
        }

        [Fact]
        public void Each_line_is_said_once_as_it_starts_and_tags_are_not_said()
        {
            var cursor = new LyricsCursor(LyricLine.From(Song));
            Assert.Null(cursor.Advance(5));     // only the [Verse 1] tag so far
            Assert.Equal("Rain on the roof", cursor.Advance(9.9)); // a little early, so speech lands on time
            Assert.Null(cursor.Advance(11));
            Assert.Equal("Light in the hall", cursor.Advance(14.2));
            Assert.Null(cursor.Advance(18.5));  // [Chorus]
            Assert.Equal("Oh", cursor.Advance(19));
        }

        [Fact]
        public void After_a_jump_only_the_line_being_sung_is_said()
        {
            var cursor = new LyricsCursor(LyricLine.From(Song));
            Assert.Equal("Light in the hall", cursor.Advance(16));
            Assert.Equal("Rain on the roof", cursor.Advance(12)); // back
            Assert.Null(cursor.Advance(2));                       // back to before the singing
            Assert.Equal("Rain on the roof", cursor.Advance(10));
        }

        [Fact]
        public void Without_line_breaks_every_eight_words_make_a_line()
        {
            var words = Enumerable.Range(0, 10).Select(i => W($"w{i}", i)).ToArray();
            var lines = LyricLine.From(words);
            Assert.Equal(2, lines.Count);
            Assert.Equal(8, lines[1].Start);
        }

        [Fact]
        public void The_lrc_file_uses_the_same_lines()
        {
            var lrc = LrcWriter.Write(new SunoClip { Title = "Rain" }, Song);
            Assert.Contains("[00:10.00]Rain on the roof\n", lrc);
            Assert.Contains("[00:14.00]Light in the hall\n", lrc);
        }
    }
}
