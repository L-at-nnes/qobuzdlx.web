using QobuzDLX.Web.Models;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class EmbeddedArtworkTests
{
    [Fact]
    public static void Artwork_is_embedded_in_flac_and_mp3()
    {
        var fx = new Fixtures();
        var album = new QobuzAlbum { Id = "1", Title = "A", TracksCount = 1 };
        var track = new QobuzTrack { Id = 1, Title = "t", TrackNumber = 1 };
        var cover = Path.Combine(Path.GetTempPath(), "qbdlx-m13-cover.jpg");
        var jpeg = new byte[2048];
        jpeg[0] = 0xFF; jpeg[1] = 0xD8; jpeg[^2] = 0xFF; jpeg[^1] = 0xD9;
        File.WriteAllBytes(cover, jpeg);

        foreach (var (kind, path) in new[] { ("flac", AudioFiles.WriteFlac()), ("mp3", AudioFiles.WriteMp3()) })
        {
            fx.Tagging.WriteToFile(path, cover, album, track);
            using var file = TagLib.File.Create(path);
            var pictures = file.Tag.Pictures;
            H.Check($"{kind}: one embedded picture", pictures.Length == 1);
            H.Check($"{kind}: front cover, image/jpeg, bytes intact",
                pictures.Length == 1 && pictures[0].Type == TagLib.PictureType.FrontCover
                && pictures[0].MimeType == "image/jpeg" && pictures[0].Data.Count == jpeg.Length);
            file.Dispose();
            File.Delete(path);
        }

        var none = AudioFiles.WriteMp3();
        fx.Tagging.WriteToFile(none, null, album, track);
        using (var file = TagLib.File.Create(none))
            H.Check("no artwork path -> no picture", file.Tag.Pictures.Length == 0);
        File.Delete(none);
        File.Delete(cover);
    }
}
