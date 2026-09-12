using Xunit;

namespace QobuzDLX.Web.Tests;
public static class TemporaryFilesTests
{
    [Fact]
    public static void Leftovers_of_an_interrupted_run_are_purged_at_startup()
    {
        var config = Path.Combine(Path.GetTempPath(), "qbdlx-b09-" + Guid.NewGuid().ToString("N")[..6]);
        var tempDir = Path.Combine(config, "qbdlx-temp");
        Directory.CreateDirectory(tempDir);

        // leftovers of a killed run, plus files that are not ours
        var partial = Path.Combine(tempDir, "qbdlx_downloading-123.flac");
        var other = Path.Combine(tempDir, "unrelated.txt");
        var cover = Path.Combine(Path.GetTempPath(), "qbdlx-abcd1234-0099-600.jpg");
        var foreignJpg = Path.Combine(Path.GetTempPath(), "holiday-b09.jpg");
        foreach (var f in new[] { partial, other, cover, foreignJpg }) File.WriteAllText(f, "x");

        _ = new Fixtures(config);   // building DownloadService runs the purge

        H.Check("stale partial audio purged at startup", !File.Exists(partial));
        H.Check("stale temporary cover purged at startup", !File.Exists(cover));
        H.Check("unrelated files untouched", File.Exists(other) && File.Exists(foreignJpg));
        File.Delete(foreignJpg);
    }
}
