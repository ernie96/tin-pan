using System.Windows.Controls;
using System.Windows.Input;
using TinPan.ViewModels;

namespace TinPan.Views
{
    public partial class LibraryPanelView : UserControl
    {
        public LibraryPanelView()
        {
            InitializeComponent();
            ListFocus.KeepFocusWhenItemsGo(songList);
        }

        private LibraryPanelViewModel? ViewModel => DataContext as LibraryPanelViewModel;

        /// <summary>Focus on the selected song, or the list itself when it's empty.</summary>
        public void FocusMain() => ListFocus.FocusItem(songList);

        public void FocusSearch()
        {
            searchBox.Focus();
            searchBox.SelectAll();
        }

        // Handled here, before ListBox uses Space, Enter and arrows itself.
        private void SongList_PreviewKeyDown(object sender, KeyEventArgs e)
        {
            var vm = ViewModel;
            if (vm == null) return;
            var mods = Keyboard.Modifiers;
            ICommand? command = null;
            object? parameter = null;

            if (mods == ModifierKeys.Alt && e.Key == Key.System && e.SystemKey == Key.Enter)
            {
                command = vm.SongInfoCommand;
            }
            else if (mods == ModifierKeys.None)
            {
                switch (e.Key)
                {
                    case Key.Enter: command = vm.PlayCommand; break;
                    case Key.Space: command = vm.PlayPauseCommand; break;
                    case Key.Delete: command = vm.TrashCommand; break;
                    case Key.Left: command = vm.SeekCommand; parameter = "-5"; break;
                    case Key.Right: command = vm.SeekCommand; parameter = "5"; break;
                }
            }
            else if (mods == ModifierKeys.Control)
            {
                switch (e.Key)
                {
                    case Key.I: command = vm.DetailsCommand; break;
                    case Key.L: command = vm.LyricsCommand; break;
                    case Key.C: command = vm.CopyLinkCommand; break;
                    case Key.D: command = vm.DownloadCommand; break;
                    case Key.E: command = vm.ExtendCommand; break;
                }
            }

            if (command == null) return;
            if (command.CanExecute(parameter)) command.Execute(parameter);
            e.Handled = true;
        }

        private async void SearchBox_PreviewKeyDown(object sender, KeyEventArgs e)
        {
            var vm = ViewModel;
            if (vm == null) return;
            if (e.Key == Key.Enter)
            {
                e.Handled = true;
                await vm.SearchCommand.ExecuteAsync(null);
                ListFocus.FocusItem(songList);
            }
            else if (e.Key == Key.Escape && searchBox.Text.Length > 0)
            {
                vm.ClearSearchCommand.Execute(null);
                e.Handled = true;
            }
        }

        private void SongList_MouseDoubleClick(object sender, MouseButtonEventArgs e) => ViewModel?.PlayCommand.Execute(null);
    }
}
