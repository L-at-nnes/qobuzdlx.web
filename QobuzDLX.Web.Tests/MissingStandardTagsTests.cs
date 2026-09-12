using QobuzDLX.Web.Models;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class MissingStandardTagsTests
{
    [Fact]
    public static void Missing_genre_composer_and_performer_leave_no_empty_frame()
    {
        var fx = new Fixtures();
        var album = new QobuzAlbum { Id = "1", Title = "Album", TracksCount = 10, MediaCount = 1, Artist = new QobuzPerformer { Name = "Artist" } };
        album.Genre = new QobuzGenre { Name = "" };                      // genre missing
        var track = new QobuzTrack { Id = 1, Title = "Song", TrackNumber = 1, Composer = null, Performer = null };

        var path = AudioFiles.WriteMp3();
        fx.Tagging.WriteToFile(path, null, album, track);
        using (var written = TagLib.File.Create(path))
        {
            Console.WriteLine($"   genres=[{string.Join("|", written.Tag.Genres)}] count={written.Tag.Genres.Length}; composers count={written.Tag.Composers.Length}; performers count={written.Tag.Performers.Length}");
            H.Check("missing genre/composer/performer leave no empty frame", written.Tag.Genres.Length == 0 && written.Tag.Composers.Length == 0 && written.Tag.Performers.Length == 0);
        }
        File.Delete(path);

        var flac = AudioFiles.WriteFlac();
        fx.Settings.Update(x => { x.AudioFormat = ".flac"; });
        fx.Tagging.WriteToFile(flac, null, album, track);
        using (var written = TagLib.File.Create(flac))
        {
            var xiph = (TagLib.Ogg.XiphComment)written.GetTag(TagLib.TagTypes.Xiph, false);
            Console.WriteLine($"   flac fields: {string.Join(", ", xiph.Select(k => k + "=[" + string.Join("|", xiph.GetField(k)) + "]"))}");
            H.Check("flac: no empty GENRE/COMPOSER/ARTIST fields", xiph.GetField("GENRE").Length == 0 && xiph.GetField("COMPOSER").Length == 0 && xiph.GetField("ARTIST").Length == 0);
        }
        File.Delete(flac);
    }
}
