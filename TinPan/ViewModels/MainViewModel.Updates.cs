using System;
using System.Diagnostics;
using System.Linq;
using System.Text.RegularExpressions;
using System.Threading.Tasks;
using System.Windows;
using CommunityToolkit.Mvvm.Input;
using TinPan.Services;
using static TinPan.Localization.Loc;

namespace TinPan.ViewModels
{
    // Checking for, offering and installing updates. Startup checks stay silent unless there is a
    // new version the user hasn't declined; Check for updates in the Help menu always answers.
    public partial class MainViewModel
    {
        private static readonly TimeSpan StartupCheckDelay = TimeSpan.FromSeconds(10);
        private bool _updating;

        /// <summary>After startup: says if the last update failed, then checks quietly if that's on.</summary>
        private async Task AfterStartupUpdatesAsync()
        {
            if (UpdateService.TidyAfterUpdate())
                Announcer.Announce(T("The last update couldn't be installed. Help, Check for updates tries again."), interrupt: false, always: true);
            if (!_config.Current.Update.CheckAtStartup) return;
            await Task.Delay(StartupCheckDelay); // after "Tin Pan ready" and the first library load
            await WhenMainWindowActiveAsync();
            await RunUpdateCheckAsync(manual: false);
        }

        /// <summary>Waits while another window (sign-in, a form, a dialog) is in front.</summary>
        private static Task WhenMainWindowActiveAsync()
        {
            var main = Application.Current?.MainWindow;
            if (main == null || main.IsActive) return Task.CompletedTask;
            var ready = new TaskCompletionSource();
            void OnActivated(object? s, EventArgs e)
            {
                main.Activated -= OnActivated;
                ready.TrySetResult();
            }
            main.Activated += OnActivated;
            return ready.Task;
        }

        [RelayCommand]
        private Task CheckForUpdatesAsync() => RunUpdateCheckAsync(manual: true);

        private async Task RunUpdateCheckAsync(bool manual)
        {
            if (_updating)
            {
                if (manual) Announcer.Announce(T("Already checking for updates."), always: true);
                return;
            }
            _updating = true;
            try
            {
                if (manual) Announcer.Announce(T("Checking for updates."));
                var update = await _updates.CheckAsync();
                if (update == null)
                {
                    if (manual) Announcer.Announce(T("Tin Pan is up to date, version {0}.", _updates.CurrentVersion.ToString(3)), always: true);
                    return;
                }

                var version = update.Version.ToString(3);
                if (!manual && _config.Current.Update.SkippedVersion == version) return;

                var notes = PlainText(UpdateService.ChangesOnly(update.ReleaseNotes));
                var whatsNew = notes.Length > 0 ? T("What's new:") + "\n" + notes + "\n\n" : "";
                var reminder = manual ? "" : "\n\n" + T("If you choose No, Tin Pan won't remind you about this version. Help, Check for updates still finds it.");
                var intro = T("Tin Pan {0} is available. You have {1}.", version, _updates.CurrentVersion.ToString(3)) + "\n\n" + whatsNew;

                if (!_updates.CanInstall)
                {
                    // The development build can't replace itself; offer the release page instead.
                    if (_dialogs.Ask(intro + T("This copy of Tin Pan can't update itself. Open the release page?") + reminder, T("Tin Pan update")))
                        OpenPage(update.PageUrl);
                    else if (!manual) SkipVersion(version);
                    return;
                }

                if (!_dialogs.Ask(intro + T("Download and install it now? Tin Pan closes and opens again by itself.") + reminder, T("Tin Pan update")))
                {
                    if (!manual) SkipVersion(version);
                    return;
                }
                await DownloadAndInstallAsync(update, version);
            }
            catch (UpdateException ex)
            {
                Log.Warn($"Update: {ex.Message}");
                if (manual) Announcer.Announce(ex.Message, always: true);
            }
            finally
            {
                _updating = false;
            }
        }

        private async Task DownloadAndInstallAsync(UpdateInfo update, string version)
        {
            Announcer.Announce(T("Downloading Tin Pan {0}.", version), always: true);
            var halfwaySaid = false;
            var progress = new Progress<double>(fraction =>
            {
                if (halfwaySaid || fraction < 0.5) return;
                halfwaySaid = true;
                Announcer.Announce(T("Update download halfway."), interrupt: false);
            });

            var file = await _updates.DownloadAsync(update, progress);

            var exe = Environment.ProcessPath;
            // Each Process holds a handle until it's disposed.
            var processes = Process.GetProcessesByName("TinPan");
            var others = processes.Count(p => p.Id != Environment.ProcessId
                && string.Equals(SafePath(p), exe, StringComparison.OrdinalIgnoreCase));
            foreach (var p in processes) p.Dispose();
            if (others > 0)
            {
                Announcer.Announce(T("Another Tin Pan window is open, and the update can't install while it is. Close it, then check for updates again."), always: true);
                return;
            }

            _updates.StartInstall(file);
            Announcer.Announce(T("Installing Tin Pan {0}. It opens again in a moment.", version), always: true);
            await Task.Delay(1500); // let that be heard
            Application.Current.Shutdown();
        }

        private static string? SafePath(Process p)
        {
            try { return p.MainModule?.FileName; }
            catch (Exception ex) when (ex is System.ComponentModel.Win32Exception or InvalidOperationException) { return null; }
        }

        private void SkipVersion(string version)
        {
            _config.Current.Update.SkippedVersion = version;
            _config.SaveSoon();
        }

        private void OpenPage(string url)
        {
            try { Process.Start(new ProcessStartInfo(url) { UseShellExecute = true }); }
            catch (Exception ex) when (ex is System.ComponentModel.Win32Exception or InvalidOperationException)
            {
                Announcer.Announce(T("Couldn't open the browser. The release page is github.com/ernie96/tin-pan/releases."), always: true);
            }
        }

        /// <summary>Release notes are Markdown; read them as plain lines.</summary>
        public static string PlainText(string markdown)
        {
            if (string.IsNullOrWhiteSpace(markdown)) return "";
            var text = markdown.Replace("\r", "");
            text = Regex.Replace(text, @"\[([^\]]+)\]\([^)]+\)", "$1");
            text = Regex.Replace(text, @"(^#+\s*|\*\*|__|`)", "", RegexOptions.Multiline);
            text = Regex.Replace(text, @"^[ \t]*[*+-][ \t]+", "- ", RegexOptions.Multiline);
            text = Regex.Replace(text, @"\n{3,}", "\n\n").Trim();
            return text.Length > 1500 ? text[..1500].TrimEnd() + "..." : text;
        }
    }
}
