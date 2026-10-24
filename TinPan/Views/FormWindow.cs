using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Input;
using static TinPan.Localization.Loc;

namespace TinPan.Views
{
    /// <summary>One labelled control in a FormWindow. Put "_" before the access key letter in the label.</summary>
    public abstract class FormField
    {
        protected FormField(string label) => Label = label;

        public string Label { get; }

        internal string SpokenLabel => Label.Replace("_", "");

        internal abstract Control Build();

        /// <summary>A short sentence about what's wrong, or null when the value is fine.</summary>
        internal virtual string? Validate() => null;

        internal abstract void Commit();
    }

    public sealed class TextField : FormField
    {
        private TextBox? _box;

        public TextField(string label, string value = "", bool multiline = false, int maxLength = 0) : base(label)
        {
            Value = value;
            Multiline = multiline;
            MaxLength = maxLength;
        }

        public string Value { get; set; }
        public bool Multiline { get; }
        public int MaxLength { get; }

        internal override Control Build()
        {
            _box = new TextBox { Text = Value };
            if (Multiline)
            {
                _box.AcceptsReturn = true;
                _box.TextWrapping = TextWrapping.Wrap;
                _box.MinHeight = 120;
                _box.VerticalScrollBarVisibility = ScrollBarVisibility.Auto;
            }
            return _box;
        }

        internal override string? Validate()
        {
            var length = _box!.Text.Trim().Length;
            return MaxLength > 0 && length > MaxLength
                ? T("{0} is {1} characters. The limit is {2}.", SpokenLabel, length, MaxLength)
                : null;
        }

        internal override void Commit() => Value = _box!.Text.Trim();

        /// <summary>What the box holds right now, before OK; setting it changes the box.</summary>
        public string Current
        {
            get => _box?.Text ?? Value;
            set
            {
                if (_box != null) _box.Text = value;
                else Value = value;
            }
        }
    }

    /// <summary>A button inside a form that does something with the other fields (Style boost, say). It never closes the form.</summary>
    public sealed class ButtonField : FormField
    {
        private readonly Func<Task> _click;

        public ButtonField(string label, Func<Task> click) : base(label) => _click = click;

        internal override Control Build()
        {
            var button = new Button { Content = Label, HorizontalAlignment = HorizontalAlignment.Left, Padding = new Thickness(10, 4, 10, 4) };
            button.Click += async (_, _) =>
            {
                button.IsEnabled = false;
                try { await _click(); }
                finally
                {
                    button.IsEnabled = true;
                    if (Keyboard.FocusedElement == null) button.Focus();
                }
            };
            return button;
        }

        internal override void Commit() { }
    }

    /// <summary>A number typed into a box. Empty means "let Suno choose" when AllowEmpty. Times accept m:ss.</summary>
    public sealed class NumberField : FormField
    {
        private TextBox? _box;

        public NumberField(string label, double? value, double min, double max, bool allowEmpty = true, bool isTime = false) : base(label)
        {
            Value = value;
            Min = min;
            Max = max;
            AllowEmpty = allowEmpty;
            IsTime = isTime;
        }

        public double? Value { get; set; }
        public double Min { get; }
        public double Max { get; }
        public bool AllowEmpty { get; }
        public bool IsTime { get; }

        internal override Control Build()
        {
            var text = Value == null ? "" : IsTime ? FormatTime(Value.Value) : Value.Value.ToString("0.##", CultureInfo.CurrentCulture);
            _box = new TextBox { Text = text, MaxWidth = 200, HorizontalAlignment = HorizontalAlignment.Left, MinWidth = 120 };
            return _box;
        }

        internal override string? Validate()
        {
            var text = _box!.Text.Trim();
            if (text.Length == 0) return AllowEmpty ? null : T("{0} is needed.", SpokenLabel);
            var parsed = Parse(text, IsTime);
            if (parsed == null) return IsTime ? T("{0}: type seconds, or minutes and seconds like 2:30.", SpokenLabel) : T("{0} must be a number.", SpokenLabel);
            if (parsed < Min || parsed > Max)
                return IsTime ? T("{0} must be between {1} and {2}.", SpokenLabel, FormatTime(Min), FormatTime(Max)) : T("{0} must be from {1} to {2}.", SpokenLabel, Min, Max);
            return null;
        }

        internal override void Commit()
        {
            var text = _box!.Text.Trim();
            Value = text.Length == 0 ? null : Parse(text, IsTime);
        }

        public static double? Parse(string text, bool isTime)
        {
            if (isTime && text.Contains(':'))
            {
                var parts = text.Split(':');
                if (parts.Length != 2 || !int.TryParse(parts[0], out var m) ||
                    !double.TryParse(parts[1], NumberStyles.Float, CultureInfo.InvariantCulture, out var s) || m < 0 || s < 0 || s >= 60)
                    return null;
                return m * 60 + s;
            }
            return double.TryParse(text, NumberStyles.Float, CultureInfo.CurrentCulture, out var v) ||
                   double.TryParse(text, NumberStyles.Float, CultureInfo.InvariantCulture, out v)
                ? v
                : null;
        }

        // Total minutes, so an hour reads 60:00 rather than 0:00.
        private static string FormatTime(double seconds)
        {
            var t = TimeSpan.FromSeconds(seconds);
            return $"{(int)t.TotalMinutes}:{t.Seconds:00}";
        }
    }

    public sealed class ChoiceField : FormField
    {
        private ComboBox? _combo;

        public ChoiceField(string label, IEnumerable<object> items, object? selected) : base(label)
        {
            Items = items.ToList();
            Selected = selected ?? Items.FirstOrDefault();
        }

        public IReadOnlyList<object> Items { get; }
        public object? Selected { get; set; }

        internal override Control Build()
        {
            _combo = new ComboBox { ItemsSource = Items, SelectedItem = Selected, MinWidth = 200, HorizontalAlignment = HorizontalAlignment.Left };
            return _combo;
        }

        internal override void Commit() => Selected = _combo!.SelectedItem;
    }

    public sealed class CheckField : FormField
    {
        private CheckBox? _check;

        public CheckField(string label, bool value) : base(label) => Value = value;

        public bool Value { get; set; }

        /// <summary>Whether it's ticked right now, before OK.</summary>
        public bool Current => _check?.IsChecked ?? Value;

        internal override Control Build()
        {
            _check = new CheckBox { Content = Label, IsChecked = Value };
            return _check;
        }

        internal override void Commit() => Value = _check!.IsChecked == true;
    }

    /// <summary>
    /// A plain dialog built from fields: label above each control, OK and Cancel at the bottom.
    /// Focus starts on the first field; Escape cancels; problems are named before closing.
    /// </summary>
    public sealed class FormWindow : Window
    {
        private readonly IReadOnlyList<FormField> _fields;
        private readonly List<Control> _controls = new();
        private readonly Func<Task>? _run;
        private Button? _ok;
        private int _running;

        /// <summary>Suno makes up to five songs at once, so OK only waits once five are still being started.</summary>
        public const int MaxAtOnce = 5;

        /// <param name="run">
        /// Null for a dialog that closes on OK. Otherwise the window stays open (see ToolWindows): OK runs this
        /// with the fields' new values and the form keeps them, so you can change one thing and run it again.
        /// </param>
        public FormWindow(string title, string intro, string okText, IReadOnlyList<FormField> fields, Func<Task>? run = null)
        {
            _fields = fields;
            _run = run;
            Title = title;
            Width = 560;
            SizeToContent = SizeToContent.Height;
            MaxHeight = SystemParameters.WorkArea.Height * 0.9;
            WindowStartupLocation = WindowStartupLocation.CenterOwner;
            ResizeMode = ResizeMode.CanResizeWithGrip;
            ShowInTaskbar = false;

            var panel = new StackPanel { Margin = new Thickness(12) };
            if (!string.IsNullOrWhiteSpace(intro))
            {
                // A read-only box, so the explanation can be reached and reread with the keyboard.
                var introBox = new TextBox
                {
                    Text = intro, IsReadOnly = true, TextWrapping = TextWrapping.Wrap, BorderThickness = new Thickness(0),
                    Margin = new Thickness(0, 0, 0, 10)
                };
                AutomationProperties.SetName(introBox, T("About this"));
                panel.Children.Add(introBox);
                _controls.Add(introBox);
            }

            foreach (var field in fields)
            {
                var control = field.Build();
                AutomationProperties.SetName(control, field.SpokenLabel);
                if (field is not CheckField and not ButtonField)
                    panel.Children.Add(new Label { Content = field.Label, Target = control, Padding = new Thickness(0, 8, 0, 3) });
                else
                    control.Margin = new Thickness(0, 10, 0, 0);
                panel.Children.Add(control);
                _controls.Add(control);
            }

            var buttons = new StackPanel { Orientation = Orientation.Horizontal, HorizontalAlignment = HorizontalAlignment.Right, Margin = new Thickness(0, 15, 0, 0) };
            var ok = _ok = new Button { Content = okText, IsDefault = true, Padding = new Thickness(15, 5, 15, 5), MinWidth = 80 };
            var cancel = new Button { Content = run == null ? T("Cancel") : T("Close"), IsCancel = run == null, Padding = new Thickness(15, 5, 15, 5), MinWidth = 80, Margin = new Thickness(10, 0, 0, 0) };
            if (run != null) cancel.Click += (_, _) => Close();
            ok.Click += async (_, _) => await AcceptAsync();
            buttons.Children.Add(ok);
            buttons.Children.Add(cancel);
            panel.Children.Add(buttons);

            Content = new ScrollViewer { Content = panel, VerticalScrollBarVisibility = ScrollBarVisibility.Auto };
            var first = _controls.Skip(string.IsNullOrWhiteSpace(intro) ? 0 : 1).FirstOrDefault();
            if (first != null && !string.IsNullOrWhiteSpace(intro)) AutomationProperties.SetHelpText(first, intro);
            Loaded += (_, _) => (first ?? (Control)ok).Focus();
        }

        private async Task AcceptAsync()
        {
            for (var i = 0; i < _fields.Count; i++)
            {
                var problem = _fields[i].Validate();
                if (problem == null) continue;
                MessageBox.Show(this, problem, Title, MessageBoxButton.OK, MessageBoxImage.Warning);
                var index = _controls.Count - _fields.Count + i;
                _controls[index].Focus();
                if (_controls[index] is TextBox box) box.SelectAll();
                return;
            }
            foreach (var field in _fields) field.Commit();
            if (_run == null)
            {
                DialogResult = true;
                return;
            }
            // Each press starts another (and spends its credits); only a sixth waits for one of the five to be taken.
            _running++;
            if (_running >= MaxAtOnce) _ok!.IsEnabled = false;
            try { await _run(); }
            finally
            {
                _running--;
                if (!_ok!.IsEnabled && _running < MaxAtOnce)
                {
                    _ok.IsEnabled = true;
                    if (!IsKeyboardFocusWithin || Keyboard.FocusedElement == null) _ok.Focus();
                }
            }
        }
    }

    /// <summary>Read-only text in a window: lyrics, song details, the key list. Escape closes it.</summary>
    public sealed class TextViewerWindow : Window
    {
        public TextViewerWindow(string title, string text)
        {
            Title = title;
            Width = 640;
            Height = 520;
            WindowStartupLocation = WindowStartupLocation.CenterOwner;
            ShowInTaskbar = false;

            var box = new TextBox
            {
                Text = text, IsReadOnly = true, TextWrapping = TextWrapping.Wrap, AcceptsReturn = true,
                VerticalScrollBarVisibility = ScrollBarVisibility.Auto, Margin = new Thickness(10), IsReadOnlyCaretVisible = true
            };
            AutomationProperties.SetName(box, title);
            var close = new Button { Content = T("Close"), IsCancel = true, Padding = new Thickness(15, 5, 15, 5), Margin = new Thickness(10), HorizontalAlignment = HorizontalAlignment.Right };

            var dock = new DockPanel();
            DockPanel.SetDock(close, Dock.Bottom);
            dock.Children.Add(close);
            dock.Children.Add(box);
            Content = dock;

            Loaded += (_, _) =>
            {
                box.Focus();
                box.CaretIndex = 0;
            };
            PreviewKeyDown += (_, e) =>
            {
                if (e.Key == Key.Escape) Close();
            };
        }
    }
}
