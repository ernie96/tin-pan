using System;
using System.Linq;
using System.Net;
using System.Net.Http;
using System.Text;
using System.Text.Json;
using System.Threading;
using System.Threading.Tasks;
using static TinPan.Localization.Loc;

namespace TinPan.Services
{
    /// <summary>
    /// Suno signs in through Clerk. The browser's "__client" cookie identifies the device; from it we
    /// read the active session id, then mint short-lived (about 60 second) JWTs for the studio API.
    /// Same flow as gcui-art/suno-api and Nsomnia/suno-api-local-manager-frontend.
    /// </summary>
    public sealed class ClerkSession
    {
        // auth.suno.com is what current suno.com uses; clerk.suno.com is the older name.
        private static readonly string[] ClerkBases = { "https://auth.suno.com", "https://clerk.suno.com" };
        private const string ClerkQuery = "__clerk_api_version=2025-11-10&_clerk_js_version=5.117.0";
        private static readonly TimeSpan RefreshMargin = TimeSpan.FromSeconds(15);

        private readonly HttpClient _http;
        private readonly SemaphoreSlim _gate = new(1, 1);
        private string? _clerkBase;
        private string? _sessionId;
        private string? _jwt;
        private DateTimeOffset _jwtExpires;

        public ClerkSession(HttpClient http, SunoCookies cookies)
        {
            _http = http;
            Cookies = cookies;
        }

        public SunoCookies Cookies { get; }

        /// <summary>Clerk username, or full name, of the signed-in account.</summary>
        public string? UserName { get; private set; }

        public bool IsReady => _sessionId != null;

        /// <summary>Raised when Clerk rotates cookies, so they can be saved.</summary>
        public event EventHandler? CookiesChanged;

        /// <summary>Finds the active session. Throws SunoSignInRequiredException if there is none.</summary>
        public async Task InitializeAsync(CancellationToken ct = default)
        {
            if (!Cookies.HasClient) throw new SunoSignInRequiredException(T("Not signed in."));

            SunoApiException? last = null;
            foreach (var baseUrl in ClerkBases)
            {
                try
                {
                    using var response = await SendClerkAsync(HttpMethod.Get, $"{baseUrl}/v1/client?{ClerkQuery}", ct);
                    if (response.StatusCode == HttpStatusCode.NotFound) continue;
                    if (!response.IsSuccessStatusCode)
                    {
                        last = new SunoApiException(T("Suno sign-in check failed ({0}).", (int)response.StatusCode), (int)response.StatusCode);
                        Log.Warn($"Clerk client lookup on {baseUrl} returned {(int)response.StatusCode}");
                        continue;
                    }

                    using var doc = JsonDocument.Parse(await response.Content.ReadAsStringAsync(ct));
                    var root = doc.RootElement;
                    // Clerk answers {"response": null} when this browser has no signed-in client left.
                    var client = root.ValueKind == JsonValueKind.Object && root.TryGetProperty("response", out var r) ? r : root;
                    if (client.ValueKind != JsonValueKind.Object)
                        throw new SunoSignInRequiredException(T("Not signed in to Suno."));
                    var sid = Text(client, "last_active_session_id");
                    if (string.IsNullOrEmpty(sid))
                        throw new SunoSignInRequiredException(T("Not signed in to Suno."));

                    _clerkBase = baseUrl;
                    _sessionId = sid;
                    UserName = ReadUserName(client, sid);
                    Log.Info($"Clerk session found via {baseUrl}");
                    return;
                }
                catch (Exception ex) when (ex is HttpRequestException || ex is TaskCanceledException && !ct.IsCancellationRequested)
                {
                    last = new SunoApiException(T("Can't reach Suno. Check your internet connection."), null, ex);
                    Log.Error($"Clerk client lookup on {baseUrl} failed", ex);
                }
                catch (Exception ex) when (ex is JsonException or InvalidOperationException)
                {
                    last = new SunoApiException(T("Suno's sign-in service sent a reply this app doesn't understand."), null, ex);
                    Log.Error($"Clerk client lookup on {baseUrl} gave an unreadable reply", ex);
                }
            }
            throw last ?? new SunoApiException(T("Suno sign-in service didn't answer."));
        }

        /// <summary>Returns a JWT that is valid for at least a few more seconds.</summary>
        public async Task<string> GetTokenAsync(bool forceRefresh = false, CancellationToken ct = default)
        {
            await _gate.WaitAsync(ct);
            try
            {
                if (!forceRefresh && _jwt != null && DateTimeOffset.UtcNow < _jwtExpires - RefreshMargin)
                    return _jwt;

                if (_sessionId == null) await InitializeAsync(ct);

                HttpResponseMessage sent;
                try
                {
                    sent = await SendClerkAsync(HttpMethod.Post,
                        $"{_clerkBase}/v1/client/sessions/{Uri.EscapeDataString(_sessionId!)}/tokens?{ClerkQuery}", ct, "{}");
                }
                catch (Exception ex) when (ex is HttpRequestException || ex is TaskCanceledException && !ct.IsCancellationRequested)
                {
                    Log.Error("Refreshing the Suno sign-in failed", ex);
                    throw new SunoApiException(T("Can't reach Suno. Check your internet connection."), null, ex);
                }
                using var response = sent;
                if (response.StatusCode is HttpStatusCode.Unauthorized or HttpStatusCode.NotFound or HttpStatusCode.Forbidden)
                {
                    _sessionId = null;
                    throw new SunoSignInRequiredException();
                }
                if (!response.IsSuccessStatusCode)
                    throw new SunoApiException(T("Couldn't refresh the Suno sign-in ({0}).", (int)response.StatusCode), (int)response.StatusCode);

                string? jwt;
                try
                {
                    using var doc = JsonDocument.Parse(await response.Content.ReadAsStringAsync(ct));
                    jwt = doc.RootElement.ValueKind == JsonValueKind.Object && doc.RootElement.TryGetProperty("jwt", out var j)
                          && j.ValueKind == JsonValueKind.String ? j.GetString() : null;
                }
                catch (JsonException ex)
                {
                    throw new SunoApiException(T("Suno's sign-in service sent a reply this app doesn't understand."), null, ex);
                }
                if (string.IsNullOrEmpty(jwt)) throw new SunoApiException(T("Suno sign-in returned no token."));

                _jwt = jwt;
                _jwtExpires = Jwt.ReadExpiry(jwt) ?? DateTimeOffset.UtcNow.AddSeconds(50);
                return jwt;
            }
            finally
            {
                _gate.Release();
            }
        }

        private async Task<HttpResponseMessage> SendClerkAsync(HttpMethod method, string url, CancellationToken ct, string? body = null)
        {
            using var request = new HttpRequestMessage(method, url);
            // Clerk's native flow takes the raw __client value, not "Bearer ...".
            request.Headers.TryAddWithoutValidation("Authorization", Cookies.Client);
            // No Origin/Referer: with them Clerk treats this as a browser call; sign-in stopped working.
            request.Headers.TryAddWithoutValidation("Cookie", Cookies.ToHeader());
            if (body != null) request.Content = new StringContent(body, Encoding.UTF8, "application/json");

            var response = await _http.SendAsync(request, ct);
            if (response.Headers.TryGetValues("Set-Cookie", out var setCookies))
            {
                var before = Cookies.Client;
                foreach (var c in setCookies) Cookies.MergeSetCookie(c);
                if (Cookies.Client != before) CookiesChanged?.Invoke(this, EventArgs.Empty);
            }
            return response;
        }

        /// <summary>A text field, or null when it's missing, null or not text (GetString throws on those).</summary>
        private static string? Text(JsonElement obj, string name) =>
            obj.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.String ? v.GetString() : null;

        private static string? ReadUserName(JsonElement client, string sid)
        {
            if (!client.TryGetProperty("sessions", out var sessions) || sessions.ValueKind != JsonValueKind.Array)
                return null;
            foreach (var session in sessions.EnumerateArray().Where(e => e.ValueKind == JsonValueKind.Object))
            {
                if (Text(session, "id") != sid) continue;
                if (!session.TryGetProperty("user", out var user) || user.ValueKind != JsonValueKind.Object) return null;
                var username = Text(user, "username");
                if (!string.IsNullOrWhiteSpace(username)) return username;
                var first = Text(user, "first_name");
                var last = Text(user, "last_name");
                var full = $"{first} {last}".Trim();
                return full.Length > 0 ? full : null;
            }
            return null;
        }
    }
}
