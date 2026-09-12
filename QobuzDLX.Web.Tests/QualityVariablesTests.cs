using QobuzDLX.Web.Models;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class QualityVariablesTests
{
    private static QobuzAlbum Album(int bits, double rate) => new() { Id = "1", Title = "A", Artist = new QobuzPerformer { Name = "X" }, TracksCount = 3, MaximumBitDepth = bits, MaximumSamplingRate = rate };
    private static QobuzTrack Track(int bits, double rate) => new() { Id = 2, Title = "T", TrackNumber = 1, MaximumBitDepth = bits, MaximumSamplingRate = rate };

    private static string Names(string quality, int bits, double rate)
    {
        var fx = new Fixtures();
        fx.Settings.Update(s => { s.QualityFormat = quality; s.AudioFormat = quality == "5" ? ".mp3" : ".flac"; });
        var album = Album(bits, rate);
        var track = Track(bits, rate);
        var a = fx.Templates.ApplyTemplate("%FormatWithQuality%_%FormatWithHiResQuality%_%BitDepth%_%SampleRate%", album, null, null);
        var t = fx.Templates.ApplyTemplate("%TrackFormatWithQuality%_%TrackFormatWithHiResQuality%_%TrackBitDepth%_%TrackSampleRate%", album, track, null);
        return a + " // " + t;
    }

    [Fact]
    public static void Quality_variables_describe_the_downloaded_quality()
    {
        // Found live: FLAC 16-bit requested on a 24/96 album gave a folder named "...24bit-96kHz" holding 16-bit files
        H.Check("16-bit requested on a 24/96 release is labelled 16-bit/44.1",
            Names("6", 24, 96) == "FLAC (16bit-44.1kHz)_FLAC_16_44.1 // FLAC (16bit-44.1kHz)_FLAC_16_44.1");

        H.Check("24/96 requested on a 24/96 release", Names("7", 24, 96) == "FLAC (24bit-96kHz)_FLAC (24bit-96kHz)_24_96 // FLAC (24bit-96kHz)_FLAC (24bit-96kHz)_24_96");
        H.Check("24/96 tier on a 24/192 release gives 96 kHz", Names("7", 24, 192).StartsWith("FLAC (24bit-96kHz)_FLAC (24bit-96kHz)_24_96"));
        H.Check("24/96 tier on a 24/176.4 release gives 88.2 kHz", Names("7", 24, 176.4).StartsWith("FLAC (24bit-88.2kHz)_FLAC (24bit-88.2kHz)_24_88.2"));
        H.Check("24/96 tier on a 24/48 release", Names("7", 24, 48).StartsWith("FLAC (24bit-48kHz)_FLAC (24bit-48kHz)_24_48"));
        H.Check("best available on a 24/192 release", Names("27", 24, 192).StartsWith("FLAC (24bit-192kHz)_FLAC (24bit-192kHz)_24_192"));
        H.Check("best available on a 24/96 release", Names("27", 24, 96).StartsWith("FLAC (24bit-96kHz)_FLAC (24bit-96kHz)_24_96"));
        H.Check("best available on a CD-quality release", Names("27", 16, 44.1).StartsWith("FLAC (16bit-44.1kHz)_FLAC_16_44.1"));
        H.Check("16-bit requested on a CD-quality release", Names("6", 16, 44.1).StartsWith("FLAC (16bit-44.1kHz)_FLAC_16_44.1"));
        H.Check("16-bit requested on a 16/48 release keeps its real rate", Names("6", 16, 48).StartsWith("FLAC (16bit-44.1kHz)_FLAC_16_44.1"));
        H.Check("MP3 on a hi-res release", Names("5", 24, 96).StartsWith("MP3_MP3_16_44.1"));
        H.Check("unknown quality values do not crash", Names("27", 0, 0).Length > 0 && Names("6", 0, 0).StartsWith("FLAC (16bit-44.1kHz)"));
    }
}
