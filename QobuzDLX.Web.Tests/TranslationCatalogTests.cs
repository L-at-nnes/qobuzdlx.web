using System.Text.RegularExpressions;
using Newtonsoft.Json;
using Xunit;

namespace QobuzDLX.Web.Tests;

public class TranslationCatalogTests
{
    private static Dictionary<string, string> Load(string language) =>
        JsonConvert.DeserializeObject<Dictionary<string, string>>(
            File.ReadAllText(Path.Combine(RepoPaths.WebRoot, "languages", language + ".json")))!;

    private static readonly Regex Placeholder = new(@"\{\d+\}", RegexOptions.Compiled);

    [Fact]
    public void French_and_english_define_the_same_keys()
    {
        var fr = Load("fr").Keys.ToHashSet();
        var en = Load("en").Keys.ToHashSet();

        Assert.Empty(en.Except(fr));
        Assert.Empty(fr.Except(en));
    }

    [Fact]
    public void A_translation_keeps_the_placeholders_of_the_other_language()
    {
        var fr = Load("fr");
        var en = Load("en");

        var mismatched = en.Where(pair => fr.TryGetValue(pair.Key, out var french)
                && !Placeholder.Matches(pair.Value).Select(m => m.Value).Order().SequenceEqual(
                    Placeholder.Matches(french).Select(m => m.Value).Order()))
            .Select(pair => pair.Key)
            .ToList();

        Assert.Empty(mismatched);
    }

    [Fact]
    public void Every_key_written_literally_in_the_code_exists_in_the_catalog()
    {
        var keys = Load("en").Keys.ToHashSet();
        var literal = new Regex(@"(?:Loc|_loc|loc)(?:\.[TP])?\[?\(?\s*""(?<key>[A-Za-z0-9_]+(?:\.[A-Za-z0-9_]+)+)""", RegexOptions.Compiled);

        var sources = new[] { "*.cs", "*.razor", "*.cshtml" }
            .SelectMany(pattern => Directory.EnumerateFiles(RepoPaths.Root, pattern, SearchOption.AllDirectories))
            .Where(path => !path.Contains($"{Path.DirectorySeparatorChar}obj{Path.DirectorySeparatorChar}")
                        && !path.Contains($"{Path.DirectorySeparatorChar}bin{Path.DirectorySeparatorChar}")
                        && !path.Contains("QobuzDLX.Web.Tests"));

        // Plural helpers (Loc.P) look up "<key>.one" and "<key>.other"
        bool Exists(string key) => keys.Contains(key) || keys.Contains(key + ".other");

        var used = sources
            .SelectMany(path => literal.Matches(File.ReadAllText(path)).Select(m => m.Groups["key"].Value))
            .Distinct()
            .ToList();
        Assert.True(used.Count > 100, $"the scan found only {used.Count} keys: the pattern no longer matches the code");

        var missing = used
            .Where(key => !Exists(key))
            .Distinct()
            .Order()
            .ToList();

        Assert.Empty(missing);
    }
}
