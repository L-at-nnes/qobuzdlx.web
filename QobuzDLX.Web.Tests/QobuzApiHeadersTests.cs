using System.Net;
using System.Reflection;
using Microsoft.Extensions.Logging.Abstractions;
using QobuzDLX.Web.Services;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class QobuzApiHeadersTests
{
    private sealed class RecordingHandler : HttpMessageHandler
    {
        public readonly List<HttpRequestMessage> Requests = new();
        public HttpStatusCode Status = HttpStatusCode.OK;
        public string Body = """{"id":1,"title":"x"}""";

        protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken ct)
        {
            Requests.Add(request);
            return Task.FromResult(new HttpResponseMessage(Status) { Content = new StringContent(Body) });
        }
    }

    private static (QobuzApiService Api, RecordingHandler Handler, HttpClient Http) Create()
    {
        var api = new QobuzApiService(NullLogger<QobuzApiService>.Instance);
        var handler = new RecordingHandler();
        var http = new HttpClient(handler);
        PrivateAccess.SetHttp(api, http);
        return (api, handler, http);
    }

    private static string Header(HttpRequestMessage request, string name) => request.Headers.GetValues(name).Single();

    [Fact]
    public static void Identification_headers_are_set_per_request()
    {
        var (api, handler, http) = Create();
        api.SetCredentials("111", "secret", "tok");
        H.Check("SetCredentials leaves default headers alone", !http.DefaultRequestHeaders.Contains("X-App-Id"));

        api.GetTrackAsync("5").GetAwaiter().GetResult();
        var req = handler.Requests[^1];
        H.Check("authenticated call carries X-App-Id and token per request",
            Header(req, "X-App-Id") == "111" && Header(req, "X-User-Auth-Token") == "tok");

        api.SetCredentials("222", "secret", "tok2");
        api.GetTrackAsync("5").GetAwaiter().GetResult();
        H.Check("new credentials apply to the next request", Header(handler.Requests[^1], "X-App-Id") == "222");

        api.SetCredentials("", "", "");
        api.GetTrackAsync("5").GetAwaiter().GetResult();
        H.Check("after logout no identification header is sent",
            !handler.Requests[^1].Headers.Contains("X-App-Id") && !handler.Requests[^1].Headers.Contains("X-User-Auth-Token"));
        H.Check("default headers still untouched", !http.DefaultRequestHeaders.Contains("X-App-Id"));

        handler.Body = """{"user_auth_token":"newtok","user":{"id":9,"display_name":"me"}}""";
        var login = api.LoginWithEmailAsync("333", "a@b.c", "pw").GetAwaiter().GetResult();
        H.Check("email login sends X-App-Id and no stale token",
            Header(handler.Requests[^1], "X-App-Id") == "333"
            && !handler.Requests[^1].Headers.Contains("X-User-Auth-Token")
            && login.UserAuthToken == "newtok");

        // error typing relied upon by the login (B1) and session-expiry (B10) fixes
        handler.Status = HttpStatusCode.Unauthorized;
        handler.Body = """{"message":"nope"}""";
        try { api.LoginWithEmailAsync("333", "a@b.c", "bad").GetAwaiter().GetResult(); H.Check("login 401 throws", false); }
        catch (HttpRequestException ex) { H.Check("login 401 -> HttpRequestException(401)", ex.StatusCode == HttpStatusCode.Unauthorized); }

        api.SetCredentials("", "", "");
        try { api.LoginWithTokenAsync("333", "badtoken").GetAwaiter().GetResult(); H.Check("token login 401 throws", false); }
        catch (HttpRequestException ex) { H.Check("token login 401 -> HttpRequestException(401)", ex.StatusCode == HttpStatusCode.Unauthorized); }
        H.Check("rejected token does not log the service in", !api.IsLoggedIn);

        api.SetCredentials("333", "s", "tok");
        try { api.GetTrackAsync("5").GetAwaiter().GetResult(); H.Check("api 401 throws", false); }
        catch (QobuzApiException ex) { H.Check("api 401 -> QobuzApiException(401)", ex.StatusCode == HttpStatusCode.Unauthorized); }

        api.SetCredentials("", "", "");
        handler.Status = HttpStatusCode.OK;
        handler.Body = """{"id":1}""";
        var ok = api.LoginWithTokenAsync("444", "goodtoken").GetAwaiter().GetResult();
        H.Check("accepted token logs the service in", api.IsLoggedIn && api.UserAuthToken == "goodtoken" && ok.UserAuthToken == "goodtoken");
    }
}
