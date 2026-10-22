using System.Windows.Controls;
using System.Windows.Input;
using TinPan.ViewModels;

namespace TinPan.Views
{
    public partial class PlaylistsView : UserControl
    {
        public PlaylistsView()
        {
            InitializeComponent();
            ListFocus.KeepFocusWhenItemsGo(playlistList);
            ListFocus.KeepFocusWhenItemsGo(songList);
        }

        private PlaylistsViewModel? ViewModel => DataContext as PlaylistsViewModel;

        public bool SongsHaveFocus => songList.IsKeyboardFocusWithin;

        public void FocusPlaylists() => ListFocus.FocusItem(playlistList);

        // Enter on a playlist opens it and moves to its songs.
        private async void Playlists_PreviewKeyDown(object sender, KeyEventArgs e)
        {
            if (ViewModel == null || Keyboard.Modifiers != ModifierKeys.None) return;
            if (e.Key == Key.Enter)
            {
                e.Handled = true;
                await ViewModel.OpenCommand.ExecuteAsync(null);
                ListFocus.FocusItem(songList);
            }
            else if (e.Key == Key.Delete)
            {
                ViewModel.DeleteCommand.Execute(null);
                e.Handled = true;
            }
        }

        private void Songs_PreviewKeyDown(object sender, KeyEventArgs e)
        {
            var vm = ViewModel;
            if (vm == null) return;
            ICommand? command = (Keyboard.Modifiers, e.Key) switch
            {
                (ModifierKeys.None, Key.Enter) => vm.PlayCommand,
                (ModifierKeys.None, Key.Space) => vm.PlayPauseCommand,
                (ModifierKeys.None, Key.Delete) => vm.RemoveSongCommand,
                (ModifierKeys.Control, Key.Up) => vm.MoveUpCommand,
                (ModifierKeys.Control, Key.Down) => vm.MoveDownCommand,
                _ => null
            };
            if (command == null) return;
            if (command.CanExecute(null)) command.Execute(null);
            e.Handled = true;
        }

        private void Songs_MouseDoubleClick(object sender, MouseButtonEventArgs e) => ViewModel?.PlayCommand.Execute(null);
    }
}
