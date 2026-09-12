using Microsoft.AspNetCore.Hosting;
using Microsoft.Extensions.FileProviders;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging.Abstractions;
using QobuzDLX.Web.Services;

namespace QobuzDLX.Web.Tests;

/// <summary>Real service graph wired by hand, without any network access.</summary>
public sealed class Fixtures
{
    private sealed class FakeLifetime : IHostApplicationLifetime
    {
        public CancellationToken ApplicationStarted => CancellationToken.None;
        public CancellationToken ApplicationStopping => CancellationToken.None;
        public CancellationToken ApplicationStopped => CancellationToken.None;
        public void StopApplication() { }
    }

    private sealed class FakeEnv : IWebHostEnvironment
    {
        public string WebRootPath { get; set; } = RepoPaths.WebRoot;
        public IFileProvider WebRootFileProvider { get; set; } = new NullFileProvider();
        public string ApplicationName { get; set; } = "x";
        public IFileProvider ContentRootFileProvider { get; set; } = new NullFileProvider();
        public string ContentRootPath { get; set; } = "";
        public string EnvironmentName { get; set; } = "Test";
    }

    public string ConfigDir { get; }
    public SettingsService Settings { get; }
    public LocalizationService Loc { get; }
    public TemplateService Templates { get; }
    public TaggingService Tagging { get; }
    public QobuzApiService Api { get; }
    public DownloadService Download { get; }

    public Fixtures(string? configDir = null)
    {
        ConfigDir = configDir ?? Path.Combine(Path.GetTempPath(), "qbdlx-fx-" + Guid.NewGuid().ToString("N")[..6]);
        Directory.CreateDirectory(ConfigDir);
        Environment.SetEnvironmentVariable("CONFIG_PATH", ConfigDir);

        Settings = new SettingsService(NullLogger<SettingsService>.Instance);
        Loc = new LocalizationService(Settings, new FakeEnv(), NullLogger<LocalizationService>.Instance);
        Templates = new TemplateService(Settings);
        Tagging = new TaggingService(Settings, Templates, NullLogger<TaggingService>.Instance);
        Api = new QobuzApiService(NullLogger<QobuzApiService>.Instance);
        Download = new DownloadService(Api, Settings, Templates, Tagging, Loc, new FakeLifetime(),
            NullLogger<DownloadService>.Instance);
    }
}
