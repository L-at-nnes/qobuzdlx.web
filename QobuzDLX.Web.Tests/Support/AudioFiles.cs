using QobuzDLX.Web.Models;

namespace QobuzDLX.Web.Tests;

public static class AudioFiles
{
    /// <summary>A silent but structurally valid MPEG-1 Layer III stream: 20 frames of 128 kbps / 44.1 kHz.</summary>
    public static string WriteMp3()
    {
        var path = Path.Combine(Path.GetTempPath(), "qbdlx-m05-" + Guid.NewGuid().ToString("N")[..6] + ".mp3");
        using var fs = File.Create(path);
        for (var i = 0; i < 20; i++)
        {
            var frame = new byte[417];
            frame[0] = 0xFF; frame[1] = 0xFB; frame[2] = 0x90; frame[3] = 0x00;
            fs.Write(frame);
        }
        return path;
    }

    /// <summary>Header-only FLAC file (fLaC marker + STREAMINFO, zero frames): enough for TagLib to open and tag it.</summary>
    public static string WriteFlac()
    {
        var path = Path.Combine(Path.GetTempPath(), "qbdlx-m05-" + Guid.NewGuid().ToString("N")[..6] + ".flac");
        var streamInfo = new List<byte> { 0x10, 0x00, 0x10, 0x00, 0, 0, 0, 0, 0, 0 };
        ulong packed = (44100UL << 44) | (1UL << 41) | (15UL << 36);   // rate(20) | channels-1(3) | bits-1(5) | samples(36)
        for (var shift = 56; shift >= 0; shift -= 8) streamInfo.Add((byte)(packed >> shift));
        streamInfo.AddRange(new byte[16]);                                // MD5
        using var fs = File.Create(path);
        fs.Write(new byte[] { (byte)'f', (byte)'L', (byte)'a', (byte)'C', 0x80, 0, 0, 34 });
        fs.Write(streamInfo.ToArray());
        return path;
    }
}
