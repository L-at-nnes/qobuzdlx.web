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
    private static readonly Regex RepeatedParenthesesRegex = new(@"\(([^()]+)\)\s*(\(\1\))+", RegexOptions.Compiled | RegexOptions.IgnoreCase);

    // Les noms de fichiers ne suivent pas la langue de l'interface : la culture du processus est
    // fr-FR quand l'utilisateur est en français, ce qui écrirait « 44,1kHz » au lieu de « 44.1kHz ».
    private static string Rate(double khz) => khz.ToString(CultureInfo.InvariantCulture);

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
        var fullBase = Path.GetFullPath(downloadLocation);
        var fullPath = Path.GetFullPath(downloadPath);
        if (!fullPath.StartsWith(fullBase, StringComparison.OrdinalIgnoreCase))
            downloadPath = downloadLocation;

        return downloadPath;
    }

    /// <summary>
    /// Remove path traversal sequences from a single path segment.
    /// Returns empty string if input was empty (intentionally blank template).
    /// Returns "_" only if input had content but became empty after sanitization.
    /// </summary>
    private static string SanitizePathSegment(string segment)
    {
        if (string.IsNullOrWhiteSpace(segment)) return "";
        segment = segment.Replace("..", "").Replace("\0", "");
        segment = segment.Trim('.', ' ');
        return string.IsNullOrWhiteSpace(segment) ? "_" : segment;
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
            int padLen = album?.TracksCount.ToString().Length ?? 2;

            template = ReplaceParentalWarningTags(template, track.ParentalWarning);
            template = template
                .Replace("%trackid%", track.Id.ToString())
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
            template = ReplaceParentalWarningTags(template, album.ParentalWarning);
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

        // CD / Disc tags (also handle here as fallback)
        template = template.Replace("%discnumber%", "");

        // Sanitize
        template = MakeSafeFilename(template);
        template = template.Replace("{slash}", "/");
        template = SpacesRegex.Replace(template, " ").Trim();
        template = template.Replace("...", "\u2026");

        return template;
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

    /// <summary>
    /// Complete parental warning tag replacement matching original QobuzDownloaderX RenameTemplates.
    /// </summary>
    private static string ReplaceParentalWarningTags(string template, bool isExplicit)
    {
        // ── Track PA tags ──
        template = template.Replace("%trackpa%", isExplicit ? "Explicit" : "Clean");
        template = template.Replace("%trackpashort%", isExplicit ? "E" : "C");
        template = template.Replace("%trackpaifex%", isExplicit ? "Explicit" : "");
        template = template.Replace("%trackpaifexshort%", isExplicit ? "E" : "");
        template = template.Replace("%trackpaifcl%", isExplicit ? "" : "Clean");
        template = template.Replace("%trackpaifclshort%", isExplicit ? "" : "C");
        template = template.Replace("%trackpaenclosed%", isExplicit ? "(Explicit)" : "(Clean)");
        template = template.Replace("%trackpaenclosed[]%", isExplicit ? "[Explicit]" : "[Clean]");
        template = template.Replace("%trackpaenclosedshort%", isExplicit ? "(E)" : "(C)");
        template = template.Replace("%trackpaenclosedshort[]%", isExplicit ? "[E]" : "[C]");
        template = template.Replace("%trackpaifexenclosed%", isExplicit ? "(Explicit)" : "");
        template = template.Replace("%trackpaifexenclosed[]%", isExplicit ? "[Explicit]" : "");
        template = template.Replace("%trackpaifexenclosedshort%", isExplicit ? "(E)" : "");
        template = template.Replace("%trackpaifexenclosedshort[]%", isExplicit ? "[E]" : "");
        template = template.Replace("%trackpaifclenclosed%", isExplicit ? "" : "(Clean)");
        template = template.Replace("%trackpaifclenclosed[]%", isExplicit ? "" : "[Clean]");
        template = template.Replace("%trackpaifclenclosedshort%", isExplicit ? "" : "(C)");
        template = template.Replace("%trackpaifclenclosedshort[]%", isExplicit ? "" : "[C]");

        // ── Album PA tags ──
        template = template.Replace("%albumpa%", isExplicit ? "Explicit" : "Clean");
        template = template.Replace("%albumpashort%", isExplicit ? "E" : "C");
        template = template.Replace("%albumpaifex%", isExplicit ? "Explicit" : "");
        template = template.Replace("%albumpaifexshort%", isExplicit ? "E" : "");
        template = template.Replace("%albumpaifcl%", isExplicit ? "" : "Clean");
        template = template.Replace("%albumpaifclshort%", isExplicit ? "" : "C");
        template = template.Replace("%albumpaenclosed%", isExplicit ? "(Explicit)" : "(Clean)");
        template = template.Replace("%albumpaenclosed[]%", isExplicit ? "[Explicit]" : "[Clean]");
        template = template.Replace("%albumpaenclosedshort%", isExplicit ? "(E)" : "(C)");
        template = template.Replace("%albumpaenclosedshort[]%", isExplicit ? "[E]" : "[C]");
        template = template.Replace("%albumpaifexenclosed%", isExplicit ? "(Explicit)" : "");
        template = template.Replace("%albumpaifexenclosed[]%", isExplicit ? "[Explicit]" : "");
        template = template.Replace("%albumpaifexenclosedshort%", isExplicit ? "(E)" : "");
        template = template.Replace("%albumpaifexenclosedshort[]%", isExplicit ? "[E]" : "");
        template = template.Replace("%albumpaifclenclosed%", isExplicit ? "" : "(Clean)");
        template = template.Replace("%albumpaifclenclosed[]%", isExplicit ? "" : "[Clean]");
        template = template.Replace("%albumpaifclenclosedshort%", isExplicit ? "" : "(C)");
        template = template.Replace("%albumpaifclenclosedshort[]%", isExplicit ? "" : "[C]");

        return template;
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
        var result = SpacesRegex.Replace(sb.ToString(), " ");
        return result.Trim(' ', '.', '-');
    }

    public static string TruncateLongName(string name, int reserveLen, int maxLen = 255)
    {
        if (string.IsNullOrEmpty(name)) return name;
        if (name.Length + reserveLen >= maxLen)
            name = name[..(maxLen - 1 - reserveLen)] + "\u2026";
        return name;
    }
}
