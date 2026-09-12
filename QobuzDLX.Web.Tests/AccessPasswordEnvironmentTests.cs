using Microsoft.Extensions.Logging.Abstractions;
using QobuzDLX.Web.Services;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class AccessPasswordEnvironmentTests
{
    [Fact]
    public static void The_access_password_can_come_from_the_environment()
    {
        var variable = AccessPasswordEnvironment.VariableName;
        try
        {
            var fx = new Fixtures();
            var logger = NullLogger.Instance;

            Environment.SetEnvironmentVariable(variable, null);
            AccessPasswordEnvironment.Apply(fx.Settings, logger);
            H.Check("no variable: nothing is set and nothing is managed", fx.Settings.Settings.AppAccessPasswordHash == "" && !AccessPasswordEnvironment.IsManaged);

            Environment.SetEnvironmentVariable(variable, "short");
            bool refused = false;
            try { AccessPasswordEnvironment.Apply(fx.Settings, logger); } catch (InvalidOperationException) { refused = true; }
            H.Check("a password under 8 characters stops the start-up", refused && fx.Settings.Settings.AppAccessPasswordHash == "");

            Environment.SetEnvironmentVariable(variable, "from-the-compose");
            AccessPasswordEnvironment.Apply(fx.Settings, logger);
            var hash = fx.Settings.Settings.AppAccessPasswordHash;
            H.Check("the variable becomes the access password", AppGuardService.VerifyPassword("from-the-compose", hash) && AccessPasswordEnvironment.IsManaged);

            AccessPasswordEnvironment.Apply(fx.Settings, logger);
            H.Check("same value at the next start: hash untouched, cookies stay valid", fx.Settings.Settings.AppAccessPasswordHash == hash);

            Environment.SetEnvironmentVariable(variable, "another-secret");
            AccessPasswordEnvironment.Apply(fx.Settings, logger);
            H.Check("a new value replaces the stored password", AppGuardService.VerifyPassword("another-secret", fx.Settings.Settings.AppAccessPasswordHash)
                && !AppGuardService.VerifyPassword("from-the-compose", fx.Settings.Settings.AppAccessPasswordHash));

            Environment.SetEnvironmentVariable(variable, null);
            AccessPasswordEnvironment.Apply(fx.Settings, logger);
            H.Check("variable removed: the stored password stays", AppGuardService.VerifyPassword("another-secret", fx.Settings.Settings.AppAccessPasswordHash));
        }
        finally
        {
            Environment.SetEnvironmentVariable(variable, null);
        }
    }
}
