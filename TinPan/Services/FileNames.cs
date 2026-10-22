using System;
using System.IO;
using System.Linq;
using System.Text;

namespace TinPan.Services
{
    public static class FileNames
    {
        private static readonly string[] Reserved =
        {
            "CON", "PRN", "AUX", "NUL",
            "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9",
            "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9"
        };

        /// <summary>
        /// "Title (1a2b3c4d).mp3". The short id keeps Suno's two same-titled takes from overwriting each other.
        /// </summary>
        public static string ForSong(string? title, string id, string extension)
        {
            var name = Clean(title);
            var shortId = id.Length > 8 ? id[..8] : id;
            return $"{name} ({Clean(shortId)}).{extension.TrimStart('.')}";
        }

        private static readonly string[] KnownExtensions = { "mp3", "m4a", "mp4", "wav", "ogg", "flac", "jpg", "jpeg", "png", "webp" };

        /// <summary>The file type in a download address ("m4a" for .../x.m4a?sig=...), or the fallback.</summary>
        public static string ExtensionOf(string? url, string fallback)
        {
            if (string.IsNullOrEmpty(url) || !Uri.TryCreate(url, UriKind.Absolute, out var uri)) return fallback;
            var ext = Path.GetExtension(uri.AbsolutePath).TrimStart('.').ToLowerInvariant();
            return KnownExtensions.Contains(ext) ? ext : fallback;
        }

        public static string Clean(string? text)
        {
            var invalid = Path.GetInvalidFileNameChars();
            var sb = new StringBuilder();
            foreach (var c in (text ?? "").Trim())
                sb.Append(invalid.Contains(c) || char.IsControl(c) ? '_' : c);

            // Windows drops trailing dots and spaces, and refuses device names.
            var name = sb.ToString().TrimEnd('.', ' ');
            if (name.Length > 120) name = name[..120].TrimEnd('.', ' ');
            if (name.Length == 0) name = "Untitled";
            var stem = name.Split('.')[0];
            if (Reserved.Contains(stem, StringComparer.OrdinalIgnoreCase)) name = "_" + name;
            return name;
        }
    }
}
