using Microsoft.Extensions.Logging.Abstractions;
using QobuzDLX.Web.Services;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class SettingsPersistenceTests
{
    private static string NewDir()
    {
        var dir = Path.Combine(Path.GetTempPath(), "qbdlx-b04-" + Guid.NewGuid().ToString("N")[..6]);
        Directory.CreateDirectory(dir);
        Environment.SetEnvironmentVariable("CONFIG_PATH", dir);
        return dir;
    }

    private static SettingsService Create() => new(NullLogger<SettingsService>.Instance);

    [Fact]
    public static void Settings_are_written_atomically_and_recovered_when_corrupted()
    {
        // first run: nothing on disk
        var dir = NewDir();
        var svc = Create();
        H.Check("first run -> defaults", svc.Settings.QualityFormat == "27");

        // save creates main, second save creates .bak with previous content, no .tmp left
        svc.Update(s => s.QualityFormat = "6");
        svc.Update(s => s.QualityFormat = "7");
        var main = Path.Combine(dir, "settings.json");
        H.Check("main written", File.Exists(main) && File.ReadAllText(main).Contains("\"7\""));
        H.Check(".bak holds previous version", File.Exists(main + ".bak") && File.ReadAllText(main + ".bak").Contains("\"6\""));
        H.Check("no .tmp left behind", !File.Exists(main + ".tmp"));
        H.Check("reload roundtrip", Create().Settings.QualityFormat == "7");

        // truncated main + valid .bak -> restored, corrupt file kept aside
        File.WriteAllText(main, "{ \"QualityFormat\": \"5\", \"Dow");
        var restored = Create();
        H.Check("corrupt main -> restored from .bak", restored.Settings.QualityFormat == "6");
        H.Check("corrupt main moved aside", Directory.GetFiles(dir, "settings.json.corrupt-*").Length == 1);
        H.Check("main absent after quarantine (next Save recreates)", !File.Exists(main));
        restored.Update(s => s.QualityFormat = "27");
        H.Check("save after recovery works", File.Exists(main) && Create().Settings.QualityFormat == "27");

        // corrupt main, no usable backup -> defaults, no throw
        dir = NewDir();
        File.WriteAllText(Path.Combine(dir, "settings.json"), "not json at all");
        H.Check("corrupt and no .bak -> defaults without throwing", Create().Settings.QualityFormat == "27");

        // empty file
        dir = NewDir();
        File.WriteAllText(Path.Combine(dir, "settings.json"), "");
        H.Check("empty file -> defaults without throwing", Create().Settings.QualityFormat == "27");

        // crash between the two renames of File.Replace: main missing, .bak present
        dir = NewDir();
        File.WriteAllText(Path.Combine(dir, "settings.json.bak"), "{ \"QualityFormat\": \"6\" }");
        H.Check("main missing + .bak -> restored", Create().Settings.QualityFormat == "6");
    }
}
