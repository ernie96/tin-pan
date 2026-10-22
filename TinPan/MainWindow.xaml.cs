using System.Linq;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Data;
using System.Windows.Input;
using System.Windows.Threading;
using TinPan.ViewModels;
using TinPan.Views;
using static TinPan.Localization.Loc;

namespace TinPan;

public partial class MainWindow : Window
{
    private bool _altAlone;
    private IInputElement? _focusBeforeMenu;

    public MainWindow()
    {
        InitializeComponent();
        DataContextChanged += (_, _) =>
        {
            if (DataContext is not MainViewModel vm) return;
            BuildMenu(vm);
            libraryView.songList.ContextMenu = BuildSongContextMenu(vm, playlistItems: false);
            playlistsView.songList.ContextMenu = BuildSongContextMenu(vm, playlistItems: true);
            trendingView.trending.ContextMenu = BuildSongContextMenu(vm, playlistItems: false);
            vm.SearchRequested += (_, _) => Dispatcher.BeginInvoke(DispatcherPriority.Input, () => libraryView.FocusSearch());
        };
        mainMenu.IsKeyboardFocusWithinChanged += (_, _) => Dispatcher.BeginInvoke(DispatcherPriority.Input, HideMenuIfLeft);
        // Choosing a command closes the bar and puts focus back first, so the command starts from where you were.
        mainMenu.AddHandler(MenuItem.ClickEvent, new RoutedEventHandler((_, e) =>
        {
            if (e.OriginalSource is MenuItem { HasItems: false }) HideMenu(restoreFocus: true);
        }));
        Deactivated += (_, _) => _altAlone = false;
        // Start where the work is: on a song, not on nothing.
        Loaded += (_, _) => libraryView.FocusMain();
    }

    /// <summary>
    /// One top-level menu per group. Top-level headers have no underlined letter, because the pages
    /// already use most letters; Alt or F10 opens the bar and the arrows move along it. Items inside
    /// each menu keep their letters, which only apply while that menu is open.
    /// </summary>
    private void BuildMenu(MainViewModel vm)
    {
        mainMenu.Items.Clear();
        foreach (var group in vm.Commands.GroupBy(c => c.Group))
        {
            var top = new MenuItem { Header = MainViewModel.GroupLabel(group.Key) };
            foreach (var entry in group) top.Items.Add(MenuItemFor(vm, entry));
            // Song commands act on the song selected in the tab you're on; decided as the menu opens.
            top.SubmenuOpened += (_, _) => UpdateSongParameters(vm, top);
            mainMenu.Items.Add(top);
        }
    }

    private static MenuItem MenuItemFor(MainViewModel vm, CommandEntry entry)
    {
        var item = new MenuItem
        {
            Header = entry.Label,
            InputGestureText = entry.Keys,
            Command = entry.Command,
            CommandParameter = entry.Parameter,
            Tag = entry
        };
        if (entry.HeaderPath != null)
            item.SetBinding(HeaderedItemsControl.HeaderProperty, new Binding(entry.HeaderPath) { Source = vm.LibraryVM });
        return item;
    }

    private static void UpdateSongParameters(MainViewModel vm, ItemsControl menu)
    {
        foreach (var item in menu.Items.OfType<MenuItem>())
            if (item.Tag is CommandEntry entry) item.CommandParameter = vm.SongParameter(entry);
    }

    /// <summary>
    /// The Applications key menu for the Library, a playlist's songs and Trending: the Song menu's
    /// commands for that song, with the playlist's own Remove and Move first.
    /// </summary>
    private static ContextMenu BuildSongContextMenu(MainViewModel vm, bool playlistItems)
    {
        var menu = new ContextMenu();
        if (playlistItems)
        {
            var p = vm.PlaylistsVM;
            menu.Items.Add(new MenuItem { Header = T("Remove from playlist"), InputGestureText = "Delete", Command = p.RemoveSongCommand });
            menu.Items.Add(new MenuItem { Header = T("Move up"), InputGestureText = "Ctrl+Up", Command = p.MoveUpCommand });
            menu.Items.Add(new MenuItem { Header = T("Move down"), InputGestureText = "Ctrl+Down", Command = p.MoveDownCommand });
            menu.Items.Add(new Separator());
        }
        foreach (var entry in vm.Commands.Where(c => c.Group == "Song")) menu.Items.Add(MenuItemFor(vm, entry));
        menu.Opened += (_, _) => UpdateSongParameters(vm, menu);
        return menu;
    }

    // ── The hidden menu bar ─────────────────────────────────────────────────

    /// <summary>Alt pressed and released on its own, or F10, opens the menu bar; Alt with another key is an access key.</summary>
    protected override void OnPreviewKeyDown(KeyEventArgs e)
    {
        var key = e.Key == Key.System ? e.SystemKey : e.Key;
        if (key is Key.LeftAlt or Key.RightAlt)
        {
            if (!e.IsRepeat) _altAlone = Keyboard.Modifiers == ModifierKeys.Alt;
        }
        else
        {
            _altAlone = false;
            // Escape closes an open menu first (WPF does that); with none open it leaves the bar.
            if (key == Key.Escape && mainMenu.IsKeyboardFocusWithin && !mainMenu.Items.OfType<MenuItem>().Any(m => m.IsSubmenuOpen))
            {
                e.Handled = true;
                HideMenu(restoreFocus: true);
                return;
            }
            if (key == Key.F10 && Keyboard.Modifiers == ModifierKeys.None && !mainMenu.IsKeyboardFocusWithin)
            {
                e.Handled = true;
                ShowMenu();
                return;
            }
        }
        base.OnPreviewKeyDown(e);
    }

    protected override void OnPreviewKeyUp(KeyEventArgs e)
    {
        var key = e.Key == Key.System ? e.SystemKey : e.Key;
        if (key is Key.LeftAlt or Key.RightAlt && _altAlone)
        {
            _altAlone = false;
            e.Handled = true;
            if (mainMenu.IsKeyboardFocusWithin) HideMenu(restoreFocus: true);
            else ShowMenu();
            return;
        }
        base.OnPreviewKeyUp(e);
    }

    private void ShowMenu()
    {
        _focusBeforeMenu = Keyboard.FocusedElement;
        mainMenu.Visibility = Visibility.Visible;
        // After this key press is fully handled, so WPF's own Alt handling can't undo it.
        Dispatcher.BeginInvoke(DispatcherPriority.Input, () =>
        {
            mainMenu.UpdateLayout();
            if (mainMenu.Items.Count > 0 && mainMenu.ItemContainerGenerator.ContainerFromIndex(0) is MenuItem first)
                first.Focus();
        });
    }

    private void HideMenuIfLeft()
    {
        if (mainMenu.Visibility != Visibility.Visible || mainMenu.IsKeyboardFocusWithin) return;
        if (mainMenu.Items.OfType<MenuItem>().Any(m => m.IsSubmenuOpen)) return;
        HideMenu(restoreFocus: false);
    }

    private void HideMenu(bool restoreFocus)
    {
        mainMenu.Visibility = Visibility.Collapsed;
        foreach (var m in mainMenu.Items.OfType<MenuItem>()) m.IsSubmenuOpen = false;
        // Back to where you were, unless a command already moved focus (a dialog, another tab).
        var focused = Keyboard.FocusedElement as DependencyObject;
        var lost = focused == null || focused == this || focused is MenuItem;
        if ((restoreFocus || lost) && _focusBeforeMenu is UIElement { IsVisible: true } before) before.Focus();
        else if (lost) FocusPage();
        _focusBeforeMenu = null;
    }

    // ── Pages ───────────────────────────────────────────────────────────────

    /// <summary>
    /// Ctrl+1 to Ctrl+6 (or a menu) changed the page while focus was on the old one, which is now
    /// hidden; put focus on the new page's main list instead of leaving it nowhere.
    /// Arrowing along the tab headers keeps focus on the headers.
    /// </summary>
    private void MainTabs_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (!ReferenceEquals(e.OriginalSource, mainTabs)) return;
        Dispatcher.BeginInvoke(DispatcherPriority.Loaded, () =>
        {
            if (Keyboard.FocusedElement is TabItem tab && ReferenceEquals(ItemsControl.ItemsControlFromItemContainer(tab), mainTabs)) return;
            if (mainMenu.IsKeyboardFocusWithin) return;
            FocusPage();
        });
    }

    private void FocusPage()
    {
        switch (mainTabs.SelectedIndex)
        {
            case MainViewModel.LibraryTab: libraryView.FocusMain(); break;
            case MainViewModel.PlaylistsTab: playlistsView.FocusPlaylists(); break;
            case MainViewModel.CreateTab: createView.FocusMain(); break;
            case MainViewModel.TrendingTab: trendingView.FocusMain(); break;
            case MainViewModel.AccountTab: accountView.FocusMain(); break;
            case MainViewModel.SettingsTab: settingsView.MoveFocus(new TraversalRequest(FocusNavigationDirection.First)); break;
        }
    }

    /// <summary>
    /// Escape steps back one level, like KeySoft: out of a playlist's songs to the playlist list,
    /// otherwise to the tab headers. Dialogs and the search box handle their own Escape first.
    /// </summary>
    protected override void OnKeyDown(KeyEventArgs e)
    {
        if (e.Key == Key.Escape && Keyboard.Modifiers == ModifierKeys.None && !e.Handled)
        {
            if (playlistsView.IsKeyboardFocusWithin && playlistsView.SongsHaveFocus)
            {
                playlistsView.FocusPlaylists();
                e.Handled = true;
                return;
            }
            if (mainTabs.ItemContainerGenerator.ContainerFromIndex(mainTabs.SelectedIndex) is TabItem tab && !tab.IsKeyboardFocused)
            {
                tab.Focus();
                e.Handled = true;
                return;
            }
        }
        base.OnKeyDown(e);
    }
}
