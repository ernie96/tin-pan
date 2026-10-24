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

        public SongListWindow(string title, IReadOnlyList<SunoClip> songs, Func<SunoClip, Task> play, bool sayCreator = true)
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
                if (e.Key is Key.Enter or Key.Space && Keyboard.Modifiers == ModifierKeys.None && list.SelectedItem is SunoClip clip)
                {
                    e.Handled = true;
                    await play(clip);
                }
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
