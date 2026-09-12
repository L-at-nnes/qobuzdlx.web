using QobuzDLX.Web.Services;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class PlaylistNameCollisionTests
{
    [Fact]
    public static void Tracks_sharing_a_file_name_are_kept_apart()
    {
        var cdn = FakeBackend.StartCdn();
        try
        {
            var (fx, qobuz, downloads) = FakeBackend.NewLibrary();
            const string url = "https://play.qobuz.com/playlist/56";

            fx.Download.Enqueue(url);
            var item = FakeBackend.WaitFinished(fx, url);
            var folder = Path.Combine(downloads, "Twins");
            var files = Directory.GetFiles(folder, "*.flac").Select(Path.GetFileName).OrderBy(n => n).ToArray();

            H.Check("both tracks are downloaded", item.Downloaded == 2 && item.Skipped == 0 && item.Failures.Count == 0);
            H.Check("the first keeps the plain name, the second gets its id", files.SequenceEqual(new[] { "1 - One (11).flac", "1 - One.flac" }));

            var m3u = File.ReadAllText(Directory.GetFiles(folder, "*.m3u8").Single());
            H.Check("the playlist file points each entry at its own file", m3u.Contains("1 - One.flac") && m3u.Contains("1 - One (11).flac"));

            // downloading the playlist again recognises both files instead of re-downloading or colliding
            fx.Download.Retry(item.Id);
            var again = FakeBackend.WaitFinished(fx, url);
            H.Check("a second run skips both, writes nothing new", again.Downloaded == 0 && again.Skipped == 2
                && Directory.GetFiles(folder, "*.flac").Length == 2);
        }
        finally
        {
            cdn.StopAsync().GetAwaiter().GetResult();
        }
    }
}
