using QobuzDLX.Web.Services;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class PlaylistFileTests
{
    [Fact]
    public static void A_playlist_download_writes_an_m3u8_file()
    {
        // ── the writer on its own ──
        var folder = Path.Combine(Path.GetTempPath(), "pl");
        var text = M3uPlaylist.Build(folder, new[]
        {
            new PlaylistEntry(1, Path.Combine(folder, "02 - Two.flac"), 122, "Art - Two"),
            new PlaylistEntry(0, Path.Combine(folder, "01 - One.flac"), 61, "Art - One"),
            new PlaylistEntry(2, Path.Combine(folder, "Sub", "03 - Très.flac"), 183, "Title\nwith break"),
        });
        var lines = text.Split('\n');
        H.Check("header", lines[0] == "#EXTM3U");
        H.Check("entries follow the playlist order, not the order they were given",
            lines[1] == "#EXTINF:61,Art - One" && lines[2] == "01 - One.flac" && lines[3] == "#EXTINF:122,Art - Two" && lines[4] == "02 - Two.flac");
        H.Check("paths are relative with forward slashes", lines[6] == "Sub/03 - Très.flac");
        H.Check("a line break in a title cannot split the entry", lines[5] == "#EXTINF:183,Title with break");
        H.Check("the file ends with a newline", text.EndsWith("\n") && lines[^1] == "");

        // ── end to end through the real pipeline ──
        var cdn = FakeBackend.StartCdn();
        try
        {
            var (fx, qobuz, downloads) = FakeBackend.NewLibrary();
            const string url = "https://play.qobuz.com/playlist/55";
            var playlistFolder = Path.Combine(downloads, "Mix - Test");
            var m3u = Path.Combine(playlistFolder, "Mix - Test.m3u8");

            fx.Download.Enqueue(url);
            var item = FakeBackend.WaitFinished(fx, url);
            H.Check("the playlist item completes with track 2 failed", item.Status == QueueItemStatus.Completed && item.Failures.Count == 1 && item.Failures[0].TrackId == 2);
            H.Check("the playlist file is written in the playlist folder", File.Exists(m3u));

            var content = File.ReadAllText(m3u);
            H.Check("only the tracks that exist are listed", content.Contains("1 - One.flac") && content.Contains("3 - Three.flac") && !content.Contains("Two"));
            H.Check("durations come from the playlist listing", content.Contains("#EXTINF:61,Art - One") && content.Contains("#EXTINF:183,Three"));
            H.Check("no byte-order mark (plain UTF-8)", File.ReadAllBytes(m3u)[0] == (byte)'#');
            H.Check("the log mentions it", fx.Download.LogSnapshot().Any(l => l.Contains("Mix - Test.m3u8")));

            // retry the failure: the file now lists everything, in playlist order, nothing twice
            qobuz.Track2Works = true;
            fx.Download.RetryFailures(item.Id);
            item = FakeBackend.WaitFinished(fx, url);
            content = File.ReadAllText(m3u);
            var files = content.Split('\n').Where(l => l.EndsWith(".flac")).ToArray();
            H.Check("after the retry all three tracks are listed in order", files.SequenceEqual(new[] { "1 - One.flac", "2 - Two.flac", "3 - Three.flac" }));
            H.Check("every listed file exists next to the playlist", files.All(f => File.Exists(Path.Combine(playlistFolder, f))));

            // switched off
            File.Delete(m3u);
            fx.Settings.Update(s => s.CreatePlaylistFile = false);
            fx.Download.Retry(item.Id);
            FakeBackend.WaitFinished(fx, url);
            H.Check("option off: no playlist file", !File.Exists(m3u));
        }
        finally
        {
            cdn.StopAsync().GetAwaiter().GetResult();
        }
    }
}
