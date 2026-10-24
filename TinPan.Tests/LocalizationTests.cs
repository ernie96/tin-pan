using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Text.RegularExpressions;
using TinPan.Localization;
using TinPan.Models;
using Xunit;

namespace TinPan.Tests
{
    /// <summary>Loc is app-wide; these tests change the language, so they run on their own.</summary>
    [CollectionDefinition("Language", DisableParallelization = true)]
    public class LanguageCollection
    {
    }

    [Collection("Language")]
    public class LocalizationTests : IDisposable
    {
        public void Dispose() => Loc.Use("en");

        private static string AppFolder()
        {
            var dir = new DirectoryInfo(AppContext.BaseDirectory);
            while (dir != null && !Directory.Exists(Path.Combine(dir.FullName, "TinPan", "Localization")))
                dir = dir.Parent;
            Assert.NotNull(dir);
            return Path.Combine(dir!.FullName, "TinPan");
        }

        [Theory]
        [InlineData("ro", 1, 0)]
        [InlineData("ro", 2, 1)]
        [InlineData("ro", 19, 1)]
        [InlineData("ro", 20, 2)]
        [InlineData("ro", 101, 1)]
        [InlineData("ro", 0, 1)]
        [InlineData("uk", 1, 0)]
        [InlineData("uk", 21, 0)]
        [InlineData("uk", 11, 2)]
        [InlineData("uk", 3, 1)]
        [InlineData("uk", 23, 1)]
        [InlineData("uk", 13, 2)]
        [InlineData("uk", 5, 2)]
        [InlineData("en", 1, 0)]
        [InlineData("en", 2, 1)]
        [InlineData("es", 1, 0)]
        [InlineData("es", 0, 1)]
        [InlineData("es", 21, 1)]
        public void Plural_rules(string language, long count, int form)
        {
            Loc.Use(language);
            Assert.Equal(form, Loc.PluralIndex(count));
        }

        [Fact]
        public void Counts_read_naturally_in_each_language()
        {
            Loc.Use("ro");
            Assert.Equal("1 melodie", Loc.Plural(1, "{0} song", "{0} songs"));
            Assert.Equal("5 melodii", Loc.Plural(5, "{0} song", "{0} songs"));
            Assert.Equal("25 de melodii", Loc.Plural(25, "{0} song", "{0} songs"));

            Loc.Use("uk");
            Assert.Equal("1 пісня", Loc.Plural(1, "{0} song", "{0} songs"));
            Assert.Equal("3 пісні", Loc.Plural(3, "{0} song", "{0} songs"));
            Assert.Equal("5 пісень", Loc.Plural(5, "{0} song", "{0} songs"));
            Assert.Equal("Грає: Rain.", Loc.T("Playing: {0}.", "Rain"));

            Loc.Use("es");
            Assert.Equal("1 canción", Loc.Plural(1, "{0} song", "{0} songs"));
            Assert.Equal("25 canciones", Loc.Plural(25, "{0} song", "{0} songs"));

            Loc.Use("en");
            Assert.Equal("1 song", Loc.Plural(1, "{0} song", "{0} songs"));
            Assert.Equal("Playing: Rain.", Loc.T("Playing: {0}.", "Rain"));
        }

        [Fact]
        public void Missing_translations_fall_back_to_English()
        {
            Loc.Use("uk");
            Assert.Equal("Something new nobody translated", Loc.T("Something new nobody translated"));
            Assert.Equal("4 widgets", Loc.Plural(4, "{0} widget", "{0} widgets"));
        }

        [Fact]
        public void Every_translation_keeps_its_placeholders_and_formats()
        {
            var placeholder = new Regex(@"\{(\d+)(?:[,:][^}]*)?\}");
            Assert.True(Loc.All.Count > 500);
            foreach (var (english, row) in Loc.All)
            {
                Assert.True(row.Length == 3 && row.All(r => r.Trim().Length > 0), $"incomplete: {english}");
                var isPlural = english.Count(c => c == '|') == 1 && english.Contains("{0");
                for (var i = 0; i < 3; i++)
                {
                    var forms = isPlural ? row[i].Split('|') : new[] { row[i] };
                    if (isPlural) Assert.Equal(i == 2 ? 2 : 3, forms.Length);
                    var wanted = placeholder.Matches(english.Split('|')[0]).Select(m => m.Groups[1].Value).Distinct().OrderBy(x => x);
                    foreach (var form in forms)
                    {
                        Assert.True(wanted.SequenceEqual(placeholder.Matches(form).Select(m => m.Groups[1].Value).Distinct().OrderBy(x => x)),
                            $"placeholders differ for \"{english}\": \"{form}\"");
                        _ = string.Format(CultureInfo.InvariantCulture, form, 1, 2, 3, 4, 5);
                    }
                }
            }
        }

        /// <summary>New English text in the code needs all three translations; this finds any that hasn't one.</summary>
        [Fact]
        public void Every_text_in_the_code_is_translated()
        {
            var app = AppFolder();
            var used = new HashSet<string>();
            var single = new Regex(@"(?<![\w.])T\(""((?:[^""\\]|\\.)*)""\s*[,)]");
            var plural = new Regex(@"Plural\([^,]+,\s*""((?:[^""\\]|\\.)*)"",\s*""((?:[^""\\]|\\.)*)""\)");
            foreach (var file in Directory.EnumerateFiles(app, "*.cs", SearchOption.AllDirectories).Where(f => !f.Contains($"{Path.DirectorySeparatorChar}obj{Path.DirectorySeparatorChar}")))
            {
                var text = File.ReadAllText(file);
                foreach (Match m in single.Matches(text)) used.Add(Regex.Unescape(m.Groups[1].Value));
                foreach (Match m in plural.Matches(text)) used.Add(Regex.Unescape(m.Groups[1].Value) + "|" + Regex.Unescape(m.Groups[2].Value));
            }
            var xaml = new Regex(@"\{l:T '((?:[^'\\]|\\.)*)'\}");
            foreach (var file in Directory.EnumerateFiles(app, "*.xaml", SearchOption.AllDirectories).Where(f => !f.Contains($"{Path.DirectorySeparatorChar}obj{Path.DirectorySeparatorChar}")))
                foreach (Match m in xaml.Matches(File.ReadAllText(file)))
                    used.Add(System.Net.WebUtility.HtmlDecode(m.Groups[1].Value.Replace("\\'", "'")));
            used.RemoveWhere(k => !Regex.IsMatch(k, "[A-Za-z]{2}"));

            Assert.True(used.Count > 500, $"only {used.Count} texts found; the scan is broken");
            var missing = used.Where(k => !Loc.All.ContainsKey(k)).OrderBy(k => k).ToList();
            Assert.True(missing.Count == 0, $"{missing.Count} texts have no translations: " + string.Join(" | ", missing.Take(10)));
        }

        /// <summary>Access keys (the letter after "_") must stay unique on each page and in each menu, in every language.</summary>
        [Fact]
        public void Access_keys_stay_unique_on_each_page_and_menu()
        {
            var app = AppFolder();
            var groups = new Dictionary<string, List<string>>();
            var xaml = new Regex(@"\{l:T '((?:[^'\\]|\\.)*)'\}");
            foreach (var file in Directory.EnumerateFiles(Path.Combine(app, "Views"), "*.xaml"))
            {
                // An Applications-key menu in a page is its own group; its letters only apply while it's open.
                var text = File.ReadAllText(file);
                var menus = Regex.Matches(text, "<ContextMenu>.*?</ContextMenu>", RegexOptions.Singleline);
                for (var i = 0; i < menus.Count; i++)
                    groups[$"{Path.GetFileName(file)} menu {i + 1}"] = xaml.Matches(menus[i].Value).Select(m => m.Groups[1].Value.Replace("\\'", "'")).ToList();
                var page = Regex.Replace(text, "<ContextMenu>.*?</ContextMenu>", "", RegexOptions.Singleline);
                groups[Path.GetFileName(file)] = xaml.Matches(page).Select(m => m.Groups[1].Value.Replace("\\'", "'")).ToList();
            }
            var catalog = File.ReadAllText(Path.Combine(app, "ViewModels", "MainViewModel.Commands.cs"));
            // Each submenu is its own group, and its own label belongs to the menu it sits in.
            var headers = new Regex(@"""(\w+)"" => T\(""((?:[^""\\]|\\.)*)""\),").Matches(catalog)
                .GroupBy(m => m.Groups[1].Value).ToDictionary(g => g.Key, g => Regex.Unescape(g.First().Groups[2].Value));
            List<string> Group(string name) => groups.TryGetValue(name, out var list) ? list : groups[name] = new List<string>();
            foreach (Match m in new Regex(@"new\(""(\w+)"", T\(""((?:[^""\\]|\\.)*)""\)([^\n]*)").Matches(catalog))
            {
                var label = Regex.Unescape(m.Groups[2].Value);
                var sub = Regex.Match(m.Groups[3].Value, @"Submenu: ""(\w+)""");
                if (!sub.Success)
                {
                    Group("Menu " + m.Groups[1].Value).Add(label);
                    continue;
                }
                var parent = Group("Menu " + m.Groups[1].Value);
                if (!parent.Contains(headers[sub.Groups[1].Value])) parent.Add(headers[sub.Groups[1].Value]);
                Group($"Menu {m.Groups[1].Value}, {sub.Groups[1].Value}").Add(label);
            }
            groups["Menu Song, Share, other labels"] = groups["Menu Song, Share"].Select(l => l == "Make p_ublic" ? "Make p_rivate" : l == "Stop remi_xes" ? "Allow remi_xes" : l).ToList();

            foreach (var language in new[] { "en", "ro", "uk", "es" })
            {
                Loc.Use(language);
                foreach (var (name, labels) in groups)
                {
                    // Forms built in code are checked by build/translations/check.py.
                    var keys = labels.Distinct().Select(Loc.T).Select(t => Regex.Match(t, "_(.)")).Where(m => m.Success)
                        .Select(m => char.ToLowerInvariant(m.Groups[1].Value[0])).ToList();
                    var clash = keys.GroupBy(k => k).Where(g => g.Count() > 1).Select(g => g.Key).ToList();
                    Assert.True(clash.Count == 0, $"{language}, {name}: access key '{string.Join(", ", clash)}' is used twice");
                }
            }
        }

        [Fact]
        public void Language_choices_are_named_so_every_voice_can_read_them()
        {
            Loc.Use("en");
            Assert.Equal(new[] { "Automatic (Windows language)", "English", "Română (Romanian)", "Українська (Ukrainian)", "Español (Spanish)" },
                Loc.Choices().Select(c => c.Label));

            Loc.Use("uk");
            var ukrainian = Loc.Choices().Select(c => c.Label).ToList();
            Assert.Contains("Українська", ukrainian);
            Assert.Contains("English (англійська)", ukrainian);
            Assert.Equal(5, ukrainian.Count);
        }

        [Fact]
        public void Language_follows_Windows_when_set_to_automatic()
        {
            var previous = CultureInfo.CurrentUICulture;
            try
            {
                CultureInfo.CurrentUICulture = new CultureInfo("uk-UA");
                Loc.Use("");
                Assert.Equal("uk", Loc.Current);
                CultureInfo.CurrentUICulture = new CultureInfo("es-MX");
                Loc.Use("");
                Assert.Equal("es", Loc.Current);
                CultureInfo.CurrentUICulture = new CultureInfo("de-DE");
                Loc.Use(null);
                Assert.Equal("en", Loc.Current);
            }
            finally
            {
                CultureInfo.CurrentUICulture = previous;
            }
        }

        [Fact]
        public void Song_text_and_billing_read_in_the_chosen_language()
        {
            var clip = new SunoClip { Id = "a1", Title = "Rain", IsLiked = true, Status = "complete", Metadata = new ClipMetadata { Duration = 151 } };
            Loc.Use("es");
            Assert.Equal("Rain, 2:31, te gusta", clip.SpokenText);
            Assert.Contains("Estado: lista", clip.InfoLines);
            Loc.Use("ro");
            Assert.Equal("Au mai rămas 25 de credite.", BillingText.Credits(new BillingInfo { TotalCreditsLeft = 25 }));
        }
    }
}
