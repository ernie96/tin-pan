using System;
using System.Collections.Generic;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Data;
using System.Windows.Input;
using TinPan.Models;
using static TinPan.Localization.Loc;

namespace TinPan.Views
{
    /// <summary>A list of songs in a small window (similar songs and the like). Enter plays one, Escape closes.</summary>
    public sealed class SongListWindow : Window
    {
        /// <summary>Set by the main window: gives a list the Song menu (Applications key) and its keys, for the selected song.</summary>
        public static Action<ListBox, Func<SunoClip?>>? AddSongMenu { get; set; }

        public SongListWindow(string title, IReadOnlyList<SunoClip> songs, Func<SunoClip, Task> play, bool sayCreator = true,
            Func<SunoClip?, Task>? playPause = null)
        {
            Title = title;
            Width = 560;
            Height = 480;
            WindowStartupLocation = WindowStartupLocation.CenterOwner;
            ShowInTaskbar = false;

            var list = new ListBox { ItemsSource = songs, Margin = new Thickness(10) };
            AutomationProperties.SetName(list, title);
            if (sayCreator) PlayingCue.SetSongsWithCreator(list, true);
            else PlayingCue.SetSongs(list, true);
            list.DisplayMemberPath = sayCreator ? nameof(SunoClip.SpokenWithCreator) : nameof(SunoClip.SpokenText);
            if (songs.Count > 0) list.SelectedIndex = 0;
            AddSongMenu?.Invoke(list, () => list.SelectedItem as SunoClip);

            list.PreviewKeyDown += async (_, e) =>
            {
                if (Keyboard.Modifiers != ModifierKeys.None) return;
                if (e.Key == Key.Enter && list.SelectedItem is SunoClip clip)
                {
                    e.Handled = true;
                    await play(clip);
                }
                else if (e.Key == Key.Space)
                {
                    // Pause and resume, as Space does in every other song list.
                    e.Handled = true;
                    if (playPause != null) await playPause(list.SelectedItem as SunoClip);
                    else if (list.SelectedItem is SunoClip selected) await play(selected);
                }
            };
            // The main window's keys (F8, F9, volume, Ctrl+Shift+F12 and the rest) work here too, and F1 says what this is.
            ToolWindows.AddMainKeys?.Invoke(this);
            PreviewKeyDown += (_, e) =>
            {
                if (e.Key != Key.F1 || Keyboard.Modifiers != ModifierKeys.None) return;
                e.Handled = true;
                ToolWindows.Say?.Invoke(T("{0}. Enter plays a song and carries on down this list, Space pauses or resumes, Left and Right seek, F9 plays the next one. The Applications key has the Song menu. Escape closes.", title));
            };
            list.MouseDoubleClick += async (_, _) =>
            {
                if (list.SelectedItem is SunoClip clip) await play(clip);
            };

            var close = new Button { Content = T("Close"), IsCancel = true, Padding = new Thickness(15, 5, 15, 5), Margin = new Thickness(10), HorizontalAlignment = HorizontalAlignment.Right };
            var dock = new DockPanel();
            DockPanel.SetDock(close, Dock.Bottom);
            dock.Children.Add(close);
            dock.Children.Add(list);
            Content = dock;

            Loaded += (_, _) =>
            {
                list.Focus();
                if (list.ItemContainerGenerator.ContainerFromIndex(0) is ListBoxItem first) first.Focus();
            };
        }
    }
}
