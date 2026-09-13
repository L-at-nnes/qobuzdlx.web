using System.Net;

namespace QobuzDLX.Web.Services;

public enum SessionCheck { Valid, Rejected, Unreachable }

/// <summary>
/// Rétablit la session Qobuz enregistrée au démarrage quand une file restaurée attend : sans cela, elle
/// ne reprendrait qu'au moment où quelqu'un ouvre l'interface et se connecte. Sans élément en attente il
/// n'y a rien à reprendre, et la connexion reste celle de la page de connexion.
/// </summary>
public sealed class SessionRestoreService : BackgroundService
{
    private const int MaxAttempts = 10;
    private static readonly TimeSpan RetryDelay = TimeSpan.FromSeconds(30);

    private readonly SettingsService _settings;
    private readonly QobuzApiService _api;
    private readonly DownloadService _download;
    private readonly ILogger<SessionRestoreService> _logger;

    public SessionRestoreService(SettingsService settings, QobuzApiService api, DownloadService download, ILogger<SessionRestoreService> logger)
    {
        _settings = settings;
        _api = api;
        _download = download;
        _logger = logger;
    }

    /// <summary>
    /// Interroge Qobuz avec le jeton enregistré. Un jeton refusé est périmé ; toute autre défaillance
    /// (Qobuz injoignable, délai dépassé) ne dit rien de sa validité, et il ne faut pas déconnecter
    /// l'utilisateur à chaque coupure réseau.
    /// </summary>
    public static async Task<SessionCheck> CheckAsync(QobuzApiService api, Models.AppSettings s, CancellationToken ct = default)
    {
        using var timeout = CancellationTokenSource.CreateLinkedTokenSource(ct);
        timeout.CancelAfter(TimeSpan.FromSeconds(10));
        try
        {
            await api.LoginWithTokenAsync(s.AppId, s.UserAuthToken, timeout.Token);
            return SessionCheck.Valid;
        }
        catch (HttpRequestException ex) when (ex.StatusCode is HttpStatusCode.Unauthorized or HttpStatusCode.Forbidden)
        {
            return SessionCheck.Rejected;
        }
        catch (Exception ex) when (ex is HttpRequestException or OperationCanceledException)
        {
            return SessionCheck.Unreachable;
        }
    }

    protected override async Task ExecuteAsync(CancellationToken stoppingToken)
    {
        if (!_download.HasPending) return;

        var s = _settings.Settings;
        if (string.IsNullOrEmpty(s.UserAuthToken) || string.IsNullOrEmpty(s.AppId) || string.IsNullOrEmpty(s.AppSecret))
            return;

        for (var attempt = 1; attempt <= MaxAttempts && !stoppingToken.IsCancellationRequested; attempt++)
        {
            if (_api.IsLoggedIn) return;   // someone signed in from the interface in the meantime

            switch (await CheckAsync(_api, s, stoppingToken))
            {
                case SessionCheck.Valid:
                    // Starts the session, which resumes the restored queue
                    _api.SetCredentials(s.AppId, s.AppSecret, s.UserAuthToken);
                    _logger.LogInformation("Qobuz session restored: the saved download queue resumes.");
                    return;

                case SessionCheck.Rejected:
                    _settings.ClearQobuzSession();
                    _logger.LogWarning("The saved Qobuz session was refused: the restored queue waits for a sign-in.");
                    return;

                default:
                    _logger.LogWarning("Qobuz is unreachable (attempt {Attempt}/{Max}); trying again shortly.", attempt, MaxAttempts);
                    try { await Task.Delay(RetryDelay, stoppingToken); }
                    catch (OperationCanceledException) { return; }
                    break;
            }
        }
    }
}
