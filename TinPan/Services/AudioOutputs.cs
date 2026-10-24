using System;
using System.Collections.Generic;
using System.Linq;
using NAudio.CoreAudioApi;
using static TinPan.Localization.Loc;

namespace TinPan.Services
{
    /// <summary>
    /// The audio outputs Windows has switched on, by the names Windows gives them ("Speakers (Realtek(R) Audio)").
    /// Suno's page uses the same names for them, so a name is what Tin Pan saves and sends.
    /// </summary>
    public static class AudioOutputs
    {
        public static IReadOnlyList<string> List()
        {
            try
            {
                using var devices = new MMDeviceEnumerator();
                return devices.EnumerateAudioEndPoints(DataFlow.Render, DeviceState.Active)
                    .Select(d => { using (d) return d.FriendlyName; })
                    .Where(n => !string.IsNullOrWhiteSpace(n))
                    .Distinct()
                    .OrderBy(n => n, StringComparer.CurrentCultureIgnoreCase)
                    .ToList();
            }
            catch (Exception ex) when (ex is System.Runtime.InteropServices.COMException or InvalidOperationException)
            {
                Log.Warn($"Couldn't list the audio outputs: {ex.Message}");
                return Array.Empty<string>();
            }
        }

        /// <summary>The output as said and shown: its name, or "Windows default".</summary>
        public static string Spoken(string? name) => string.IsNullOrEmpty(name) ? T("Windows default output") : name;
    }
}
