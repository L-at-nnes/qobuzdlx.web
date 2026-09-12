using QobuzDLX.Web.Services;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class FileNameTests
{
    [Fact]
    public static void File_names_are_made_safe_without_losing_meaningful_characters()
    {
        void Is(string input, string expected) =>
            H.Check($"'{input}' -> '{expected}'", TemplateService.MakeSafeFilename(input) == expected);

        Is("Song: Intro", "Song - Intro");
        Is("Song:", "Song");
        Is("-OSC-", "-OSC-");
        Is("Re-mix", "Re-mix");
        Is("A - B", "A - B");
        Is("01 - ", "01");
        Is("- Intro", "Intro");
        Is("01 - - ", "01");
        Is("What? Now!", "What Now!");
        Is("AC/DC", "AC-DC");
        Is("Title.", "Title");
        Is("  spaced   out  ", "spaced out");
        Is("-", "-");
        Is("", "");
    }
}
