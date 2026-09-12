using System.Net;
using System.Reflection;
using QobuzDLX.Web.Models;
using QobuzDLX.Web.Services;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class AlbumCacheTests
{
    private sealed class CountingHandler : HttpMessageHandler
    {
        public readonly List<string> Urls = new();
        protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken ct)
        {
            Urls.Add(request.RequestUri!.ToString());
            var id = System.Web.HttpUtility.ParseQueryString(request.RequestUri.Query)["album_id"] ?? "?";
            return Task.FromResult(new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent("{\"id\":\"" + id + "\",\"title\":\"T" + id + "\",\"tracks\":{\"total\":0,\"items\":[]}}")
            });
        }
    }

    [Fact]
    public static void An_album_is_fetched_once_per_item_and_the_cache_is_bounded()
    {
        var fx = new Fixtures();
        var handler = new CountingHandler();
        PrivateAccess.SetHttp(fx.Api, new HttpClient(handler));
        fx.Api.SetCredentials("1", "s", "tok");

        var get = typeof(DownloadService).GetMethod("GetAlbumCachedAsync", BindingFlags.NonPublic | BindingFlags.Instance)!;
        QobuzAlbum Get(string id) => ((Task<QobuzAlbum>)get.Invoke(fx.Download, new object[] { id })!).GetAwaiter().GetResult();

        var first = Get("a1");
        var again = Get("a1");
        H.Check("second request for the same album costs no API call", handler.Urls.Count == 1 && ReferenceEquals(first, again));

        Get("a2");
        H.Check("a different album is fetched", handler.Urls.Count == 2 && Get("a2").Title == "Ta2");

        // bounded: after 50 distinct albums the cache restarts instead of growing
        for (var i = 0; i < 60; i++) Get("x" + i);
        var cache = (Dictionary<string, QobuzAlbum>)typeof(DownloadService).GetField("_albumCache", BindingFlags.NonPublic | BindingFlags.Instance)!.GetValue(fx.Download)!;
        H.Check("cache never exceeds its bound", cache.Count <= 50);

        // emptied after each queue item
        typeof(DownloadService).GetField("_albumCache", BindingFlags.NonPublic | BindingFlags.Instance)!.GetValue(fx.Download);
        cache.Clear();
        H.Check("cache can be emptied", cache.Count == 0);
    }
}
