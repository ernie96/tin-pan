using System;
using System.Net.Http;
using System.Threading;
using System.Threading.Tasks;
using System.Windows;
using TinPan.Views;
using static TinPan.Localization.Loc;

namespace TinPan.Services
{
    public class AuthService : IAuthService
    {
        private readonly IConfigService _config;
        private readonly HttpClient _http;
        private readonly SemaphoreSlim _restoreGate = new(1, 1);
        private ClerkSession? _session;

        public event EventHandler? AuthStateChanged;
        public event EventHandler? SignInExpired;

        public bool HasSavedSignIn => !string.IsNullOrEmpty(_config.Current.User.ProtectedCookies);

        public AuthService(IConfigService config, HttpClient http)
        {
            _config = config;
            _http = http;
        }

        public bool IsAuthenticated => _session?.IsReady == true;
        public string? UserName => _session?.UserName;
        public string DeviceId => _session?.Cookies.AnonymousId ?? _config.Current.User.DeviceId;
        public string CookieHeader => _session?.Cookies.ToHeader() ?? "";

        public async Task<bool> RestoreAsync()
        {
            // Several API calls can start at once; only one of them should restore the session.
            await _restoreGate.WaitAsync();
            try
            {
                if (IsAuthenticated) return true;
                var saved = _config.LoadCookies();
                if (string.IsNullOrEmpty(saved)) return false;

                var session = NewSession(saved);
                await session.InitializeAsync();
                Adopt(session);
                return true;
            }
            catch (SunoSignInRequiredException)
            {
                Log.Info("Saved sign-in is no longer valid");
                _config.SaveCookies(null);
                SignInExpired?.Invoke(this, EventArgs.Empty);
                return false;
            }
            finally
            {
                _restoreGate.Release();
            }
            // Network problems throw and leave the saved sign-in in place for the next try.
        }

        public Task<bool> SignInAsync()
        {
            var window = new LoginWindow(TryAcceptCookiesAsync, _config.Current.User.ClearBrowserOnNextSignIn)
            {
                Owner = Application.Current.MainWindow
            };
            var ok = window.ShowDialog() == true && IsAuthenticated;
            if (ok && _config.Current.User.ClearBrowserOnNextSignIn)
            {
                _config.Current.User.ClearBrowserOnNextSignIn = false;
                _config.Save();
            }
            return Task.FromResult(ok);
        }

        /// <summary>Called by the sign-in window with the browser's cookies until one set works.</summary>
        private async Task<bool> TryAcceptCookiesAsync(string cookieHeader)
        {
            var session = NewSession(cookieHeader);
            if (!session.Cookies.HasClient) return false;
            try
            {
                await session.InitializeAsync();
                await session.GetTokenAsync();
            }
            catch (SunoApiException ex)
            {
                // Usually just "not signed in yet". Logged so a real refusal can be told apart.
                if (session.Cookies.Get("__session") != null) Log.Warn($"Sign-in cookies found but Clerk said no: {ex.Message}");
                return false;
            }
            Adopt(session);
            return true;
        }

        public void SignOut()
        {
            _session = null;
            _config.Current.User.ClearBrowserOnNextSignIn = true;
            _config.SaveCookies(null);
            AuthStateChanged?.Invoke(this, EventArgs.Empty);
        }

        public async Task<string> GetTokenAsync(bool forceRefresh = false, CancellationToken ct = default)
        {
            var session = _session;
            if (session == null)
            {
                if (!await RestoreAsync()) throw new SunoSignInRequiredException(T("Not signed in. Sign in from Settings."));
                session = _session!;
            }

            try
            {
                return await session.GetTokenAsync(forceRefresh, ct);
            }
            catch (SunoSignInRequiredException)
            {
                if (ReferenceEquals(session, _session))
                {
                    _session = null;
                    _config.SaveCookies(null);
                    Log.Info("Suno ended the sign-in");
                    AuthStateChanged?.Invoke(this, EventArgs.Empty);
                    SignInExpired?.Invoke(this, EventArgs.Empty);
                }
                throw;
            }
        }

        private ClerkSession NewSession(string cookieHeader)
        {
            var session = new ClerkSession(_http, SunoCookies.Parse(cookieHeader));
            session.CookiesChanged += (_, _) =>
            {
                if (ReferenceEquals(session, _session)) _config.SaveCookies(session.Cookies.ToHeader());
            };
            return session;
        }

        private void Adopt(ClerkSession session)
        {
            _session = session;
            _config.SaveCookies(session.Cookies.ToHeader());
            Log.Info("Signed in");
            AuthStateChanged?.Invoke(this, EventArgs.Empty);
        }
    }
}
