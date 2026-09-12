using QobuzDLX.Web.Models;
using QobuzDLX.Web.Services;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class LogDetailTests
{
    private static IReadOnlyList<string> RunAlbum(LogDetail level, out Fixtures fx)
    {
        var (fixtures, qobuz, downloads) = FakeBackend.NewLibrary();
        fx = fixtures;
        fx.Settings.Update(s => s.LogDetail = level);
        const string url = "https://play.qobuz.com/album/alb";
        fx.Download.Enqueue(url);
        FakeBackend.WaitFinished(fx, url, seconds: 60);     // track 2 fails: 5 attempts with back-off
        return fx.Download.LogSnapshot();
    }

    [Fact]
    public static void The_log_keeps_only_the_requested_level_of_detail()
    {
        var cdn = FakeBackend.StartCdn();
        try
        {
            // ── errors only ──
            var log = RunAlbum(LogDetail.Errors, out var fx);
            var loc = fx.Loc;
            H.Check("errors level keeps the failure", log.Any(l => l.Contains("2")) && log.Any(l => l.Contains("Track restricted")));
            H.Check("errors level keeps the item summary", log.Contains(loc.T("log.itemSummary", 2, 0, 1)));
            H.Check("errors level drops the narration",
                !log.Contains(loc.T("log.queueItemStart", "https://play.qobuz.com/album/alb"))
                && !log.Any(l => l.Contains(loc["log.checkingLink"])) && !log.Contains(loc["log.done"]) && !log.Contains(loc["log.complete"]));

            // ── normal (the default) ──
            log = RunAlbum(LogDetail.Normal, out fx);
            loc = fx.Loc;
            H.Check("normal level narrates each item and track", log.Contains(loc.T("log.queueItemStart", "https://play.qobuz.com/album/alb")) && log.Contains(loc["log.done"]) && log.Contains(loc["log.complete"]));
            H.Check("normal level has no per-track transfer line", !log.Any(l => l.Contains("MB/s") || l.Contains("Mo/s")));
            H.Check("normal level has no per-attempt line", !log.Any(l => l.Contains("/5 ")));

            // ── detailed ──
            log = RunAlbum(LogDetail.Detailed, out fx);
            loc = fx.Loc;
            H.Check("detailed level adds the quality, size and speed of each downloaded track",
                log.Count(l => l.StartsWith(loc["log.trackTransfer"].Split('{')[0])) == 2 && log.Any(l => l.Contains("FLAC 16bit/44.1kHz")));
            H.Check("detailed level adds each failed attempt", log.Count(l => l.Contains("Track restricted")) >= 3);
            H.Check("detailed still has everything from normal", log.Contains(loc["log.complete"]) && log.Contains(loc.T("log.itemSummary", 2, 0, 1)));

            // ── export ──
            var exported = fx.Download.ExportLog();
            var lines = exported.Split('\n');
            H.Check("export starts with a header carrying the version and the level", lines[0].StartsWith("# QobuzDLX " + AppInfo.Version) && lines[0].EndsWith("log detail: Detailed"));
            // (some messages, such as the item separator, begin with a blank line of their own)
            H.Check("export holds every line of the log, in order", exported.Contains("\n" + string.Join("\n", log) + "\n"));
            H.Check("export ends with a newline", exported.EndsWith("\n"));

            // ── the setting round-trips through settings.json as before for old files ──
            H.Check("default level is Normal", new AppSettings().LogDetail == LogDetail.Normal);
        }
        finally
        {
            cdn.StopAsync().GetAwaiter().GetResult();
        }
    }
}
