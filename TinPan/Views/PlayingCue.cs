using System;
using System.ComponentModel;
using System.Globalization;
using System.IO;
using System.Media;
using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Data;
using System.Windows.Input;
using TinPan.Models;

namespace TinPan.Views
{
    /// <summary>What happens when the arrow keys land on the song that is playing or paused.</summary>
    public enum PlayingCueMode
    {
        Click,
        Say,
        ClickAndSay,
        Off
    }

    /// <summary>
    /// Marks the playing song in song lists: a short click when focus lands on it, and/or ", playing"
    /// at the end of its name, which speech and braille both get. Turn it on with
    /// PlayingCue.Songs="True" on a ListBox of SunoClips; this also sets the items' spoken names.
    /// </summary>
    public sealed class PlayingCue : INotifyPropertyChanged
    {
        public static PlayingCue Instance { get; } = new();

        private string? _clipId;
        private PlayingCueMode _mode = PlayingCueMode.Click;
        private SoundPlayer? _click;

        public event PropertyChangedEventHandler? PropertyChanged;

        /// <summary>The song playing or paused, or null.</summary>
        public string? ClipId
        {
            get => _clipId;
            set { if (_clipId != value) { _clipId = value; Changed(nameof(ClipId)); } }
        }

        public PlayingCueMode Mode
        {
            get => _mode;
            set { if (_mode != value) { _mode = value; Changed(nameof(Mode)); } }
        }

        public bool Says => Mode is PlayingCueMode.Say or PlayingCueMode.ClickAndSay;

        private bool Clicks => Mode is PlayingCueMode.Click or PlayingCueMode.ClickAndSay;

        private void Changed(string name) => PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name));

        /// <summary>The spoken name of a song item, with ", playing" when it is the current song.</summary>
        public static string NameFor(string? spokenText, string? id, string? playingId, bool says) =>
            says && id != null && id == playingId ? spokenText + ", playing" : spokenText ?? "";

        public static readonly DependencyProperty SongsProperty = DependencyProperty.RegisterAttached(
            "Songs", typeof(bool), typeof(PlayingCue), new PropertyMetadata(false, OnSongsChanged));

        public static bool GetSongs(DependencyObject d) => (bool)d.GetValue(SongsProperty);
        public static void SetSongs(DependencyObject d, bool value) => d.SetValue(SongsProperty, value);

        private static void OnSongsChanged(DependencyObject d, DependencyPropertyChangedEventArgs e)
        {
            if (d is not ListBox list || e.NewValue is not true) return;

            var name = new MultiBinding { Converter = NameConverter.Instance };
            name.Bindings.Add(new Binding(nameof(SunoClip.SpokenText)));
            name.Bindings.Add(new Binding(nameof(SunoClip.Id)));
            name.Bindings.Add(new Binding(nameof(ClipId)) { Source = Instance });
            name.Bindings.Add(new Binding(nameof(Mode)) { Source = Instance });
            var style = new Style(typeof(ListBoxItem), list.ItemContainerStyle);
            style.Setters.Add(new Setter(AutomationProperties.NameProperty, name));
            list.ItemContainerStyle = style;

            list.AddHandler(UIElement.GotKeyboardFocusEvent, new KeyboardFocusChangedEventHandler((_, args) =>
            {
                if (args.NewFocus is ListBoxItem { DataContext: SunoClip clip } && clip.Id == Instance.ClipId && Instance.Clicks)
                    Instance.PlayClick();
            }));
        }

        private void PlayClick()
        {
            try
            {
                _click ??= new SoundPlayer(MakeClick());
                _click.Play();
            }
            catch (Exception ex)
            {
                Services.Log.Warn($"Couldn't play the playing-song click: {ex.Message}");
            }
        }

        /// <summary>A soft 30 ms tick, made here so there's no sound file to ship.</summary>
        internal static MemoryStream MakeClick()
        {
            const int rate = 44100;
            const int samples = rate * 30 / 1000;
            var stream = new MemoryStream();
            var w = new BinaryWriter(stream);
            w.Write("RIFF"u8);
            w.Write(36 + samples * 2);
            w.Write("WAVEfmt "u8);
            w.Write(16);
            w.Write((short)1);      // PCM
            w.Write((short)1);      // mono
            w.Write(rate);
            w.Write(rate * 2);
            w.Write((short)2);
            w.Write((short)16);
            w.Write("data"u8);
            w.Write(samples * 2);
            for (var i = 0; i < samples; i++)
            {
                var t = (double)i / rate;
                var envelope = Math.Exp(-t * 180);
                w.Write((short)(Math.Sin(2 * Math.PI * 1800 * t) * envelope * 0.3 * short.MaxValue));
            }
            w.Flush();
            stream.Position = 0;
            return stream;
        }

        private sealed class NameConverter : IMultiValueConverter
        {
            public static readonly NameConverter Instance = new();

            public object Convert(object[] values, Type targetType, object parameter, CultureInfo culture) =>
                NameFor(values[0] as string, values[1] as string, values[2] as string,
                    values[3] is PlayingCueMode.Say or PlayingCueMode.ClickAndSay);

            public object[] ConvertBack(object value, Type[] targetTypes, object parameter, CultureInfo culture) =>
                throw new NotSupportedException();
        }
    }
}
