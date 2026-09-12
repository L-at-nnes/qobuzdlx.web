using QobuzDLX.Web.Models;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class MissingTagValuesTests
{
    [Fact]
    public static void Tags_without_a_value_are_left_out()
    {
        var fx = new Fixtures();
        var album = new QobuzAlbum { Id = "1", Title = "Album", TracksCount = 10, MediaCount = 1, Artist = new QobuzPerformer { Name = "Artist" }, Description = "<p>  </p>" };
        var track = new QobuzTrack { Id = 1, Title = "Song", TrackNumber = 1 };   // no ISRC, composer, performer, label, UPC, genre, copyright

        fx.Settings.Update(s => { s.AudioFormat = ".flac"; s.CommentTag = true; s.CommentText = "%description%"; });
        var flac = AudioFiles.WriteFlac();
        fx.Tagging.WriteToFile(flac, null, album, track);
        using (var written = TagLib.File.Create(flac))
        {
            var xiph = (TagLib.Ogg.XiphComment)written.GetTag(TagLib.TagTypes.Xiph, false);
            var empty = xiph.Where(k => xiph.GetField(k).All(string.IsNullOrEmpty)).ToArray();
            Console.WriteLine($"   flac fields: {string.Join(", ", xiph.Select(k => k + "=[" + string.Join("|", xiph.GetField(k)) + "]"))}");
            H.Check("flac: no field is written empty", empty.Length == 0);
            H.Check("flac: ISRC, LABEL, BARCODE and COMMENT are left out", new[] { "ISRC", "LABEL", "BARCODE", "COMMENT" }.All(k => xiph.GetField(k).Length == 0));
        }
        File.Delete(flac);

        var mp3 = AudioFiles.WriteMp3();
        fx.Tagging.WriteToFile(mp3, null, album, track);
        using (var written = TagLib.File.Create(mp3))
        {
            var id3 = (TagLib.Id3v2.Tag)written.GetTag(TagLib.TagTypes.Id3v2, false);
            var frames = id3.Select(f => f is TagLib.Id3v2.UserTextInformationFrame u ? "TXXX:" + u.Description + "=[" + string.Join("|", u.Text) + "]" : f.FrameId.ToString()).ToArray();
            Console.WriteLine($"   mp3 frames: {string.Join(", ", frames)}");
            H.Check("mp3: TSRC, TPUB and COMM are left out", !frames.Contains("TSRC") && !frames.Contains("TPUB") && !frames.Contains("COMM"));
            H.Check("mp3: no barcode frame without a barcode", !frames.Any(f => f.StartsWith("TXXX:BARCODE")));
        }
        File.Delete(mp3);
    }
}
