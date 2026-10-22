using System;
using System.Collections.Generic;
using System.Threading.Tasks;
using System.Windows;
using Microsoft.Win32;
using TinPan.Views;

namespace TinPan.Services
{
    public interface IDialogService
    {
        /// <summary>Yes/No question; No is the default button.</summary>
        bool Confirm(string message, string title);

        /// <summary>Yes/No question where Yes is the default button (offering an update).</summary>
        bool Ask(string message, string title);

        /// <summary>Read-only text the user can arrow through: lyrics, details, help.</summary>
        void ShowDocument(string title, string text);

        /// <summary>A form of labelled fields. Returns false on Cancel; fields hold the new values on OK.</summary>
        bool ShowForm(string title, string intro, string okText, IReadOnlyList<FormField> fields);

        string? PickFolder(string? startIn);

        /// <summary>
        /// A list of short lines (message history, song information, the command list).
        /// When onEnter is given, Enter closes the window and runs it with the chosen line's index.
        /// </summary>
        void ShowLines(string title, IReadOnlyList<string> lines, Action<int>? onEnter = null);

        /// <summary>A list of songs; Enter plays the chosen one through the given action.</summary>
        void ShowSongs(string title, IReadOnlyList<Models.SunoClip> songs, Func<Models.SunoClip, Task> play);
    }

    public sealed class DialogService : IDialogService
    {
        public bool Confirm(string message, string title) =>
            MessageBox.Show(Application.Current.MainWindow, message, title,
                MessageBoxButton.YesNo, MessageBoxImage.Question, MessageBoxResult.No) == MessageBoxResult.Yes;

        public bool Ask(string message, string title) =>
            MessageBox.Show(Application.Current.MainWindow, message, title,
                MessageBoxButton.YesNo, MessageBoxImage.Information, MessageBoxResult.Yes) == MessageBoxResult.Yes;

        public void ShowDocument(string title, string text) =>
            new TextViewerWindow(title, text) { Owner = Application.Current.MainWindow }.ShowDialog();

        public bool ShowForm(string title, string intro, string okText, IReadOnlyList<FormField> fields) =>
            new FormWindow(title, intro, okText, fields) { Owner = Application.Current.MainWindow }.ShowDialog() == true;

        public void ShowLines(string title, IReadOnlyList<string> lines, Action<int>? onEnter = null)
        {
            var window = new LineListWindow(title, lines, onEnter != null) { Owner = Application.Current.MainWindow };
            if (window.ShowDialog() == true && window.ChosenIndex is { } index)
                Application.Current.Dispatcher.BeginInvoke(() => onEnter!(index));
        }

        public void ShowSongs(string title, IReadOnlyList<Models.SunoClip> songs, Func<Models.SunoClip, Task> play) =>
            new SongListWindow(title, songs, play) { Owner = Application.Current.MainWindow }.ShowDialog();

        public string? PickFolder(string? startIn)
        {
            var dialog = new OpenFolderDialog { Title = "Choose where songs are saved" };
            if (!string.IsNullOrEmpty(startIn)) dialog.InitialDirectory = startIn;
            return dialog.ShowDialog(Application.Current.MainWindow) == true ? dialog.FolderName : null;
        }
    }
}
