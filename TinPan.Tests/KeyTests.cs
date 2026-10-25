using System.IO;
using System.Text.RegularExpressions;
using Xunit;

namespace TinPan.Tests
{
    /// <summary>Every key in the menus does one thing, and none is also a window-wide key.</summary>
    public class KeyTests
    {
        [Fact]
        public void No_two_commands_share_a_key()
        {
            var app = AppFolder();
            var catalog = File.ReadAllText(Path.Combine(app, "ViewModels", "MainViewModel.Commands.cs"));
            var keys = Regex.Matches(catalog, @"new\(""\w+"", T\(""(?:[^""\\]|\\.)*""\), ""([^""]+)"", (\w+(?:\.\w+)?Command)(?:, ""([^""]*)"")?")
                .Select(m => (Keys: m.Groups[1].Value, Command: m.Groups[2].Value + "|" + m.Groups[3].Value))
                .Distinct()
                .ToList();
            Assert.True(keys.Count > 40, $"only {keys.Count} keyed commands found; the pattern may be stale");
            var shared = keys.GroupBy(k => k.Keys).Where(g => g.Select(k => k.Command).Distinct().Count() > 1).Select(g => g.Key).ToList();
            Assert.True(shared.Count == 0, "keys used by two commands: " + string.Join(", ", shared));
        }

        [Fact]
        public void Song_keys_are_not_also_window_keys()
        {
            var app = AppFolder();
            var catalog = File.ReadAllText(Path.Combine(app, "ViewModels", "MainViewModel.Commands.cs"));
            var song = Regex.Matches(catalog, @"new\(""Song"", T\(""(?:[^""\\]|\\.)*""\), ""([^""]+)""").Select(m => m.Groups[1].Value).ToHashSet();
            var window = File.ReadAllText(Path.Combine(app, "MainWindow.xaml"));
            var windowKeys = Regex.Matches(window, @"<KeyBinding Key=""(\w+)""(?: Modifiers=""([\w+]+)"")?")
                .Select(m => (m.Groups[2].Success ? m.Groups[2].Value + "+" : "") + Name(m.Groups[1].Value))
                .ToHashSet();
            var both = song.Intersect(windowKeys).ToList();
            Assert.True(both.Count == 0, "song keys that are also window keys: " + string.Join(", ", both));
        }

        // KeyBinding names digits D1..D9; the menus write them as 1..9.
        private static string Name(string key) => Regex.IsMatch(key, @"^D\d$") ? key[1..] : key;

        private static string AppFolder()
        {
            var dir = new DirectoryInfo(AppContext.BaseDirectory);
            while (dir != null && !Directory.Exists(Path.Combine(dir.FullName, "TinPan", "Localization")))
                dir = dir.Parent;
            Assert.NotNull(dir);
            return Path.Combine(dir!.FullName, "TinPan");
        }
    }
}
