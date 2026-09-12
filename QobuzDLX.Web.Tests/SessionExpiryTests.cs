using System.Net;
using System.Reflection;
using QobuzDLX.Web.Services;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class SessionExpiryTests
{
    private sealed class Rejecting : HttpMessageHandler
    {
        public int Calls;
        protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken ct)
        {
            Calls++;
            return Task.FromResult(new HttpResponseMessage(HttpStatusCode.Unauthorized)
            {
                Content = new StringContent("""{"code":401,"message":"Invalid or expired user_auth_token"}""")
            });
        }
    }

    private static QueueItem? Wait(Fixtures fx, string url, TimeSpan timeout)
    {
        var until = DateTime.UtcNow + timeout;
        while (DateTime.UtcNow < until)
        {
            var item = fx.Download.QueueSnapshot().FirstOrDefault(i => i.Url == url);
            if (item is { IsFinished: true }) return item;
            Thread.Sleep(50);
        }
        return null;
    }

    [Fact]
    public static void An_expired_session_fails_the_queue_without_calling_qobuz()
    {
        var fx = new Fixtures();
        var handler = new Rejecting();
        PrivateAccess.SetHttp(fx.Api, new HttpClient(handler));
        fx.Api.SetCredentials("1", "secret", "tok");
        fx.Settings.Update(s => { s.UserAuthToken = "tok"; s.AppId = "1"; s.AppSecret = "secret"; s.Email = "me@example.com"; });
        var expiredEvents = 0;
        fx.Download.OnSessionExpired += () => Interlocked.Increment(ref expiredEvents);

        fx.Download.Enqueue("https://play.qobuz.com/album/abc");
        var item = Wait(fx, "https://play.qobuz.com/album/abc", TimeSpan.FromSeconds(15));
        H.Check("the item fails instead of hanging", item is { Status: QueueItemStatus.Failed });
        H.Check("it fails with the session-expired message", item?.Error == fx.Loc["error.sessionExpired"]);
        H.Check("the session is forgotten in memory", !fx.Api.IsLoggedIn);
        H.Check("and in the persisted settings", fx.Settings.Settings.UserAuthToken == "" && fx.Settings.Settings.AppSecret == "");
        H.Check("pages are told to send the user back to sign-in", expiredEvents == 1);
        H.Check("the log carries the message", fx.Download.LogSnapshot().Any(l => l.Contains(fx.Loc["error.sessionExpired"])));

        // later items do not hit Qobuz with a dead session
        var callsBefore = handler.Calls;
        fx.Download.Enqueue("https://play.qobuz.com/album/def");
        var next = Wait(fx, "https://play.qobuz.com/album/def", TimeSpan.FromSeconds(15));
        H.Check("the next item fails fast with the same message", next is { Status: QueueItemStatus.Failed } && next.Error == fx.Loc["error.sessionExpired"]);
        H.Check("without any API call", handler.Calls == callsBefore);
    }
}
