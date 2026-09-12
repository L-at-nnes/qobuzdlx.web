using System.Net;
using System.Reflection;
using Microsoft.Extensions.Logging.Abstractions;
using QobuzDLX.Web.Services;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class ApiErrorMessageTests
{
    private sealed class FixedHandler : HttpMessageHandler
    {
        public HttpStatusCode Status = HttpStatusCode.Unauthorized;
        public string Body = "";

        protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken ct) =>
            Task.FromResult(new HttpResponseMessage(Status) { Content = new StringContent(Body) });
    }

    private static string ErrorMessage(HttpStatusCode status, string body)
    {
        var api = new QobuzApiService(NullLogger<QobuzApiService>.Instance);
        PrivateAccess.SetHttp(api, new HttpClient(new FixedHandler { Status = status, Body = body }));
        api.SetCredentials("1", "s", "tok");
        try { api.GetTrackAsync("5").GetAwaiter().GetResult(); return "<no exception>"; }
        catch (QobuzApiException ex) { return ex.Message; }
    }

    [Fact]
    public static void Api_errors_expose_only_a_short_message()
    {
        H.Check("JSON message is kept",
            ErrorMessage(HttpStatusCode.Unauthorized, """{"code":401,"message":"Invalid or expired user_auth_token","status":"error"}""")
            == "Qobuz API error 401: Invalid or expired user_auth_token");

        H.Check("HTML error page is not echoed",
            ErrorMessage(HttpStatusCode.BadGateway, "<html><body><h1>502 Bad Gateway</h1><p>nginx</p></body></html>") == "Qobuz API error 502");

        H.Check("empty body", ErrorMessage(HttpStatusCode.InternalServerError, "") == "Qobuz API error 500");

        var longMessage = ErrorMessage(HttpStatusCode.BadRequest, "{\"message\":\"" + new string('x', 500) + "\"}");
        H.Check("long message is truncated", longMessage.Length <= "Qobuz API error 400: ".Length + 201 && longMessage.EndsWith('…'));

        H.Check("other JSON fields are never echoed",
            ErrorMessage(HttpStatusCode.Unauthorized, """{"message":"nope","user_auth_token":"SECRET","email":"a@b.c"}""") == "Qobuz API error 401: nope");

        H.Check("non-string message ignored", ErrorMessage(HttpStatusCode.Unauthorized, """{"message":{"a":1}}""") == "Qobuz API error 401");
        H.Check("JSON array ignored", ErrorMessage(HttpStatusCode.Unauthorized, "[1,2]") == "Qobuz API error 401");
        H.Check("blank message ignored", ErrorMessage(HttpStatusCode.Unauthorized, """{"message":"   "}""") == "Qobuz API error 401");
    }
}
