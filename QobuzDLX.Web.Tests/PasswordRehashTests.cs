using System.Security.Cryptography;
using QobuzDLX.Web.Services;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class PasswordRehashTests
{
    [Fact]
    public static void Old_hash_formats_are_recognised_for_migration()
    {
        H.Check("current hash needs no rehash", !AppGuardService.NeedsRehash(AppGuardService.HashPassword("x")));
        H.Check("unsalted SHA-256 needs rehash", AppGuardService.NeedsRehash(PasswordHashTests.LegacySha256("x")));
        H.Check("first PBKDF2 format needs rehash", AppGuardService.NeedsRehash(PasswordHashTests.LegacyPbkdf2("x")));

        var salt = RandomNumberGenerator.GetBytes(16);
        string Tagged(int iterations) =>
            $"pbkdf2-sha256${iterations}${Convert.ToBase64String(salt)}${Convert.ToBase64String(Rfc2898DeriveBytes.Pbkdf2("x", salt, iterations, HashAlgorithmName.SHA256, 32))}";
        H.Check("versioned hash with fewer iterations needs rehash", AppGuardService.NeedsRehash(Tagged(100_000)));
        H.Check("versioned hash with more iterations needs no rehash", !AppGuardService.NeedsRehash(Tagged(900_000)));
        H.Check("unreadable hash needs rehash (never trusted)", AppGuardService.NeedsRehash("garbage"));
    }
}
