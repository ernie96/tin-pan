using System.Windows.Controls;

namespace TinPan.Views
{
    public partial class CreateView : UserControl
    {
        public CreateView() => InitializeComponent();

        /// <summary>Focus on the first field, Model.</summary>
        public void FocusMain() => model.Focus();
    }
}
