using Microsoft.AspNetCore.Hosting;
using Microsoft.Extensions.FileProviders;
using Microsoft.Extensions.Logging.Abstractions;
using QobuzDLX.Web.Services;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class LanguageDetectionTests
{
    private sealed class FakeEnv : IWebHostEnvironment
    {
        public string WebRootPath { get; set; } = RepoPaths.WebRoot;
        public IFileProvider WebRootFileProvider { get; set; } = new NullFileProvider();
        public string ApplicationName { get; set; } = "x";
        public IFileProvider ContentRootFileProvider { get; set; } = new NullFileProvider();
        public string ContentRootPath { get; set; } = "";
        public string EnvironmentName { get; set; } = "Test";
    }

    private static LocalizationService Create()
    {
        var dir = Path.Combine(Path.GetTempPath(), "qbdlx-b07-" + Guid.NewGuid().ToString("N")[..6]);
        Directory.CreateDirectory(dir);
        Environment.SetEnvironmentVariable("CONFIG_PATH", dir);
        var settings = new SettingsService(NullLogger<SettingsService>.Instance);
        return new LocalizationService(settings, new FakeEnv(), NullLogger<LocalizationService>.Instance);
    }

    [Fact]
    public static void First_launch_language_comes_from_page_requests_only()
    {
        var loc = Create();
        loc.DetectFromBrowser(null);
        loc.DetectFromBrowser("");
        loc.DetectFromBrowser("   ");
        H.Check("missing/blank header decides nothing", !loc.IsChosen);

        loc.DetectFromBrowser("en-US,en;q=0.9");
        H.Check("english header -> en persisted", loc.IsChosen && loc.Current == LocalizationService.English);

        loc.DetectFromBrowser("fr-FR");
        H.Check("detection runs only once", loc.Current == LocalizationService.English);

        loc = Create();
        loc.DetectFromBrowser("de-DE,de;q=0.9");
        H.Check("other language -> french fallback persisted", loc.IsChosen && loc.Current == LocalizationService.French);
    }
}
