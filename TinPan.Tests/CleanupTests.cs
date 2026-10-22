using System;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Security.Cryptography;
using System.Threading.Tasks;
using TinPan.Models;
using TinPan.Services;
using Xunit;

namespace TinPan.Tests
{
    public class CleanupTests
    {
        [Fact]
        public void Defaults_are_sunscreens_recommended_chain_with_its_own_values()
        {
            var args = CleanupService.Arguments("in.mp3", "out", new CleanupConfig());
            Assert.Equal(new[] { "in.mp3", "out", "song", "--smooth=85", "--widen=1", "--excite=-3", "--match", "--master=-14" }, args);
        }

        [Fact]
        public void Every_step_can_be_turned_off()
        {
            var off = new CleanupConfig { Debuzz = false, Gate = false, Smooth = false, Widen = false, Excite = false, Match = false, Master = false };
            Assert.Equal(new[] { "in.wav", "out", "song", "--no-debuzz", "--no-gate" }, CleanupService.Arguments("in.wav", "out", off));
        }

        [Fact]
        public void Values_and_a_reference_song_are_passed_on_with_a_dot_in_any_language()
        {
            var previous = CultureInfo.CurrentCulture;
            try
            {
                CultureInfo.CurrentCulture = new CultureInfo("ro-RO"); // writes 0,5
                var o = new CleanupConfig { SmoothMs = 150, WidenAmount = 0.5, ExciteDb = -6, MatchReference = @"C:\Songs\O'Brien ref.wav", MasterLufs = -9 };
                var args = CleanupService.Arguments("in.wav", "out", o);
                Assert.Contains("--smooth=150", args);
                Assert.Contains("--widen=0.5", args);
                Assert.Contains("--excite=-6", args);
                Assert.Contains(@"--match=C:\Songs\O'Brien ref.wav", args);
                Assert.Contains("--master=-9", args);
            }
            finally
            {
                CultureInfo.CurrentCulture = previous;
            }
        }

        [Fact]
        public void The_cleaned_copy_sits_next_to_the_download()
        {
            Assert.Equal(Path.Combine(@"C:\Music\Suno", "Rain (1a2b3c4d) (clean).wav"),
                CleanupService.CleanedPath(Path.Combine(@"C:\Music\Suno", "Rain (1a2b3c4d).m4a")));
        }

        [Fact]
        public void The_built_in_sunscreen_is_the_pinned_release_build()
        {
            Assert.Equal(CleanupService.PinnedSha256, EmbeddedFiles.Hash(CleanupService.ExeName));
            Assert.Contains("MIT License", EmbeddedFiles.ReadText("sunscreen-license.txt"));
        }

        /// <summary>Runs the real sunscreen on a short generated song: a tone under hiss.</summary>
        [Fact]
        public async Task Cleans_a_real_file_and_keeps_or_replaces_the_original()
        {
            var dir = Path.Combine(Path.GetTempPath(), "TinPanCleanupTest-" + Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(dir);
            try
            {
                var song = Path.Combine(dir, "Test song.wav");
                WriteTestWav(song, seconds: 4);

                var cleaned = await new CleanupService().CleanAsync(song, new CleanupConfig());
                Assert.Equal(CleanupService.CleanedPath(song), cleaned);
                Assert.True(File.Exists(song));
                var header = File.ReadAllBytes(cleaned).Take(36).ToArray();
                Assert.Equal("WAVE", System.Text.Encoding.ASCII.GetString(header, 8, 4));
                Assert.Equal(24, BitConverter.ToInt16(header, 34));

                var replaced = await new CleanupService().CleanAsync(song, new CleanupConfig { KeepOriginal = false, Smooth = false, Widen = false, Excite = false, Match = false, Master = false });
                Assert.True(File.Exists(replaced));
                Assert.False(File.Exists(song));
            }
            finally
            {
                Directory.Delete(dir, recursive: true);
            }
        }

        [Fact]
        public async Task A_missing_reference_song_is_reported_before_running()
        {
            var dir = Path.Combine(Path.GetTempPath(), "TinPanCleanupTest-" + Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(dir);
            try
            {
                var song = Path.Combine(dir, "a.wav");
                WriteTestWav(song, seconds: 1);
                await Assert.ThrowsAsync<SunoApiException>(() =>
                    new CleanupService().CleanAsync(song, new CleanupConfig { MatchReference = Path.Combine(dir, "gone.wav") }));
            }
            finally
            {
                Directory.Delete(dir, recursive: true);
            }
        }

        private static void WriteTestWav(string path, int seconds)
        {
            const int rate = 48000;
            var frames = rate * seconds;
            using var w = new BinaryWriter(File.Create(path));
            w.Write("RIFF"u8); w.Write(36 + frames * 4); w.Write("WAVEfmt "u8); w.Write(16);
            w.Write((short)1); w.Write((short)2); w.Write(rate); w.Write(rate * 4); w.Write((short)4); w.Write((short)16);
            w.Write("data"u8); w.Write(frames * 4);
            var noise = new Random(7);
            for (var i = 0; i < frames; i++)
            {
                var tone = i % rate < rate * 2 / 5 ? 0.3 * Math.Sin(2 * Math.PI * 440 * i / rate) : 0;
                for (var c = 0; c < 2; c++) w.Write((short)((tone + 0.01 * (noise.NextDouble() - 0.5)) * short.MaxValue));
            }
        }
    }
}
