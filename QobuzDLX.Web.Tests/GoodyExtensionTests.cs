using System.Reflection;
using QobuzDLX.Web.Services;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class GoodyExtensionTests
{
    [Fact]
    public static void The_extension_comes_from_the_path_not_the_query()
    {
        var method = typeof(DownloadService).GetMethod("GoodyExtension", BindingFlags.NonPublic | BindingFlags.Static)!;
        string Ext(string? url) => (string)method.Invoke(null, new object?[] { url })!;

        H.Check("plain url", Ext("https://static.qobuz.com/goodies/booklet.pdf") == ".pdf");
        H.Check("signed url keeps only the extension", Ext("https://cdn.qobuz.com/g/booklet.pdf?Expires=1790000000&Signature=abc") == ".pdf");
        H.Check("fragment ignored", Ext("https://cdn.qobuz.com/g/booklet.zip#x") == ".zip");
        H.Check("no extension -> empty", Ext("https://cdn.qobuz.com/g/booklet") == "");
        H.Check("null -> empty", Ext(null) == "");
        H.Check("garbage -> empty", Ext("not a url") == "");
    }
}
