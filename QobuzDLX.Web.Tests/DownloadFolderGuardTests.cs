using System.Reflection;
using QobuzDLX.Web.Services;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class DownloadFolderGuardTests
{
    [Fact]
    public static void A_path_is_inside_the_folder_only_by_whole_components()
    {
        var method = typeof(TemplateService).GetMethod("IsInside", BindingFlags.NonPublic | BindingFlags.Static)!;
        bool Inside(string b, string p) => (bool)method.Invoke(null, new object[] { Path.GetFullPath(b), Path.GetFullPath(p) })!;

        H.Check("nested path is inside", Inside("/downloads", "/downloads/Artist/Album"));
        H.Check("the base itself is inside", Inside("/downloads", "/downloads"));
        H.Check("base with trailing slash", Inside("/downloads/", "/downloads/Artist"));
        H.Check("sibling sharing the prefix is outside", !Inside("/downloads", "/downloads-evil/x"));
        H.Check("parent is outside", !Inside("/downloads/a", "/downloads"));
        H.Check("traversal is outside", !Inside("/downloads", "/downloads/../etc"));
        H.Check("a folder literally starting with two dots is inside", Inside("/downloads", "/downloads/..hidden/x"));
    }
}
