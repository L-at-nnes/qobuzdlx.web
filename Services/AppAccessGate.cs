using System.Security.Cryptography;
using System.Text;

namespace QobuzDLX.Web.Services;

/// <summary>
/// Étranglement des tentatives de déverrouillage, au niveau de l'application.
/// Le compteur vivait auparavant dans AppGuardService, un service scoped : il était donc remis à
/// zéro par le simple fait d'ouvrir un nouvel onglet, ce qui rendait la limite décorative.
/// </summary>
/// <remarks>
/// Un seul seau, pas un seau par adresse : derrière un reverse proxy sans ForwardedHeaders, toutes
/// les requêtes portent l'adresse du proxy. Découper par IP donnerait exactement le même
/// comportement, avec l'illusion d'une granularité en plus.
///
/// Corollaire : tout visiteur peut verrouiller les autres en échouant volontairement. Le palier
/// redescend après une heure sans tentative (<see cref="LevelDecay"/>), de sorte qu'un blocage ne
/// se prolonge que tant que l'attaque dure ; seul un seau par adresse, donc des en-têtes de proxy
/// de confiance, l'empêcherait tout à fait.
/// </remarks>
public sealed class AppAccessGate
{
    /// <summary>Type du claim qui fige, dans le cookie, le mot de passe pour lequel il a été émis.</summary>
    public const string StampClaim = "qbdlx:stamp";

    /// <summary>Identifiant d'appareil tiré à chaque déverrouillage : il sépare l'état de recherche de
    /// chaque appareil (<see cref="SearchStateStore"/>).</summary>
    public const string DeviceClaim = "qbdlx:device";

    private const int MaxAttempts = 5;

    // Palier suivant à chaque série d'échecs : réessayer en boucle coûte de plus en plus cher,
    // là où un délai fixe se contente d'imposer une pause avant de repartir au même rythme.
    private static readonly TimeSpan[] Backoff =
    {
        TimeSpan.FromSeconds(30),
        TimeSpan.FromMinutes(1),
        TimeSpan.FromMinutes(5),
        TimeSpan.FromMinutes(15),
    };

    /// <summary>Durée de calme qui fait redescendre le palier d'un cran.</summary>
    private static readonly TimeSpan LevelDecay = TimeSpan.FromHours(1);

    private readonly TimeProvider _time;
    private readonly object _lock = new();
    private int _failures;
    private int _level;
    private DateTime _lastFailure = DateTime.MinValue;
    private DateTime _lockedUntil = DateTime.MinValue;

    public AppAccessGate(TimeProvider time)
    {
        _time = time;
    }

    private DateTime Now => _time.GetUtcNow().UtcDateTime;

    public bool IsLockedOut
    {
        get { lock (_lock) return Now < _lockedUntil; }
    }

    public int RemainingSeconds
    {
        get
        {
            lock (_lock)
            {
                var remaining = _lockedUntil - Now;
                return remaining > TimeSpan.Zero ? (int)remaining.TotalSeconds + 1 : 0;
            }
        }
    }

    public void RegisterFailure()
    {
        lock (_lock)
        {
            var now = Now;
            DecayLocked(now);
            _lastFailure = now;

            if (++_failures < MaxAttempts) return;
            _failures = 0;
            _lockedUntil = now + Backoff[Math.Min(_level, Backoff.Length - 1)];
            _level++;
        }
    }

    /// <summary>
    /// Appelé sous verrou, avant d'enregistrer un échec : une heure de calme (depuis le dernier échec
    /// ou la fin du blocage en cours) fait descendre le palier d'un cran et efface les échecs isolés.
    /// </summary>
    private void DecayLocked(DateTime now)
    {
        var quietSince = _lockedUntil > _lastFailure ? _lockedUntil : _lastFailure;
        var steps = (int)Math.Min((now - quietSince) / LevelDecay, int.MaxValue);
        if (steps <= 0) return;

        _level = Math.Max(0, _level - steps);
        _failures = 0;
    }

    /// <summary>Un déverrouillage réussi ramène le palier à zéro : les échecs d'hier ne pénalisent
    /// pas quelqu'un qui connaît le mot de passe.</summary>
    public void RegisterSuccess()
    {
        lock (_lock)
        {
            _failures = 0;
            _level = 0;
            _lockedUntil = DateTime.MinValue;
        }
    }

    /// <summary>
    /// Empreinte du hachage courant, embarquée dans le cookie puis revalidée à chaque requête.
    /// Changer ou retirer le mot de passe dans les Paramètres change l'empreinte, ce qui invalide
    /// d'un coup les cookies émis pour l'ancien — sans quoi un appareil déverrouillé le resterait
    /// indéfiniment après un changement de mot de passe.
    /// </summary>
    public static string StampFor(string? passwordHash) =>
        string.IsNullOrEmpty(passwordHash)
            ? "none"
            : Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes(passwordHash)))[..16];
}
