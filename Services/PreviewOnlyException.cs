namespace QobuzDLX.Web.Services;

/// <summary>
/// Qobuz ne propose que l'extrait de 30 secondes d'une piste (abonnement expiré ou insuffisant, piste
/// restreinte). Les extraits sont identiques quelle que soit la qualité demandée : réessayer ou
/// descendre en qualité ne servirait à rien, l'exception traverse donc la boucle de repli.
/// </summary>
public class PreviewOnlyException : Exception
{
    public PreviewOnlyException(string message) : base(message)
    {
    }
}
