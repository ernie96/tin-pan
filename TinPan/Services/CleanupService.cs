using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Diagnostics;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Text;
using System.Threading;
using System.Threading.Tasks;
using NAudio.Wave;
using TinPan.Models;
using static TinPan.Localization.Loc;

namespace TinPan.Services
{
    public interface ICleanupService
    {
        /// <summary>
        /// Cleans up a downloaded song with sunscreen and saves "&lt;name&gt; (clean).wav" next to it.
        /// Removes the original afterwards unless options.KeepOriginal. Returns the cleaned file's path.
        /// Throws SunoApiException with a speakable message.
        /// </summary>
        Task<string> CleanAsync(string audioPath, CleanupConfig options, CancellationToken ct = default);
    }

    /// <summary>
    /// Runs sunscreen (github.com/abradburne/sunscreen, MIT), built into TinPan.exe and pinned to one
    /// version. sunscreen reads MP3 and WAV; other files (Suno's free M4A) are first decoded to WAV with
    /// Windows' own Media Foundation. It writes a 24-bit WAV per stage; only the last one is kept.
    /// </summary>
    public sealed class CleanupService : ICleanupService
    {
        public const string ExeName = "sunscreen.exe";
        public const string Version = "0.7.1";

        /// <summary>SHA-256 of sunscreen-v0.7.1-x86_64-pc-windows-msvc.zip's sunscreen.exe, from its GitHub release.</summary>
        public const string PinnedSha256 = "BFEF0C7CDD22EF3C0BDCC669A3F9E58E6BCCE130573BF028C92E7E2EE406B6E3";

        private const string Title = "song";
        private static readonly TimeSpan Limit = TimeSpan.FromMinutes(15);

        public static string CleanedPath(string audioPath) =>
            Path.Combine(Path.GetDirectoryName(audioPath)!, Path.GetFileNameWithoutExtension(audioPath) + " (clean).wav");

        public async Task<string> CleanAsync(string audioPath, CleanupConfig options, CancellationToken ct = default)
        {
            var name = Path.GetFileNameWithoutExtension(audioPath);
            if (!File.Exists(audioPath)) throw new SunoApiException(T("Couldn't find the downloaded file for {0}.", name));
            if (options.Match && options.MatchReference.Length > 0 && !File.Exists(options.MatchReference))
                throw new SunoApiException(T("The reference song for tone matching isn't there any more. Choose another in Settings, or clear it."));

            var exe = UnpackSunscreen();
            var work = Path.Combine(Path.GetTempPath(), "TinPanCleanup", Guid.NewGuid().ToString("N"));
            try
            {
                Directory.CreateDirectory(work);
                var input = await ReadableInputAsync(audioPath, work, ct);
                var outDir = Path.Combine(work, "out");
                await RunAsync(exe, Arguments(input, outDir, options), name, ct);

                var final = Directory.GetFiles(outDir, Title + " - Clean*.wav")
                    .OrderBy(f => Path.GetFileName(f).Count(c => c == '+'))
                    .LastOrDefault() ?? throw new SunoApiException(T("sunscreen didn't write a cleaned-up file for {0}.", name));
                var target = CleanedPath(audioPath);
                File.Move(final, target, overwrite: true);
                if (!options.KeepOriginal && !string.Equals(Path.GetFullPath(audioPath), Path.GetFullPath(target), StringComparison.OrdinalIgnoreCase))
                    File.Delete(audioPath);
                Log.Info($"Cleaned up with sunscreen {Version}: {Path.GetFileName(final)}");
                return target;
            }
            catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
            {
                throw new SunoApiException(T("Couldn't save the cleaned-up file. Check the download folder in Settings."), null, ex);
            }
            finally
            {
                try { if (Directory.Exists(work)) Directory.Delete(work, recursive: true); }
                catch (Exception ex) when (ex is IOException or UnauthorizedAccessException) { Log.Warn($"Couldn't remove a clean-up work folder: {ex.Message}"); }
            }
        }

        /// <summary>sunscreen's command line for these options. Numbers always use a dot.</summary>
        public static IReadOnlyList<string> Arguments(string input, string outDir, CleanupConfig o)
        {
            static string N(double v) => v.ToString("0.###", CultureInfo.InvariantCulture);
            var args = new List<string> { input, outDir, Title };
            if (!o.Debuzz) args.Add("--no-debuzz");
            if (!o.Gate) args.Add("--no-gate");
            if (o.Smooth) args.Add("--smooth=" + N(o.SmoothMs));
            if (o.Widen) args.Add("--widen=" + N(o.WidenAmount));
            if (o.Excite) args.Add("--excite=" + N(o.ExciteDb));
            if (o.Match) args.Add(string.IsNullOrWhiteSpace(o.MatchReference) ? "--match" : "--match=" + o.MatchReference);
            if (o.Master) args.Add("--master=" + N(o.MasterLufs));
            return args;
        }

        /// <summary>The built-in sunscreen, unpacked and checked against the pinned hash.</summary>
        private static string UnpackSunscreen()
        {
            if (EmbeddedFiles.Hash(ExeName) != PinnedSha256)
                throw new SunoApiException(T("This copy of Tin Pan has no clean-up tool built in."));
            try
            {
                return EmbeddedFiles.Extract(ExeName) ?? throw new SunoApiException(T("This copy of Tin Pan has no clean-up tool built in."));
            }
            catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
            {
                throw new SunoApiException(T("Couldn't unpack the clean-up tool."), null, ex);
            }
        }

        /// <summary>MP3 and WAV go straight in; anything else (M4A) is decoded to a WAV in the work folder first.</summary>
        private static async Task<string> ReadableInputAsync(string audioPath, string work, CancellationToken ct)
        {
            var ext = Path.GetExtension(audioPath).ToLowerInvariant();
            if (ext is ".mp3" or ".wav") return audioPath;
            var wav = Path.Combine(work, "input.wav");
            try
            {
                await Task.Run(() =>
                {
                    using var reader = new MediaFoundationReader(audioPath);
                    WaveFileWriter.CreateWaveFile(wav, reader);
                }, ct);
            }
            catch (Exception ex) when (ex is System.Runtime.InteropServices.COMException or InvalidOperationException or ArgumentException)
            {
                Log.Warn($"Couldn't decode {ext} for clean-up: {ex.Message}");
                throw new SunoApiException(T("Windows couldn't read this {0} file to clean it up.", ext.TrimStart('.').ToUpperInvariant()), null, ex);
            }
            return wav;
        }

        private static async Task RunAsync(string exe, IReadOnlyList<string> args, string name, CancellationToken ct)
        {
            var info = new ProcessStartInfo(exe)
            {
                UseShellExecute = false,
                CreateNoWindow = true,
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                StandardOutputEncoding = Encoding.UTF8,
                StandardErrorEncoding = Encoding.UTF8
            };
            foreach (var a in args) info.ArgumentList.Add(a);

            using var process = new Process { StartInfo = info };
            var output = new StringBuilder();
            process.OutputDataReceived += (_, e) => { if (e.Data != null) lock (output) output.AppendLine(e.Data); };
            process.ErrorDataReceived += (_, e) => { if (e.Data != null) lock (output) output.AppendLine(e.Data); };
            try
            {
                process.Start();
            }
            catch (Exception ex) when (ex is Win32Exception or InvalidOperationException)
            {
                throw new SunoApiException(T("Couldn't start the clean-up tool. Your antivirus may have blocked it."), null, ex);
            }
            process.BeginOutputReadLine();
            process.BeginErrorReadLine();

            using var limit = CancellationTokenSource.CreateLinkedTokenSource(ct);
            limit.CancelAfter(Limit);
            try
            {
                await process.WaitForExitAsync(limit.Token);
            }
            catch (OperationCanceledException)
            {
                try { process.Kill(entireProcessTree: true); } catch (InvalidOperationException) { }
                throw new SunoApiException(ct.IsCancellationRequested ? T("Clean-up cancelled.") : T("Cleaning up {0} took too long, so it was stopped.", name));
            }

            if (process.ExitCode != 0)
            {
                string tail;
                lock (output) tail = string.Join(" | ", output.ToString().Split('\n', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries).TakeLast(3));
                Log.Warn($"sunscreen exited with {process.ExitCode}: {tail}");
                throw new SunoApiException(T("sunscreen couldn't clean up {0}. Details are in the log.", name));
            }
        }
    }
}
