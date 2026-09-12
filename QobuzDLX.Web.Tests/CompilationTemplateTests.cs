using QobuzDLX.Web.Models;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class CompilationTemplateTests
{
    private static QobuzAlbum Compilation() => new()
    {
        Id = "va1", Title = "Radio Vol. 2", Artist = new QobuzPerformer { Name = "Various Artists" }, TracksCount = 9, MediaCount = 1
    };

    private static QobuzTrack Track() => new()
    {
        Id = 5, Title = "Ponpon Shit", TrackNumber = 1, MediaNumber = 1, Performer = new QobuzPerformer { Name = "namakopuri" }
    };

    [Fact]
    public static void The_compilation_template_only_applies_to_file_names()
    {
        var fx = new Fixtures();
        var t = fx.Templates;
        var playlist = new QobuzPlaylist { Id = 9, Name = "My Mix", TracksCount = 3 };
        var album = Compilation();
        var track = Track();

        // found while testing against the live service: a track of a compilation inside a playlist
        var folder = t.CreatePath("/dl", "%ArtistName%", "%AlbumTitle%", "%PlaylistTitle%", album, track, playlist).Replace('\\', '/');
        H.Check("the playlist folder of a compilation track is the playlist template, not the track template", folder == "/dl/My Mix");

        var file = t.TrackFilePath(folder, "playlist", album, track, playlist).Replace('\\', '/');
        H.Check("the file still uses the compilation track template (track artist in the name)", file == "/dl/My Mix/1 - namakopuri - Ponpon Shit.flac");

        // %ArtistName% means the track's artist in the compilation template
        fx.Settings.Update(s => s.VaTrackTemplate = "%ArtistName% - %TrackTitle%");
        H.Check("%ArtistName% in the compilation template is the track artist", t.ApplyTrackTemplate(album, track, null) == "namakopuri - Ponpon Shit");

        // a slash in the compilation template is a folder separator, as in every other template
        fx.Settings.Update(s => s.VaTrackTemplate = "%TrackArtist%/%TrackNumber% - %TrackTitle%");
        H.Check("a slash in the compilation template makes a sub-folder", t.TrackFilePath("/dl/A", "album", album, track, null).Replace('\\', '/') == "/dl/A/namakopuri/1 - Ponpon Shit.flac");

        // ordinary releases are unaffected
        var normal = new QobuzAlbum { Id = "n", Title = "Alb", Artist = new QobuzPerformer { Name = "Art" }, TracksCount = 5, MediaCount = 1 };
        H.Check("an ordinary release uses the default track template", t.ApplyTrackTemplate(normal, track, null) == "1 - Ponpon Shit");
    }
}
