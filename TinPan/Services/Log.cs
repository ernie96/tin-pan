using System;
using System.IO;

namespace TinPan.Services
{
    /// <summary>
    /// Tiny append-only log in %LOCALAPPDATA%\Tin Pan\logs.
    /// Never pass tokens, cookies or file paths with the user name in them.
    /// </summary>
    public static class Log
    {
        private static readonly object Gate = new();
        private const long MaxBytes = 1024 * 1024;

        /// <summary>%LOCALAPPDATA%\Tin Pan\logs\tinpan.log, or TINPAN_LOG when set (the test run sets it).</summary>
        public static string FilePath { get; } = Environment.GetEnvironmentVariable("TINPAN_LOG") is { Length: > 0 } path
            ? path
            : Path.Combine(AppFolders.Local, "logs", "tinpan.log");

        static Log()
        {
            try
            {
                Directory.CreateDirectory(Path.GetDirectoryName(FilePath)!);
                var info = new FileInfo(FilePath);
                if (info.Exists && info.Length > MaxBytes)
                    File.Move(FilePath, FilePath + ".old", overwrite: true);
            }
            catch
            {
                // Logging must never take the app down.
            }
        }

        public static void Info(string message) => Write("INFO", message);
        public static void Warn(string message) => Write("WARN", message);
        public static void Error(string message, Exception? ex = null) =>
            Write("ERROR", ex == null ? message : $"{message}: {ex.GetType().Name}: {ex.Message}");

        private static void Write(string level, string message)
        {
            try
            {
                lock (Gate)
                {
                    // One line per entry, so text from Suno can't fake log lines.
                    File.AppendAllText(FilePath, $"{DateTime.Now:yyyy-MM-dd HH:mm:ss} {level} {message.ReplaceLineEndings(" ")}{Environment.NewLine}");
                }
            }
            catch
            {
            }
        }
    }
}
