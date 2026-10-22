using System.IO;
using System.Net;
using System.Net.Http;
using System.Reflection;
using System.Runtime.CompilerServices;
using TinPan.Services;
using Xunit;

namespace TinPan.Tests
{
    /// <summary>Keeps the tests' log lines out of the user's own log.</summary>
    internal static class TestLog
    {
        [ModuleInitializer]
        internal static void UseTempLog() =>
            Environment.SetEnvironmentVariable("TINPAN_LOG", Path.Combine(Path.GetTempPath(), "TinPan.Tests", "tinpan.log"));
    }

    public class RateLimitTests
    {
        private const string Retryable = "{\"detail\": \"Too many requests\", \"retryable\": true}";

        [Fact]
        public void Tests_log_to_a_temp_file()
        {
            Assert.StartsWith(Path.GetTempPath(), Log.FilePath);
        }

        [Fact]
        public async Task A_retryable_429_is_tried_again()
        {
            var suno = new FakeSuno((HttpStatusCode.TooManyRequests, Retryable), (HttpStatusCode.OK, "{}"));
            await Api(suno).GetBillingInfoAsync();
            Assert.Equal(2, suno.Calls);
        }

        [Fact]
        public async Task A_429_that_keeps_coming_gives_up_after_two_retries()
        {
            var suno = new FakeSuno((HttpStatusCode.TooManyRequests, Retryable), (HttpStatusCode.TooManyRequests, Retryable),
                (HttpStatusCode.TooManyRequests, Retryable), (HttpStatusCode.OK, "{}"));
            var ex = await Assert.ThrowsAsync<SunoApiException>(() => Api(suno).GetBillingInfoAsync());
            Assert.Equal(429, ex.StatusCode);
            Assert.Equal(3, suno.Calls);
        }

        [Fact]
        public async Task A_429_not_marked_retryable_fails_at_once()
        {
            var suno = new FakeSuno((HttpStatusCode.TooManyRequests, "{\"detail\": \"Too many requests\"}"), (HttpStatusCode.OK, "{}"));
            var ex = await Assert.ThrowsAsync<SunoApiException>(() => Api(suno).GetBillingInfoAsync());
            Assert.Equal(429, ex.StatusCode);
            Assert.Equal(1, suno.Calls);
        }

        [Theory]
        [InlineData(Retryable, true)]
        [InlineData("{\"retryable\": false}", false)]
        [InlineData("{\"retryable\": \"yes\"}", false)]
        [InlineData("<html>Too many requests</html>", false)]
        [InlineData("[]", false)]
        [InlineData("", false)]
        public void Only_a_true_retryable_counts(string body, bool expected) =>
            Assert.Equal(expected, SunoApiService.RetryableRateLimit(body));

        private static SunoApiService Api(FakeSuno suno) =>
            new(new HttpClient(suno), FakeAuth.Create(), null!) { RateLimitWait = _ => TimeSpan.FromMilliseconds(1) };

        private sealed class FakeSuno : HttpMessageHandler
        {
            private readonly Queue<(HttpStatusCode, string)> _answers;
            public FakeSuno(params (HttpStatusCode, string)[] answers) => _answers = new(answers);
            public int Calls { get; private set; }

            protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken ct)
            {
                Calls++;
                var (status, body) = _answers.Dequeue();
                return Task.FromResult(new HttpResponseMessage(status) { Content = new StringContent(body) });
            }
        }

        /// <summary>A signed-in account that hands out a token and nothing else.</summary>
        public class FakeAuth : DispatchProxy
        {
            public static IAuthService Create() => DispatchProxy.Create<IAuthService, FakeAuth>();

            protected override object? Invoke(MethodInfo? method, object?[]? args) => method!.Name switch
            {
                nameof(IAuthService.GetTokenAsync) => Task.FromResult("token"),
                "get_" + nameof(IAuthService.DeviceId) => "device",
                "get_" + nameof(IAuthService.CookieHeader) => "",
                _ => throw new InvalidOperationException("Unexpected call: " + method.Name),
            };
        }
    }
}
