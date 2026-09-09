using Newtonsoft.Json;
using QobuzDLX.Web.Models;

namespace QobuzDLX.Web.Services;

public class SettingsService
{
    private const string TempSuffix = ".tmp";
    private const string BackupSuffix = ".bak";

    private readonly string _configPath;
    private readonly ILogger<SettingsService> _logger;
    private AppSettings _settings;
    private readonly object _lock = new();

    /// <summary>Dossier de configuration (volume /config dans l'image Docker).</summary>
    public string ConfigDirectory { get; }

    public SettingsService(ILogger<SettingsService> logger)
    {
        _logger = logger;
        ConfigDirectory = Environment.GetEnvironmentVariable("CONFIG_PATH") ?? "/config";
        _configPath = Path.Combine(ConfigDirectory, "settings.json");

        _settings = Load();
    }

    public AppSettings Settings
    {
        get { lock (_lock) return _settings; }
    }

    public void Save()
    {
        lock (_lock)
        {
            var dir = Path.GetDirectoryName(_configPath);
            if (!string.IsNullOrEmpty(dir) && !Directory.Exists(dir))
                Directory.CreateDirectory(dir);

            var json = JsonConvert.SerializeObject(_settings, Formatting.Indented);
            WriteAtomically(json);
        }
    }

    /// <summary>
    /// Écrit dans un fichier voisin puis le substitue à l'original : une coupure en cours d'écriture
    /// laisse l'ancien fichier intact au lieu d'un JSON tronqué. L'ancienne version est conservée
    /// en <c>.bak</c>, dernier recours de <see cref="Load"/>.
    /// </summary>
    private void WriteAtomically(string json)
    {
        var tempPath = _configPath + TempSuffix;
        File.WriteAllText(tempPath, json);

        if (File.Exists(_configPath))
            File.Replace(tempPath, _configPath, _configPath + BackupSuffix);
        else
            File.Move(tempPath, _configPath);
    }

    public void Update(Action<AppSettings> action)
    {
        lock (_lock)
        {
            action(_settings);
            Save();
        }
    }

    /// <summary>Oublie la session Qobuz persistée (déconnexion, ou session expirée).</summary>
    public void ClearQobuzSession() => Update(s =>
    {
        s.UserAuthToken = "";
        s.UserId = "";
        s.UserDisplayName = "";
        s.UserLabel = "";
        s.Email = "";
        s.AppId = "";
        s.AppSecret = "";
    });

    /// <summary>
    /// Ne lève jamais : un réglage illisible ne doit pas empêcher l'application de démarrer. Le
    /// fichier fautif est mis de côté (<c>.corrupt-*</c>) plutôt qu'écrasé par la prochaine
    /// sauvegarde, puis la dernière version saine (<c>.bak</c>) est restaurée si elle existe.
    /// </summary>
    private AppSettings Load()
    {
        if (File.Exists(_configPath))
        {
            if (TryRead(_configPath) is { } settings) return settings;
            Quarantine();
        }

        var backupPath = _configPath + BackupSuffix;
        if (File.Exists(backupPath) && TryRead(backupPath) is { } restored)
        {
            _logger.LogWarning("Settings restored from {Backup}", backupPath);
            return restored;
        }

        return new AppSettings();
    }

    private AppSettings? TryRead(string path)
    {
        try
        {
            return JsonConvert.DeserializeObject<AppSettings>(File.ReadAllText(path));
        }
        catch (Exception ex) when (ex is JsonException or IOException or UnauthorizedAccessException)
        {
            _logger.LogError(ex, "Unable to read settings file {Path}", path);
            return null;
        }
    }

    private void Quarantine()
    {
        var quarantinePath = $"{_configPath}.corrupt-{DateTime.UtcNow:yyyyMMddHHmmss}";
        try
        {
            File.Move(_configPath, quarantinePath);
            _logger.LogWarning("Unreadable settings file moved to {Path}", quarantinePath);
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            _logger.LogError(ex, "Unable to set aside the unreadable settings file {Path}", _configPath);
        }
    }
}
