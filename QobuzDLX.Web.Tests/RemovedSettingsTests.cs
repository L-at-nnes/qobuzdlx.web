using Microsoft.Extensions.Logging.Abstractions;
using QobuzDLX.Web.Services;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class RemovedSettingsTests
{
    [Fact]
    public static void Removed_settings_are_dropped_on_the_next_save()
    {
        var dir = Path.Combine(Path.GetTempPath(), "qbdlx-s07-" + Guid.NewGuid().ToString("N")[..6]);
        Directory.CreateDirectory(dir);
        Environment.SetEnvironmentVariable("CONFIG_PATH", dir);
        var path = Path.Combine(dir, "settings.json");
        File.WriteAllText(path, """{ "Password": "hunter2", "UserAvatar": "a.png", "Theme": "dark", "QualityFormat": "6", "Language": "en" }""");

        var svc = new SettingsService(NullLogger<SettingsService>.Instance);
        H.Check("a settings file carrying the removed keys still loads", svc.Settings.QualityFormat == "6");

        svc.Save();
        var rewritten = File.ReadAllText(path);
        H.Check("the plain-text Password is gone after the next save", !rewritten.Contains("hunter2") && !rewritten.Contains("\"Password\""));
        H.Check("other removed keys are gone too", !rewritten.Contains("UserAvatar") && !rewritten.Contains("\"Theme\""));
        H.Check("real settings survive", rewritten.Contains("\"QualityFormat\": \"6\""));
    }
}
