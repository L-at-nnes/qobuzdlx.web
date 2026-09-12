using QobuzDLX.Web.Services;
using Xunit;

namespace QobuzDLX.Web.Tests;

public class AppInfoTests
{
    [Fact]
    public void The_version_comes_from_the_project_as_major_minor_build()
        => Assert.Matches("^[0-9]+[.][0-9]+[.][0-9]+$", AppInfo.Version);
}
