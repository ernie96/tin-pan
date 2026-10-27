using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Net;
using System.Net.Http;
using System.Reflection;
using System.Threading;
using System.Threading.Tasks;
using TinPan.Services;
using Xunit;

namespace TinPan.Tests
{
    public class UploadTests
    {
        private const string Ticket = """{"id":"up1","url":"https://suno-uploads.s3.amazonaws.com/","is_file_uploaded":false,"fields":{"AWSAccessKeyId":"AK","Content-Type":"audio/mp4","key":"raw/up1.m4a","policy":"P","signature":"S"}}""";

        [Fact]
        public void A_ticket_is_read_with_its_signed_fields()
        {
            var t = SunoApiService.ParseUploadTicket(Ticket);
            Assert.Equal("up1", t.Id);
            Assert.Equal("audio/mp4", t.Fields["Content-Type"]);
            Assert.Equal(5, t.Fields.Count);
        }

        [Theory]
        [InlineData("https://evil.example/")]
        [InlineData("http://suno-uploads.s3.amazonaws.com/")]
        [InlineData("https://suno-uploads.s3.amazonaws.com.evil.example/")]
        [InlineData("https://user@suno-uploads.s3.amazonaws.com/")]
        [InlineData("https://suno-uploads.s3.amazonaws.com:8443/")]
        public void Only_Sunos_upload_storage_is_accepted(string url) =>
            Assert.Throws<SunoApiException>(() => SunoApiService.ParseUploadTicket(Ticket.Replace("https://suno-uploads.s3.amazonaws.com/", url)));

        [Fact]
        public void A_ticket_missing_a_signed_field_is_refused() =>
            Assert.Throws<SunoApiException>(() => SunoApiService.ParseUploadTicket(Ticket.Replace("\"policy\":\"P\",", "")));

        [Fact]
        public async Task Types_suno_takes_go_as_they_are_and_others_become_m4a()
        {
            var dir = TempDir();
            try
            {
                var wav = Path.Combine(dir, "song.wav");
                WriteTestWav(wav, 2);
                Assert.Equal(wav, await UploadService.AsUploadableAsync(wav, Path.Combine(dir, "work"), CancellationToken.None));
                var other = Path.Combine(dir, "song.wave");
                File.Copy(wav, other);
                var m4a = await UploadService.AsUploadableAsync(other, Path.Combine(dir, "work2"), CancellationToken.None);
                Assert.EndsWith(".m4a", m4a);
                Assert.Equal("ftyp", System.Text.Encoding.ASCII.GetString(File.ReadAllBytes(m4a), 4, 4));
            }
            finally
            {
                Directory.Delete(dir, recursive: true);
            }
        }

        [Fact]
        public async Task An_upload_runs_every_step_and_retries_storage_once_with_a_fresh_ticket()
        {
            var dir = TempDir();
            try
            {
                var wav = Path.Combine(dir, "My Song.wav");
                WriteTestWav(wav, 2);
                var api = FakeApi.Create();
                var storage = new FakeStorage(HttpStatusCode.ServiceUnavailable, HttpStatusCode.NoContent);
                var service = new UploadService(api, storage) { PollEvery = TimeSpan.FromMilliseconds(10) };
                var stages = new List<UploadStage>();

                var clipId = await service.UploadAsync(wav, "My Song", "La la", new SyncProgress(stages));

                Assert.Equal("clip9", clipId);
                Assert.Equal(new[] { UploadStage.Preparing, UploadStage.Sending, UploadStage.Processing, UploadStage.Creating }, stages);
                var fake = (FakeApi)(object)api;
                Assert.Equal(2, fake.Tickets);                          // a fresh ticket after the 503
                Assert.Equal("wav", fake.Extension);                    // sent as it is
                Assert.Equal(("up2", "My Song.wav"), fake.Finished);    // finished with the second ticket's id
                Assert.True(fake.Polls >= 2);                            // waited for "complete"
                Assert.Equal(("clip9", "My Song", "La la", "https://cdn/img.jpg"), fake.Named);
                // The signed fields go before the file, and the file goes as the ticket's Content-Type.
                var body = storage.LastBody!;
                Assert.True(body.IndexOf("name=\"policy\"", StringComparison.Ordinal) < body.IndexOf("name=\"file\"", StringComparison.Ordinal));
                Assert.Contains("Content-Type: audio/mp4", body);
                Assert.False(Directory.Exists(Path.Combine(Path.GetTempPath(), "TinPanUpload")) && Directory.GetDirectories(Path.Combine(Path.GetTempPath(), "TinPanUpload")).Length > 0);
            }
            finally
            {
                Directory.Delete(dir, recursive: true);
            }
        }

        [Fact]
        public async Task A_blank_title_takes_the_one_suno_read_from_the_file()
        {
            var dir = TempDir();
            try
            {
                var wav = Path.Combine(dir, "a.wav");
                WriteTestWav(wav, 1);
                var api = FakeApi.Create();
                var service = new UploadService(api, new FakeStorage(HttpStatusCode.NoContent)) { PollEvery = TimeSpan.FromMilliseconds(10) };
                await service.UploadAsync(wav, " ", null, null);
                Assert.Equal(("clip9", "Tagged Title", null, "https://cdn/img.jpg"), ((FakeApi)(object)api).Named);
            }
            finally
            {
                Directory.Delete(dir, recursive: true);
            }
        }

        [Fact]
        public async Task Sunos_processing_error_is_said_in_its_own_words()
        {
            var dir = TempDir();
            try
            {
                var wav = Path.Combine(dir, "a.wav");
                WriteTestWav(wav, 1);
                var api = FakeApi.Create(failProcessing: true);
                var service = new UploadService(api, new FakeStorage(HttpStatusCode.NoContent)) { PollEvery = TimeSpan.FromMilliseconds(10) };
                var ex = await Assert.ThrowsAsync<SunoApiException>(() => service.UploadAsync(wav, "a", null, null));
                Assert.Contains("copyrighted", ex.Message);
            }
            finally
            {
                Directory.Delete(dir, recursive: true);
            }
        }

        private static string TempDir()
        {
            var dir = Path.Combine(Path.GetTempPath(), "TinPanUploadTest-" + Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(dir);
            return dir;
        }

        private static void WriteTestWav(string path, int seconds)
        {
            const int rate = 44100;
            var frames = rate * seconds;
            using var w = new BinaryWriter(File.Create(path));
            w.Write("RIFF"u8); w.Write(36 + frames * 4); w.Write("WAVEfmt "u8); w.Write(16);
            w.Write((short)1); w.Write((short)2); w.Write(rate); w.Write(rate * 4); w.Write((short)4); w.Write((short)16);
            w.Write("data"u8); w.Write(frames * 4);
            for (var i = 0; i < frames; i++)
                for (var c = 0; c < 2; c++) w.Write((short)(0.3 * Math.Sin(2 * Math.PI * 440 * i / rate) * short.MaxValue));
        }

        private sealed class SyncProgress : IProgress<UploadStage>
        {
            private readonly List<UploadStage> _seen;
            public SyncProgress(List<UploadStage> seen) => _seen = seen;
            public void Report(UploadStage value) => _seen.Add(value);
        }

        /// <summary>Stands in for S3: answers each POST with the next status, and keeps the last body.</summary>
        private sealed class FakeStorage : HttpMessageHandler
        {
            private readonly Queue<HttpStatusCode> _answers;
            public FakeStorage(params HttpStatusCode[] answers) => _answers = new Queue<HttpStatusCode>(answers);
            public string? LastBody { get; private set; }

            protected override async Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken ct)
            {
                Assert.Null(request.Headers.Authorization); // never the Suno bearer
                LastBody = await request.Content!.ReadAsStringAsync(ct);
                return new HttpResponseMessage(_answers.Count > 0 ? _answers.Dequeue() : HttpStatusCode.NoContent);
            }
        }

        /// <summary>A Suno API that answers only the upload calls; anything else fails the test.</summary>
        public class FakeApi : DispatchProxy
        {
            public int Tickets, Polls;
            public (string, string)? Finished;
            public (string, string, string?, string?)? Named;
            public string? Extension;
            private bool _failProcessing;

            public static ISunoApiService Create(bool failProcessing = false)
            {
                var api = DispatchProxy.Create<ISunoApiService, FakeApi>();
                ((FakeApi)(object)api)._failProcessing = failProcessing;
                return api;
            }

            protected override object? Invoke(MethodInfo? method, object?[]? args)
            {
                switch (method!.Name)
                {
                    case nameof(ISunoApiService.StartAudioUploadAsync):
                        Tickets++;
                        Extension = (string)args![0]!;
                        return Task.FromResult(SunoApiService.ParseUploadTicket(Ticket.Replace("\"up1\"", $"\"up{Tickets}\"")));
                    case nameof(ISunoApiService.FinishAudioUploadAsync):
                        Finished = ((string)args![0]!, (string)args[1]!);
                        return Task.CompletedTask;
                    case nameof(ISunoApiService.GetAudioUploadAsync):
                        Polls++;
                        if (_failProcessing) return Task.FromResult(new AudioUploadState("error", null, "This sounds copyrighted."));
                        return Task.FromResult(new AudioUploadState(Polls < 2 ? "processing" : "complete", "Tagged Title", null, "https://cdn/img.jpg"));
                    case nameof(ISunoApiService.CreateClipFromUploadAsync):
                        return Task.FromResult("clip9");
                    case nameof(ISunoApiService.NameUploadedSongAsync):
                        Named = ((string)args![0]!, (string)args[1]!, (string?)args[2], (string?)args[3]);
                        return Task.CompletedTask;
                    default:
                        throw new InvalidOperationException("Unexpected call: " + method.Name);
                }
            }
        }
    }
}
