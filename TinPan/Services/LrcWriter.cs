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
        public static string Write(SunoClip clip, IReadOnlyList<AlignedWord> words)
        {
            var sb = new StringBuilder();
            sb.Append("[ti:").Append(clip.DisplayTitle).Append("]\n");
            sb.Append("[ar:Suno]\n");
            if (clip.Duration is > 0) sb.Append("[length:").Append(Stamp(clip.Duration.Value)).Append("]\n");

            foreach (var line in LyricLine.From(words))
                sb.Append('[').Append(Stamp(line.Start)).Append(']').Append(line.Text).Append('\n');
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
