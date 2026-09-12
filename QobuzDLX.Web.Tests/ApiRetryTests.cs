using System.Diagnostics;
using System.Net;
using System.Reflection;
using Microsoft.Extensions.Logging.Abstractions;
using QobuzDLX.Web.Services;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class ApiRetryTests
{
    private sealed class ScriptedHandler : HttpMessageHandler
    {
        private readonly Queue<(HttpStatusCode Status, int? RetryAfter)> _script;
        public int Calls;
        public ScriptedHandler(params (HttpStatusCode, int?)[] script) => _script = new(script);

        protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken ct)
        {
            Calls++;
            // the last scripted answer repeats
            var (status, retryAfter) = _script.Count > 1 ? _script.Dequeue() : _script.Peek();
            var response = new HttpResponseMessage(status) { Content = new StringContent("""{"id":1,"title":"x"}""") };
            if (retryAfter is { } seconds) response.Headers.RetryAfter = new System.Net.Http.Headers.RetryConditionHeaderValue(TimeSpan.FromSeconds(seconds));
            return Task.FromResult(response);
        }
    }

    private static (QobuzApiService Api, ScriptedHandler Handler) Create(params (HttpStatusCode, int?)[] script)
    {
        var api = new QobuzApiService(NullLogger<QobuzApiService>.Instance);
        var handler = new ScriptedHandler(script);
        PrivateAccess.SetHttp(api, new HttpClient(handler));
        api.SetCredentials("1", "s", "tok");
        return (api, handler);
    }

    [Fact]
    public static void Transient_api_errors_are_retried()
    {
        var (api, handler) = Create((HttpStatusCode.ServiceUnavailable, 0), (HttpStatusCode.TooManyRequests, 0), (HttpStatusCode.OK, null));
        var track = api.GetTrackAsync("5").GetAwaiter().GetResult();
        H.Check("503 then 429 then 200 succeeds after 3 calls (Retry-After: 0)", track?.Id == 1 && handler.Calls == 3);

        (api, handler) = Create((HttpStatusCode.TooManyRequests, 0));
        try { api.GetTrackAsync("5").GetAwaiter().GetResult(); H.Check("persistent 429 throws", false); }
        catch (QobuzApiException ex) { H.Check("persistent 429: gives up after 3 attempts with the 429", handler.Calls == 3 && ex.StatusCode == HttpStatusCode.TooManyRequests); }

        foreach (var status in new[] { HttpStatusCode.Unauthorized, HttpStatusCode.NotFound, HttpStatusCode.InternalServerError, HttpStatusCode.BadRequest })
        {
            (api, handler) = Create((status, null));
            try { api.GetTrackAsync("5").GetAwaiter().GetResult(); }
            catch (QobuzApiException) { }
            H.Check($"{(int)status} is not retried", handler.Calls == 1);
        }

        // no Retry-After: 1 s of back-off before the second attempt
        (api, handler) = Create((HttpStatusCode.BadGateway, null), (HttpStatusCode.OK, null));
        var watch = Stopwatch.StartNew();
        api.GetTrackAsync("5").GetAwaiter().GetResult();
        H.Check("default back-off is about 1 s", handler.Calls == 2 && watch.Elapsed >= TimeSpan.FromMilliseconds(900) && watch.Elapsed < TimeSpan.FromSeconds(4));

        // the signed stream URL call goes through the same path
        (api, handler) = Create((HttpStatusCode.GatewayTimeout, 0), (HttpStatusCode.OK, null));
        api.SetCredentials("1", "secret", "tok");
        api.GetStreamUrlAsync("5", "27").GetAwaiter().GetResult();
        H.Check("getFileUrl is retried too", handler.Calls == 2);
    }
}
