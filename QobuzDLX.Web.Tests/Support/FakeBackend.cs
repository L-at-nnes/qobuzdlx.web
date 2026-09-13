using System.Net;
using System.Reflection;
using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.Http;
using Microsoft.AspNetCore.Routing;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging;
using QobuzDLX.Web.Models;
using QobuzDLX.Web.Services;

namespace QobuzDLX.Web.Tests;

/// <summary>A fake Qobuz: album of three tracks served through the real download pipeline.</summary>
public static class FakeBackend
{
    public const int Port = 5197;

    public sealed class FakeQobuz : HttpMessageHandler
    {
        public bool Track2Works;
        public readonly List<string> Requests = new();

        protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken ct)
        {
            var path = request.RequestUri!.AbsolutePath;
            var query = System.Web.HttpUtility.ParseQueryString(request.RequestUri.Query);
            lock (Requests) Requests.Add(path + "?" + (query["track_id"] ?? query["album_id"]));

            string body;
            var status = HttpStatusCode.OK;
            if (path.EndsWith("/album/get"))
            {
                body = """
                {"id":"alb","title":"Alb","artist":{"id":1,"name":"Art"},"tracks_count":3,"media_count":1,"streamable":true,
                 "maximum_bit_depth":16,"maximum_sampling_rate":44.1,"image":{"large":"http://127.0.0.1:5197/cover_600.jpg","thumbnail":"x"},
                 "tracks":{"total":3,"items":[
                   {"id":1,"title":"One","track_number":1,"streamable":true},
                   {"id":2,"title":"Two","track_number":2,"streamable":true},
                   {"id":3,"title":"Three","track_number":3,"streamable":true}]}}
                """;
            }
            else if (path.EndsWith("/playlist/get") && query["playlist_id"] == "56")
            {
                // two different tracks that the templates name alike ("1 - One")
                body = """
                {"id":56,"name":"Twins","tracks_count":2,"tracks":{"total":2,"items":[
                   {"id":1,"title":"One","position":1,"track_number":1,"duration":61,"performer":{"name":"Art"}},
                   {"id":11,"title":"One","position":2,"track_number":1,"duration":62,"performer":{"name":"Art"}}]}}
                """;
            }
            else if (path.EndsWith("/playlist/get"))
            {
                body = """
                {"id":55,"name":"Mix: Test","tracks_count":3,"tracks":{"total":3,"items":[
                   {"id":1,"title":"One","position":1,"track_number":1,"duration":61,"performer":{"name":"Art"}},
                   {"id":2,"title":"Two","position":2,"track_number":2,"duration":122,"performer":{"name":"Art"}},
                   {"id":3,"title":"Three","position":3,"track_number":3,"duration":183}]}}
                """;
            }
            else if (path.EndsWith("/track/get"))
            {
                var id = int.Parse(query["track_id"]!);
                var title = id switch { 1 or 11 => "One", 2 => "Two", _ => "Three" };
                body = $$"""{"id":{{id}},"title":"{{title}}","track_number":{{(id == 11 ? 1 : id)}},"media_number":1,"streamable":true,"performer":{"name":"Art"},"album":{"id":"alb"} }""";
            }
            else if (path.EndsWith("/track/getFileUrl"))
            {
                if (query["track_id"] == "2" && !Track2Works)
                {
                    status = HttpStatusCode.NotFound;
                    body = """{"code":404,"message":"Track restricted by right holders"}""";
                }
                else
                {
                    body = """{"url":"http://127.0.0.1:5197/track.flac","format_id":6,"mime_type":"audio/flac"}""";
                }
            }
            else
            {
                status = HttpStatusCode.NotFound;
                body = "{}";
            }

            return Task.FromResult(new HttpResponseMessage(status) { Content = new StringContent(body) });
        }
    }

    public static WebApplication StartCdn()
    {
        // A content root without an appsettings.json: the web app's own one, whose Kestrel endpoint wins
        // over UseUrls, must not be picked up
        var builder = WebApplication.CreateBuilder(new WebApplicationOptions { ContentRootPath = TestEnvironment.Root });
        builder.WebHost.UseUrls($"http://127.0.0.1:{Port}");
        builder.Logging.ClearProviders();
        var server = builder.Build();
        var flac = File.ReadAllBytes(AudioFiles.WriteFlac());
        server.MapGet("/track.flac", () => Results.Bytes(flac, "audio/flac"));
        server.MapGet("/cover_600.jpg", () => Results.Bytes(new byte[] { 0xFF, 0xD8, 0xFF, 0xD9 }, "image/jpeg"));
        server.MapGet("/cover_org.jpg", () => Results.Bytes(new byte[] { 1, 2, 3, 4, 5, 6, 7, 8 }, "image/jpeg"));
        server.MapGet("/other_600.jpg", () => Results.Bytes(new byte[] { 9, 9 }, "image/jpeg"));
        server.Start();
        return server;
    }

    /// <summary>
    /// A service graph talking to the fake Qobuz, downloading into a fresh folder. <paramref name="configDir"/>
    /// reuses a configuration folder (a restart); <paramref name="signIn"/> false leaves the session closed
    /// until the test opens it.
    /// </summary>
    public static (Fixtures Fx, FakeQobuz Qobuz, string Downloads) NewLibrary(string? configDir = null, bool signIn = true)
    {
        var fx = new Fixtures(configDir);
        var qobuz = new FakeQobuz();
        PrivateAccess.SetHttp(fx.Api, new HttpClient(qobuz));

        var downloads = Path.Combine(Path.GetTempPath(), "qbdlx-lib-" + Guid.NewGuid().ToString("N")[..6]);
        Directory.CreateDirectory(downloads);
        fx.Settings.Update(s =>
        {
            s.DownloadLocation = downloads;
            s.QualityFormat = "6";
            s.AudioFormat = ".flac";
            s.VerifyIntegrity = false;
            s.DownloadGoodies = false;
        });
        if (signIn) SignIn(fx);
        return (fx, qobuz, downloads);
    }

    public static void SignIn(Fixtures fx) => fx.Api.SetCredentials("1", "secret", "tok");

    public static QueueItem WaitFinished(Fixtures fx, string url, int seconds = 30)
    {
        var until = DateTime.UtcNow.AddSeconds(seconds);
        while (DateTime.UtcNow < until)
        {
            var item = fx.Download.QueueSnapshot().FirstOrDefault(i => i.Url == url);
            if (item is { IsFinished: true }) return item;
            Thread.Sleep(50);
        }
        throw new TimeoutException("item did not finish: " + url);
    }
}
