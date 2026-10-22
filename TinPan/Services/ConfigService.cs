using System;
using System.IO;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using System.Threading;
using TinPan.Models;

namespace TinPan.Services
{
    public interface IConfigService
    {
        AppConfig Current { get; }
        void Save();

        /// <summary>Saves a second after the last call, for settings that change while sliding or typing.</summary>
        void SaveSoon();

        string? LoadCookies();
        void SaveCookies(string? cookieHeader);
    }

    public class ConfigService : IConfigService
    {
        private static readonly JsonSerializerOptions JsonOptions = new() { WriteIndented = true };
        private static readonly byte[] Entropy = Encoding.UTF8.GetBytes("SunoCompanion.cookies.v1");

        private readonly string _configPath;
        private readonly object _gate = new();
        private readonly Timer _saveTimer;

        public AppConfig Current { get; private set; } = new();

        public ConfigService()
        {
            _configPath = Path.Combine(AppFolders.Settings, "config.json");
            _saveTimer = new Timer(_ => Save());
            Load();
        }

        private void Load()
        {
            if (File.Exists(_configPath))
            {
                try
                {
                    Current = JsonSerializer.Deserialize<AppConfig>(File.ReadAllText(_configPath)) ?? new AppConfig();
                }
                catch (Exception ex) when (ex is JsonException or IOException)
                {
                    // Keep the damaged file for inspection instead of silently overwriting it.
                    var broken = _configPath + $".broken-{DateTime.Now:yyyyMMdd-HHmmss}";
                    try { File.Move(_configPath, broken); } catch (IOException) { }
                    Log.Error("Settings file was unreadable; started with defaults", ex);
                    Current = new AppConfig();
                }
            }

            Current.Audio ??= new AudioConfig();
            Current.User ??= new UserConfig();
            Current.Library ??= new LibraryConfig();
            Current.Resume ??= new ResumeConfig();
            Current.Update ??= new UpdateConfig();
            if (string.IsNullOrEmpty(Current.User.DeviceId))
                Current.User.DeviceId = Guid.NewGuid().ToString();

            // Earlier versions stored a cookie string in plain text. Only one with __client is usable.
            if (Current.User.CookieUat != null || Current.User.SessionToken != null)
            {
                var legacy = Current.User.CookieUat;
                Current.User.CookieUat = null;
                Current.User.SessionToken = null;
                if (legacy != null && SunoCookies.Parse(legacy).HasClient)
                    SaveCookies(legacy);
            }
            Save();
        }

        public void Save()
        {
            lock (_gate)
            {
                try
                {
                    var temp = _configPath + ".tmp";
                    File.WriteAllText(temp, JsonSerializer.Serialize(Current, JsonOptions));
                    File.Move(temp, _configPath, overwrite: true);
                }
                catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
                {
                    Log.Error("Couldn't save settings", ex);
                }
            }
        }

        public void SaveSoon() => _saveTimer.Change(TimeSpan.FromSeconds(1), Timeout.InfiniteTimeSpan);

        public string? LoadCookies()
        {
            if (string.IsNullOrEmpty(Current.User.ProtectedCookies)) return null;
            try
            {
                var bytes = ProtectedData.Unprotect(Convert.FromBase64String(Current.User.ProtectedCookies), Entropy, DataProtectionScope.CurrentUser);
                return Encoding.UTF8.GetString(bytes);
            }
            catch (Exception ex) when (ex is CryptographicException or FormatException)
            {
                Log.Warn("Saved sign-in couldn't be decrypted; a new sign-in is needed");
                return null;
            }
        }

        public void SaveCookies(string? cookieHeader)
        {
            Current.User.ProtectedCookies = string.IsNullOrEmpty(cookieHeader)
                ? ""
                : Convert.ToBase64String(ProtectedData.Protect(Encoding.UTF8.GetBytes(cookieHeader), Entropy, DataProtectionScope.CurrentUser));
            Save();
        }
    }
}
