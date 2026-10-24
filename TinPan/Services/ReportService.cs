using System;
using System.IO;
using System.Net.Http;
using System.Reflection;
using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;
using System.Text.RegularExpressions;
using System.Threading;
using System.Threading.Tasks;
using static TinPan.Localization.Loc;

namespace TinPan.Services
{
    /// <summary>What Report a problem (Ctrl+Shift+B) or Suggest a feature (Ctrl+Shift+F) sends.</summary>
    public sealed record Report
    {
        public const string Problem = "bug";
        public const string Feature = "feature";

        [JsonPropertyName("kind")] public string Kind { get; init; } = Problem;
        [JsonPropertyName("description")] public required string Description { get; init; }
        [JsonPropertyName("contact")] public string? Contact { get; init; }
        [JsonPropertyName("version")] public string Version { get; init; } = Assembly.GetExecutingAssembly().GetName().Version?.ToString(3) ?? "?";
        [JsonPropertyName("language")] public string Language { get; init; } = Current;
        [JsonPropertyName("windows")] public string Windows { get; init; } = RuntimeInformation.OSDescription;
        [JsonPropertyName("log")] public string? Log { get; init; }
    }

    public interface IReportService
    {
        /// <summary>The end of Tin Pan's log, with the Windows user name taken out.</summary>
        string ReadRecentLog();

        /// <summary>Sends the report and returns its reference number. Throws SunoApiException with a message to read out.</summary>
        Task<int> SendAsync(Report report, CancellationToken ct = default);
    }

    /// <summary>
    /// Sends reports to Tin Pan's report relay (a small Cloudflare Worker, in report-relay/), which files each one
    /// as an issue in a private GitHub repository. Nobody needs an account, and logs aren't public.
    /// Made the same way as TimePlayer's.
    /// </summary>
    public sealed class ReportService : IReportService
    {
        /// <summary>Where reports go; see report-relay/README.md.</summary>
        public const string RelayUrl = "https://tinpan-reports.ernie96.workers.dev/report";

        public const int MaxDescriptionLength = 5000;
        public const int MaxContactLength = 200;
        public const int MaxLogCharacters = 200_000;

        private static readonly JsonSerializerOptions Json = new() { Encoder = System.Text.Encodings.Web.JavaScriptEncoder.UnsafeRelaxedJsonEscaping };

        private readonly HttpClient _http;

        public ReportService(HttpClient http) => _http = http;

        public string ReadRecentLog()
        {
            var text = new StringBuilder();
            foreach (var path in new[] { Services.Log.FilePath + ".old", Services.Log.FilePath })
            {
                try
                {
                    if (!File.Exists(path)) continue;
                    using var stream = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete);
                    using var reader = new StreamReader(stream);
                    text.Append(reader.ReadToEnd());
                }
                catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
                {
                    Services.Log.Warn($"Couldn't read the log for a report: {ex.Message}");
                }
            }
            var log = text.Length > MaxLogCharacters ? text.ToString(text.Length - MaxLogCharacters, MaxLogCharacters) : text.ToString();
            return Anonymize(log, Environment.UserName);
        }

        /// <summary>
        /// Paths hold the Windows user name, often a real name: C:\Users\Ana\... becomes C:\Users\&lt;user&gt;\....
        /// The name on its own is taken out as a whole word only, so a short name doesn't cut into other words.
        /// </summary>
        public static string Anonymize(string log, string? userName)
        {
            log = Regex.Replace(log, @"(?i)(\\Users\\)[^\\/:*?""<>|\r\n]+", "$1<user>");
            if (string.IsNullOrWhiteSpace(userName) || userName.Length < 3) return log;
            return Regex.Replace(log, $@"(?i)(?<!\w){Regex.Escape(userName)}(?!\w)", "<user>");
        }

        public async Task<int> SendAsync(Report report, CancellationToken ct = default)
        {
            if (string.IsNullOrWhiteSpace(report.Description))
                throw new SunoApiException(T("Please describe it first."));
            report = report with
            {
                Description = Clip(report.Description.Trim(), MaxDescriptionLength),
                Contact = string.IsNullOrWhiteSpace(report.Contact) ? null : Clip(report.Contact.Trim(), MaxContactLength),
                Log = report.Log is null ? null : report.Log.Length > MaxLogCharacters ? report.Log[^MaxLogCharacters..] : report.Log
            };
            try
            {
                using var timeout = CancellationTokenSource.CreateLinkedTokenSource(ct);
                timeout.CancelAfter(TimeSpan.FromSeconds(60));
                using var request = new HttpRequestMessage(HttpMethod.Post, RelayUrl)
                {
                    Content = new StringContent(JsonSerializer.Serialize(report, Json), Encoding.UTF8, "application/json")
                };
                request.Headers.Add("X-TinPan-Report", "1");
                using var response = await _http.SendAsync(request, timeout.Token);
                var body = await response.Content.ReadAsStringAsync(timeout.Token);
                if (response.IsSuccessStatusCode && ParseNumber(body) is { } number)
                {
                    Services.Log.Info($"Report sent as number {number}");
                    return number;
                }
                Services.Log.Warn($"The report relay answered {(int)response.StatusCode}: {Clip(body, 300)}");
                throw new SunoApiException((int)response.StatusCode == 429
                    ? T("Too many reports were sent from this connection just now. Wait a minute and try again.")
                    : T("The report service answered with an error ({0}).", (int)response.StatusCode));
            }
            catch (OperationCanceledException) when (ct.IsCancellationRequested)
            {
                throw;
            }
            catch (Exception ex) when (ex is HttpRequestException or OperationCanceledException)
            {
                Services.Log.Warn($"Couldn't send a report: {ex.Message}");
                throw new SunoApiException(T("Couldn't reach the report service. Check your internet connection and try again."), null, ex);
            }
        }

        /// <summary>The relay answers {"number": 12}.</summary>
        public static int? ParseNumber(string json)
        {
            try
            {
                using var doc = JsonDocument.Parse(json);
                return doc.RootElement.ValueKind == JsonValueKind.Object
                       && doc.RootElement.TryGetProperty("number", out var n) && n.TryGetInt32(out var number) ? number : null;
            }
            catch (JsonException)
            {
                return null;
            }
        }

        private static string Clip(string text, int max) => text.Length <= max ? text : text[..max];
    }
}
