using System;

namespace TinPan.Services
{
    /// <summary>
    /// Your main audio output and a second one to flip to (Ctrl+Shift+F12), say external speakers to show
    /// someone a song. Both are saved; Tin Pan always starts on the main one.
    /// </summary>
    public sealed class OutputSwitch
    {
        private readonly IAudioService _audio;
        private readonly IConfigService _config;

        public OutputSwitch(IAudioService audio, IConfigService config)
        {
            _audio = audio;
            _config = config;
            _audio.OutputDevice = Main;
        }

        /// <summary>The main output's Windows name; "" is the Windows default.</summary>
        public string Main => _config.Current.Audio.OutputDevice ?? "";

        /// <summary>The second output's Windows name; null when none is chosen.</summary>
        public string? Second => string.IsNullOrEmpty(_config.Current.Audio.SecondOutputDevice) ? null
            : _config.Current.Audio.SecondOutputDevice == WindowsDefault ? "" : _config.Current.Audio.SecondOutputDevice;

        /// <summary>Saved for a second output that is the Windows default, since "" means none.</summary>
        public const string WindowsDefault = "*";

        public bool OnSecond { get; private set; }

        /// <summary>What's playing on now.</summary>
        public string Current => _audio.OutputDevice;

        public event EventHandler? Changed;

        public void SetMain(string name)
        {
            _config.Current.Audio.OutputDevice = name;
            _config.SaveSoon();
            if (!OnSecond) _audio.OutputDevice = name;
            Changed?.Invoke(this, EventArgs.Empty);
        }

        /// <param name="name">The Windows name, "" for the Windows default, or null for none.</param>
        public void SetSecond(string? name)
        {
            _config.Current.Audio.SecondOutputDevice = name == null ? "" : name.Length == 0 ? WindowsDefault : name;
            _config.SaveSoon();
            if (OnSecond)
            {
                if (name == null) OnSecond = false;
                _audio.OutputDevice = name ?? Main;
            }
            Changed?.Invoke(this, EventArgs.Empty);
        }

        /// <summary>Flips between the main and second output. False when no second output is chosen.</summary>
        public bool Toggle()
        {
            if (Second is not { } second) return false;
            OnSecond = !OnSecond;
            _audio.OutputDevice = OnSecond ? second : Main;
            Changed?.Invoke(this, EventArgs.Empty);
            return true;
        }
    }
}
