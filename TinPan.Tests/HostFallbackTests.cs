using System.Net;
using System.Net.Http;
using TinPan.Services;
using Xunit;

namespace TinPan.Tests
{
    public class HostFallbackTests
    {
        private const string Site = "https://suno.com/a/api/";
        private const string Old = "https://studio-api-prod.suno.com/api/";

        [Fact]
        public async Task Requests_go_to_the_new_site_host_with_the_session_cookie()
        {
            var suno = new Fake(r => (HttpStatusCode.OK, "{}"));
            await Api(suno).GetBillingInfoAsync();
            var request = Assert.Single(suno.Requests);
            Assert.StartsWith(Site, request.Url);
            Assert.Equal("__session=token", request.Cookie);
        }

        [Fact]
        public async Task A_route_the_new_host_lacks_goes_to_the_old_one_which_then_goes_first()
        {
            var suno = new Fake(r => r.Url.StartsWith(Site) ? (HttpStatusCode.NotFound, "{\"detail\":\"Not Found\"}") : (HttpStatusCode.OK, "{}"));
            var api = Api(suno);
            await api.GetBillingInfoAsync();
            Assert.Equal(new[] { Site, Old }, suno.Requests.Select(r => r.Url[..(r.Url.IndexOf("api/") + 4)]));
            await api.GetBillingInfoAsync();
            Assert.StartsWith(Old, suno.Requests[2].Url);
            Assert.Null(suno.Requests[1].Cookie);
        }

        [Fact]
        public async Task A_read_that_cant_connect_tries_the_old_host()
        {
            var suno = new Fake(r => r.Url.StartsWith(Site) ? throw new HttpRequestException("no route") : (HttpStatusCode.OK, "{}"));
            await Api(suno).GetBillingInfoAsync();
            Assert.Equal(2, suno.Requests.Count);
        }

        [Fact]
        public async Task A_song_request_that_cant_connect_is_never_sent_twice()
        {
            // It may have arrived; sending it again could make (and charge for) a second song.
            var suno = new Fake(r => r.Url.StartsWith(Site) ? throw new HttpRequestException("reset") : (HttpStatusCode.OK, "{}"));
            await Assert.ThrowsAsync<SunoApiException>(() => Api(suno).SetPublicAsync("c1", true));
            Assert.Single(suno.Requests);
        }

        [Fact]
        public async Task Sunos_own_refusal_is_not_sent_elsewhere()
        {
            var suno = new Fake(r => (HttpStatusCode.Forbidden, "{\"detail\":\"Not allowed\"}"));
            await Assert.ThrowsAsync<SunoApiException>(() => Api(suno).SetPublicAsync("c1", true));
            // 403 retries once with a fresh token, on the same host each time.
            Assert.All(suno.Requests, r => Assert.StartsWith(Site, r.Url));
        }

        [Theory]
        [InlineData("GET", 404, "{\"detail\":\"Not Found\"}", true)]
        [InlineData("POST", 404, "{\"detail\":\"Not Found\"}", true)]
        [InlineData("POST", 405, "", true)]
        [InlineData("POST", 403, "<html>blocked</html>", true)]
        [InlineData("POST", 403, "{\"detail\":\"no\"}", false)]
        [InlineData("POST", 500, "<html>oops</html>", false)]
        [InlineData("GET", 500, "<html>oops</html>", true)]
        [InlineData("POST", 422, "{\"detail\":\"bad\"}", false)]
        [InlineData("GET", 200, "{}", false)]
        public void Only_requests_a_host_didnt_take_move_on(string method, int status, string body, bool moves) =>
            Assert.Equal(moves, SunoApiService.NotTakenHere(new HttpMethod(method), status, body, siteHost: false));

        [Fact]
        public void The_site_host_refusing_the_sign_in_lets_the_old_host_try() =>
            Assert.True(SunoApiService.NotTakenHere(HttpMethod.Get, 401, "{}", siteHost: true));

        [Fact]
        public void Notifications_v3_read_as_sentences()
        {
            var json = "{\"notified_at\":\"x\",\"notifications\":[{\"id\":\"n1\",\"notification_type\":\"like\",\"is_read\":false,"
                       + "\"text\":[{\"text\":\"Ana\",\"bold\":true},{\"text\":\" liked \",\"bold\":false},{\"text\":\"Rain\",\"bold\":true}]},"
                       + "{\"id\":\"n2\",\"is_read\":true,\"text\":[{\"text\":\"Welcome  back\"}]}],\"next_before_datetime_utc\":\"y\"}";
            var list = SunoJson.ReadNotifications(json);
            Assert.Equal("New: Ana liked Rain", list[0].ToString());
            Assert.Equal("Welcome back", list[1].ToString());
        }

        private static SunoApiService Api(Fake suno) => new(new HttpClient(suno), RateLimitTests.FakeAuth.Create(), null!);

        private sealed class Fake : HttpMessageHandler
        {
            private readonly Func<(string Url, string? Cookie), (HttpStatusCode, string)> _answer;
            public Fake(Func<(string Url, string? Cookie), (HttpStatusCode, string)> answer) => _answer = answer;
            public List<(string Url, string? Cookie)> Requests { get; } = new();

            protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken ct)
            {
                var seen = (request.RequestUri!.ToString(), request.Headers.TryGetValues("Cookie", out var c) ? string.Join("; ", c) : null);
                Requests.Add(seen);
                var (status, body) = _answer(seen);
                return Task.FromResult(new HttpResponseMessage(status) { Content = new StringContent(body) });
            }
        }
    }
}
