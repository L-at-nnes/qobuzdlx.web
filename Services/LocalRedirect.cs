namespace QobuzDLX.Web.Services;

/// <summary>
/// Valide la destination d'une redirection issue d'un formulaire. Seul un chemin du site est
/// accepté : tout le reste ramène à l'accueil, ce qui ferme la redirection ouverte.
/// </summary>
public static class LocalRedirect
{
    public static string Sanitize(string? candidate) => IsLocalPath(candidate) ? candidate! : "/";

    // « // » est une URL protocol-relative et « : » introduit un schéma absolu. L'antislash et
    // les caractères de contrôle ferment les contournements : les navigateurs lisent l'antislash
    // comme « / » et retirent tabulations et sauts de ligne, si bien que « /\evil.com » et
    // « /<TAB>/evil.com » deviennent « //evil.com ».
    private static bool IsLocalPath(string? candidate) =>
        !string.IsNullOrEmpty(candidate)
        && candidate[0] == '/'
        && !candidate.StartsWith("//")
        && candidate.All(c => c != '\\' && c != ':' && !char.IsControl(c));
}
