using System;
using System.IO;
using System.Linq;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Threading;
using Microsoft.Web.WebView2.Core;
using TinPan.Services;
using static TinPan.Localization.Loc;

namespace TinPan.Views
{
    /// <summary>
    /// Shows suno.com's own sign-in page. Once the browser holds a signed-in Clerk "__client" cookie,
    /// the cookies are handed to the app and the window closes by itself.
    /// </summary>
    public partial class LoginWindow : Window
    {
        // __client lives on the Clerk domain, __session and ajs_anonymous_id on suno.com.
        // Later sites win a name clash. This order is the one sign-in worked with; keep it.
        private static readonly string[] CookieSites = { "https://suno.com", "https://auth.suno.com", "https://clerk.suno.com" };

        private readonly Func<string, Task<bool>> _tryAccept;
        private readonly bool _clearCookies;
        private readonly DispatcherTimer _pollTimer = new() { Interval = TimeSpan.FromMilliseconds(1500) };
        private string? _lastTried;
        private bool _busy;
        private bool _loggedMissingClient;

        public LoginWindow(Func<string, Task<bool>> tryAccept, bool clearCookies)
        {
            _tryAccept = tryAccept;
            _clearCookies = clearCookies;
            InitializeComponent();
            _pollTimer.Tick += async (_, _) => await PollAsync();
            Loaded += async (_, _) => await InitializeAsync();
        }

        private async Task InitializeAsync()
        {
            try
            {
                await webView.EnsureCoreWebView2Async(await WebViewEnvironment.GetAsync());
            }
            catch (WebView2RuntimeNotFoundException)
            {
                MessageBox.Show(this, T("Signing in needs the Microsoft Edge WebView2 Runtime. Install it from Microsoft, then try again."),
                    T("Sign in to Suno"), MessageBoxButton.OK, MessageBoxImage.Error);
                Close();
                return;
            }

            // Let the browser offer to save Google and Suno passwords and fill them in next time,
            // like Edge does. Both are off by default in WebView2.
            webView.CoreWebView2.Settings.IsPasswordAutosaveEnabled = true;
            webView.CoreWebView2.Settings.IsGeneralAutofillEnabled = true;

            if (_clearCookies) await WebViewEnvironment.ForgetSunoAsync(webView.CoreWebView2.CookieManager);

            webView.CoreWebView2.Navigate("https://suno.com/sign-in");
            webView.Focus();
            _pollTimer.Start();
        }

        private async Task PollAsync()
        {
            if (_busy || webView.CoreWebView2 == null) return;
            _busy = true;
            try
            {
                var jar = new SunoCookies();
                foreach (var site in CookieSites)
                {
                    foreach (var cookie in await webView.CoreWebView2.CookieManager.GetCookiesAsync(site))
                        jar.Set(cookie.Name, cookie.Value);
                }

                if (!jar.HasClient)
                {
                    if (jar.Get("__session") != null && !_loggedMissingClient)
                    {
                        _loggedMissingClient = true;
                        Log.Warn($"Signed in on suno.com but no __client cookie yet. Cookie names: {string.Join(", ", jar.Names.OrderBy(n => n))}");
                    }
                    return;
                }

                var header = jar.ToHeader();
                if (header == _lastTried) return;
                _lastTried = header;

                if (await _tryAccept(header))
                {
                    _pollTimer.Stop();
                    DialogResult = true;
                }
            }
            catch (Exception ex) when (ex is InvalidOperationException or System.Runtime.InteropServices.COMException)
            {
                Log.Error("Reading sign-in cookies failed", ex);
            }
            finally
            {
                _busy = false;
            }
        }

        protected override void OnClosed(EventArgs e)
        {
            _pollTimer.Stop();
            base.OnClosed(e);
        }
    }
}
