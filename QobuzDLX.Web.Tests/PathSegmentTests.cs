using System.Reflection;
using QobuzDLX.Web.Services;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class PathSegmentTests
{
    [Fact]
    public static void Dots_and_separators_are_cleaned_per_path_component()
    {
        var method = typeof(TemplateService).GetMethod("SanitizePathSegment", BindingFlags.NonPublic | BindingFlags.Static)!;
        string San(string s) => (string)method.Invoke(null, new object[] { s })!;

        H.Check("blank stays blank (intentional empty template)", San("") == "" && San("   ") == "");
        H.Check("inner double dot preserved", San("Vol..2") == "Vol..2");
        H.Check("bare .. becomes _", San("..") == "_");
        H.Check("traversal component dropped", San("A/../B") == "A/B");
        H.Check("leading traversal dropped", San("../../etc") == "etc");
        H.Check("trailing dots and spaces trimmed", San("Mr. ") == "Mr");
        H.Check("every component is trimmed", San("Artist. /Album ..") == "Artist/Album");
        H.Check("NUL removed", San("A" + (char)0 + "B") == "AB");
        H.Check("only dots and spaces -> _", San(" . . ") == "_");

        var fx = new Fixtures();
        var album = new QobuzDLX.Web.Models.QobuzAlbum { Id = "1", Title = "..", Artist = new QobuzDLX.Web.Models.QobuzPerformer { Name = "Sun" } };
        var path = fx.Templates.CreatePath("/downloads", "%ArtistName%", "%AlbumTitle%", null, album, null, null);
        H.Check("album titled '..' stays inside the download folder", Path.GetFullPath(path).StartsWith(Path.GetFullPath("/downloads")) && Path.GetFileName(path) != "..");
    }
}
