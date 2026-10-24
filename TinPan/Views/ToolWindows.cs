using System;
using System.Collections.Generic;
using System.Linq;
using System.Windows;
using System.Windows.Input;
using static TinPan.Localization.Loc;

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

        // The Tin Pan window you were last in, and the one to go back to when Tin Pan comes back to the front.
        private static Window? _lastActive;
        private static Window? _returnTo;
        private static bool _watching;

        /// <summary>
        /// Called once by the main window. Alt+Tab (or the taskbar) brings Tin Pan back through its main
        /// window, so if you were in Create or a Cover form when you left, focus goes back there.
        /// </summary>
        public static void WatchFocus(Window main)
        {
            if (_watching) return;
            _watching = true;
            main.Activated += (_, _) => _lastActive = main;
            var app = Application.Current;
            app.Deactivated += (_, _) => _returnTo = _lastActive;
            app.Activated += (_, _) =>
            {
                var back = _returnTo;
                _returnTo = null;
                if (back == null || back == main || !Open.Contains(back) || !back.IsVisible) return;
                // After Windows has finished putting the main window in front.
                main.Dispatcher.BeginInvoke(System.Windows.Threading.DispatcherPriority.Input, () =>
                {
                    if (Open.Contains(back) && back.IsVisible && !back.IsActive) back.Activate();
                });
            };
        }

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
                    Say?.Invoke(T("F6 moves between this window and the main window.") + " " + help);
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
            window.Activated += (_, _) => _lastActive = window;
            window.Closed += (_, _) =>
            {
                Open.Remove(window);
                if (_lastActive == window) _lastActive = null;
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
