using System.Reflection;
using Microsoft.Extensions.Logging.Abstractions;
using QobuzDLX.Web.Services;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class ApiTimeoutTests
{
    [Fact]
    public static void The_api_client_has_an_explicit_timeout()
    {
        var api = new QobuzApiService(NullLogger<QobuzApiService>.Instance);
        var http = (HttpClient)typeof(QobuzApiService).GetField("_http", BindingFlags.NonPublic | BindingFlags.Instance)!.GetValue(api)!;
        H.Check("API client has an explicit 30 s timeout", http.Timeout == TimeSpan.FromSeconds(30));
    }
}
