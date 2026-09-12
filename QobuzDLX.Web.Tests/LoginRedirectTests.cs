using QobuzDLX.Web.Services;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class LoginRedirectTests
{
    [Fact]
    public static void The_login_returns_to_the_requested_local_page()
    {
        H.Check("a local page is kept", LoginRedirect.ReturnUrl("/telechargement") == "/telechargement");
        H.Check("a local page with a query is kept", LoginRedirect.ReturnUrl("/settings?x=1") == "/settings?x=1");
        H.Check("nothing asked: home", LoginRedirect.ReturnUrl(null) == "/" && LoginRedirect.ReturnUrl("") == "/");
        H.Check("the login page itself is never the destination", LoginRedirect.ReturnUrl("/login") == "/" && LoginRedirect.ReturnUrl("/login?expired=1") == "/");
        H.Check("a protocol-relative URL is refused", LoginRedirect.ReturnUrl("//evil.example") == "/");
        H.Check("an absolute URL is refused", LoginRedirect.ReturnUrl("https://evil.example/x") == "/");
        H.Check("a backslash trick is refused", LoginRedirect.ReturnUrl("/\\evil.example") == "/");
    }
}
