using System.Net;
using System.Reflection;
using System.Text;
using QobuzDLX.Web.Services;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class LabelPagingTests
{
    private sealed class PagedLabel : HttpMessageHandler
    {
        public const int Total = 1994;
        public int Calls;

        protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken ct)
        {
            Calls++;
            var offset = int.Parse(System.Text.RegularExpressions.Regex.Match(request.RequestUri!.Query, @"offset=(\d+)").Groups[1].Value);
            var count = Math.Max(0, Math.Min(500, Total - offset));
            var items = string.Join(",", Enumerable.Range(offset, count).Select(i => "{\"id\":\"a" + i + "\"}"));
            var body = "{\"id\":9,\"name\":\"Big Label\",\"albums\":{\"offset\":" + offset + ",\"limit\":500,\"total\":" + Total + ",\"items\":[" + items + "]}}";
            return Task.FromResult(new HttpResponseMessage(HttpStatusCode.OK) { Content = new StringContent(body, Encoding.UTF8, "application/json") });
        }
    }

    [Fact]
    public static void Every_page_of_a_label_is_fetched()
    {
        var fx = new Fixtures();
        var handler = new PagedLabel();
        PrivateAccess.SetHttp(fx.Api, new HttpClient(handler));
        fx.Api.SetCredentials("1", "secret", "tok");

        var label = fx.Api.GetLabelAsync("9").GetAwaiter().GetResult();
        var ids = label?.Albums?.Items.Select(a => a.Id).ToList() ?? new();
        H.Check("the whole label catalogue is fetched", ids.Count == PagedLabel.Total && ids.Distinct().Count() == PagedLabel.Total);
        H.Check("four calls for 1994 albums", handler.Calls == 4);
    }
}
