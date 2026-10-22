using System;
using System.Windows.Markup;

namespace TinPan.Localization
{
    /// <summary>
    /// Translated text in XAML: <c>Content="{l:T 'Sign _in'}"</c>. The language is chosen at
    /// startup, so the text is looked up once when the window loads.
    /// </summary>
    [MarkupExtensionReturnType(typeof(string))]
    public sealed class TExtension : MarkupExtension
    {
        public TExtension()
        {
        }

        public TExtension(string text) => Text = text;

        [ConstructorArgument("text")]
        public string Text { get; set; } = "";

        public override object ProvideValue(IServiceProvider serviceProvider) => Loc.T(Text);
    }
}
