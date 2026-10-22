using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Automation;
using System.Windows.Automation.Peers;

namespace TinPan.Services
{
    /// <summary>How much Tin Pan says by itself. Errors and things you ask for (F1, F2, F3, Ctrl+R) are always spoken.</summary>
    public enum SpeechLevel
    {
        /// <summary>Everything, including background news such as songs finishing.</summary>
        Everything,

        /// <summary>Results of what you just did, but not background news.</summary>
        AskedFor,

        /// <summary>Only errors and the things you explicitly ask to hear.</summary>
        ErrorsOnly
    }

    public interface IAnnouncer
    {
        /// <summary>
        /// Speaks a short message. interrupt=true for the result of something the user just did;
        /// false for background news, which waits its turn. always=true for errors and for
        /// information the user asked for, which the speech level never silences.
        /// Every message goes into the history, spoken or not.
        /// </summary>
        void Announce(string text, bool interrupt = true, bool always = false);

        /// <summary>Speaks the last message again (Ctrl+R).</summary>
        void RepeatLast();

        /// <summary>Recent messages, newest first, each with its time.</summary>
        IReadOnlyList<string> History { get; }

        SpeechLevel Level { get; set; }

        bool IsNvdaRunning { get; }

        /// <summary>"NVDA", "another screen reader" or "Windows speech", for the About screen.</summary>
        string OutputName { get; }
    }

    /// <summary>
    /// NVDA through nvdaControllerClient.dll when it is running; otherwise a UI Automation
    /// notification, which JAWS and Narrator read; Windows speech only when no screen reader is on.
    /// </summary>
    public sealed class ScreenReaderService : IAnnouncer, IDisposable
    {
        // EmbeddedFiles resolves these to the copy next to TinPan.exe or the built-in one;
        // the search path below is only the fallback, and never looks elsewhere on the system.
        [DllImport("nvdaControllerClient.dll", CharSet = CharSet.Unicode)]
        [DefaultDllImportSearchPaths(DllImportSearchPath.AssemblyDirectory)]
        private static extern int nvdaController_speakText(string text);

        [DllImport("nvdaControllerClient.dll")]
        [DefaultDllImportSearchPaths(DllImportSearchPath.AssemblyDirectory)]
        private static extern int nvdaController_cancelSpeech();

        [DllImport("nvdaControllerClient.dll")]
        [DefaultDllImportSearchPaths(DllImportSearchPath.AssemblyDirectory)]
        private static extern int nvdaController_testIfRunning();

        [DllImport("nvdaControllerClient.dll", CharSet = CharSet.Unicode)]
        [DefaultDllImportSearchPaths(DllImportSearchPath.AssemblyDirectory)]
        private static extern int nvdaController_brailleMessage(string message);

        [DllImport("user32.dll", SetLastError = true)]
        [DefaultDllImportSearchPaths(DllImportSearchPath.System32)]
        [return: MarshalAs(UnmanagedType.Bool)]
        private static extern bool SystemParametersInfo(uint action, uint param, ref bool value, uint winIni);

        private const uint SpiGetScreenReader = 0x0046;

        private const int HistorySize = 100;

        private readonly LinkedList<string> _history = new();
        private System.Speech.Synthesis.SpeechSynthesizer? _sapi;
        private bool _nvdaDllMissing;
        private string? _last;

        public SpeechLevel Level { get; set; }

        public IReadOnlyList<string> History => new List<string>(_history);

        public string OutputName => IsNvdaRunning ? "NVDA" : OtherScreenReaderIsOn() ? "another screen reader" : "Windows speech";

        public bool IsNvdaRunning
        {
            get
            {
                if (_nvdaDllMissing) return false;
                try { return nvdaController_testIfRunning() == 0; }
                catch (Exception ex) when (ex is DllNotFoundException or EntryPointNotFoundException or BadImageFormatException)
                {
                    _nvdaDllMissing = true;
                    Log.Warn("nvdaControllerClient.dll not usable; NVDA direct speech is off");
                    return false;
                }
            }
        }

        public void Announce(string text, bool interrupt = true, bool always = false)
        {
            if (string.IsNullOrWhiteSpace(text)) return;
            _last = text;
            _history.AddFirst($"{DateTime.Now:HH:mm} {text}");
            while (_history.Count > HistorySize) _history.RemoveLast();

            var speak = always || Level switch
            {
                SpeechLevel.Everything => true,
                SpeechLevel.AskedFor => interrupt,
                _ => false
            };
            if (speak) Speak(text, interrupt);
        }

        public void RepeatLast() => Speak(_last ?? "Nothing said yet.", interrupt: true);

        private void Speak(string text, bool interrupt)
        {
            if (IsNvdaRunning)
            {
                if (interrupt) nvdaController_cancelSpeech();
                if (nvdaController_speakText(text) == 0)
                {
                    nvdaController_brailleMessage(text);
                    return;
                }
            }

            if (OtherScreenReaderIsOn() && RaiseUiaNotification(text, interrupt)) return;

            _sapi ??= new System.Speech.Synthesis.SpeechSynthesizer();
            if (interrupt) _sapi.SpeakAsyncCancelAll();
            _sapi.SpeakAsync(text);
        }

        private static bool OtherScreenReaderIsOn()
        {
            var on = false;
            return SystemParametersInfo(SpiGetScreenReader, 0, ref on, 0) && on;
        }

        private static bool RaiseUiaNotification(string text, bool interrupt)
        {
            var window = Application.Current?.MainWindow;
            if (window == null) return false;
            var peer = UIElementAutomationPeer.FromElement(window) ?? UIElementAutomationPeer.CreatePeerForElement(window);
            if (peer == null) return false;
            peer.RaiseNotificationEvent(
                AutomationNotificationKind.Other,
                interrupt ? AutomationNotificationProcessing.ImportantMostRecent : AutomationNotificationProcessing.All,
                text,
                "TinPan.Announcement");
            return true;
        }

        public void Dispose() => _sapi?.Dispose();
    }
}
