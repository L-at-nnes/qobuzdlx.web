using QobuzDLX.Web.Services;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class LocalRedirectTests
{
    [Fact]
    public static void Only_local_paths_are_accepted_as_redirect_targets()
    {
        foreach (var ok in new[] { "/", "/search", "/telechargement", "/settings?x=1", "/a/b" })
            H.Check($"keeps local path {ok}", LocalRedirect.Sanitize(ok) == ok);

        var rejected = new Dictionary<string, string?>
        {
            ["null"] = null,
            ["empty"] = "",
            ["protocol-relative"] = "//evil.com",
            ["backslash"] = "/\\evil.com",
            ["backslash after slash"] = "/a\\b",
            ["tab"] = "/\t/evil.com",
            ["newline"] = "/\n/evil.com",
            ["crlf injection"] = "/x\r\nSet-Cookie: a=b",
            ["absolute"] = "https://evil.com",
            ["scheme only"] = "javascript:alert(1)",
            ["no leading slash"] = "evil.com",
            ["colon in path"] = "/a:b",
        };
        foreach (var (name, value) in rejected)
            H.Check($"rejects {name}", LocalRedirect.Sanitize(value) == "/");
    }
}
