using Xunit;

namespace QobuzDLX.Web.Tests;
public static class ApiErrorQueryStringTests
{
    [Fact]
    public static void A_connection_failure_does_not_leak_the_query_string()
    {
        // What does the framework put in the exception when a request carrying a token in its query string fails?
        using var http = new HttpClient { Timeout = TimeSpan.FromSeconds(5) };
        try { http.GetAsync("http://127.0.0.1:1/track/getFileUrl?user_auth_token=SECRETTOKEN&request_sig=SECRETSIG").GetAwaiter().GetResult(); }
        catch (Exception ex)
        {
            Console.WriteLine($"   exception: {ex.GetType().Name}: {ex.Message}");
            H.Check("connection failure does not carry the query string", !ex.ToString().Contains("SECRET"));
        }
    }
}
