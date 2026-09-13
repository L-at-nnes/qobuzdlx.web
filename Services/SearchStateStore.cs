using Microsoft.AspNetCore.Components.Authorization;

namespace QobuzDLX.Web.Services;

/// <summary>
/// Un <see cref="SearchStateService"/> par appareil. L'état de recherche (requête, résultats, tris)
/// doit survivre à la navigation et au rechargement de la page, donc il vit sur le serveur ; mais un
/// objet unique partagé faisait que le téléphone et l'ordinateur voyaient et écrasaient la recherche
/// l'un de l'autre. Un appareil est reconnu à la revendication <see cref="AppAccessGate.DeviceClaim"/>
/// de son cookie d'accès ; sans mot de passe d'accès il n'y a pas de cookie, et tous les visiteurs se
/// partagent alors un seul état, comme avant.
/// </summary>
public sealed class SearchStateStore
{
    /// <summary>Au-delà, l'état de l'appareil le moins récemment utilisé est oublié (cookies d'anciens
    /// mots de passe, appareils remplacés) : la mémoire reste bornée.</summary>
    public const int MaxDevices = 20;

    private readonly object _lock = new();
    private readonly Dictionary<string, (SearchStateService State, long LastUse)> _states = new();
    private long _clock;

    public SearchStateService For(string? deviceId)
    {
        var key = deviceId ?? "";
        lock (_lock)
        {
            if (!_states.TryGetValue(key, out var entry))
                entry = (new SearchStateService(), 0);

            _states[key] = (entry.State, ++_clock);

            if (_states.Count > MaxDevices)
                _states.Remove(_states.MinBy(pair => pair.Value.LastUse).Key);

            return entry.State;
        }
    }

    /// <summary>
    /// Identifiant de l'appareil porté par le cookie d'accès, ou null sans cookie. Lu de façon
    /// synchrone comme le fait <see cref="AppGuardService"/> : l'utilisateur est déposé dans le
    /// fournisseur d'authentification avant le premier rendu.
    /// </summary>
    public static string? DeviceIdOf(AuthenticationStateProvider auth)
    {
        var state = auth.GetAuthenticationStateAsync();
        return state.IsCompletedSuccessfully
            ? state.Result.User.FindFirst(AppAccessGate.DeviceClaim)?.Value
            : null;
    }
}
