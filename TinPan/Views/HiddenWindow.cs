using System;
using System.Linq;
using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Interop;

namespace TinPan.Services
{
    /// <summary>
    /// Keeps the windows that hold Suno's own pages (the player, the captcha check) out of reach: never
    /// in Alt+Tab, never activated, and off screen. Minimising Tin Pan (Alt+Space, Win+D) used to let
    /// Windows activate one of them, or move it back on screen, which left people on the Suno web site.
    /// If one is activated anyway, Tin Pan's own window takes over at once.
    /// </summary>
    public static class HiddenWindow
    {
        private const int GwlExStyle = -20;
        private const long WsExToolWindow = 0x00000080;
        private const long WsExAppWindow = 0x00040000;
        private const long WsExNoActivate = 0x08000000;

        /// <summary>Far off screen, but not at -32000, which Windows treats as "minimised" and may restore on screen.</summary>
        public const double OffScreen = -20000;

        [DllImport("user32.dll", EntryPoint = "GetWindowLongPtrW")]
        private static extern IntPtr GetWindowLongPtr(IntPtr hWnd, int index);

        [DllImport("user32.dll", EntryPoint = "SetWindowLongPtrW")]
        private static extern IntPtr SetWindowLongPtr(IntPtr hWnd, int index, IntPtr value);

        /// <summary>Applies the rules; call before Show. allowed(): true while the window may be on screen (a captcha the user must press).</summary>
        public static void Keep(Window window, Func<bool>? allowed = null)
        {
            window.ShowActivated = false;
            window.ShowInTaskbar = false;
            window.Left = OffScreen;
            window.Top = OffScreen;
            window.SourceInitialized += (_, _) => SetReachable(window, false);
            window.Activated += (_, _) =>
            {
                if (allowed?.Invoke() == true) return;
                Log.Warn($"\"{window.Title}\" was activated; back to Tin Pan");
                window.Dispatcher.BeginInvoke(() => BackToTinPan(window));
            };
            window.LocationChanged += (_, _) =>
            {
                if (allowed?.Invoke() == true || window.Left <= OffScreen / 2) return;
                window.Left = OffScreen;
                window.Top = OffScreen;
            };
            window.StateChanged += (_, _) =>
            {
                if (allowed?.Invoke() != true && window.WindowState != WindowState.Normal) window.WindowState = WindowState.Normal;
            };
        }

        /// <summary>On for a captcha the user has to press (in Alt+Tab, can take focus); off again afterwards.</summary>
        public static void SetReachable(Window window, bool reachable)
        {
            var handle = new WindowInteropHelper(window).Handle;
            if (handle == IntPtr.Zero) return;
            var style = GetWindowLongPtr(handle, GwlExStyle).ToInt64();
            style = reachable
                ? (style & ~(WsExToolWindow | WsExNoActivate)) | WsExAppWindow
                : (style | WsExToolWindow | WsExNoActivate) & ~WsExAppWindow;
            SetWindowLongPtr(handle, GwlExStyle, new IntPtr(style));
        }

        private static void BackToTinPan(Window hidden)
        {
            var app = Application.Current;
            if (app == null) return;
            // The Tin Pan window that was in use: a form or Create if one is open, else the main window.
            var target = Views.ToolWindows.All.LastOrDefault(w => w.IsVisible) ?? app.MainWindow;
            if (target == null || target == hidden || !target.IsVisible) return;
            if (target.WindowState == WindowState.Minimized) return; // Tin Pan itself is minimised: leave it so
            target.Activate();
        }
    }
}
