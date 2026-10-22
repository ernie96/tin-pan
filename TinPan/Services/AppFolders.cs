using System;
using System.IO;

namespace TinPan.Services
{
    /// <summary>
    /// Where Tin Pan keeps its files: %APPDATA%\Tin Pan (settings) and %LOCALAPPDATA%\Tin Pan
    /// (log, sign-in browser). The first run after the rename copies the settings from the old
    /// "Suno Desktop" folder and moves the sign-in browser from "Suno Companion", so nobody has to
    /// set things up or sign in to Google again.
    /// </summary>
    public static class AppFolders
    {
        private const string Name = "Tin Pan";
        private const string OldSettingsName = "Suno Desktop";   // shared with the C++ app, so only copied
        private const string OldLocalName = "Suno Companion";

        public static string Settings { get; } = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), Name);
        public static string Local { get; } = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), Name);

        /// <summary>The sign-in browser's profile. Stays in the old place if it couldn't be moved.</summary>
        public static string WebView2 { get; }

        static AppFolders()
        {
            Directory.CreateDirectory(Settings);
            Directory.CreateDirectory(Local);
            CopyOldSettings();
            WebView2 = MoveOldBrowserProfile();
        }

        private static void CopyOldSettings()
        {
            var target = Path.Combine(Settings, "config.json");
            var old = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), OldSettingsName, "config.json");
            try
            {
                if (!File.Exists(target) && File.Exists(old)) File.Copy(old, target);
            }
            catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
            {
                // Starting with default settings is fine; the old file is untouched.
            }
        }

        private static string MoveOldBrowserProfile()
        {
            var target = Path.Combine(Local, "WebView2");
            var old = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), OldLocalName, "WebView2");
            if (Directory.Exists(target) || !Directory.Exists(old)) return target;
            try
            {
                Directory.Move(old, target);
                return target;
            }
            catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
            {
                // Something still has it open; keep using it where it is so the sign-in survives.
                return old;
            }
        }
    }
}
