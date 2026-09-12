using System.Security.Cryptography;
using System.Text;
using QobuzDLX.Web.Services;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class PasswordHashTests
{
    // The two historical formats, rebuilt independently of the implementation.
    public static string LegacyPbkdf2(string password)
    {
        var salt = RandomNumberGenerator.GetBytes(16);
        var hash = Rfc2898DeriveBytes.Pbkdf2(password, salt, 100_000, HashAlgorithmName.SHA256, 32);
        return Convert.ToBase64String(salt.Concat(hash).ToArray());
    }

    public static string LegacySha256(string password) =>
        Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes(password))).ToLowerInvariant();

    [Fact]
    public static void Password_hashes_are_salted_versioned_and_verified()
    {
        var current = AppGuardService.HashPassword("correct horse");
        var parts = current.Split('$');
        H.Check("new hash is self-describing", parts.Length == 4 && parts[0] == "pbkdf2-sha256" && parts[1] == "600000");
        H.Check("new hash verifies", AppGuardService.VerifyPassword("correct horse", current));
        H.Check("new hash rejects a wrong password", !AppGuardService.VerifyPassword("wrong", current));
        H.Check("same password hashed twice differs (salted)", AppGuardService.HashPassword("correct horse") != current);

        var first = LegacyPbkdf2("secret");
        H.Check("first PBKDF2 format (100k) still verifies", AppGuardService.VerifyPassword("secret", first));
        H.Check("first PBKDF2 format rejects wrong password", !AppGuardService.VerifyPassword("nope", first));

        var oldest = LegacySha256("secret");
        H.Check("unsalted SHA-256 still verifies", AppGuardService.VerifyPassword("secret", oldest));
        H.Check("unsalted SHA-256 rejects wrong password", !AppGuardService.VerifyPassword("nope", oldest));

        // an hash written with another iteration count is honoured as written
        var salt = RandomNumberGenerator.GetBytes(16);
        var fewer = $"pbkdf2-sha256$1000${Convert.ToBase64String(salt)}${Convert.ToBase64String(Rfc2898DeriveBytes.Pbkdf2("pw", salt, 1000, HashAlgorithmName.SHA256, 32))}";
        H.Check("iteration count is read from the hash", AppGuardService.VerifyPassword("pw", fewer) && !AppGuardService.VerifyPassword("px", fewer));

        foreach (var junk in new[] { "", "garbage", "pbkdf2-sha256$", "pbkdf2-sha256$abc$AAAA$AAAA", "pbkdf2-sha256$0$AAAA$AAAA", "pbkdf2-sha256$600000$not*base64$AAAA", "pbkdf2-sha256$600000$AAAA", "pbkdf2-sha256$600000$$", "pbkdf2-sha256$600000$AAAAAAAAAAAAAAAAAAAAAA==$", "pbkdf2-sha256$600000$AAAAAAAAAAAAAAAAAAAAAA==$AA==" })
            H.Check($"malformed hash '{junk}' is rejected without throwing", !AppGuardService.VerifyPassword("x", junk));
    }
}
