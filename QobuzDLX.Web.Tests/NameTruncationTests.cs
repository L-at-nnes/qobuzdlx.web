using System.Globalization;
using System.Text;
using QobuzDLX.Web.Models;
using QobuzDLX.Web.Services;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class NameTruncationTests
{
    private static int Bytes(string s) => Encoding.UTF8.GetByteCount(s);

    [Fact]
    public static void Long_names_are_cut_by_bytes_not_by_characters()
    {
        H.Check("short name untouched", TemplateService.TruncateToBytes("Hello") == "Hello");

        var longAscii = new string('a', 300);
        var cut = TemplateService.TruncateToBytes(longAscii);
        H.Check("long ascii <= 240 bytes and ends with ellipsis", Bytes(cut) <= 240 && cut.EndsWith('…'));

        var japanese = string.Concat(Enumerable.Repeat("音楽の旅", 40)); // 160 chars, 480 bytes
        cut = TemplateService.TruncateToBytes(japanese);
        H.Check("multibyte stays within the byte budget", Bytes(cut) <= 240 && cut.Length < japanese.Length);
        H.Check("multibyte result is valid text", cut.AsSpan().ToString() == new string(cut.ToCharArray()) && !cut.Any(char.IsSurrogate));

        var emoji = string.Concat(Enumerable.Repeat("\U0001F3B5", 100)); // surrogate pairs, 4 bytes each
        cut = TemplateService.TruncateToBytes(emoji);
        var lone = false;
        for (var i = 0; i < cut.Length; i++)
            if (char.IsHighSurrogate(cut[i]) && (i + 1 >= cut.Length || !char.IsLowSurrogate(cut[i + 1]))) lone = true;
            else if (char.IsLowSurrogate(cut[i]) && (i == 0 || !char.IsHighSurrogate(cut[i - 1]))) lone = true;
        H.Check("never splits a surrogate pair", !lone && Bytes(cut) <= 240);

        var combining = string.Concat(Enumerable.Repeat("é", 200)); // e + combining acute
        cut = TemplateService.TruncateToBytes(combining);
        var elements = StringInfo.ParseCombiningCharacters(cut.TrimEnd('…'));
        H.Check("never separates a letter from its combining mark",
            !cut.TrimEnd('…').StartsWith("́") && elements.Length * 2 == cut.TrimEnd('…').Length && Bytes(cut) <= 240);

        H.Check("exactly at the limit is kept", TemplateService.TruncateToBytes(new string('a', 240)).Length == 240);
        H.Check("one byte over is cut", TemplateService.TruncateToBytes(new string('a', 241)).EndsWith('…'));

        // through the template pipeline
        var fx = new Fixtures();
        var album = new QobuzAlbum { Id = "1", Title = new string('b', 400), Artist = new QobuzPerformer { Name = "Artist" }, TracksCount = 12 };
        var track = new QobuzTrack { Id = 5, Title = new string('c', 400), TrackNumber = 3 };
        var name = fx.Templates.ApplyTemplate("%TrackNumber% - %TrackTitle%", album, track, null);
        H.Check("track file name fits (plus extension and rename suffix)", Bytes(name) + ".flac".Length + " (99)".Length <= 255);

        var multi = fx.Templates.ApplyTemplate("%ArtistName%/%AlbumTitle%", album, null, null).Split('/');
        H.Check("each path component is limited on its own", multi.Length == 2 && multi.All(p => Bytes(p) <= 240) && multi[0] == "Artist");

        var japaneseAlbum = new QobuzAlbum { Id = "2", Title = japanese, Artist = new QobuzPerformer { Name = "A" } };
        var dir = fx.Templates.CreatePath("/downloads", "%ArtistName%", "%AlbumTitle%", null, japaneseAlbum, null, null);
        H.Check("album directory component fits", Bytes(Path.GetFileName(dir)) <= 255);
    }
}
