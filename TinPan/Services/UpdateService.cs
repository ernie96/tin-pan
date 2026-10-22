using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Net;
using System.Net.Http;
using System.Net.Http.Headers;
using System.Reflection;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;
using System.Threading;
using System.Threading.Tasks;

namespace TinPan.Services
{
    public sealed record UpdateInfo(Version Version, string ReleaseNotes, string DownloadUrl, string SignatureUrl, string PageUrl);

    public interface IUpdateService
    {
        Version CurrentVersion { get; }

        /// <summary>
        /// True for the one-file TinPan.exe, which can replace itself. The development build
        /// (TinPan.exe beside TinPan.dll) can't; updates there just open the release page.
        /// </summary>
        bool CanInstall { get; }

        /// <summary>The newer release, or null when up to date. Throws UpdateException.</summary>
        Task<UpdateInfo?> CheckAsync(CancellationToken ct = default);

        /// <summary>Downloads TinPan.exe and checks its signature. Returns the verified file. Throws UpdateException.</summary>
        Task<string> DownloadAsync(UpdateInfo update, IProgress<double>? progress, CancellationToken ct = default);

        /// <summary>
        /// Starts a hidden helper that waits for Tin Pan to exit, swaps in the new exe and starts it.
        /// The caller must then close the app. Throws UpdateException.
        /// </summary>
        void StartInstall(string verifiedFile);
    }

    public sealed class UpdateException : Exception
    {
        public UpdateException(string message, Exception? inner = null) : base(message, inner) { }
    }

    /// <summary>
    /// Updates from GitHub Releases, after trypsynth/ship-shape and TimePlayer: the latest release's
    /// version tag against ours, one asset (TinPan.exe) with an Ed25519 ".sig", and a PowerShell
    /// helper that replaces the exe after Tin Pan exits and starts it again.
    /// </summary>
    public sealed class UpdateService : IUpdateService
    {
        public const string Repository = "ernie96/tin-pan";

        /// <summary>The release signing public key. Its private half is the UPDATE_SIGNING_KEY secret.</summary>
        public const string PublicKey = "dAJzkGbji8/wUluGvr2CG/4IOW7SobcI5WCtrUPoWyw=";

        public const string AssetName = "TinPan.exe";

        private readonly HttpClient _http;

        public UpdateService()
        {
            // Its own client: the shared one has a 30 second limit, too short for a 60 MB download.
            _http = new HttpClient { Timeout = Timeout.InfiniteTimeSpan };
            _http.DefaultRequestHeaders.UserAgent.Add(new ProductInfoHeaderValue("TinPan", CurrentVersion.ToString(3)));
        }

        public Version CurrentVersion
        {
            get
            {
                var v = Assembly.GetExecutingAssembly().GetName().Version ?? new Version(0, 0, 0);
                return new Version(v.Major, v.Minor, Math.Max(0, v.Build));
            }
        }

        // The development build has TinPan.dll beside the exe; the one-file build carries it inside.
        public bool CanInstall => Environment.ProcessPath != null && !File.Exists(Path.Combine(AppContext.BaseDirectory, "TinPan.dll"));

        public static string Folder => Path.Combine(Path.GetTempPath(), "TinPanUpdate");

        public async Task<UpdateInfo?> CheckAsync(CancellationToken ct = default)
        {
            using var timeout = CancellationTokenSource.CreateLinkedTokenSource(ct);
            timeout.CancelAfter(TimeSpan.FromSeconds(15));
            try
            {
                using var request = new HttpRequestMessage(HttpMethod.Get, $"https://api.github.com/repos/{Repository}/releases/latest");
                request.Headers.Accept.ParseAdd("application/vnd.github+json");
                using var response = await _http.SendAsync(request, timeout.Token);
                if (response.StatusCode == HttpStatusCode.NotFound) return null; // no release yet
                if (!response.IsSuccessStatusCode)
                    throw new UpdateException($"GitHub answered {(int)response.StatusCode} when checking for updates.");

                var release = await JsonSerializer.DeserializeAsync<GitHubRelease>(await response.Content.ReadAsStreamAsync(timeout.Token), cancellationToken: timeout.Token);
                if (release == null || ParseVersion(release.TagName) is not { } latest)
                    throw new UpdateException("The latest release has no usable version number.");

                Log.Info($"Update check: running {CurrentVersion.ToString(3)}, latest release {latest.ToString(3)}");
                if (latest <= CurrentVersion) return null;
                return PickAssets(release, latest)
                       ?? throw new UpdateException($"Tin Pan {latest.ToString(3)} is out, but its signed download is missing. Get it from the release page.");
            }
            catch (OperationCanceledException) when (!ct.IsCancellationRequested)
            {
                throw new UpdateException("Checking for updates timed out.");
            }
            catch (Exception ex) when (ex is HttpRequestException or JsonException)
            {
                throw new UpdateException("Couldn't check for updates. Check your internet connection.", ex);
            }
        }

        internal static UpdateInfo? PickAssets(GitHubRelease release, Version version)
        {
            var assets = release.Assets ?? new List<GitHubAsset>();
            var file = assets.FirstOrDefault(a => a.Name.Equals(AssetName, StringComparison.OrdinalIgnoreCase));
            var signature = assets.FirstOrDefault(a => a.Name.Equals(AssetName + ".sig", StringComparison.OrdinalIgnoreCase));
            var page = release.HtmlUrl ?? $"https://github.com/{Repository}/releases/latest";
            return file == null || signature == null
                ? null
                : new UpdateInfo(version, release.Body ?? "", file.DownloadUrl, signature.DownloadUrl, page);
        }

        /// <summary>"v1.2.0", "1.2", "1.2.0-beta" → 1.2.0.</summary>
        internal static Version? ParseVersion(string? text)
        {
            if (string.IsNullOrWhiteSpace(text)) return null;
            var core = text.Trim().TrimStart('v', 'V');
            var cut = core.IndexOfAny(new[] { '-', '+', ' ' });
            if (cut > 0) core = core[..cut];
            return Version.TryParse(core, out var v) ? new Version(v.Major, v.Minor, Math.Max(0, v.Build)) : null;
        }

        public async Task<string> DownloadAsync(UpdateInfo update, IProgress<double>? progress, CancellationToken ct = default)
        {
            Directory.CreateDirectory(Folder);
            var target = Path.Combine(Folder, AssetName);
            try
            {
                var signature = await _http.GetStringAsync(update.SignatureUrl, ct);
                using (var response = await _http.GetAsync(update.DownloadUrl, HttpCompletionOption.ResponseHeadersRead, ct))
                {
                    response.EnsureSuccessStatusCode();
                    var total = response.Content.Headers.ContentLength ?? 0;
                    await using var source = await response.Content.ReadAsStreamAsync(ct);
                    await using var file = File.Create(target);
                    var buffer = new byte[81920];
                    long received = 0;
                    int read;
                    while ((read = await source.ReadAsync(buffer, ct)) > 0)
                    {
                        await file.WriteAsync(buffer.AsMemory(0, read), ct);
                        received += read;
                        if (total > 0) progress?.Report((double)received / total);
                    }
                }

                if (!UpdateSignature.Verify(PublicKey, target, AssetName, signature))
                {
                    TryDelete(target);
                    Log.Error($"Update {update.Version.ToString(3)} failed signature verification; discarded");
                    throw new UpdateException("The downloaded update isn't signed by Tin Pan's publisher, so it wasn't installed.");
                }
                Log.Info($"Update {update.Version.ToString(3)} downloaded and verified");
                return target;
            }
            catch (OperationCanceledException)
            {
                TryDelete(target);
                throw new UpdateException("The update download was cancelled.");
            }
            catch (Exception ex) when (ex is HttpRequestException or IOException or UnauthorizedAccessException)
            {
                TryDelete(target);
                throw new UpdateException("Couldn't download the update. Check your internet connection and try again.", ex);
            }
        }

        public void StartInstall(string verifiedFile)
        {
            var exe = Environment.ProcessPath ?? throw new UpdateException("Couldn't find where Tin Pan is running from.");
            var script = InstallScript(Environment.ProcessId, verifiedFile, exe);
            try
            {
                Process.Start(new ProcessStartInfo("powershell.exe")
                {
                    ArgumentList = { "-NoProfile", "-ExecutionPolicy", "Bypass", "-WindowStyle", "Hidden", "-Command", script },
                    CreateNoWindow = true,
                    UseShellExecute = false
                });
                Log.Info("Update helper started");
            }
            catch (Exception ex) when (ex is Win32Exception or InvalidOperationException)
            {
                throw new UpdateException("Couldn't start the update.", ex);
            }
        }

        /// <summary>
        /// A PowerShell single-quoted string. PowerShell also ends such strings at the curly quotes
        /// U+2018 to U+201B (common in names like O’Brien), so each is doubled as well.
        /// </summary>
        internal static string Quote(string value)
        {
            var escaped = new StringBuilder(value.Length + 8);
            foreach (var c in value)
            {
                escaped.Append(c);
                if (c is '\'' or '‘' or '’' or '‚' or '‛') escaped.Append(c);
            }
            return "'" + escaped + "'";
        }

        /// <summary>
        /// Wait for Tin Pan to exit, move the old exe aside, copy the new one in (retrying while the
        /// old one is still locked), put the old one back if that fails and leave a note saying so,
        /// then start Tin Pan. The old copy is removed at the next start.
        /// </summary>
        internal static string InstallScript(int pid, string newExe, string exe)
        {
            var old = exe + ".old";
            return
                $"Wait-Process -Id {pid} -Timeout 60 -ErrorAction SilentlyContinue; " +
                $"$ok = $false; " +
                $"for ($i = 0; $i -lt 15 -and -not $ok; $i++) {{ try {{ " +
                $"Move-Item -LiteralPath {Quote(exe)} -Destination {Quote(old)} -Force -ErrorAction Stop; " +
                $"try {{ Copy-Item -LiteralPath {Quote(newExe)} -Destination {Quote(exe)} -Force -ErrorAction Stop; $ok = $true }} " +
                $"catch {{ Move-Item -LiteralPath {Quote(old)} -Destination {Quote(exe)} -Force -ErrorAction SilentlyContinue; throw }} " +
                $"}} catch {{ Start-Sleep -Seconds 1 }} }}; " +
                $"if (-not $ok) {{ Set-Content -LiteralPath {Quote(FailureNotePath)} -Value 'failed' }}; " +
                $"Remove-Item -LiteralPath {Quote(newExe)} -Force -ErrorAction SilentlyContinue; " +
                $"Start-Process -FilePath {Quote(exe)}";
        }

        /// <summary>Written by the helper when the exe couldn't be replaced; read once at the next start.</summary>
        public static string FailureNotePath => Path.Combine(Folder, "install-failed.txt");

        /// <summary>
        /// At startup: removes the previous exe left by an update, and returns true once if the
        /// last update couldn't be installed.
        /// </summary>
        public static bool TidyAfterUpdate()
        {
            if (Environment.ProcessPath is { } exe) TryDelete(exe + ".old");
            if (!File.Exists(FailureNotePath)) return false;
            TryDelete(FailureNotePath);
            return true;
        }

        private static void TryDelete(string path)
        {
            try { if (File.Exists(path)) File.Delete(path); }
            catch (Exception ex) when (ex is IOException or UnauthorizedAccessException) { Log.Warn($"Couldn't remove {Path.GetFileName(path)}: {ex.Message}"); }
        }
    }

    internal sealed class GitHubRelease
    {
        [JsonPropertyName("tag_name")] public string? TagName { get; set; }
        [JsonPropertyName("body")] public string? Body { get; set; }
        [JsonPropertyName("html_url")] public string? HtmlUrl { get; set; }
        [JsonPropertyName("assets")] public List<GitHubAsset>? Assets { get; set; }
    }

    internal sealed class GitHubAsset
    {
        [JsonPropertyName("name")] public string Name { get; set; } = "";
        [JsonPropertyName("browser_download_url")] public string DownloadUrl { get; set; } = "";
    }
}
