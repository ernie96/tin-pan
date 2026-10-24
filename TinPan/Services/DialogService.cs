using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading.Tasks;
using System.Windows;
using Microsoft.Win32;
using TinPan.Views;
using static TinPan.Localization.Loc;

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

        /// <summary>
        /// A form that stays open beside the main window (F6 moves between them) until Close or Escape.
        /// Each OK runs the action with the fields' current values, so you can try again with a change.
        /// </summary>
        void ShowToolForm(string title, string intro, string okText, IReadOnlyList<FormField> fields, Func<Task> run);

        string? PickFolder(string? startIn);

        /// <summary>An MP3 or WAV file, or null on Cancel.</summary>
        string? PickAudioFile(string title);

        /// <summary>A song to upload: any common audio file. Null on Cancel.</summary>
        string? PickSongToUpload();

        /// <summary>
        /// A list of short lines (message history, song information, the command list).
        /// When onEnter is given, Enter closes the window and runs it with the chosen line's index.
        /// </summary>
        void ShowLines(string title, IReadOnlyList<string> lines, Action<int>? onEnter = null);

        /// <summary>A list of songs; Enter plays the chosen one through the given action. Each says who made it unless sayCreator is false.</summary>
        void ShowSongs(string title, IReadOnlyList<Models.SunoClip> songs, Func<Models.SunoClip, Task> play, bool sayCreator = true);
    }

    public sealed class DialogService : IDialogService
    {
        /// <summary>The window you are in, so a dialog opens over a Cover form or the Create window and goes back there.</summary>
        private static Window Parent =>
            Application.Current.Windows.OfType<Window>().FirstOrDefault(w => w.IsActive && w.IsVisible && w.ShowActivated) ?? Application.Current.MainWindow;

        public bool Confirm(string message, string title) =>
            MessageBox.Show(Parent, message, title,
                MessageBoxButton.YesNo, MessageBoxImage.Question, MessageBoxResult.No) == MessageBoxResult.Yes;

        public bool Ask(string message, string title) =>
            MessageBox.Show(Parent, message, title,
                MessageBoxButton.YesNo, MessageBoxImage.Information, MessageBoxResult.Yes) == MessageBoxResult.Yes;

        public void ShowDocument(string title, string text) =>
            new TextViewerWindow(title, text) { Owner = Parent }.ShowDialog();

        public void ShowToolForm(string title, string intro, string okText, IReadOnlyList<FormField> fields, Func<Task> run) =>
            ToolWindows.Show(new FormWindow(title, intro, okText, fields, run),
                T("{0}. Tab moves between the fields. {1} starts another with these settings, and this stays open to try again. F8 plays or pauses from here. Escape closes.", title, okText));

        public bool ShowForm(string title, string intro, string okText, IReadOnlyList<FormField> fields) =>
            new FormWindow(title, intro, okText, fields) { Owner = Parent }.ShowDialog() == true;

        public void ShowLines(string title, IReadOnlyList<string> lines, Action<int>? onEnter = null)
        {
            var window = new LineListWindow(title, lines, onEnter != null) { Owner = Parent };
            if (window.ShowDialog() == true && window.ChosenIndex is { } index)
                Application.Current.Dispatcher.BeginInvoke(() => onEnter!(index));
        }

        public void ShowSongs(string title, IReadOnlyList<Models.SunoClip> songs, Func<Models.SunoClip, Task> play, bool sayCreator = true) =>
            new SongListWindow(title, songs, play, sayCreator) { Owner = Parent }.ShowDialog();

        public string? PickFolder(string? startIn)
        {
            var dialog = new OpenFolderDialog { Title = T("Choose where songs are saved") };
            if (!string.IsNullOrEmpty(startIn)) dialog.InitialDirectory = startIn;
            return dialog.ShowDialog(Parent) == true ? dialog.FolderName : null;
        }

        public string? PickSongToUpload()
        {
            var dialog = new OpenFileDialog
            {
                Title = T("Choose a song to upload to Suno"),
                Filter = T("Audio files") + " (*.mp3;*.wav;*.m4a;*.flac;*.aac;*.wma)|*.mp3;*.wav;*.m4a;*.flac;*.aac;*.wma",
                CheckFileExists = true
            };
            return dialog.ShowDialog(Parent) == true ? dialog.FileName : null;
        }

        public string? PickAudioFile(string title)
        {
            var dialog = new OpenFileDialog { Title = title, Filter = T("MP3 or WAV songs") + " (*.mp3;*.wav)|*.mp3;*.wav", CheckFileExists = true };
            return dialog.ShowDialog(Parent) == true ? dialog.FileName : null;
        }
    }
}
