using QobuzDLX.Web.Models;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class ParentalWarningTagTests
{
    // Expected mapping, written out independently of the implementation (taken from the former code).
    private static readonly Dictionary<string, (string Ex, string Cl)> Expected = new()
    {
        ["pa"] = ("Explicit", "Clean"), ["pashort"] = ("E", "C"),
        ["paifex"] = ("Explicit", ""), ["paifexshort"] = ("E", ""),
        ["paifcl"] = ("", "Clean"), ["paifclshort"] = ("", "C"),
        ["paenclosed"] = ("(Explicit)", "(Clean)"), ["paenclosed[]"] = ("[Explicit]", "[Clean]"),
        ["paenclosedshort"] = ("(E)", "(C)"), ["paenclosedshort[]"] = ("[E]", "[C]"),
        ["paifexenclosed"] = ("(Explicit)", ""), ["paifexenclosed[]"] = ("[Explicit]", ""),
        ["paifexenclosedshort"] = ("(E)", ""), ["paifexenclosedshort[]"] = ("[E]", ""),
        ["paifclenclosed"] = ("", "(Clean)"), ["paifclenclosed[]"] = ("", "[Clean]"),
        ["paifclenclosedshort"] = ("", "(C)"), ["paifclenclosedshort[]"] = ("", "[C]"),
    };

    [Fact]
    public static void Album_and_track_advisory_tags_follow_their_own_flag()
    {
        var fx = new Fixtures();
        var cleanAlbum = new QobuzAlbum { Id = "a1", Title = "T", ParentalWarning = false, TracksCount = 10 };
        var explicitAlbum = new QobuzAlbum { Id = "a2", Title = "T", ParentalWarning = true, TracksCount = 10 };
        var explicitTrack = new QobuzTrack { Id = 1, Title = "x", ParentalWarning = true, TrackNumber = 1 };
        var cleanTrack = new QobuzTrack { Id = 2, Title = "x", ParentalWarning = false, TrackNumber = 1 };

        H.Check("%AlbumPA% follows the album, not the track",
            fx.Templates.ApplyTemplate("%AlbumPA%_%TrackPA%", cleanAlbum, explicitTrack, null) == "Clean_Explicit");
        H.Check("explicit album + clean track",
            fx.Templates.ApplyTemplate("%AlbumPA%_%TrackPA%", explicitAlbum, cleanTrack, null) == "Explicit_Clean");

        var allGood = true;
        foreach (var (suffix, (ex, cl)) in Expected)
        {
            foreach (var (scope, explicitFlag) in new[] { ("Track", true), ("Track", false), ("Album", true), ("Album", false) })
            {
                var album = scope == "Album" ? (explicitFlag ? explicitAlbum : cleanAlbum) : cleanAlbum;
                var track = scope == "Track" ? (explicitFlag ? explicitTrack : cleanTrack) : cleanTrack;
                var actual = fx.Templates.ApplyTemplate($"[%{scope}{suffix}%]", album, track, null);
                var wanted = $"[{(explicitFlag ? ex : cl)}]";
                // MakeSafeFilename may turn nothing here: parentheses/brackets are legal
                if (actual != wanted) { allGood = false; Console.WriteLine($"   {scope}{suffix} explicit={explicitFlag}: got '{actual}', wanted '{wanted}'"); }
            }
        }
        H.Check("all 18 variants x 2 scopes x 2 flags", allGood);
        H.Check("album tag with no track still uses the album flag",
            fx.Templates.ApplyTemplate("%AlbumPA%", explicitAlbum, null, null) == "Explicit");
    }
}
