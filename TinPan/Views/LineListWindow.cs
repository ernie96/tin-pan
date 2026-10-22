using System.Collections.Generic;
using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Input;
using static TinPan.Localization.Loc;

namespace TinPan.Views
{
    /// <summary>
    /// A list of short lines, read one at a time with the arrow keys: message history, song
    /// information, the command list. When runnable, Enter closes it and reports the chosen line.
    /// Escape closes it.
    /// </summary>
    public sealed class LineListWindow : Window
    {
        public LineListWindow(string title, IReadOnlyList<string> lines, bool runnable)
        {
            Title = title;
            Width = 620;
            Height = 520;
            WindowStartupLocation = WindowStartupLocation.CenterOwner;
            ShowInTaskbar = false;

            var list = new ListBox { ItemsSource = lines, Margin = new Thickness(10) };
            AutomationProperties.SetName(list, title);
            if (lines.Count > 0) list.SelectedIndex = 0;
            list.PreviewKeyDown += (_, e) =>
            {
                if (!runnable || e.Key != Key.Enter || list.SelectedIndex < 0) return;
                e.Handled = true;
                ChosenIndex = list.SelectedIndex;
                DialogResult = true;
            };
            list.MouseDoubleClick += (_, _) =>
            {
                if (!runnable || list.SelectedIndex < 0) return;
                ChosenIndex = list.SelectedIndex;
                DialogResult = true;
            };

            var close = new Button { Content = T("Close"), IsCancel = true, Padding = new Thickness(15, 5, 15, 5), Margin = new Thickness(10), HorizontalAlignment = HorizontalAlignment.Right };
            var dock = new DockPanel();
            DockPanel.SetDock(close, Dock.Bottom);
            dock.Children.Add(close);
            dock.Children.Add(list);
            Content = dock;

            Loaded += (_, _) => ListFocus.FocusItem(list);
        }

        public int? ChosenIndex { get; private set; }
    }
}
