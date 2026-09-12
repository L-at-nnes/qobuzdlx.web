using Microsoft.Extensions.Logging.Abstractions;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class LegacyTempFolderTests
{
    private static string Dir(string label)
    {
        var dir = Path.Combine(Path.GetTempPath(), $"qbdlx-p07-{label}-" + Guid.NewGuid().ToString("N")[..6]);
        Directory.CreateDirectory(dir);
        return dir;
    }

    private static void UseDownloadLocation(string config, string downloads) =>
        File.WriteAllText(Path.Combine(config, "settings.json"), "{ \"DownloadLocation\": " + System.Text.Json.JsonSerializer.Serialize(downloads) + " }");

    [Fact]
    public static void The_legacy_temp_folder_is_cleaned_in_place()
    {
        // legacy folder holding only leftovers: emptied and removed, current temp folder cleaned too
        var config = Dir("cfg"); var downloads = Dir("dl");
        UseDownloadLocation(config, downloads);
        var legacy = Directory.CreateDirectory(Path.Combine(downloads, "qbdlx-temp")).FullName;
        var current = Directory.CreateDirectory(Path.Combine(config, "qbdlx-temp")).FullName;
        File.WriteAllText(Path.Combine(legacy, "qbdlx_downloading-1.flac"), "x");
        File.WriteAllText(Path.Combine(current, "qbdlx_downloading-2.flac"), "x");
        _ = new Fixtures(config);
        H.Check("legacy folder of leftovers is removed", !Directory.Exists(legacy));
        H.Check("current temp folder is kept and cleaned", Directory.Exists(current) && Directory.GetFiles(current).Length == 0);

        // legacy folder that also holds something else: that file is never touched
        config = Dir("cfg"); downloads = Dir("dl");
        UseDownloadLocation(config, downloads);
        legacy = Directory.CreateDirectory(Path.Combine(downloads, "qbdlx-temp")).FullName;
        File.WriteAllText(Path.Combine(legacy, "qbdlx_downloading-1.flac"), "x");
        File.WriteAllText(Path.Combine(legacy, "my-notes.txt"), "keep me");
        _ = new Fixtures(config);
        H.Check("a legacy folder with other content is not deleted", Directory.Exists(legacy) && File.Exists(Path.Combine(legacy, "my-notes.txt")));
        H.Check("only our leftovers were removed from it", !File.Exists(Path.Combine(legacy, "qbdlx_downloading-1.flac")));

        // download location equal to the config folder: the temp folder must survive
        config = Dir("cfg");
        UseDownloadLocation(config, config);
        current = Directory.CreateDirectory(Path.Combine(config, "qbdlx-temp")).FullName;
        _ = new Fixtures(config);
        H.Check("when both locations coincide the folder is not deleted", Directory.Exists(current));

        // nothing to clean: no error
        config = Dir("cfg"); downloads = Dir("dl");
        UseDownloadLocation(config, downloads);
        _ = new Fixtures(config);
        H.Check("fresh install starts cleanly", true);
    }
}
