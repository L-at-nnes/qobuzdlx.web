using System.Net;

namespace QobuzDLX.Web.Services;

/// <summary>
/// Réponse d'erreur de l'API Qobuz. Le code HTTP est conservé pour que l'appelant reconnaisse une
/// session expirée (401) sans analyser le texte du message.
/// </summary>
public class QobuzApiException : Exception
{
    public HttpStatusCode StatusCode { get; }

    public QobuzApiException(HttpStatusCode statusCode, string message) : base(message)
    {
        StatusCode = statusCode;
    }
}
