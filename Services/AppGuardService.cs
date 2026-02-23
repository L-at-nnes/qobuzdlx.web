using System.Security.Cryptography;
using System.Text;

namespace QobuzDLX.Web.Services;

/// <summary>
/// Scoped service (one instance per Blazor circuit / browser tab).
/// Gates access to the app when a password is configured.
/// </summary>
public class AppGuardService
{
    public bool IsUnlocked { get; private set; }

    public bool TryUnlock(string password, string storedHash)
    {
        if (string.IsNullOrEmpty(storedHash))
        {
            IsUnlocked = true;
            return true;
        }

        if (HashPassword(password) == storedHash)
        {
            IsUnlocked = true;
            return true;
        }

        return false;
    }

    public static string HashPassword(string password)
    {
        var bytes = SHA256.HashData(Encoding.UTF8.GetBytes(password));
        return Convert.ToHexString(bytes).ToLowerInvariant();
    }
}
