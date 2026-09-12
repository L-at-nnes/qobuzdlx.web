using System.Reflection;
using System.Runtime.CompilerServices;
using QobuzDLX.Web.Services;
using Xunit;

// The services read process-wide state (CONFIG_PATH, the temp folder) and several tests start a
// listener on a fixed port: run the tests one after the other.
[assembly: CollectionBehavior(DisableTestParallelization = true)]

namespace QobuzDLX.Web.Tests;

/// <summary>
/// Sends everything the tests write to the temp folder into one throw-away directory, removed when the
/// test process exits, so a run leaves nothing behind.
/// </summary>
internal static class TestEnvironment
{
    public static string Root { get; } = Path.Combine(Path.GetTempPath(), "qobuzdlx-tests-" + Environment.ProcessId);

    [ModuleInitializer]
    internal static void Initialize()
    {
        Directory.CreateDirectory(Root);
        foreach (var variable in new[] { "TMP", "TEMP", "TMPDIR" })
            Environment.SetEnvironmentVariable(variable, Root);

        AppDomain.CurrentDomain.ProcessExit += (_, _) =>
        {
            try { Directory.Delete(Root, recursive: true); }
            catch (IOException) { /* a file still open at exit: the OS temp cleanup will get it */ }
            catch (UnauthorizedAccessException) { }
        };
    }
}

/// <summary>Where the repository's files are, recorded in the test assembly at build time (see the csproj).</summary>
internal static class RepoPaths
{
    public static string Root { get; } = typeof(RepoPaths).Assembly
        .GetCustomAttributes<AssemblyMetadataAttribute>()
        .Single(attribute => attribute.Key == "RepoRoot").Value!;

    public static string WebRoot => Path.Combine(Root, "wwwroot");
}

/// <summary>
/// The few places where the tests need to replace a private collaborator. The reflection is kept here
/// so that a rename in the service breaks one obvious spot.
/// </summary>
internal static class PrivateAccess
{
    public static void SetHttp(QobuzApiService api, HttpClient http) =>
        typeof(QobuzApiService).GetField("_http", BindingFlags.NonPublic | BindingFlags.Instance)!.SetValue(api, http);
}

/// <summary>Assertion with the description of what was expected as the failure message.</summary>
internal static class H
{
    public static void Check(string expectation, bool condition) => Assert.True(condition, expectation);
}
