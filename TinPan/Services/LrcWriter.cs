using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using System.Text;
using TinPan.Models;

namespace TinPan.Services
{
    /// <summary>Turns Suno's word timings into an .lrc file: one timed line per sung line.</summary>
    public static class LrcWriter
    {
        private const int WordsPerLineWithoutBreaks = 8;

        public static string Write(SunoClip clip, IReadOnlyList<AlignedWord> words)
        {
            var sb = new StringBuilder();
            sb.Append("[ti:").Append(clip.DisplayTitle).Append("]\n");
            sb.Append("[ar:Suno]\n");
            if (clip.Duration is > 0) sb.Append("[length:").Append(Stamp(clip.Duration.Value)).Append("]\n");

            var hasBreaks = words.Any(w => w.Word.Contains('\n'));
            var line = new StringBuilder();
            double? lineStart = null;
            var count = 0;

            void Flush()
            {
                var text = line.ToString().Trim();
                if (text.Length > 0 && lineStart != null) sb.Append('[').Append(Stamp(lineStart.Value)).Append(']').Append(text).Append('\n');
                line.Clear();
                lineStart = null;
                count = 0;
            }

            foreach (var w in words)
            {
                var word = w.Word.Replace("\r", "");
                var parts = word.Split('\n');
                for (var i = 0; i < parts.Length; i++)
                {
                    if (i > 0) Flush();
                    if (parts[i].Trim().Length == 0) continue;
                    lineStart ??= w.Start;
                    line.Append(parts[i]);
                    if (!parts[i].EndsWith(' ')) line.Append(' ');
                    count++;
                }
                if (!hasBreaks && count >= WordsPerLineWithoutBreaks) Flush();
            }
            Flush();
            return sb.ToString();
        }

        /// <summary>mm:ss.xx, as LRC players expect.</summary>
        public static string Stamp(double seconds)
        {
            var t = TimeSpan.FromSeconds(Math.Max(0, seconds));
            return string.Create(CultureInfo.InvariantCulture, $"{(int)t.TotalMinutes:00}:{t.Seconds:00}.{t.Milliseconds / 10:00}");
        }
    }
}
