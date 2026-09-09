using System.Globalization;
using System.Text;
using System.Text.RegularExpressions;
using QobuzDLX.Web.Models;

namespace QobuzDLX.Web.Services;

public class TemplateService
{
    private readonly SettingsService _settings;

    private static readonly Regex PercentRegex = new(@"%(.*?)%", RegexOptions.Compiled | RegexOptions.CultureInvariant);
    private static readonly Regex SpacesRegex = new(@"\s+", RegexOptions.Compiled | RegexOptions.CultureInvariant);
    // A dash glued to the edge of a name by a separator: "Song:" becomes "Song -", an empty field in
    // "%TrackNumber% - %TrackTitle%" leaves "- Title". A dash with no blank next to it ("-OSC-") is
    // part of the name and stays.
    private static readonly Regex EdgeSeparatorRegex = new(@"^(?:-\s+)+|(?:\s+-)+$", RegexOptions.Compiled);
    private static readonly Regex RepeatedParenthesesRegex = new(@"\(([^()]+)\)\s*(\(\1\))+", RegexOptions.Compiled | RegexOptions.IgnoreCase);

    // Les noms de fichiers ne suivent pas la langue de l'interface : la culture du processus est
    // fr-FR quand l'utilisateur est en français, ce qui écrirait « 44,1kHz » au lieu de « 44.1kHz ».
    private static string Rate(double khz) => khz.ToString(CultureInfo.InvariantCulture);

    // 255 bytes is the limit of a path component on ext4 and most NAS filesystems. The margin leaves
    // room for the audio extension (".flac") and the " (99)" suffix of the "rename" duplicate mode.
    private const int MaxSegmentBytes = 240;

    private static readonly string[] VariousArtistsNames = {
        "Various Artists, Array", "Various Artists", "Various Aritsts",
        "Various Artist", "Various Interpreters", "Various Interpreter", "Various Interprets"
    };

    public TemplateService(SettingsService settings)
    {
        _settings = settings;
    }

    public string CreatePath(string downloadLocation, string artistTemplate, string albumTemplate,
        string? playlistTemplate, QobuzAlbum? album, QobuzTrack? track, QobuzPlaylist? playlist)
    {
        string downloadPath;
        if (playlist == null && album != null)
        {
            var artistConverted = SanitizePathSegment(ApplyTemplate(artistTemplate, album, null, null));
            var albumConverted = SanitizePathSegment(ApplyTemplate(albumTemplate, album, null, null));

            // Build path, skipping empty segments (user left template blank on purpose)
            if (!string.IsNullOrEmpty(artistConverted) && !string.IsNullOrEmpty(albumConverted))
                downloadPath = Path.Combine(downloadLocation, artistConverted, albumConverted);
            else if (!string.IsNullOrEmpty(artistConverted))
                downloadPath = Path.Combine(downloadLocation, artistConverted);
            else if (!string.IsNullOrEmpty(albumConverted))
                downloadPath = Path.Combine(downloadLocation, albumConverted);
            else
                downloadPath = downloadLocation;
        }
        else if (playlist != null && !string.IsNullOrEmpty(playlistTemplate))
        {
            var playlistConverted = SanitizePathSegment(ApplyTemplate(playlistTemplate, album, track, playlist));
            downloadPath = !string.IsNullOrEmpty(playlistConverted)
                ? Path.Combine(downloadLocation, playlistConverted)
                : downloadLocation;
        }
        else
        {
            downloadPath = downloadLocation;
        }

        // Ensure final path stays within downloadLocation
        if (!IsInside(Path.GetFullPath(downloadLocation), Path.GetFullPath(downloadPath)))
            downloadPath = downloadLocation;

        return downloadPath;
    }

    /// <summary>
    /// Compares path components, not characters: a plain prefix test would accept
    /// "/downloads-evil" as being inside "/downloads".
    /// </summary>
    private static bool IsInside(string basePath, string path)
    {
        var relative = Path.GetRelativePath(basePath, path);
        return relative != ".."
            && !relative.StartsWith(".." + Path.DirectorySeparatorChar)
            && !Path.IsPathRooted(relative);
    }

    /// <summary>
    /// Makes a folder name safe. It may hold several components (a template can contain "/"):
    /// each one loses the dots and spaces at its ends, so "." and ".." vanish and the path cannot
    /// climb out of the download folder, while a ".." inside a name ("Vol..2") is left alone.
    /// Returns empty string if input was empty (intentionally blank template).
    /// Returns "_" only if input had content but became empty after sanitization.
    /// </summary>
    private static string SanitizePathSegment(string segment)
    {
        if (string.IsNullOrWhiteSpace(segment)) return "";

        var components = segment.Replace("\0", "")
            .Split('/')
            .Select(component => component.Trim('.', ' '))
            .Where(component => component.Length > 0)
            .ToArray();

        return components.Length == 0 ? "_" : string.Join('/', components);
    }

    public string ApplyTemplate(string template, QobuzAlbum? album, QobuzTrack? track, QobuzPlaylist? playlist)
    {
        var s = _settings.Settings;

        // Lowercase all %tags%
        template = PercentRegex.Replace(template, m => m.Value.ToLower());

        // Preserve path separators
        template = template.Replace("/", "{slash}");

        // VA track template swap
        if (track != null && album != null)
        {
            var artistName = GetReleaseArtists(album);
            if (VariousArtistsNames.Any(n => n.Equals(artistName, StringComparison.OrdinalIgnoreCase)))
            {
                template = PercentRegex.Replace(s.VaTrackTemplate, m => m.Value.ToLower());
                template = template.Replace("%artistname%", "%trackartist%");
            }
        }

        string fileFormat = s.AudioFormat.ToUpper().TrimStart('.');

        // Track tags
        if (track != null)
        {
            // With playlist positions the number is a rank in the playlist, not in the album it comes
            // from: padding to the album's size would sort "7" before "35" but after "120".
            var numberedTracks = playlist != null && s.UseItemPosInPlaylist && playlist.TracksCount > 0
                ? playlist.TracksCount
                : album?.TracksCount;
            int padLen = numberedTracks?.ToString().Length ?? 2;

            template = ReplaceParentalWarningTags(template, "track", track.ParentalWarning);
            template = template
                .Replace("%trackid%", track.Id.ToString())
                .Replace("%discnumber%", track.MediaNumber.ToString())
                .Replace("%trackcomposer%", track.Composer?.Name ?? "")
                .Replace("%tracknumber%", track.TrackNumber.ToString().PadLeft(padLen, '0'))
                .Replace("%isrc%", track.ISRC ?? "")
                .Replace("%trackbitdepth%", track.MaximumBitDepth.ToString())
                .Replace("%tracksamplerate%", Rate(track.MaximumSamplingRate));

            var titleFormatted = FormatTrackTitle(track);
            template = template.Replace("%tracktitle%", titleFormatted);

            var performerName = track.Performer?.Name ?? "";
            template = template.Replace("%trackartist%", performerName);
            template = template.Replace("%trackformat%", fileFormat);

            // Track format with quality
            template = RenameFormatTemplate(template, s.QualityFormat, fileFormat,
                track.MaximumBitDepth, track.MaximumSamplingRate,
                "%trackformatwithhiresquality%", "%trackformatwithquality%");
        }

        // Album tags
        if (album != null)
        {
            template = ReplaceParentalWarningTags(template, "album", album.ParentalWarning);
            var year = "";
            if (!string.IsNullOrEmpty(album.ReleaseDateOriginal) && album.ReleaseDateOriginal.Length >= 4)
                year = album.ReleaseDateOriginal.Trim()[..4];

            var albumTitle = album.Version == null ? album.Title : $"{album.Title.TrimEnd()} ({album.Version})";
            var productType = string.IsNullOrEmpty(album.ProductType) ? "" :
                char.ToUpper(album.ProductType[0]) + album.ProductType[1..].ToLower();

            template = template
                .Replace("%albumid%", album.Id)
                .Replace("%albumurl%", album.Url ?? "")
                .Replace("%artistname%", GetReleaseArtists(album))
                .Replace("%albumgenre%", album.Genre?.Name ?? "")
                .Replace("%albumcomposer%", album.Composer?.Name ?? "")
                .Replace("%label%", SpacesRegex.Replace(album.Label?.Name ?? "", " "))
                .Replace("%copyright%", album.Copyright ?? "")
                .Replace("%upc%", album.UPC ?? "")
                .Replace("%releasedate%", album.ReleaseDateOriginal?.Trim() ?? "")
                .Replace("%year%", year)
                .Replace("%releasetype%", productType)
                .Replace("%bitdepth%", album.MaximumBitDepth.ToString())
                .Replace("%samplerate%", Rate(album.MaximumSamplingRate))
                .Replace("%albumtitle%", albumTitle)
                .Replace("%format%", fileFormat);

            // Album format with quality
            template = RenameFormatTemplate(template, s.QualityFormat, fileFormat,
                album.MaximumBitDepth, album.MaximumSamplingRate,
                "%formatwithhiresquality%", "%formatwithquality%");
        }

        // Playlist tags
        if (playlist != null)
        {
            template = template
                .Replace("%playlistid%", playlist.Id.ToString())
                .Replace("%playlisttitle%", playlist.Name)
                .Replace("%format%", fileFormat);

            // Playlist format with quality (use album data if available, else just format name)
            if (album != null)
            {
                template = RenameFormatTemplate(template, s.QualityFormat, fileFormat,
                    album.MaximumBitDepth, album.MaximumSamplingRate,
                    "%formatwithhiresquality%", "%formatwithquality%");
            }
            else
            {
                template = template
                    .Replace("%formatwithhiresquality%", fileFormat)
                    .Replace("%formatwithquality%", fileFormat);
            }
        }

        // Disc number without a track (album and playlist folders): nothing to resolve it against
        template = template.Replace("%discnumber%", "");

        // Sanitize
        template = MakeSafeFilename(template);
        template = template.Replace("{slash}", "/");
        template = SpacesRegex.Replace(template, " ").Trim();
        template = template.Replace("...", "\u2026");

        // The template may contain "/" (folder separators): the limit applies to each component.
        return string.Join('/', template.Split('/').Select(segment => TruncateToBytes(segment)));
    }

    /// <summary>
    /// Replaces %FormatWithQuality% and %FormatWithHiResQuality% based on format_id and audio specs.
    /// Ported from original QobuzDownloaderX RenameTemplates.RenameFormatTemplate()
    /// </summary>
    private static string RenameFormatTemplate(string template, string formatId, string fileFormat,
        int maximumBitDepth, double maximumSamplingRate,
        string hiResPlaceholder, string qualityPlaceholder)
    {
        switch (formatId)
        {
            case "5": // MP3 320
                template = template
                    .Replace(hiResPlaceholder, fileFormat)
                    .Replace(qualityPlaceholder, fileFormat);
                break;

            case "6": // FLAC 16bit
                template = template
                    .Replace(hiResPlaceholder, fileFormat)
                    .Replace(qualityPlaceholder, $"{fileFormat} ({maximumBitDepth}bit-{Rate(maximumSamplingRate)}kHz)");
                break;

            case "7":  // FLAC 24bit <=96kHz
            case "27": // FLAC 24bit (best available)
                if (maximumBitDepth == 16)
                {
                    // Not actually hi-res
                    template = template
                        .Replace(hiResPlaceholder, fileFormat)
                        .Replace(qualityPlaceholder, $"{fileFormat} ({maximumBitDepth}bit-{Rate(maximumSamplingRate)}kHz)");
                }
                else if (maximumSamplingRate < 192)
                {
                    // Merge quality into hires placeholder
                    template = template.Replace(qualityPlaceholder, hiResPlaceholder);

                    if (maximumSamplingRate < 96)
                    {
                        template = template.Replace(hiResPlaceholder,
                            $"{fileFormat} ({maximumBitDepth}bit-{Rate(maximumSamplingRate)}kHz)");
                    }
                    else if (maximumSamplingRate > 96 && maximumSamplingRate < 192)
                    {
                        if (formatId == "7" && maximumSamplingRate == 176.4)
                        {
                            template = template.Replace(hiResPlaceholder, $"{fileFormat} (24bit-88.2kHz)");
                        }
                        else if (formatId == "7")
                        {
                            template = template.Replace(hiResPlaceholder, $"{fileFormat} (24bit-96kHz)");
                        }
                        else
                        {
                            template = template.Replace(hiResPlaceholder,
                                $"{fileFormat} ({maximumBitDepth}bit-{Rate(maximumSamplingRate)}kHz)");
                        }
                    }
                    else // exactly 96
                    {
                        template = template.Replace(hiResPlaceholder,
                            $"{fileFormat} ({maximumBitDepth}bit-{Rate(maximumSamplingRate)}kHz)");
                    }
                }
                else // >= 192
                {
                    template = template.Replace(qualityPlaceholder, hiResPlaceholder);
                    template = template.Replace(hiResPlaceholder, $"{fileFormat} (24bit-192kHz)");
                }
                break;

            default:
                // Unknown format, just replace with fileFormat
                template = template
                    .Replace(hiResPlaceholder, fileFormat)
                    .Replace(qualityPlaceholder, fileFormat);
                break;
        }

        return template;
    }

    public string FormatTrackTitle(QobuzTrack track)
    {
        var title = track.Version == null ? track.Title : $"{track.Title.TrimEnd()} ({track.Version})";
        title = RepeatedParenthesesRegex.Replace(title, "($1)");
        return title;
    }

    public string GetReleaseArtists(QobuzAlbum album)
    {
        if (_settings.Settings.MergeArtistNames && _settings.Settings.MergeArtistNamesInDirNames)
        {
            var mainArtists = album.Artists?.Where(a => a.Roles.Contains("main-artist")).ToList();
            if (mainArtists != null && mainArtists.Count > 1)
            {
                var allButLast = string.Join(", ", mainArtists.Take(mainArtists.Count - 1).Select(a => a.Name));
                return $"{allButLast} & {mainArtists.Last().Name}";
            }
        }
        return album.Artist?.Name ?? "Unknown Artist";
    }

    // (tag suffix, value when explicit, value when clean). Each variant exists once per scope:
    // %track<suffix>% follows the track's parental warning, %album<suffix>% the album's.
    private static readonly (string Suffix, string Explicit, string Clean)[] ParentalWarningVariants =
    {
        ("pa",                    "Explicit",   "Clean"),
        ("pashort",               "E",          "C"),
        ("paifex",                "Explicit",   ""),
        ("paifexshort",           "E",          ""),
        ("paifcl",                "",           "Clean"),
        ("paifclshort",           "",           "C"),
        ("paenclosed",            "(Explicit)", "(Clean)"),
        ("paenclosed[]",          "[Explicit]", "[Clean]"),
        ("paenclosedshort",       "(E)",        "(C)"),
        ("paenclosedshort[]",     "[E]",        "[C]"),
        ("paifexenclosed",        "(Explicit)", ""),
        ("paifexenclosed[]",      "[Explicit]", ""),
        ("paifexenclosedshort",   "(E)",        ""),
        ("paifexenclosedshort[]", "[E]",        ""),
        ("paifclenclosed",        "",           "(Clean)"),
        ("paifclenclosed[]",      "",           "[Clean]"),
        ("paifclenclosedshort",   "",           "(C)"),
        ("paifclenclosedshort[]", "",           "[C]"),
    };

    /// <summary>
    /// Parental warning tag replacement matching original QobuzDownloaderX RenameTemplates.
    /// <paramref name="scope"/> is "track" or "album": the two scopes must be resolved separately,
    /// each with its own flag, otherwise %AlbumPA% would reflect the track's warning.
    /// </summary>
    private static string ReplaceParentalWarningTags(string template, string scope, bool isExplicit)
    {
        foreach (var (suffix, explicitValue, cleanValue) in ParentalWarningVariants)
            template = template.Replace($"%{scope}{suffix}%", isExplicit ? explicitValue : cleanValue);

        return template;
    }

    /// <summary>
    /// Full path of the file a track is written to. Shared by the download and by the preview in
    /// Settings, so that what the preview shows is what the download does.
    /// </summary>
    public string TrackFilePath(string downloadPath, string downloadType, QobuzAlbum album, QobuzTrack track, QobuzPlaylist? playlist)
    {
        var s = _settings.Settings;

        // The template is user input and may hold "/" (sub-folders), ".." or a leading "/": every
        // component is cleaned like the folder names, so the file cannot leave the download folder.
        var fileName = SanitizePathSegment(ApplyTemplate(s.TrackTemplate, album, track, playlist));
        if (fileName.Length == 0) fileName = track.Id.ToString();   // blank template: still a usable name
        fileName += s.AudioFormat;

        // Multi-disc releases get one folder per disc, except for a lone track or a playlist entry
        if (track.PlaylistTrackId == null && downloadType != "track" && album.MediaCount > 1 && !string.IsNullOrEmpty(s.CdTemplate))
        {
            var discFolder = SanitizePathSegment(Regex.Replace(s.CdTemplate, "%DiscNumber%", track.MediaNumber.ToString(), RegexOptions.IgnoreCase));
            if (discFolder.Length > 0)
                return Path.Combine(downloadPath, discFolder, fileName);
        }

        return Path.Combine(downloadPath, fileName);
    }

    // ─── Preview and validation of the naming templates ───

    // Invented release, big enough to exercise every template: two discs, a track version, hi-res.
    private static readonly QobuzAlbum SampleAlbum = new()
    {
        Id = "0123456789012",
        Title = "Blue Hour",
        Artist = new QobuzPerformer { Id = 1, Name = "Nina Alvarez" },
        Artists = new List<QobuzArtistRole> { new() { Id = 1, Name = "Nina Alvarez", Roles = new List<string> { "main-artist" } } },
        Genre = new QobuzGenre { Id = 1, Name = "Jazz" },
        Label = new QobuzLabel { Id = 1, Name = "Northern Lights" },
        ReleaseDateOriginal = "2013-05-17",
        UPC = "0123456789012",
        Copyright = "(P) 2013 Northern Lights",
        ProductType = "album",
        TracksCount = 14,
        MediaCount = 2,
        MaximumBitDepth = 24,
        MaximumSamplingRate = 96
    };

    private static readonly QobuzAlbum SampleCompilation = new()
    {
        Id = "0123456789999",
        Title = "Summer Mix",
        Artist = new QobuzPerformer { Id = 2, Name = "Various Artists" },
        Genre = new QobuzGenre { Id = 1, Name = "Jazz" },
        Label = new QobuzLabel { Id = 1, Name = "Northern Lights" },
        ReleaseDateOriginal = "2020-06-01",
        ProductType = "album",
        TracksCount = 20,
        MediaCount = 1,
        MaximumBitDepth = 16,
        MaximumSamplingRate = 44.1
    };

    private static readonly QobuzPlaylist SamplePlaylist = new() { Id = 99, Name = "Morning Playlist", TracksCount = 30 };

    private static QobuzTrack NewSampleTrack(int trackNumber = 5, int discNumber = 2, int? playlistTrackId = null) => new()
    {
        Id = 4242,
        Title = "Midnight Garden",
        Version = "Live",
        TrackNumber = trackNumber,
        MediaNumber = discNumber,
        Position = 3,
        PlaylistTrackId = playlistTrackId,
        Performer = new QobuzPerformer { Id = 1, Name = "Nina Alvarez" },
        Composer = new QobuzComposer { Id = 2, Name = "R. Castillo" },
        ISRC = "XX0000000001",
        MaximumBitDepth = 24,
        MaximumSamplingRate = 96
    };

    /// <summary>The path a track of an album, of a compilation and of a playlist would get today.</summary>
    public TemplatePreview Preview()
    {
        var s = _settings.Settings;

        string PathOf(string downloadType, QobuzAlbum album, QobuzTrack track, QobuzPlaylist? playlist)
        {
            // Same calls, in the same order, as the download itself
            var folder = CreatePath(s.DownloadLocation, s.ArtistTemplate, s.AlbumTemplate,
                playlist == null ? null : s.PlaylistTemplate, album, playlist == null ? null : track, playlist);
            return TrackFilePath(folder, downloadType, album, track, playlist).Replace('\\', '/');
        }

        return new TemplatePreview(
            PathOf("album", SampleAlbum, NewSampleTrack(), null),
            PathOf("album", SampleCompilation, NewSampleTrack(trackNumber: 7, discNumber: 1), null),
            PathOf("playlist", SampleAlbum, NewSampleTrack(playlistTrackId: 1), SamplePlaylist));
    }

    // Characters that MakeSafeFilename rewrites: a backslash is not a separator, only "/" is
    private static readonly char[] ReplacedCharacters = { '\\', ':', '*', '?', '"', '<', '>', '|' };

    private static readonly Regex DriveLetterRegex = new(@"^[A-Za-z]:", RegexOptions.Compiled);

    // A %tag% still standing once the template has been applied to the sample: unknown, misspelt,
    // or not available in that kind of template (%TrackNumber% in an album folder, say)
    private static readonly Regex UnresolvedTagRegex = new(@"%[^%\s/]+%", RegexOptions.Compiled);

    /// <summary>Problems with the templates as currently set, each pointing at a field. Empty when all is well.</summary>
    public IReadOnlyList<TemplateIssue> Validate()
    {
        var s = _settings.Settings;
        var issues = new List<TemplateIssue>();
        var album = SampleAlbum;
        var track = NewSampleTrack();

        void Check(string field, string template, bool required, Func<string> resolve)
        {
            if (string.IsNullOrWhiteSpace(template))
            {
                // A folder template left blank is a choice (no such folder); a file name cannot be
                if (required) issues.Add(new TemplateIssue(field, "empty"));
                return;
            }

            var trimmed = template.Trim();
            if (trimmed.StartsWith('/') || trimmed.StartsWith('\\') || DriveLetterRegex.IsMatch(trimmed))
                issues.Add(new TemplateIssue(field, "rooted"));

            if (trimmed.Split('/', '\\').Any(part => part.Trim() == ".."))
                issues.Add(new TemplateIssue(field, "parent"));

            var replaced = PercentRegex.Replace(template, "").Where(ReplacedCharacters.Contains).Distinct().ToArray();
            if (replaced.Length > 0)
                issues.Add(new TemplateIssue(field, "illegalChars", string.Join(" ", replaced)));

            var unresolved = UnresolvedTagRegex.Matches(resolve()).Select(m => m.Value).Distinct().ToList();
            if (unresolved.Count > 0)
                issues.Add(new TemplateIssue(field, "unknownTag", string.Join(" ", unresolved)));
        }

        Check(nameof(AppSettings.ArtistTemplate), s.ArtistTemplate, false, () => ApplyTemplate(s.ArtistTemplate, album, null, null));
        Check(nameof(AppSettings.AlbumTemplate), s.AlbumTemplate, false, () => ApplyTemplate(s.AlbumTemplate, album, null, null));
        Check(nameof(AppSettings.TrackTemplate), s.TrackTemplate, true, () => ApplyTemplate(s.TrackTemplate, album, track, SamplePlaylist));
        Check(nameof(AppSettings.VaTrackTemplate), s.VaTrackTemplate, true, () => ApplyTemplate(s.VaTrackTemplate, album, track, SamplePlaylist));
        Check(nameof(AppSettings.PlaylistTemplate), s.PlaylistTemplate, false, () => ApplyTemplate(s.PlaylistTemplate, album, track, SamplePlaylist));
        Check(nameof(AppSettings.CdTemplate), s.CdTemplate, false, () => Regex.Replace(s.CdTemplate, "%DiscNumber%", "2", RegexOptions.IgnoreCase));

        return issues;
    }

    public static string MakeSafeFilename(string name)
    {
        if (string.IsNullOrWhiteSpace(name)) return name;

        // Replace Windows/NAS-forbidden characters with readable ASCII substitutes so file and
        // folder names stay legible and cross-platform safe (the container runs on Linux, but
        // downloads are frequently stored on SMB/Windows shares).
        var invalid = Path.GetInvalidFileNameChars();
        var sb = new StringBuilder(name.Length);
        var replacements = new Dictionary<char, string>
        {
            { ':', " -" },   // "Song: Intro" -> "Song - Intro"
            { '/', "-" },    // metadata slash (e.g. "AC/DC") -> "AC-DC"; template slashes are protected as {slash} beforehand
            { '\\', "-" },
            { '*', "" }, { '?', "" }, { '|', "" }, { '"', "" }, { '<', "" }, { '>', "" }
        };

        foreach (var c in name)
        {
            if (replacements.TryGetValue(c, out var replacement))
                sb.Append(replacement);
            else if (invalid.Contains(c))
                sb.Append('_');
            else
                sb.Append(c);
        }

        // Collapse whitespace left by removed characters and trim leftover separator artifacts
        // (e.g. a trailing " -" from a title ending in ':'). The main template path also collapses
        // spaces, but the standalone goodies call does not \u2014 so normalize here too.
        var result = SpacesRegex.Replace(sb.ToString(), " ").Trim(' ', '.');
        return EdgeSeparatorRegex.Replace(result, "").Trim(' ', '.');
    }

    /// <summary>
    /// Truncates to at most <paramref name="maxBytes"/> bytes of UTF-8 and marks the cut with an
    /// ellipsis. The limit is in bytes, not characters, because that is what filesystems count:
    /// 255 per path component on ext4, and a title in Japanese or Cyrillic takes 2 to 3 bytes per
    /// character. Cuts fall between text elements, never inside a surrogate pair or a base letter
    /// and its combining marks.
    /// </summary>
    public static string TruncateToBytes(string name, int maxBytes = MaxSegmentBytes)
    {
        if (Encoding.UTF8.GetByteCount(name) <= maxBytes) return name;

        const string ellipsis = "\u2026";
        var budget = maxBytes - Encoding.UTF8.GetByteCount(ellipsis);

        var kept = new StringBuilder();
        var used = 0;
        var elements = StringInfo.GetTextElementEnumerator(name);
        while (elements.MoveNext())
        {
            var element = (string)elements.Current;
            var size = Encoding.UTF8.GetByteCount(element);
            if (used + size > budget) break;

            kept.Append(element);
            used += size;
        }

        return kept.Append(ellipsis).ToString();
    }
}

/// <summary>Chemins d'exemple produits par les modèles en vigueur (voir <see cref="TemplateService.Preview"/>).</summary>
public sealed record TemplatePreview(string Album, string Compilation, string Playlist);

/// <summary>Un défaut d'un modèle : le champ concerné, un code d'erreur et, au besoin, le détail.</summary>
public sealed record TemplateIssue(string Field, string Code, string? Detail = null);
