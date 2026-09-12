using QobuzDLX.Web.Services;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class TemplatePreviewTests
{
    [Fact]
    public static void The_playlist_preview_follows_the_position_option()
    {
        var fx = new Fixtures();
        fx.Settings.Update(s => { s.DownloadLocation = "/downloads"; s.TrackTemplate = "%TrackNumber% - %TrackTitle%"; s.UseItemPosInPlaylist = false; });
        H.Check("playlist preview keeps the album track number by default", fx.Templates.Preview().Playlist == "/downloads/Morning Playlist/05 - Midnight Garden (Live).flac");
        fx.Settings.Update(s => s.UseItemPosInPlaylist = true);
        H.Check("playlist preview uses the playlist position when asked", fx.Templates.Preview().Playlist == "/downloads/Morning Playlist/03 - Midnight Garden (Live).flac");
        H.Check("album preview is unaffected", fx.Templates.Preview().Album == "/downloads/Nina Alvarez/Blue Hour/CD 2/05 - Midnight Garden (Live).flac");
    }
}
