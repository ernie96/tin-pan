using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Reflection;
using System.Text.Json;

namespace TinPan.Localization
{
    /// <summary>
    /// Tin Pan's translations, the same way TimePlayer does them. Text is written in English in the
    /// code and looked up here; anything without a translation stays in English, so a missing entry
    /// never leaves a gap. Counts go through <see cref="Plural"/>, because Romanian and Ukrainian
    /// have three plural forms. The texts are in translations.json, built into the exe: each English
    /// text with its Romanian, Ukrainian and Spanish, in that order.
    /// </summary>
    public static class Loc
    {
        /// <summary>Code, the language's own name, and its English name (translated where it's shown).</summary>
        public static IReadOnlyList<(string Code, string Name, string EnglishName)> Languages { get; } = new[]
        {
            ("", "Automatic (Windows language)", "Automatic (Windows language)"),
            ("en", "English", "English"),
            ("ro", "Română", "Romanian"),
            ("uk", "Українська", "Ukrainian"),
            ("es", "Español", "Spanish"),
        };

        private static readonly string[] Translated = { "ro", "uk", "es" };
        private static Dictionary<string, string[]>? _all;
        private static int _column = -1;

        /// <summary>The language in use: "en", "ro", "uk" or "es".</summary>
        public static string Current { get; private set; } = "en";

        /// <summary>Number and date formatting for translated text. The app's own culture is left alone.</summary>
        public static CultureInfo Culture { get; private set; } = CultureInfo.GetCultureInfo("en-US");

        /// <summary>Chooses the language; null or "" follows Windows, falling back to English.</summary>
        public static void Use(string? code)
        {
            var chosen = string.IsNullOrWhiteSpace(code) ? CultureInfo.CurrentUICulture.TwoLetterISOLanguageName : code.Trim().ToLowerInvariant();
            _column = Array.IndexOf(Translated, chosen);
            Current = _column >= 0 ? chosen : "en";
            Culture = CultureInfo.GetCultureInfo(Current switch { "ro" => "ro-RO", "uk" => "uk-UA", "es" => "es-419", _ => "en-US" });
        }

        /// <summary>Every English text and its translations (Romanian, Ukrainian, Spanish).</summary>
        public static IReadOnlyDictionary<string, string[]> All => _all ??= Load();

        private static Dictionary<string, string[]> Load()
        {
            using var stream = Assembly.GetExecutingAssembly().GetManifestResourceStream("TinPan.translations.json");
            if (stream == null) return new Dictionary<string, string[]>();
            return JsonSerializer.Deserialize<Dictionary<string, string[]>>(stream) ?? new Dictionary<string, string[]>();
        }

        /// <summary>The translation of an English text.</summary>
        public static string T(string english) =>
            _column >= 0 && All.TryGetValue(english, out var row) && row.Length > _column && row[_column].Length > 0 ? row[_column] : english;

        /// <summary>Translates a format string ("Saved {0}.") and fills it in.</summary>
        public static string T(string englishFormat, params object?[] args) =>
            string.Format(Culture, T(englishFormat), args);

        /// <summary>
        /// A count with the right noun form: <c>Plural(n, "{0} song", "{0} songs")</c>. Translations hold
        /// their forms separated by "|" (Romanian: one|few|other, Ukrainian: one|few|many, Spanish: one|other).
        /// </summary>
        public static string Plural(long count, string englishOne, string englishOther)
        {
            var key = englishOne + "|" + englishOther;
            string form;
            if (_column >= 0 && All.TryGetValue(key, out var row) && row.Length > _column && row[_column].Length > 0)
            {
                var forms = row[_column].Split('|');
                form = forms[Math.Min(PluralIndex(count), forms.Length - 1)];
            }
            else
            {
                form = count == 1 ? englishOne : englishOther;
            }
            return string.Format(Culture, form, count);
        }

        /// <summary>Which plural form a count takes in the current language.</summary>
        public static int PluralIndex(long count)
        {
            var n = Math.Abs(count);
            switch (Current)
            {
                case "ro":
                    // 1 cântec; 2-19 cântece (and 0, 101-119, ...); 20+ de cântece.
                    if (n == 1) return 0;
                    if (n == 0 || (n % 100 >= 1 && n % 100 <= 19)) return 1;
                    return 2;
                case "uk":
                    // 1, 21, 31 пісня; 2-4, 22-24 пісні; 5-20, 25-30 пісень.
                    if (n % 10 == 1 && n % 100 != 11) return 0;
                    if (n % 10 >= 2 && n % 10 <= 4 && (n % 100 < 12 || n % 100 > 14)) return 1;
                    return 2;
                default:
                    return n == 1 ? 0 : 1;
            }
        }

        /// <summary>
        /// The language choices for Settings. Each is named in its own language and, when that differs,
        /// in the current one too, because a voice for one alphabet often reads another as silence.
        /// </summary>
        public static IReadOnlyList<(string Code, string Label)> Choices()
        {
            var list = new List<(string, string)>();
            foreach (var (code, name, english) in Languages)
            {
                if (code.Length == 0) list.Add((code, T(name)));
                else if (code == Current) list.Add((code, name));
                else list.Add((code, $"{name} ({T(english)})"));
            }
            return list;
        }
    }
}
