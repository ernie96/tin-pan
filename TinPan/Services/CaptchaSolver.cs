using System;
using System.Text.Json;
using System.Threading.Tasks;
using System.Windows;
using Microsoft.Web.WebView2.Core;
using Microsoft.Web.WebView2.Wpf;
using static TinPan.Localization.Loc;

namespace TinPan.Services
{
    public interface ICaptchaSolver
    {
        /// <summary>
        /// Gets a token from Suno's own captcha on suno.com/create, signed in as the user.
        /// provider 1 is hCaptcha, 2 is Cloudflare Turnstile (from /api/c/check's captcha_version).
        /// Returns the token and the provider that produced it.
        /// </summary>
        Task<(string Token, int Provider)> SolveAsync(int provider);
    }

    /// <summary>
    /// Same approach as paperfoot/suno-cli: render Suno's captcha widget invisibly on suno.com/create
    /// and execute it. For trusted accounts no puzzle appears. If one is needed, the window comes
    /// on screen with a visible check box so the user can press it.
    /// </summary>
    public sealed class CaptchaSolver : ICaptchaSolver, IDisposable
    {
        private const string HCaptchaSiteKey = "d65453de-3f1a-4aac-9366-a0f06e52b2ce";
        private const string TurnstileSiteKey = "0x4AAAAAADI7xDNyj-3LcIbi";

        private readonly IAnnouncer _announcer;
        private Window? _window;
        private WebView2? _view;
        private bool _shown;
        private Window? _returnTo;

        // Several songs can start at once (up to five); they take turns on the one check page.
        private readonly System.Threading.SemaphoreSlim _gate = new(1, 1);

        public CaptchaSolver(IAnnouncer announcer) => _announcer = announcer;

        public async Task<(string Token, int Provider)> SolveAsync(int provider)
        {
            _announcer.Announce(T("Passing Suno's captcha first. This can take half a minute."), interrupt: false);
            await _gate.WaitAsync();
            try
            {
                await EnsureWindowAsync();
                // One short invisible try. Trusted accounts pass here; others need the check box,
                // so don't keep them waiting. (hCaptcha never loads when Suno asks for Turnstile.)
                var token = await TryAsync(provider == 2 ? 2 : 1, visible: false);
                if (token != null) return (token, provider == 2 ? 2 : 1);

                // Not passed silently: show Suno's check box so the user can press it.
                ShowOnScreen();
                _announcer.Announce(T("Suno wants you to confirm you're a person. A Suno check window is open; press the check box in it if there is one."));
                var shown = await TryAsync(provider == 2 ? 2 : 1, visible: true);
                if (shown != null) return (shown, provider == 2 ? 2 : 1);
                throw new SunoApiException(T("Suno's captcha wasn't passed, so no song was made. You can try again, or make this one on suno.com."));
            }
            finally
            {
                HideOffScreen();
                _gate.Release();
            }
        }

        private async Task<string?> TryAsync(int provider, bool visible)
        {
            try
            {
                return await TryOnPageAsync(provider, visible);
            }
            catch (Exception ex) when (ex is InvalidOperationException or ObjectDisposedException or NullReferenceException
                                           or System.Runtime.InteropServices.COMException or JsonException)
            {
                // The window was closed, or the page went away mid-check.
                Log.Warn($"Captcha check stopped: {ex.GetType().Name}");
                return null;
            }
        }

        private async Task<string?> TryOnPageAsync(int provider, bool visible)
        {
            if (_view?.CoreWebView2 == null) await EnsureWindowAsync();
            var core = _view!.CoreWebView2;
            var loaded = new TaskCompletionSource<bool>(TaskCreationOptions.RunContinuationsAsynchronously);
            void OnDone(object? s, CoreWebView2NavigationCompletedEventArgs e) => loaded.TrySetResult(e.IsSuccess);
            core.NavigationCompleted += OnDone;
            core.Navigate("https://suno.com/create");
            await Task.WhenAny(loaded.Task, Task.Delay(TimeSpan.FromSeconds(30)));
            core.NavigationCompleted -= OnDone;

            if (provider == 2)
            {
                await core.ExecuteScriptAsync(
                    "if (!window.turnstile && !document.querySelector('script[src*=\"challenges.cloudflare.com/turnstile\"]')) {" +
                    " const s = document.createElement('script'); s.src = 'https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit'; document.head.appendChild(s); }");
            }

            var ready = provider == 2 ? "typeof turnstile !== 'undefined' && !!turnstile.render" : "typeof hcaptcha !== 'undefined' && !!hcaptcha.render";
            var sdkLoaded = false;
            for (var i = 0; i < 20 && !sdkLoaded; i++)
            {
                await Task.Delay(1000);
                sdkLoaded = await core.ExecuteScriptAsync(ready) == "true";
            }
            if (!sdkLoaded)
            {
                Log.Warn($"Captcha provider {provider} never loaded on suno.com/create");
                return null;
            }
            await Task.Delay(2000); // let the SDK finish wiring itself up

            var result = await EvaluateAsync(provider == 2 ? TurnstileScript(visible) : HCaptchaScript(visible), visible ? 180 : 20);
            if (string.IsNullOrEmpty(result) || result.StartsWith("ERR:", StringComparison.Ordinal))
            {
                Log.Warn($"Captcha provider {provider} ({(visible ? "shown" : "invisible")}) failed: {result}");
                return null;
            }
            return result;
        }

        /// <summary>Runs a script that returns a promise and waits for its string result.</summary>
        private async Task<string?> EvaluateAsync(string expression, int timeoutSeconds)
        {
            var call = _view!.CoreWebView2.CallDevToolsProtocolMethodAsync("Runtime.evaluate",
                JsonSerializer.Serialize(new { expression, awaitPromise = true, returnByValue = true, timeout = timeoutSeconds * 1000 }));
            var finished = await Task.WhenAny(call, Task.Delay(TimeSpan.FromSeconds(timeoutSeconds + 5)));
            if (finished != call) return "ERR:timed out";
            using var doc = JsonDocument.Parse(await call);
            return doc.RootElement.TryGetProperty("result", out var r) && r.TryGetProperty("value", out var v) && v.ValueKind == JsonValueKind.String
                ? v.GetString()
                : null;
        }

        private static string HCaptchaScript(bool visible) => $@"
(async () => {{
  try {{
    const div = document.createElement('div');
    div.style.cssText = {(visible ? "'position:fixed;top:40px;left:40px;z-index:2147483647;'" : "'position:fixed;top:-9999px;left:-9999px;'")};
    document.body.appendChild(div);
    const id = hcaptcha.render(div, {{
      sitekey: '{HCaptchaSiteKey}', size: '{(visible ? "normal" : "invisible")}', sentry: false,
      endpoint: 'https://hcaptcha-endpoint-prod.suno.com', assethost: 'https://hcaptcha-assets-prod.suno.com',
      imghost: 'https://hcaptcha-imgs-prod.suno.com', reportapi: 'https://hcaptcha-reportapi-prod.suno.com'
    }});
    const r = await hcaptcha.execute(id, {{ async: true }});
    return (r && r.response) ? r.response : '';
  }} catch (e) {{ return 'ERR:' + String(e); }}
}})()";

        private static string TurnstileScript(bool visible) => $@"
(async () => new Promise(resolve => {{
  const div = document.createElement('div');
  div.style.cssText = 'position:fixed;top:40px;left:40px;z-index:2147483647;';
  document.body.appendChild(div);
  let settled = false;
  const finish = v => {{ if (!settled) {{ settled = true; clearTimeout(timer); resolve(v); }} }};
  const timer = setTimeout(() => finish('ERR:Turnstile timed out'), {(visible ? 175000 : 15000)});
  try {{
    const w = turnstile.render(div, {{
      sitekey: '{TurnstileSiteKey}', execution: 'execute', appearance: '{(visible ? "always" : "interaction-only")}',
      callback: t => finish(t), 'error-callback': c => finish('ERR:Turnstile ' + c), 'timeout-callback': () => finish('ERR:Turnstile challenge timed out')
    }});
    turnstile.execute(w);
  }} catch (e) {{ finish('ERR:' + String(e)); }}
}}))()";

        private async Task EnsureWindowAsync()
        {
            if (_view?.CoreWebView2 != null) return;
            _window = new Window
            {
                Title = T("Suno check"),
                Width = 520,
                Height = 640,
                Left = -32000,
                Top = -32000,
                ShowInTaskbar = false,
                ShowActivated = false,
                Owner = Application.Current.MainWindow
            };
            // Closing it gives up on the check; a new window is made next time.
            _window.Closed += (_, _) =>
            {
                _window = null;
                _view = null;
            };
            _view = new WebView2();
            System.Windows.Automation.AutomationProperties.SetName(_view, T("Suno check"));
            _window.Content = _view;
            _window.Show();
            try
            {
                await _view.EnsureCoreWebView2Async(await WebViewEnvironment.GetAsync());
            }
            catch (WebView2RuntimeNotFoundException ex)
            {
                throw new SunoApiException(T("Suno's captcha needs the Microsoft Edge WebView2 Runtime. Install it from Microsoft, then try again."), null, ex);
            }
            WebViewEnvironment.KeepOnSuno(_view.CoreWebView2, "captcha");
        }

        private void ShowOnScreen()
        {
            if (_window == null) return;
            var area = SystemParameters.WorkArea;
            _window.Left = area.Left + (area.Width - _window.Width) / 2;
            _window.Top = area.Top + (area.Height - _window.Height) / 2;
            _window.ShowInTaskbar = true;
            _shown = true;
            _returnTo = Application.Current.Windows.OfType<Window>().FirstOrDefault(w => w.IsActive && w != _window);
            _window.Activate();
            _view?.Focus();
        }

        private void HideOffScreen()
        {
            if (_window == null) return;
            _window.Left = -32000;
            _window.Top = -32000;
            _window.ShowInTaskbar = false;
            // Give focus back only if the check window took it; otherwise leave the user where they are.
            if (_shown) (_returnTo is { IsVisible: true } back ? back : Application.Current.MainWindow)?.Activate();
            _shown = false;
            _returnTo = null;
        }

        public void Dispose()
        {
            _view?.Dispose();
            _window = null;
        }
    }
}
