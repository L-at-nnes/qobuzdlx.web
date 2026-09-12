using QobuzDLX.Web.Models;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class DiscNumberVariableTests
{
    [Fact]
    public static void Disc_number_works_in_the_track_template()
    {
        var fx = new Fixtures();
        var album = new QobuzAlbum { Id = "1", Title = "T", TracksCount = 20, MediaCount = 2 };
        var track = new QobuzTrack { Id = 1, Title = "x", TrackNumber = 3, MediaNumber = 2 };

        H.Check("%DiscNumber% in a track template", fx.Templates.ApplyTemplate("%DiscNumber%_%TrackNumber%", album, track, null) == "2_03");
        H.Check("no track: tag resolves to nothing, as before", fx.Templates.ApplyTemplate("A%DiscNumber%B", album, null, null) == "AB");
    }
}
