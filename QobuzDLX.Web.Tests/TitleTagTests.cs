using QobuzDLX.Web.Models;
using QobuzDLX.Web.Services;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class TitleTagTests
{
    private static string TitleOf(Fixtures fx, bool explicitTrack)
    {
        var album = new QobuzAlbum { Id = "1", Title = "A", TracksCount = 1 };
        var track = new QobuzTrack { Id = 1, Title = "Song", TrackNumber = 1, ParentalWarning = explicitTrack };
        var flac = AudioFiles.WriteFlac();
        try
        {
            fx.Tagging.WriteToFile(flac, null, album, track);
            using var file = TagLib.File.Create(flac);
            return file.Tag.Title;
        }
        finally { File.Delete(flac); }
    }

    [Fact]
    public static void Explicit_and_clean_markers_are_added_to_the_title()
    {
        H.Check("suffix", TaggingService.ApplyTitleTag("Song", " (Explicit)", 1) == "Song (Explicit)");
        H.Check("prefix", TaggingService.ApplyTitleTag("Song", "[E] ", 0) == "[E] Song");
        H.Check("spaces are kept exactly as typed", TaggingService.ApplyTitleTag("Song", "(Explicit)", 1) == "Song(Explicit)");

        var fx = new Fixtures();
        H.Check("off by default", TitleOf(fx, true) == "Song" && TitleOf(fx, false) == "Song");

        fx.Settings.Update(s => { s.ExplicitTagInTitle = true; });
        H.Check("explicit track gets the default suffix", TitleOf(fx, true) == "Song (Explicit)");
        H.Check("clean track is untouched while only the explicit tag is on", TitleOf(fx, false) == "Song");

        fx.Settings.Update(s => { s.ExplicitTitleTagText = "[E] "; s.ExplicitTitleTagPrefixOrSuffix = 0; s.CleanTagInTitle = true; s.CleanTitleTagText = " [CLEAN]"; });
        H.Check("custom explicit text as a prefix", TitleOf(fx, true) == "[E] Song");
        H.Check("custom clean text as a suffix", TitleOf(fx, false) == "Song [CLEAN]");

        fx.Settings.Update(s => s.ExplicitTitleTagText = "   ");
        H.Check("a blank text adds nothing", TitleOf(fx, true) == "Song");
    }
}
