using System.Net;

namespace QobuzDLX.Web.Services;

public enum CheckStatus { Ok, Warning, Error }

/// <summary>
/// Verdict sur un point de la configuration. <see cref="Code"/> désigne le message à afficher et
/// <see cref="Detail"/> le complète (chemin, version, erreur système).
/// </summary>
public sealed record CheckResult(string Check, CheckStatus Status, string Code, string? Detail = null);

/// <summary>
/// Le « test de configuration » des Réglages : ce qui empêche un téléchargement d'aboutir se découvre
/// d'ordinaire piste après piste dans le journal ; ici, tout est vérifié d'un coup et nommé.
/// </summary>
public class ConfigurationCheckService
{
    private static readonly TimeSpan NetworkTimeout = TimeSpan.FromSeconds(10);

    private readonly SettingsService _settings;
    private readonly QobuzApiService _api;

    public ConfigurationCheckService(SettingsService settings, QobuzApiService api)
    {
        _settings = settings;
        _api = api;
    }

    public async Task<IReadOnlyList<CheckResult>> RunAsync(CancellationToken ct = default) => new[]
    {
        CheckFolderWritable("downloadFolder", _settings.Settings.DownloadLocation),
        CheckFolderWritable("configFolder", _settings.ConfigDirectory),
        await CheckFlacAsync(ct),
        await CheckSessionAsync(ct),
        await CheckAppCredentialsAsync(),
    };

    /// <summary>Crée puis supprime un fichier d'essai : seule une écriture réelle prouve les droits.</summary>
    private static CheckResult CheckFolderWritable(string check, string folder)
    {
        string? probe = null;
        try
        {
            Directory.CreateDirectory(folder);
            probe = Path.Combine(folder, $".qbdlx-write-test-{Guid.NewGuid():N}");
            File.WriteAllText(probe, "ok");
            return new CheckResult(check, CheckStatus.Ok, "writable", folder);
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or ArgumentException)
        {
            return new CheckResult(check, CheckStatus.Error, "notWritable", $"{folder} — {ex.Message}");
        }
        finally
        {
            if (probe != null)
            {
                try { File.Delete(probe); }
                catch (Exception ex) when (ex is IOException or UnauthorizedAccessException) { /* best effort */ }
            }
        }
    }

    /// <summary>Absent, flac n'est pas bloquant (il n'est utile qu'à deux options) : un avertissement.</summary>
    private static async Task<CheckResult> CheckFlacAsync(CancellationToken ct)
    {
        try
        {
            var run = await FlacCli.RunAsync(new[] { "--version" }, ct);
            return run.ExitCode == 0
                ? new CheckResult("flac", CheckStatus.Ok, "found", run.Output.Split('\n')[0])
                : new CheckResult("flac", CheckStatus.Warning, "failed", run.Error);
        }
        catch (OperationCanceledException) { throw; }
        catch (Exception)
        {
            return new CheckResult("flac", CheckStatus.Warning, "missing");
        }
    }

    private async Task<CheckResult> CheckSessionAsync(CancellationToken ct)
    {
        if (!_api.IsLoggedIn)
            return new CheckResult("session", CheckStatus.Error, "notSignedIn");

        using var timeout = CancellationTokenSource.CreateLinkedTokenSource(ct);
        timeout.CancelAfter(NetworkTimeout);

        try
        {
            var login = await _api.LoginWithTokenAsync(_api.AppId ?? "", _api.UserAuthToken!, timeout.Token);
            return new CheckResult("session", CheckStatus.Ok, "valid", login.User?.DisplayName);
        }
        catch (OperationCanceledException) when (ct.IsCancellationRequested) { throw; }
        catch (HttpRequestException ex) when (ex.StatusCode is HttpStatusCode.Unauthorized or HttpStatusCode.Forbidden)
        {
            return new CheckResult("session", CheckStatus.Error, "rejected");
        }
        catch (Exception ex) when (ex is HttpRequestException or OperationCanceledException)
        {
            return new CheckResult("session", CheckStatus.Error, "unreachable", ex.Message);
        }
    }

    private async Task<CheckResult> CheckAppCredentialsAsync()
    {
        if (string.IsNullOrEmpty(_api.AppSecret))
            return new CheckResult("credentials", CheckStatus.Error, "missing");

        return await _api.AppCredentialsAreValidAsync()
            ? new CheckResult("credentials", CheckStatus.Ok, "accepted")
            : new CheckResult("credentials", CheckStatus.Error, "refused");
    }
}
