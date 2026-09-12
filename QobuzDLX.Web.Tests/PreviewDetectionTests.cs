using System.Net;
using System.Reflection;
using QobuzDLX.Web.Models;
using QobuzDLX.Web.Services;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class PreviewDetectionTests
{
    private sealed class JsonHandler : HttpMessageHandler
    {
        public int Calls;
        public string Body = "";
        protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken ct)
        {
            Calls++;
            return Task.FromResult(new HttpResponseMessage(HttpStatusCode.OK) { Content = new StringContent(Body) });
        }
    }

    private static (Fixtures Fx, JsonHandler Handler) Create(string body)
    {
        var fx = new Fixtures();
        var handler = new JsonHandler { Body = body };
        PrivateAccess.SetHttp(fx.Api, new HttpClient(handler));
        fx.Api.SetCredentials("1", "secret", "tok");
        return (fx, handler);
    }

    private static Task Run(Fixtures fx, QobuzTrack track, string temp) =>
        (Task)typeof(DownloadService).GetMethod("DownloadWithQualityFallbackAsync", BindingFlags.NonPublic | BindingFlags.Instance)!
            .Invoke(fx.Download, new object[] { track, temp, CancellationToken.None })!;

    [Fact]
    public static void A_thirty_second_preview_is_refused()
    {
        var track = new QobuzTrack { Id = 7, Title = "t", TrackNumber = 1 };
        var temp = Path.Combine(Path.GetTempPath(), "qbdlx-f02.flac");

        var (fx, handler) = Create("""{"url":"https://example.invalid/preview.mp3","format_id":5,"mime_type":"audio/mpeg","sample":true}""");
        var task = Run(fx, track, temp);
        PreviewOnlyException? caught = null;
        try { task.GetAwaiter().GetResult(); } catch (PreviewOnlyException ex) { caught = ex; }
        H.Check("a sample response raises PreviewOnlyException", caught != null);
        H.Check("message explains the preview limit", caught != null && caught.Message.Contains("30"));
        H.Check("no retry, no quality fallback, no download (1 API call)", handler.Calls == 1);
        H.Check("nothing written to disk", !File.Exists(temp));

        var model = Newtonsoft.Json.JsonConvert.DeserializeObject<QobuzStreamUrl>("""{"url":"u","format_id":27,"sampling_rate":96,"bit_depth":24}""")!;
        H.Check("full-track response is not flagged as sample", !model.Sample);
        var preview = Newtonsoft.Json.JsonConvert.DeserializeObject<QobuzStreamUrl>("""{"url":"u","sample":true}""")!;
        H.Check("sample flag is read from the response", preview.Sample);
    }
}
