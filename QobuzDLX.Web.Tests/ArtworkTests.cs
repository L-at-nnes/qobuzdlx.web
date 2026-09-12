using System.Reflection;
using Microsoft.AspNetCore.Builder;
using QobuzDLX.Web.Models;
using QobuzDLX.Web.Services;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class ArtworkTests
{
    private static Task Run(Fixtures fx, string folder, QobuzAlbum album) =>
        (Task)typeof(DownloadService).GetMethod("DownloadArtworkAsync", BindingFlags.NonPublic | BindingFlags.Instance)!
            .Invoke(fx.Download, new object[] { folder, album })!;

    [Fact]
    public static void Artwork_sizes_fall_back_and_never_break_a_download()
    {
        var album = new QobuzAlbum { Id = "9", Image = new QobuzImage { Large = "https://static.qobuz.com/images/covers/ab/cd/abcdef_600.jpg" } };
        H.Check("numeric size", DownloadService.CoverUrl(album, "300") == "https://static.qobuz.com/images/covers/ab/cd/abcdef_300.jpg");
        H.Check("original size", DownloadService.CoverUrl(album, "org") == "https://static.qobuz.com/images/covers/ab/cd/abcdef_org.jpg");
        H.Check("default size is the URL as given", DownloadService.CoverUrl(album, "600") == album.Image.Large);

        var cdn = FakeBackend.StartCdn();
        try
        {
            var fx = new Fixtures();
            fx.Settings.Update(s => { s.SavedArtSize = "org"; s.EmbeddedArtSize = "org"; });

            // the original exists: it is what gets saved and embedded
            var folder = Path.Combine(Path.GetTempPath(), "qbdlx-f07-" + Guid.NewGuid().ToString("N")[..6]);
            var withOriginal = new QobuzAlbum { Id = "o1", Image = new QobuzImage { Large = "http://127.0.0.1:5197/cover_600.jpg" } };
            Run(fx, folder, withOriginal).GetAwaiter().GetResult();
            H.Check("original cover saved to disk", File.ReadAllBytes(Path.Combine(folder, "Cover.jpg")).Length == 8);

            // the original is missing: falls back to 600 px instead of no cover at all
            folder = Path.Combine(Path.GetTempPath(), "qbdlx-f07-" + Guid.NewGuid().ToString("N")[..6]);
            var noOriginal = new QobuzAlbum { Id = "o2", Image = new QobuzImage { Large = "http://127.0.0.1:5197/other_600.jpg" } };
            Run(fx, folder, noOriginal).GetAwaiter().GetResult();
            H.Check("missing original falls back to the 600 px cover", File.ReadAllBytes(Path.Combine(folder, "Cover.jpg")).Length == 2);

            var embedded = Path.Combine(Path.GetTempPath(), $"qbdlx-{typeof(DownloadService).GetField("_sessionId", BindingFlags.NonPublic | BindingFlags.Instance)!.GetValue(fx.Download)}-o2-org.jpg");
            H.Check("embedded copy also falls back", File.Exists(embedded) && File.ReadAllBytes(embedded).Length == 2);
            File.Delete(embedded);

            // a 600 px request that fails is not retried with itself
            fx.Settings.Update(s => { s.SavedArtSize = "600"; s.EmbeddedArtSize = "600"; });
            folder = Path.Combine(Path.GetTempPath(), "qbdlx-f07-" + Guid.NewGuid().ToString("N")[..6]);
            var broken = new QobuzAlbum { Id = "o3", Image = new QobuzImage { Large = "http://127.0.0.1:5197/missing_600.jpg" } };
            Run(fx, folder, broken).GetAwaiter().GetResult();   // artwork errors are logged, never thrown
            H.Check("an unreachable cover does not break the download", !File.Exists(Path.Combine(folder, "Cover.jpg")));
        }
        finally
        {
            cdn.StopAsync().GetAwaiter().GetResult();
        }
    }
}
