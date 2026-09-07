using System.Security.Cryptography;

namespace QobuzDLX.Web.Services;

/// <summary>
/// Scoped service (one instance per Blazor circuit / browser tab).
/// Gates access to the app when a password is configured.
/// </summary>
public class AppGuardService
{
    private const int SaltSize = 16;
    private const int HashSize = 32;

    // OWASP recommande 600 000 itérations pour PBKDF2-HMAC-SHA256. Le nombre est écrit dans
    // l'empreinte : le relever plus tard ne rend pas les empreintes existantes illisibles.
    private const int Iterations = 600_000;
    private const string FormatTag = "pbkdf2-sha256";

    // Première empreinte PBKDF2 de l'application : base64(sel ‖ hash), sans marqueur de format.
    private const int LegacyIterations = 100_000;
    private static readonly HashAlgorithmName Algorithm = HashAlgorithmName.SHA256;

    // Rate limiting (per circuit)
    private int _failedAttempts;
    private DateTime _lockoutUntil = DateTime.MinValue;
    private const int MaxAttempts = 5;
    private static readonly TimeSpan LockoutDuration = TimeSpan.FromSeconds(30);

    public bool IsUnlocked { get; private set; }
    public bool IsLockedOut => DateTime.UtcNow < _lockoutUntil;
    public int RemainingSeconds => IsLockedOut ? (int)(_lockoutUntil - DateTime.UtcNow).TotalSeconds + 1 : 0;

    public bool TryUnlock(string password, string storedHash)
    {
        if (string.IsNullOrEmpty(storedHash))
        {
            IsUnlocked = true;
            return true;
        }

        if (IsLockedOut)
            return false;

        if (VerifyPassword(password, storedHash))
        {
            IsUnlocked = true;
            _failedAttempts = 0;
            return true;
        }

        _failedAttempts++;
        if (_failedAttempts >= MaxAttempts)
        {
            _lockoutUntil = DateTime.UtcNow + LockoutDuration;
            _failedAttempts = 0;
        }

        return false;
    }

    /// <summary>
    /// Hash password with PBKDF2 + random salt.
    /// Returns "pbkdf2-sha256$iterations$salt$hash" (salt and hash in base64).
    /// </summary>
    public static string HashPassword(string password)
    {
        var salt = RandomNumberGenerator.GetBytes(SaltSize);
        var hash = Rfc2898DeriveBytes.Pbkdf2(password, salt, Iterations, Algorithm, HashSize);
        return $"{FormatTag}${Iterations}${Convert.ToBase64String(salt)}${Convert.ToBase64String(hash)}";
    }

    /// <summary>
    /// Verify password against a stored hash. Reads the current format, the first PBKDF2 format
    /// (100 000 iterations, no marker) and the oldest one (unsalted SHA-256 in hex), so that an
    /// installation keeps working across upgrades.
    /// </summary>
    public static bool VerifyPassword(string password, string storedHash)
    {
        if (IsLegacySha256(storedHash))
        {
            var sha = SHA256.HashData(System.Text.Encoding.UTF8.GetBytes(password));
            return CryptographicOperations.FixedTimeEquals(sha, Convert.FromHexString(storedHash));
        }

        if (ParsePbkdf2(storedHash) is not { } parsed) return false;

        var computed = Rfc2898DeriveBytes.Pbkdf2(password, parsed.Salt, parsed.Iterations, Algorithm, parsed.Hash.Length);
        return CryptographicOperations.FixedTimeEquals(parsed.Hash, computed);
    }

    private readonly record struct Pbkdf2Hash(int Iterations, byte[] Salt, byte[] Hash);

    private static bool IsLegacySha256(string storedHash) =>
        storedHash.Length == 64 && storedHash.All(c => "0123456789abcdef".Contains(c));

    private static Pbkdf2Hash? ParsePbkdf2(string storedHash)
    {
        try
        {
            if (storedHash.StartsWith(FormatTag + "$", StringComparison.Ordinal))
            {
                var parts = storedHash.Split('$');
                if (parts.Length != 4 || !int.TryParse(parts[1], out var iterations) || iterations <= 0)
                    return null;

                var salt = Convert.FromBase64String(parts[2]);
                var hash = Convert.FromBase64String(parts[3]);

                // An empty hash would "verify" any password: the sizes are checked, not trusted.
                return salt.Length == SaltSize && hash.Length == HashSize ? new Pbkdf2Hash(iterations, salt, hash) : null;
            }

            var decoded = Convert.FromBase64String(storedHash);
            return decoded.Length == SaltSize + HashSize
                ? new Pbkdf2Hash(LegacyIterations, decoded[..SaltSize], decoded[SaltSize..])
                : null;
        }
        catch (FormatException)
        {
            return null;
        }
    }
}
