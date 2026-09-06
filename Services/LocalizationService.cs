using System.Globalization;
using Newtonsoft.Json;

namespace QobuzDLX.Web.Services;

/// <summary>
/// Traduction de l'interface. Les dictionnaires sont chargés une fois au démarrage depuis
/// wwwroot/languages/{fr,en}.json. La langue est un réglage <b>global</b> (comme le reste de
/// settings.json) : ce service est donc un singleton, à l'image de <see cref="SettingsService"/>.
/// </summary>
public class LocalizationService
{
    public const string French = "fr";
    public const string English = "en";

    private readonly SettingsService _settings;
    private readonly ILogger<LocalizationService> _logger;
    private readonly Dictionary<string, Dictionary<string, string>> _catalogs = new();

    public LocalizationService(SettingsService settings, IWebHostEnvironment env,
        ILogger<LocalizationService> logger)
    {
        _settings = settings;
        _logger = logger;

        var dir = Path.Combine(env.WebRootPath ?? "wwwroot", "languages");
        _catalogs[French] = LoadCatalog(Path.Combine(dir, "fr.json"));
        _catalogs[English] = LoadCatalog(Path.Combine(dir, "en.json"));

        WarnAboutMissingKeys();
        ApplyCulture(Current);
    }

    /// <summary>Langue effective. Le français est le repli tant qu'aucun choix n'est enregistré.</summary>
    public string Current
    {
        get
        {
            var lang = _settings.Settings.UiLanguage;
            return lang == English ? English : French;
        }
    }

    public bool IsChosen => !string.IsNullOrEmpty(_settings.Settings.UiLanguage);

    public string this[string key] => Lookup(key);

    /// <summary>Chaîne à trous : la valeur traduite est un modèle <c>string.Format</c> ({0}, {1}...).</summary>
    public string T(string key, params object?[] args)
    {
        var format = Lookup(key);
        try
        {
            return string.Format(CultureInfo.CurrentCulture, format, args);
        }
        catch (FormatException)
        {
            // Un modèle mal formé ne doit jamais faire tomber un rendu.
            _logger.LogWarning("Modèle de traduction invalide pour la clé '{Key}' : {Format}", key, format);
            return format;
        }
    }

    /// <summary>
    /// Pluriel simple à deux formes, via les clés <c>{key}.one</c> et <c>{key}.other</c>. Le nombre
    /// est injecté en {0}. Les règles diffèrent : le français met 0 au singulier (« 0 piste »),
    /// l'anglais au pluriel (« 0 tracks »).
    /// </summary>
    public string P(string key, int count)
    {
        var singular = Current == French ? count <= 1 : count == 1;
        return T($"{key}.{(singular ? "one" : "other")}", count);
    }

    /// <summary>
    /// Signalé après un changement de langue. Les composants affichés s'y abonnent pour se
    /// redessiner : la bascule ne recharge pas la page, ce qui préserverait sinon la session
    /// (le déverrouillage par mot de passe vit dans le circuit Blazor).
    /// </summary>
    public event Action? OnLanguageChanged;

    public void SetLanguage(string lang)
    {
        var normalized = lang == English ? English : French;
        // Comparé à la valeur enregistrée et non à Current : au premier lancement le réglage est
        // vide alors que Current vaut déjà "fr", et le choix doit malgré tout être persisté.
        if (normalized == _settings.Settings.UiLanguage) return;

        _settings.Update(s => s.UiLanguage = normalized);
        ApplyCulture(normalized);
        OnLanguageChanged?.Invoke();
    }

    /// <summary>
    /// Premier lancement : aucune langue enregistrée, on suit l'en-tête Accept-Language du
    /// navigateur. Anglais si le navigateur est anglophone, français dans tous les autres cas.
    /// Le choix est persisté immédiatement pour que la détection ne rejoue pas.
    /// Sans en-tête, rien n'est décidé : la requête ne vient pas d'un navigateur (sonde de l'application
    /// Android, supervision) et ne doit pas figer la langue de tous les utilisateurs.
    /// </summary>
    public void DetectFromBrowser(string? acceptLanguage)
    {
        if (IsChosen || string.IsNullOrWhiteSpace(acceptLanguage)) return;

        var first = (acceptLanguage ?? "")
            .Split(',', StringSplitOptions.RemoveEmptyEntries)
            .FirstOrDefault()?
            .Split(';')[0]
            .Trim();

        var detected = first != null && first.StartsWith("en", StringComparison.OrdinalIgnoreCase)
            ? English
            : French;

        SetLanguage(detected);
        _logger.LogInformation("Langue détectée au premier lancement : {Lang} (Accept-Language: {Header})",
            detected, acceptLanguage);
    }

    // ─── Interne ───

    private string Lookup(string key)
    {
        // Repli en cascade : langue courante -> français -> la clé elle-même (visible à l'écran,
        // donc immédiatement repérable en cas d'oubli).
        if (_catalogs.TryGetValue(Current, out var catalog) && catalog.TryGetValue(key, out var value))
            return value;
        if (_catalogs.TryGetValue(French, out var fr) && fr.TryGetValue(key, out var frValue))
            return frValue;
        return key;
    }

    private Dictionary<string, string> LoadCatalog(string path)
    {
        try
        {
            if (!File.Exists(path))
            {
                _logger.LogError("Fichier de traduction introuvable : {Path}", path);
                return new Dictionary<string, string>();
            }

            var json = File.ReadAllText(path);
            return JsonConvert.DeserializeObject<Dictionary<string, string>>(json)
                   ?? new Dictionary<string, string>();
        }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Impossible de charger le fichier de traduction {Path}", path);
            return new Dictionary<string, string>();
        }
    }

    /// <summary>
    /// Filet de sécurité : sans tests ni build local, un déséquilibre entre les deux catalogues ne
    /// se verrait qu'à l'écran. On le signale au démarrage.
    /// </summary>
    private void WarnAboutMissingKeys()
    {
        var fr = _catalogs[French].Keys.ToHashSet();
        var en = _catalogs[English].Keys.ToHashSet();

        var missingInEn = fr.Except(en).OrderBy(k => k).ToList();
        var missingInFr = en.Except(fr).OrderBy(k => k).ToList();

        if (missingInEn.Count > 0)
            _logger.LogWarning("Traductions manquantes en anglais ({Count}) : {Keys}",
                missingInEn.Count, string.Join(", ", missingInEn));
        if (missingInFr.Count > 0)
            _logger.LogWarning("Traductions manquantes en français ({Count}) : {Keys}",
                missingInFr.Count, string.Join(", ", missingInFr));
    }

    /// <summary>
    /// Aligne la culture du processus sur la langue choisie (formats de nombres et de dates).
    /// L'image Alpine tourne en <i>globalization-invariant mode</i> (ICU absent) : dans ce cas
    /// aucune culture nommée n'existe et on reste en culture invariante. Ce n'est que cosmétique —
    /// les traductions viennent des fichiers JSON, pas d'ICU — donc on ne fait jamais échouer le
    /// démarrage pour ça.
    /// </summary>
    private void ApplyCulture(string lang)
    {
        try
        {
            var culture = new CultureInfo(lang == English ? "en-US" : "fr-FR");
            CultureInfo.DefaultThreadCurrentCulture = culture;
            CultureInfo.DefaultThreadCurrentUICulture = culture;
        }
        catch (CultureNotFoundException)
        {
            _logger.LogInformation(
                "Cultures non disponibles (globalization-invariant mode) : les nombres et dates " +
                "restent au format invariant. Les traductions ne sont pas affectées.");
        }
    }
}
