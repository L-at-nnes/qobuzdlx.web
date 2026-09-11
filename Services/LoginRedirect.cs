using Microsoft.AspNetCore.Components;

namespace QobuzDLX.Web.Services;

/// <summary>
/// Renvoi vers la connexion en retenant la page demandée : après un redémarrage du serveur, la session
/// Qobuz se rétablit en passant par /login, et sans mémoire de la page d'origine l'utilisateur
/// atterrissait toujours sur la recherche, quelle que soit l'adresse qu'il avait ouverte.
/// </summary>
public static class LoginRedirect
{
    public static void RedirectToLogin(this NavigationManager nav)
    {
        var returnUrl = "/" + nav.ToBaseRelativePath(nav.Uri);
        // The home page is where the login leads anyway: no need to clutter the address with it
        nav.NavigateTo(returnUrl == "/" ? "/login" : $"/login?returnUrl={Uri.EscapeDataString(returnUrl)}");
    }

    /// <summary>
    /// Page où renvoyer après la connexion : un chemin local uniquement (<see cref="LocalRedirect"/>), jamais
    /// la page de connexion elle-même, et l'accueil à défaut.
    /// </summary>
    public static string ReturnUrl(string? requested)
    {
        var path = LocalRedirect.Sanitize(requested);
        return path.StartsWith("/login", StringComparison.OrdinalIgnoreCase) ? "/" : path;
    }
}
