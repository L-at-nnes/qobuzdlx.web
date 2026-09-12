using System.Reflection;
using QobuzDLX.Web.Services;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class ProgressScopeTests
{
    private const BindingFlags NonPublic = BindingFlags.NonPublic | BindingFlags.Instance;

    [Fact]
    public static void Nested_scopes_map_progress_into_their_share_of_the_bar()
    {
        var fx = new Fixtures();
        var type = typeof(DownloadService);
        var update = type.GetMethod("UpdateProgress", NonPublic)!;
        var enter = type.GetMethod("EnterScope", NonPublic)!;
        void Report(int percent) => update.Invoke(fx.Download, new object[] { percent, "s", "" });
        IDisposable Enter(int index, int total) => (IDisposable)enter.Invoke(fx.Download, new object[] { index, total })!;

        Report(40);
        H.Check("no scope: percent unchanged", fx.Download.Progress.Percent == 40);

        // artist with 4 albums, 3rd album (index 2) halfway through
        using (Enter(2, 4))
        {
            Report(0);
            H.Check("album 3/4 starts at 50 %", fx.Download.Progress.Percent == 50);
            Report(50);
            H.Check("album 3/4 halfway is 62 %", fx.Download.Progress.Percent == 62);
            Report(100);
            H.Check("album 3/4 end reaches 75 %", fx.Download.Progress.Percent == 75);
        }
        Report(10);
        H.Check("leaving the scope restores the full range", fx.Download.Progress.Percent == 10);

        // nested: favourite artists (2) -> artist -> albums (5)
        using (Enter(1, 2))          // second artist: 50..100
        using (Enter(4, 5))          // fifth album: 90..100 of it
        {
            Report(0);
            H.Check("nested scopes compose (second artist, last album) = 90 %", fx.Download.Progress.Percent == 90);
            Report(100);
            H.Check("nested scopes end at 100 %", fx.Download.Progress.Percent == 100);
        }
        Report(0);
        H.Check("nested scopes fully released", fx.Download.Progress.Percent == 0);
    }
}
