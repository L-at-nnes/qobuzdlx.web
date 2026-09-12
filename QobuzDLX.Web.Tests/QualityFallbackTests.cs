using System.Reflection;
using QobuzDLX.Web.Services;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class QualityFallbackTests
{
    [Fact]
    public static void The_fallback_chain_never_leaves_flac_for_mp3()
    {
        var method = typeof(DownloadService).GetMethod("BuildQualityFallbackChain",
            BindingFlags.NonPublic | BindingFlags.Static)!;
        string[] Chain(string q) =>
            ((IEnumerable<(string format, string label)>)method.Invoke(null, new object[] { q })!)
            .Select(x => x.format).ToArray();

        H.Check("chain 27 = 27,7,6 (no MP3)", Chain("27").SequenceEqual(new[] { "27", "7", "6" }));
        H.Check("chain 7 = 7,6", Chain("7").SequenceEqual(new[] { "7", "6" }));
        H.Check("chain 6 = 6", Chain("6").SequenceEqual(new[] { "6" }));
        H.Check("chain 5 = 5", Chain("5").SequenceEqual(new[] { "5" }));
        H.Check("chain unknown = raw then FLAC", Chain("99").SequenceEqual(new[] { "99", "27", "7", "6" }));
    }
}
