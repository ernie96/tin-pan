using System;
using System.IO;
using System.Net.Http;
using System.Net.Http.Headers;
using System.Threading;
using System.Threading.Tasks;
using NAudio.MediaFoundation;
using NAudio.Wave;
using static TinPan.Localization.Loc;

namespace TinPan.Services
{
    public enum UploadStage { Preparing, Sending, Processing, Creating }

    public interface IUploadService
    {
        /// <summary>Length of an audio file Windows can read, or null if it can't.</summary>
        TimeSpan? Duration(string path);

        /// <summary>
        /// Uploads an audio file to the signed-in Suno account and makes a song from it.
        /// Returns the new song's clip id. Throws SunoApiException with a speakable message.
        /// </summary>
        Task<string> UploadAsync(string path, string title, IProgress<UploadStage>? stage, CancellationToken ct = default);
    }

    /// <summary>
    /// Uploads a song, following Nsomnia/chadvis-projectm-qt's SunoAudioUploadService for the
    /// transport and Suno's web app for the steps chadvis doesn't cover (see SunoApiService.Uploads).
    /// chadvis captured only M4A uploads, so anything else is first encoded to AAC M4A with Windows'
    /// own Media Foundation encoder.
    /// </summary>
    public sealed class UploadService : IUploadService
    {
        /// <summary>chadvis's bound on what it will push through a user-chosen file (not a Suno limit).</summary>
        public const long MaxBytes = 500L * 1024 * 1024;

        private static readonly TimeSpan ProcessingLimit = TimeSpan.FromMinutes(5);
        private const int StorageAttempts = 2;                                    // chadvis: one retry, fresh ticket

        private readonly ISunoApiService _api;
        private readonly HttpClient _storage;

        /// <summary>How often to ask whether Suno has processed the upload: every 4 seconds, as its web app does.</summary>
        internal TimeSpan PollEvery { get; init; } = TimeSpan.FromSeconds(4);

        public UploadService(ISunoApiService api) : this(api, new HttpClientHandler()) { }

        /// <summary>For tests: a storage handler that stands in for S3.</summary>
        internal UploadService(ISunoApiService api, HttpMessageHandler storage)
        {
            _api = api;
            _storage = new HttpClient(storage) { Timeout = Timeout.InfiniteTimeSpan };
        }

        public TimeSpan? Duration(string path)
        {
            try
            {
                using var reader = new MediaFoundationReader(path);
                return reader.TotalTime;
            }
            catch (Exception ex) when (ex is System.Runtime.InteropServices.COMException or InvalidOperationException or ArgumentException or IOException)
            {
                return null;
            }
        }

        public async Task<string> UploadAsync(string path, string title, IProgress<UploadStage>? stage, CancellationToken ct = default)
        {
            var info = new FileInfo(path);
            if (!info.Exists || info.Length == 0) throw new SunoApiException(T("That file is empty or missing."));
            if (info.Length > MaxBytes) throw new SunoApiException(T("That file is over 500 MB, too big to upload."));

            stage?.Report(UploadStage.Preparing);
            var work = Path.Combine(Path.GetTempPath(), "TinPanUpload", Guid.NewGuid().ToString("N"));
            try
            {
                var m4a = await AsM4aAsync(path, work, ct);
                var fileName = Path.GetFileNameWithoutExtension(path) + ".m4a";

                stage?.Report(UploadStage.Sending);
                var uploadId = await SendWithRetryAsync(m4a, fileName, ct);
                await _api.FinishAudioUploadAsync(uploadId, fileName, ct);

                stage?.Report(UploadStage.Processing);
                await WaitForProcessingAsync(uploadId, ct);

                stage?.Report(UploadStage.Creating);
                var clipId = await _api.CreateClipFromUploadAsync(uploadId, ct);
                await _api.NameUploadedSongAsync(clipId, title, ct);
                Log.Info($"Uploaded {fileName} as clip {clipId}");
                return clipId;
            }
            finally
            {
                try { if (Directory.Exists(work)) Directory.Delete(work, recursive: true); }
                catch (Exception ex) when (ex is IOException or UnauthorizedAccessException) { Log.Warn($"Couldn't remove an upload work folder: {ex.Message}"); }
            }
        }

        /// <summary>An M4A goes as it is; anything else Windows can read is encoded to AAC M4A first.</summary>
        internal static async Task<string> AsM4aAsync(string path, string work, CancellationToken ct)
        {
            if (Path.GetExtension(path).Equals(".m4a", StringComparison.OrdinalIgnoreCase)) return path;
            Directory.CreateDirectory(work);
            var target = Path.Combine(work, "upload.m4a");
            try
            {
                await Task.Run(() =>
                {
                    MediaFoundationApi.Startup();
                    using var reader = new MediaFoundationReader(path);
                    MediaFoundationEncoder.EncodeToAac(reader, target, 256000);
                }, ct);
            }
            catch (Exception ex) when (ex is System.Runtime.InteropServices.COMException or InvalidOperationException or ArgumentException)
            {
                Log.Warn($"Couldn't encode {Path.GetExtension(path)} to M4A: {ex.Message}");
                throw new SunoApiException(T("Windows couldn't turn this file into M4A for uploading. Try an MP3, WAV or M4A file."), null, ex);
            }
            return target;
        }

        /// <summary>
        /// Gets a ticket and POSTs the file to it. A dropped connection or a storage-side error gets one
        /// more try with a fresh ticket, since a signed ticket can't be replayed (chadvis T0035).
        /// </summary>
        private async Task<string> SendWithRetryAsync(string file, string fileName, CancellationToken ct)
        {
            for (var attempt = 1; ; attempt++)
            {
                var ticket = await _api.StartAudioUploadAsync(ct);
                var (ok, transient, status) = await PostToStorageAsync(ticket, file, fileName, ct);
                if (ok) return ticket.Id;
                Log.Warn($"Upload storage POST failed ({status}), attempt {attempt}");
                if (!transient || attempt >= StorageAttempts)
                    throw new SunoApiException(T("The upload didn't go through. Check your internet connection and try again."));
            }
        }

        private async Task<(bool Ok, bool Transient, int Status)> PostToStorageAsync(AudioUploadTicket ticket, string file, string fileName, CancellationToken ct)
        {
            if (!SunoApiService.IsUploadStorage(ticket.Url)) return (false, false, 0);
            using var form = new MultipartFormDataContent();
            // The signed fields first, then the file, as S3's POST policy expects (and as chadvis sends them).
            foreach (var (name, value) in ticket.Fields)
            {
                var part = new StringContent(value);
                part.Headers.ContentDisposition = new ContentDispositionHeaderValue("form-data") { Name = Quote(name) };
                form.Add(part);
            }
            await using var stream = File.OpenRead(file);
            var filePart = new StreamContent(stream);
            filePart.Headers.ContentType = MediaTypeHeaderValue.TryParse(ticket.Fields["Content-Type"], out var type) ? type : new MediaTypeHeaderValue("audio/mp4");
            filePart.Headers.ContentDisposition = new ContentDispositionHeaderValue("form-data") { Name = Quote("file"), FileName = Quote(fileName) };
            form.Add(filePart);

            try
            {
                using var request = new HttpRequestMessage(HttpMethod.Post, ticket.Url) { Content = form };
                using var response = await _storage.SendAsync(request, ct);
                var status = (int)response.StatusCode;
                return (response.IsSuccessStatusCode, status >= 500, status);
            }
            catch (HttpRequestException ex)
            {
                Log.Warn($"Upload storage POST couldn't connect: {ex.Message}");
                return (false, true, 0);
            }
        }

        private static string Quote(string value) => "\"" + value.Replace("\\", "\\\\").Replace("\"", "\\\"") + "\"";

        /// <summary>Asks every 4 seconds, for up to 5 minutes, until Suno says complete or error (as its web app does).</summary>
        private async Task WaitForProcessingAsync(string uploadId, CancellationToken ct)
        {
            var until = DateTime.UtcNow + ProcessingLimit;
            while (true)
            {
                var state = await _api.GetAudioUploadAsync(uploadId, ct);
                if (state.Status == "complete") return;
                if (state.Status == "error")
                    throw new SunoApiException(string.IsNullOrWhiteSpace(state.ErrorMessage)
                        ? T("Suno couldn't process the upload.")
                        : T("Suno couldn't process the upload: {0}", state.ErrorMessage.Trim()));
                if (DateTime.UtcNow >= until) throw new SunoApiException(T("Suno took too long processing the upload. It may still appear in your library later."));
                await Task.Delay(PollEvery, ct);
            }
        }
    }
}
