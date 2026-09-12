using System.Net;
using System.Reflection;
using Microsoft.Extensions.Logging.Abstractions;
using QobuzDLX.Web.Models;
using QobuzDLX.Web.Services;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class SearchPagingTests
{
    private sealed class PagedAlbums : HttpMessageHandler
    {
        public readonly List<string> Queries = new();
        protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken ct)
        {
            var q = System.Web.HttpUtility.ParseQueryString(request.RequestUri!.Query);
            Queries.Add($"{q["query"]}@{q["offset"]}x{q["limit"]}");
            var offset = int.Parse(q["offset"]!);
            var limit = int.Parse(q["limit"]!);
            var count = Math.Max(0, Math.Min(limit, 120 - offset));   // 120 results in all
            var items = string.Join(",", Enumerable.Range(offset, count).Select(i => $$"""{"id":"a{{i}}","title":"T{{i}}"}"""));
            return Task.FromResult(new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent($$"""{"albums":{"offset":{{offset}},"limit":{{limit}},"total":120,"items":[{{items}}]} }""")
            });
        }
    }

    [Fact]
    public static void Search_results_are_paged_and_gathered_once()
    {
        var state = new SearchStateService();
        var first = new List<QobuzAlbum> { new() { Id = "a0" }, new() { Id = "a1" } };

        state.AppendAlbums(first, total: 5);
        H.Check("first page is stored with its total", state.Albums!.Count == 2 && state.AlbumsTotal == 5 && state.HasMoreAlbums);

        var before = state.Albums;
        state.AppendAlbums(new[] { new QobuzAlbum { Id = "a2" }, new QobuzAlbum { Id = "a3" } }, total: 5);
        H.Check("next page is added after the first", state.Albums!.Select(a => a.Id).SequenceEqual(new[] { "a0", "a1", "a2", "a3" }));
        H.Check("the previous list instance is untouched (other circuits may be reading it)", before.Count == 2 && !ReferenceEquals(before, state.Albums));
        H.Check("more pages remain while count < total", state.HasMoreAlbums);

        state.AppendAlbums(new[] { new QobuzAlbum { Id = "a4" } }, total: 5);
        H.Check("no more pages once everything is loaded", !state.HasMoreAlbums);

        H.Check("tracks and artists have their own totals", !state.HasMoreTracks && !state.HasMoreArtists);
        state.AppendTracks(new[] { new QobuzTrack { Id = 1 } }, total: 3);
        state.AppendArtists(new[] { new QobuzArtist { Id = 1 } }, total: 1);
        H.Check("tracks paging", state.HasMoreTracks && state.Tracks!.Count == 1);
        H.Check("artists paging", !state.HasMoreArtists && state.Artists!.Count == 1);

        // through the real API client: offsets walk through the result set
        var api = new QobuzApiService(NullLogger<QobuzApiService>.Instance);
        var handler = new PagedAlbums();
        PrivateAccess.SetHttp(api, new HttpClient(handler));
        api.SetCredentials("1", "s", "tok");

        var paged = new SearchStateService();
        for (var offset = 0; ; )
        {
            var page = api.SearchAlbumsAsync("miles davis", 50, offset).GetAwaiter().GetResult()!;
            paged.AppendAlbums(page.Albums.Items, page.Albums.Total);
            offset = paged.Albums!.Count;
            if (!paged.HasMoreAlbums) break;
        }
        H.Check("three requests cover 120 results", handler.Queries.SequenceEqual(new[] { "miles davis@0x50", "miles davis@50x50", "miles davis@100x50" }));
        H.Check("all 120 results gathered, in order, once", paged.Albums!.Count == 120 && paged.Albums.Select(a => a.Id).Distinct().Count() == 120 && paged.Albums[119].Id == "a119");
    }
}
