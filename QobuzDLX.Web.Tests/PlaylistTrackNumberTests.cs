using QobuzDLX.Web.Models;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class PlaylistTrackNumberTests
{
    [Fact]
    public static void Track_numbers_are_padded_to_the_size_of_what_they_number()
    {
        var fx = new Fixtures();
        var album = new QobuzAlbum { Id = "1", Title = "T", TracksCount = 9 };
        var track = new QobuzTrack { Id = 1, Title = "x", TrackNumber = 7 };
        var playlist = new QobuzPlaylist { Id = 5, Name = "P", TracksCount = 120 };

        fx.Settings.Update(s => s.UseItemPosInPlaylist = false);
        H.Check("album padding unchanged outside playlist", fx.Templates.ApplyTemplate("%TrackNumber%", album, track, null) == "7");
        H.Check("playlist without positions pads like the album", fx.Templates.ApplyTemplate("%TrackNumber%", album, track, playlist) == "7");

        fx.Settings.Update(s => s.UseItemPosInPlaylist = true);
        H.Check("playlist positions pad to the playlist size", fx.Templates.ApplyTemplate("%TrackNumber%", album, track, playlist) == "007");

        playlist.TracksCount = 0;
        H.Check("unknown playlist size falls back to the album", fx.Templates.ApplyTemplate("%TrackNumber%", album, track, playlist) == "7");

        album.TracksCount = 12;
        H.Check("two-digit album", fx.Templates.ApplyTemplate("%TrackNumber%", album, track, null) == "07");
    }
}
