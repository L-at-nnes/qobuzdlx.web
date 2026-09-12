using System.Net;
using System.Reflection;
using System.Text;
using QobuzDLX.Web.Services;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class FavoritesPagingTests
{
    // Qobuz answers at most 500 favourites per call and says how many there are in total
    private sealed class PagedFavorites : HttpMessageHandler
    {
        public const int Total = 1200;
        public readonly List<string> Queries = new();

        protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken ct)
        {
            var query = request.RequestUri!.Query;
            Queries.Add(query);
            var offset = int.Parse(System.Text.RegularExpressions.Regex.Match(query, @"offset=(\d+)").Groups[1].Value);
            var count = Math.Min(500, Total - offset);
            var items = string.Join(",", Enumerable.Range(offset, count).Select(i => "{\"id\":\"a" + i + "\",\"title\":\"T" + i + "\"}"));
            var body = "{\"albums\":{\"offset\":" + offset + ",\"limit\":500,\"total\":" + Total + ",\"items\":[" + items + "]}}";
            return Task.FromResult(new HttpResponseMessage(HttpStatusCode.OK) { Content = new StringContent(body, Encoding.UTF8, "application/json") });
        }
    }

    [Fact]
    public static void Every_page_of_favorites_is_fetched()
    {
        var fx = new Fixtures();
        var handler = new PagedFavorites();
        PrivateAccess.SetHttp(fx.Api, new HttpClient(handler));
        fx.Api.SetCredentials("1", "secret", "tok");

        var favorites = fx.Api.GetFavoritesAsync("7", "albums").GetAwaiter().GetResult();
        var ids = favorites?.Albums?.Items.Select(a => a.Id).ToList() ?? new();
        H.Check("every favourite is fetched, not only the first 500", ids.Count == PagedFavorites.Total);
        H.Check("no favourite is read twice", ids.Distinct().Count() == PagedFavorites.Total);
        H.Check("three calls for 1200 favourites", handler.Queries.Count == 3);
    }
}
