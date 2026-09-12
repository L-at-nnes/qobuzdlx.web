using QobuzDLX.Web.Models;
using QobuzDLX.Web.Services;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class HiResFilterTests
{
    [Fact]
    public static void The_hi_res_filter_keeps_only_better_than_cd_quality()
    {
        bool Album(int bits, double rate) => SearchSorter.IsHiRes(new QobuzAlbum { MaximumBitDepth = bits, MaximumSamplingRate = rate });
        bool Track(int bits, double rate) => SearchSorter.IsHiRes(new QobuzTrack { MaximumBitDepth = bits, MaximumSamplingRate = rate });

        H.Check("CD quality is not Hi-Res", !Album(16, 44.1) && !Track(16, 44.1));
        H.Check("24-bit is Hi-Res", Album(24, 44.1) && Track(24, 96));
        H.Check("a higher sampling rate alone is Hi-Res", Album(16, 96) && Track(16, 88.2));
        H.Check("missing values do not count as Hi-Res", !Album(0, 0) && !Track(0, 0));
        H.Check("24/192 is Hi-Res", Album(24, 192));
    }
}
