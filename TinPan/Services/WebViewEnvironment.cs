using System;
using System.IO;
using System.Threading.Tasks;
using Microsoft.Web.WebView2.Core;

namespace TinPan.Services
{
    /// <summary>
    /// One browser profile for the sign-in window and the hidden player, so the player is signed in
    /// as the user. Every WebView2 sharing a profile must use identical options, hence one place.
    /// </summary>
    public static class WebViewEnvironment
    {
        private static Task<CoreWebView2Environment>? _environment;

        public static string DataFolder => AppFolders.WebView2;

        /// <summary>
        /// Signs the browser out of Suno only. Google and other sites stay signed in and remembered,
        /// so the next Suno sign-in is a click instead of a full Google sign-in.
        /// </summary>
        public static async Task ForgetSunoAsync(CoreWebView2CookieManager cookies)
        {
            foreach (var site in new[] { "https://suno.com", "https://auth.suno.com", "https://clerk.suno.com", "https://studio-api-prod.suno.com" })
                foreach (var cookie in await cookies.GetCookiesAsync(site))
                    cookies.DeleteCookie(cookie);
        }

        /// <summary>https on suno.com or one of its subdomains.</summary>
        public static bool IsSuno(Uri uri) =>
            uri.Scheme == Uri.UriSchemeHttps &&
            (uri.Host.Equals("suno.com", StringComparison.OrdinalIgnoreCase) || uri.Host.EndsWith(".suno.com", StringComparison.OrdinalIgnoreCase));

        /// <summary>
        /// For the hidden player and the captcha window: the page may not open pop-ups or take the
        /// signed-in browser away from Suno. Frames inside the page (the captcha's) are not affected.
        /// </summary>
        public static void KeepOnSuno(CoreWebView2 core, string what)
        {
            core.NewWindowRequested += (_, e) =>
            {
                e.Handled = true;
                Log.Warn($"The {what} page tried to open a window; blocked");
            };
            core.NavigationStarting += (_, e) =>
            {
                if (e.Uri == "about:blank" || Uri.TryCreate(e.Uri, UriKind.Absolute, out var uri) && IsSuno(uri)) return;
                e.Cancel = true;
                Log.Warn($"The {what} page tried to leave Suno; blocked");
            };
        }

        public static Task<CoreWebView2Environment> GetAsync() =>
            _environment ??= CoreWebView2Environment.CreateAsync(null, DataFolder, new CoreWebView2EnvironmentOptions
            {
                // The hidden player starts songs without a click on the page.
                AdditionalBrowserArguments = "--autoplay-policy=no-user-gesture-required"
            });
    }
}
