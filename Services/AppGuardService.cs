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
    private const int Iterations = 100_000;
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
    /// Returns base64 string: salt(16) + hash(32).
    /// </summary>
    public static string HashPassword(string password)
    {
        var salt = RandomNumberGenerator.GetBytes(SaltSize);
        var hash = Rfc2898DeriveBytes.Pbkdf2(password, salt, Iterations, Algorithm, HashSize);
        var result = new byte[SaltSize + HashSize];
        salt.CopyTo(result, 0);
        hash.CopyTo(result, SaltSize);
        return Convert.ToBase64String(result);
    }

    /// <summary>
    /// Verify password against stored PBKDF2 hash.
    /// Also supports legacy SHA256 hex hashes for migration.
    /// </summary>
    public static bool VerifyPassword(string password, string storedHash)
    {
        // Legacy SHA256 hex hash (64 hex chars) — migrate on next save
        if (storedHash.Length == 64 && storedHash.All(c => "0123456789abcdef".Contains(c)))
        {
            var sha = System.Security.Cryptography.SHA256.HashData(System.Text.Encoding.UTF8.GetBytes(password));
            return Convert.ToHexString(sha).ToLowerInvariant() == storedHash;
        }

        try
        {
            var decoded = Convert.FromBase64String(storedHash);
            if (decoded.Length != SaltSize + HashSize) return false;

            var salt = decoded[..SaltSize];
            var storedHashBytes = decoded[SaltSize..];
            var computedHash = Rfc2898DeriveBytes.Pbkdf2(password, salt, Iterations, Algorithm, HashSize);

            return CryptographicOperations.FixedTimeEquals(storedHashBytes, computedHash);
        }
        catch
        {
            return false;
        }
    }
}
