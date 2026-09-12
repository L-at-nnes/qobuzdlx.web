using QobuzDLX.Web.Models;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class QobuzUrlTagTests
{
    private static string? UrlTagOf(Fixtures fx, string? trackUrl, string? albumUrl)
    {
        var album = new QobuzAlbum { Id = "1", Title = "A", TracksCount = 1, Url = albumUrl };
        var track = new QobuzTrack { Id = 1, Title = "t", TrackNumber = 1, Url = trackUrl };
        var flac = AudioFiles.WriteFlac();
        try
        {
            fx.Tagging.WriteToFile(flac, null, album, track);
            using var file = TagLib.File.Create(flac);
            var xiph = (TagLib.Ogg.XiphComment)file.GetTag(TagLib.TagTypes.Xiph, false);
            return xiph.GetField("Qobuz URL").FirstOrDefault();
        }
        finally { File.Delete(flac); }
    }

    [Fact]
    public static void Every_store_locale_is_removed_from_the_url_tag()
    {
        var fx = new Fixtures();
        H.Check("fr-fr locale removed", UrlTagOf(fx, null, "https://www.qobuz.com/fr-fr/album/x/abc") == "https://www.qobuz.com/album/x/abc");
        H.Check("other locales removed too", UrlTagOf(fx, null, "https://www.qobuz.com/us-en/album/x/abc") == "https://www.qobuz.com/album/x/abc");
        H.Check("de-de removed", UrlTagOf(fx, "https://www.qobuz.com/de-de/album/x/abc", null) == "https://www.qobuz.com/album/x/abc");
        H.Check("url without locale untouched", UrlTagOf(fx, null, "https://www.qobuz.com/album/x/abc") == "https://www.qobuz.com/album/x/abc");
        H.Check("only the locale segment right after the domain is touched", UrlTagOf(fx, null, "https://www.qobuz.com/fr-fr/album/ab-cd/xy") == "https://www.qobuz.com/album/ab-cd/xy");
        H.Check("track url wins over album url", UrlTagOf(fx, "https://www.qobuz.com/fr-fr/track/1", "https://www.qobuz.com/fr-fr/album/2") == "https://www.qobuz.com/track/1");
        H.Check("no url -> no field", UrlTagOf(fx, null, null) == null);
    }
}
