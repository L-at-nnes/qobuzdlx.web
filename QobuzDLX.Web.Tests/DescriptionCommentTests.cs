using QobuzDLX.Web.Services;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class DescriptionCommentTests
{
    [Fact]
    public static void The_description_comment_is_plain_text()
    {
        var nl = Environment.NewLine;
        H.Check("entities are decoded", TaggingService.BuildComment("%description%", "Caf&eacute; &copy; 2020 &amp; co") == "Café © 2020 & co");
        H.Check("line breaks still apply", TaggingService.BuildComment("a<br/>b<br />c", null) == $"a{nl}b{nl}c");
        H.Check("paragraphs and emphasis become plain text", TaggingService.BuildComment("%description%", "<p>A <em>big</em> album</p><p>Second &amp; last</p>") == $"A big album{nl}Second & last");
        H.Check("no description", TaggingService.BuildComment("Ripped: %description%", null) == "Ripped: ");
    }
}
