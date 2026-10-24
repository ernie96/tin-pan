using System;
using System.Collections.Generic;
using System.Linq;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Data;
using System.Windows.Input;
using System.Windows.Threading;
using TinPan.Models;
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
            // The Library list handles its own song keys; these lists get the same ones from the Song menu.
            AddSongKeys(vm, playlistsView.songList);
            AddSongKeys(vm, trendingView.trending);
            // Song lists in their own window (similar songs, a creator's songs, Explore) get the Song menu too.
            SongListWindow.AddSongMenu = (list, selected) =>
            {
                list.ContextMenu = BuildSongContextMenu(vm, playlistItems: false, selected);
                AddSongKeys(vm, list, selected);
            };
            vm.SearchRequested += (_, _) => Dispatcher.BeginInvoke(DispatcherPriority.Input, () => libraryView.FocusSearch());
            vm.CreateRequested += (_, _) =>
            {
                if (ToolWindows.Find<CreateWindow>(_ => true) is { } open) open.Activate();
                else ToolWindows.Show(new CreateWindow(vm.SunoVM), HelpText.Create);
            };
            ToolWindows.Say = text => vm.Say(text);
            ToolWindows.AddMainKeys = AddMainKeys;
        };
        mainMenu.IsKeyboardFocusWithinChanged += (_, _) => Dispatcher.BeginInvoke(DispatcherPriority.Input, HideMenuIfLeft);
        // Choosing a command closes the bar and puts focus back first, so the command starts from where you were.
        mainMenu.AddHandler(MenuItem.ClickEvent, new RoutedEventHandler((_, e) =>
        {
            if (e.OriginalSource is MenuItem { HasItems: false }) HideMenu(restoreFocus: true);
        }));
        Deactivated += (_, _) => _altAlone = false;
        // Start where the work is: on a song, not on nothing.
        Loaded += (_, _) =>
        {
            libraryView.FocusMain();
            ToolWindows.WatchFocus(this);
        };
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
            AddEntries(vm, top, group);
            // Song commands act on the song selected in the tab you're on; decided as the menu opens.
            top.SubmenuOpened += (_, e) => { if (ReferenceEquals(e.OriginalSource, top)) UpdateSongItems(vm, top); };
            mainMenu.Items.Add(top);
        }
    }

    /// <summary>
    /// The main window's keys in a tool window, so you can play, pause, skip and switch output while you
    /// fine-tune. Not the ones that move around the main window (Ctrl+1 to Ctrl+5, Ctrl+F), F1 (the tool
    /// window says its own help) or keys the tool window already has (F5 in Create).
    /// </summary>
    private void AddMainKeys(Window window)
    {
        foreach (var binding in InputBindings.OfType<KeyBinding>())
        {
            if (binding.Gesture is not KeyGesture gesture || binding.Command == null) continue;
            if (gesture.Key == Key.F1 || gesture.Key == Key.F6 || (gesture.Modifiers == ModifierKeys.Control && gesture.Key is >= Key.D1 and <= Key.D9 or Key.F)) continue;
            if (window.InputBindings.OfType<KeyBinding>().Any(b => b.Gesture is KeyGesture g && g.Key == gesture.Key && g.Modifiers == gesture.Modifiers)) continue;
            window.InputBindings.Add(new KeyBinding(binding.Command, gesture) { CommandParameter = binding.CommandParameter });
        }
    }

    /// <summary>Adds entries in order, with a line where one starts a new section and submenus where they ask for one.</summary>
    private static void AddEntries(MainViewModel vm, ItemsControl menu, IEnumerable<CommandEntry> entries)
    {
        var submenus = new Dictionary<string, MenuItem>();
        foreach (var entry in entries)
        {
            var parent = menu;
            if (entry.Submenu != null)
            {
                if (!submenus.TryGetValue(entry.Submenu, out var sub))
                {
                    if (entry.SeparatorBefore && menu.Items.Count > 0) menu.Items.Add(new Separator());
                    sub = new MenuItem { Header = MainViewModel.SubmenuLabel(entry.Submenu) };
                    submenus[entry.Submenu] = sub;
                    menu.Items.Add(sub);
                }
                parent = sub;
            }
            else if (entry.SeparatorBefore && menu.Items.Count > 0) menu.Items.Add(new Separator());
            parent.Items.Add(MenuItemFor(vm, entry));
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

    /// <summary>
    /// Points every song entry, in submenus too, at the song the menu acts on, and fits the labels that
    /// depend on it. On the Playlists tab Delete removes from the playlist, so Move to trash shows no key there.
    /// </summary>
    private static void UpdateSongItems(MainViewModel vm, ItemsControl menu, bool inPlaylist = false, Func<SunoClip?>? selected = null)
    {
        var song = selected != null ? selected() : vm.CurrentSong;
        inPlaylist |= selected == null && vm.SelectedTab == MainViewModel.PlaylistsTab;
        foreach (var item in menu.Items.OfType<MenuItem>())
        {
            if (item.Tag is CommandEntry entry)
            {
                item.CommandParameter = ParameterFor(vm, entry, selected);
                if (vm.SongLabel(entry, song) is { } label) item.Header = label;
                if (entry.Keys == "Delete") item.InputGestureText = inPlaylist || selected != null ? "" : entry.Keys;
            }
            else if (item.HasItems) UpdateSongItems(vm, item, inPlaylist, selected);
        }
    }

    /// <summary>The song a Song entry acts on: the given list's selection, or else the tab's song.</summary>
    private static object? ParameterFor(MainViewModel vm, CommandEntry entry, Func<SunoClip?>? selected) =>
        selected != null && entry.Group == "Song" && entry.Parameter == null ? selected() : vm.SongParameter(entry);

    /// <summary>
    /// The Song menu's shortcuts (Ctrl+I, Alt+Enter, Ctrl+L, Ctrl+C, Ctrl+D, Ctrl+E) on a list other than the
    /// Library's, acting on its selected song. Enter and Delete keep the list's own meaning.
    /// </summary>
    private static void AddSongKeys(MainViewModel vm, ListBox list, Func<SunoClip?>? selected = null)
    {
        list.PreviewKeyDown += (_, e) =>
        {
            var key = e.Key == Key.System ? e.SystemKey : e.Key;
            var mods = Keyboard.Modifiers;
            if (mods == ModifierKeys.None) return;
            var gesture = (mods.HasFlag(ModifierKeys.Control) ? "Ctrl+" : "") + (mods.HasFlag(ModifierKeys.Shift) ? "Shift+" : "")
                          + (mods.HasFlag(ModifierKeys.Alt) ? "Alt+" : "") + (key == Key.Return ? "Enter" : key.ToString());
            var entry = vm.Commands.FirstOrDefault(c => c.Group == "Song" && c.Keys == gesture);
            if (entry == null) return;
            var parameter = ParameterFor(vm, entry, selected);
            if (entry.Command.CanExecute(parameter)) entry.Command.Execute(parameter);
            e.Handled = true;
        };
    }

    /// <summary>
    /// The Applications key menu for the Library, a playlist's songs and Trending: the Song menu's
    /// commands for that song, with the playlist's own Remove and Move first.
    /// </summary>
    private static ContextMenu BuildSongContextMenu(MainViewModel vm, bool playlistItems, Func<SunoClip?>? selected = null)
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
        // A window's own list plays with Enter itself; the Song menu's Play is for the tab's song.
        AddEntries(vm, menu, vm.Commands.Where(c => c.Group == "Song" && (selected == null || c.Command != vm.PlaySongCommand)));
        menu.Opened += (_, _) => UpdateSongItems(vm, menu, playlistItems, selected);
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
    /// Ctrl+1 to Ctrl+5 (or a menu) changed the page while focus was on the old one, which is now
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
