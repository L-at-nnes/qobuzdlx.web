using QobuzDLX.Web.Models;
using QobuzDLX.Web.Services;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class AlbumArtistTagTests
{
    private static QobuzAlbum Collaboration(params string[] names) => new()
    {
        Artist = new QobuzPerformer { Name = names[0] },
        Artists = names.Select((n, i) => new QobuzArtistRole { Id = i, Name = n, Roles = new List<string> { "main-artist" } }).ToList()
    };

    [Fact]
    public static void The_album_artist_follows_the_merge_option()
    {
        H.Check("option off: the release artist only", TaggingService.AlbumArtistName(Collaboration("John Coltrane", "Johnny Hartman"), false) == "John Coltrane");
        H.Check("option on, two artists", TaggingService.AlbumArtistName(Collaboration("John Coltrane", "Johnny Hartman"), true) == "John Coltrane & Johnny Hartman");
        H.Check("option on, three artists", TaggingService.AlbumArtistName(Collaboration("A", "B", "C"), true) == "A, B & C");
        H.Check("option on, a single artist", TaggingService.AlbumArtistName(Collaboration("Solo"), true) == "Solo");
        H.Check("no artist at all", TaggingService.AlbumArtistName(new QobuzAlbum(), true) == "");
    }
}
