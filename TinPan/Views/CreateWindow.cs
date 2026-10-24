using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using TinPan.ViewModels;
using static TinPan.Localization.Loc;

namespace TinPan.Views
{
    /// <summary>
    /// Making a song, in a window of its own (Ctrl+N) rather than a tab: it stays open beside the Library,
    /// so you can make a song, listen to it, change the style and make another. F6 moves between them;
    /// Escape or Close closes it. The fields keep what you typed until Tin Pan closes.
    /// </summary>
    public sealed class CreateWindow : Window
    {
        private readonly CreateView _view;

        public CreateWindow(SunoPanelViewModel vm)
        {
            Title = T("Create a song");
            Width = 720;
            Height = 760;
            MaxHeight = SystemParameters.WorkArea.Height;
            WindowStartupLocation = WindowStartupLocation.CenterOwner;
            ResizeMode = ResizeMode.CanResizeWithGrip;
            DataContext = vm;

            _view = new CreateView();
            var close = new Button { Content = T("Close"), Padding = new Thickness(15, 5, 15, 5), Margin = new Thickness(10), HorizontalAlignment = HorizontalAlignment.Right };
            close.Click += (_, _) => Close();
            var dock = new DockPanel();
            DockPanel.SetDock(close, Dock.Bottom);
            dock.Children.Add(close);
            dock.Children.Add(_view);
            Content = dock;

            // F5 reloads the models and plan here, as it did on the Create tab.
            InputBindings.Add(new KeyBinding(vm.RefreshAccountCommand, Key.F5, ModifierKeys.None));
            Loaded += (_, _) => _view.FocusMain();
        }
    }
}
