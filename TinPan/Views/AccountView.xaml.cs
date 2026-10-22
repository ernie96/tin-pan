using System.Windows.Controls;

namespace TinPan.Views
{
    public partial class AccountView : UserControl
    {
        public AccountView() => InitializeComponent();

        /// <summary>Focus on the account summary, which is read first.</summary>
        public void FocusMain() => accountText.Focus();
    }
}
