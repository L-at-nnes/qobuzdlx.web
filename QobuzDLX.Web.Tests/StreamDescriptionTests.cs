using System.Reflection;
using QobuzDLX.Web.Models;
using QobuzDLX.Web.Services;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class StreamDescriptionTests
{
    private static string Describe(QobuzStreamUrl stream, string requested) =>
        (string)typeof(DownloadService).GetMethod("DescribeStream", BindingFlags.NonPublic | BindingFlags.Static)!
            .Invoke(null, new object[] { stream, requested })!;

    [Fact]
    public static void The_log_describes_the_stream_actually_served()
    {
        // fields as returned by the live service (see track/getFileUrl)
        H.Check("16-bit answer to a 192 kHz request is described as what it is",
            Describe(new QobuzStreamUrl { FormatId = 6, MimeType = "audio/flac", BitDepth = 16, SamplingRate = 44.1 }, "FLAC 24bit/192kHz") == "FLAC 16bit/44.1kHz");
        H.Check("24/96 answer to a 192 kHz request",
            Describe(new QobuzStreamUrl { FormatId = 7, MimeType = "audio/flac", BitDepth = 24, SamplingRate = 96 }, "FLAC 24bit/192kHz") == "FLAC 24bit/96kHz");
        H.Check("24/192 answer", Describe(new QobuzStreamUrl { FormatId = 27, BitDepth = 24, SamplingRate = 192 }, "x") == "FLAC 24bit/192kHz");
        H.Check("88.2 kHz is written with a dot whatever the culture",
            Describe(new QobuzStreamUrl { FormatId = 7, BitDepth = 24, SamplingRate = 88.2 }, "x") == "FLAC 24bit/88.2kHz");
        H.Check("MP3", Describe(new QobuzStreamUrl { FormatId = 5, MimeType = "audio/mpeg" }, "MP3 320") == "MP3 320");
        H.Check("a response without details falls back to the requested label",
            Describe(new QobuzStreamUrl { FormatId = 6 }, "FLAC 16bit/44.1kHz") == "FLAC 16bit/44.1kHz");
    }
}
