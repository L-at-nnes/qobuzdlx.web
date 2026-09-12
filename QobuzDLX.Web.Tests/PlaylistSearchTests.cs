using System.Net;
using System.Reflection;
using Microsoft.Extensions.Logging.Abstractions;
using QobuzDLX.Web.Services;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class PlaylistSearchTests
{
    private sealed class Recorder : HttpMessageHandler
    {
        public string? LastUrl;
        protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken ct)
        {
            LastUrl = request.RequestUri!.AbsoluteUri;
            return Task.FromResult(new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent("""
                {"playlists":{"offset":50,"limit":50,"total":130,"items":[
                  {"id":1234567,"name":"Late Night Jazz","tracks_count":42,"owner":{"id":9,"name":"Qobuz"},"images150":["https://img/1.jpg","https://img/2.jpg"]},
                  {"id":7,"name":"No cover","tracks_count":3}
                ]}}
                """)
            });
        }
    }

    [Fact]
    public static void Playlist_search_pages_like_the_other_result_types()
    {
        var api = new QobuzApiService(NullLogger<QobuzApiService>.Instance);
        var handler = new Recorder();
        PrivateAccess.SetHttp(api, new HttpClient(handler));
        api.SetCredentials("1", "s", "tok");

        var result = api.SearchPlaylistsAsync("late night & jazz", 50, 50).GetAwaiter().GetResult()!;
        var url = handler.LastUrl!;
        H.Check("calls playlist/search with the escaped query, limit and offset",
            url.Contains("/playlist/search?") && url.Contains("query=late%20night%20%26%20jazz") && url.Contains("limit=50") && url.Contains("offset=50"));
        H.Check("total and items are parsed", result.Playlists.Total == 130 && result.Playlists.Items.Count == 2);

        var first = result.Playlists.Items[0];
        H.Check("playlist fields", first.Id == 1234567 && first.Name == "Late Night Jazz" && first.TracksCount == 42 && first.Owner?.Name == "Qobuz" && first.Images150![0] == "https://img/1.jpg");
        H.Check("optional fields may be absent", result.Playlists.Items[1].Owner == null && result.Playlists.Items[1].Images150 == null);

        var state = new SearchStateService();
        state.AppendPlaylists(result.Playlists.Items, result.Playlists.Total);
        H.Check("playlists page like the other result types", state.HasMorePlaylists && state.PlaylistsTotal == 130);

        // the queue accepts the URL a playlist result produces
        H.Check("playlist URL is a valid queue entry",
            DownloadService.TryParseQobuzUrl($"https://play.qobuz.com/playlist/{first.Id}", out var canonical, out var kind, out var refId)
            && kind == "playlist" && refId == "1234567" && canonical.EndsWith("/playlist/1234567"));
    }
}
