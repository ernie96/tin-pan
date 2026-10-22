using System.Windows.Controls;
using System.Windows.Input;
using TinPan.ViewModels;

namespace TinPan.Views
{
    public partial class TrendingView : UserControl
    {
        public TrendingView()
        {
            InitializeComponent();
            ListFocus.KeepFocusWhenItemsGo(trending);
            // Opened before the songs arrived: once they do, move from the empty list onto the first one.
            ((System.Collections.Specialized.INotifyCollectionChanged)trending.Items).CollectionChanged += (_, _) =>
            {
                if (trending.IsKeyboardFocused && trending.Items.Count > 0) ListFocus.FocusItem(trending);
            };
        }

        private SunoPanelViewModel? ViewModel => DataContext as SunoPanelViewModel;

        public void FocusMain() => ListFocus.FocusItem(trending);

        private void Trending_PreviewKeyDown(object sender, KeyEventArgs e)
        {
            if (e.Key is Key.Enter or Key.Space && Keyboard.Modifiers == ModifierKeys.None && ViewModel != null)
            {
                ViewModel.PlayTrendingCommand.Execute(null);
                e.Handled = true;
            }
        }

        private void Trending_MouseDoubleClick(object sender, MouseButtonEventArgs e) => ViewModel?.PlayTrendingCommand.Execute(null);
    }
}
