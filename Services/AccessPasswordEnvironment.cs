namespace QobuzDLX.Web.Services;

/// <summary>
/// Mot de passe d'accès fourni par la variable d'environnement <c>ACCESS_PASSWORD</c> (typiquement
/// dans docker-compose.yml) : l'interface est protégée dès le premier démarrage, sans passer par les
/// Paramètres, ce qui compte pour un conteneur joignable depuis l'extérieur ou le réseau local.
/// </summary>
/// <remarks>
/// La variable fait foi tant qu'elle est définie : la valeur est vérifiée à chaque démarrage et
/// l'empreinte enregistrée n'est remplacée que si elle ne correspond plus. Re-hacher à chaque
/// démarrage changerait le sel, donc invaliderait les cookies de déverrouillage déjà émis.
/// </remarks>
public static class AccessPasswordEnvironment
{
    public const string VariableName = "ACCESS_PASSWORD";

    private static string? Value => Environment.GetEnvironmentVariable(VariableName);

    /// <summary>Vrai quand le mot de passe est imposé par l'environnement et non modifiable depuis les Paramètres.</summary>
    public static bool IsManaged => !string.IsNullOrEmpty(Value);

    /// <summary>
    /// Aligne le mot de passe enregistré sur la variable. Un mot de passe trop court arrête le démarrage :
    /// l'ignorer laisserait l'interface ouverte à tous alors que son propriétaire la croit protégée.
    /// </summary>
    public static void Apply(SettingsService settings, ILogger logger)
    {
        var password = Value;
        if (string.IsNullOrEmpty(password)) return;

        if (password.Length < AppGuardService.MinPasswordLength)
            throw new InvalidOperationException(
                $"{VariableName} must be at least {AppGuardService.MinPasswordLength} characters long.");

        var stored = settings.Settings.AppAccessPasswordHash;
        if (!string.IsNullOrEmpty(stored) && AppGuardService.VerifyPassword(password, stored)) return;

        settings.Update(s => s.AppAccessPasswordHash = AppGuardService.HashPassword(password));
        logger.LogInformation("Access password set from the {Variable} environment variable.", VariableName);
    }
}
