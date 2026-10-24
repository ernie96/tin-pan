using System;
using System.Collections.Generic;
using System.Linq;
using System.Windows;
using System.Windows.Input;

namespace TinPan.Views
{
    /// <summary>
    /// Windows that stay open beside the main one: Create, and the Extend, Cover and Remaster forms, so you
    /// can make another version while you listen to the last. F6 moves between them and the main window;
    /// the main window's playback keys (F7, F8, F9, Ctrl+Shift+F12 and the rest) work in them too.
    /// </summary>
    public static class ToolWindows
    {
        private static readonly List<Window> Open = new();

        /// <summary>Set by the main window: gives a tool window the main window's playback keys.</summary>
        public static Action<Window>? AddMainKeys { get; set; }

        /// <summary>Set by the main window: speaks a line, for F1 in a tool window.</summary>
        public static Action<string>? Say { get; set; }

        public static IReadOnlyList<Window> All => Open;

        /// <summary>Shows a tool window beside the main window, never modal. Escape closes it; F6 moves on; F1 says help.</summary>
        public static void Show(Window window, string help)
        {
            window.Owner ??= Application.Current.MainWindow;
            window.ShowInTaskbar = false;
            window.PreviewKeyDown += (_, e) =>
            {
                if (e.Key == Key.F6 && Keyboard.Modifiers is ModifierKeys.None or ModifierKeys.Shift)
                {
                    e.Handled = true;
                    Next(window, back: Keyboard.Modifiers == ModifierKeys.Shift);
                }
                else if (e.Key == Key.F1 && Keyboard.Modifiers == ModifierKeys.None)
                {
                    e.Handled = true;
                    Say?.Invoke(help);
                }
            };
            // After the controls: an open drop-down list takes Escape for itself first.
            window.KeyDown += (_, e) =>
            {
                if (e.Key != Key.Escape || Keyboard.Modifiers != ModifierKeys.None || e.Handled) return;
                e.Handled = true;
                window.Close();
            };
            AddMainKeys?.Invoke(window);
            window.Closed += (_, _) =>
            {
                Open.Remove(window);
                // Back to the main window, not to whatever Windows picks.
                Application.Current.MainWindow?.Activate();
            };
            Open.Add(window);
            window.Show();
        }

        /// <summary>The next window after this one (the main window counts as one): F6 forward, Shift+F6 back.</summary>
        public static void Next(Window? from, bool back = false)
        {
            var main = Application.Current.MainWindow;
            var ring = new List<Window>();
            if (main != null) ring.Add(main);
            ring.AddRange(Open.Where(w => w.IsVisible));
            if (ring.Count < 2) return;
            var index = from == null ? 0 : Math.Max(0, ring.IndexOf(from));
            var next = ring[(index + (back ? ring.Count - 1 : 1)) % ring.Count];
            next.Activate();
        }

        /// <summary>A tool window already showing this, to bring forward instead of opening another.</summary>
        public static T? Find<T>(Func<T, bool> match) where T : Window => Open.OfType<T>().FirstOrDefault(match);
    }
}
