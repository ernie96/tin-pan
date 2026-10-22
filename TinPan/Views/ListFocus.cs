using System.Collections.Specialized;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Threading;

namespace TinPan.Views
{
    public static class ListFocus
    {
        /// <summary>
        /// When the focused song is removed (trash, remove from playlist) or the list is reloaded,
        /// WPF drops keyboard focus and the screen reader goes quiet. This puts it back on an item.
        /// </summary>
        public static void KeepFocusWhenItemsGo(ListBox list)
        {
            var hadFocus = false;
            list.PreviewLostKeyboardFocus += (_, _) => hadFocus = list.IsKeyboardFocusWithin;
            list.GotKeyboardFocus += (_, _) => hadFocus = true;
            ((INotifyCollectionChanged)list.Items).CollectionChanged += (_, e) =>
            {
                if (e.Action is not (NotifyCollectionChangedAction.Remove or NotifyCollectionChangedAction.Reset)) return;
                list.Dispatcher.BeginInvoke(DispatcherPriority.Loaded, () =>
                {
                    var focused = Keyboard.FocusedElement as DependencyObject;
                    var lost = focused == null || focused is Window || !list.IsKeyboardFocusWithin && focused == Window.GetWindow(list);
                    if (hadFocus && lost && list.IsVisible) FocusItem(list);
                });
            };
        }

        /// <summary>
        /// Puts keyboard focus on the selected item (or the first one) rather than on the list itself,
        /// so a screen reader reads a song instead of just "list". Waits for the items to be drawn.
        /// </summary>
        public static void FocusItem(ListBox list)
        {
            list.Dispatcher.BeginInvoke(DispatcherPriority.Loaded, () =>
            {
                if (list.Items.Count == 0)
                {
                    list.Focus();
                    return;
                }
                if (list.SelectedIndex < 0) list.SelectedIndex = 0;
                list.ScrollIntoView(list.SelectedItem);
                if (list.ItemContainerGenerator.ContainerFromIndex(list.SelectedIndex) is ListBoxItem item) item.Focus();
                else list.Focus();
            });
        }
    }
}
