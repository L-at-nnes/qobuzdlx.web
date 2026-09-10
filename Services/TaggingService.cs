using System.Net;
using System.Text.RegularExpressions;
using QobuzDLX.Web.Models;
using TagLib;
using Id3v2Tag = TagLib.Id3v2.Tag;
using TagLib.Id3v2;

namespace QobuzDLX.Web.Services;

public class TaggingService
{
    // Store URLs carry a locale segment (/fr-fr/, /us-en/, /de-de/...) that has nothing to do with
    // the release: the link must be the same whatever the language it was read in.
    private static readonly Regex LocaleSegmentRegex =
        new("(?<=qobuz[.]com)/[a-z]{2}-[a-z]{2}(?=/)", RegexOptions.Compiled | RegexOptions.IgnoreCase);

    private readonly SettingsService _settings;
    private readonly TemplateService _templates;
    private readonly ILogger<TaggingService> _logger;

    public TaggingService(SettingsService settings, TemplateService templates, ILogger<TaggingService> logger)
    {
        _settings = settings;
        _templates = templates;
        _logger = logger;
    }

    public void WriteToFile(string filePath, string? artworkPath, QobuzAlbum album, QobuzTrack track)
    {
        Id3v2Tag.DefaultVersion = 4;
        Id3v2Tag.ForceDefaultVersion = true;

        using var file = TagLib.File.Create(filePath);
        file.RemoveTags(TagTypes.Id3v1);

        bool isFlac = filePath.EndsWith(".flac", StringComparison.OrdinalIgnoreCase);
        var s = _settings.Settings;

        if (isFlac)
        {
            var xiph = (TagLib.Ogg.XiphComment)file.GetTag(TagTypes.Xiph, true);
            SetFlacTags(file, xiph, album, track, s);
        }
        else
        {
            var id3 = (Id3v2Tag)file.GetTag(TagTypes.Id3v2, true);
            SetMp3Tags(file, id3, album, track, s);
        }

        SetCommonTags(file, album, track, s);
        SetAlbumArtists(file, album, s);
        EmbedArtwork(file, artworkPath, s);

        file.Save();
        _logger.LogDebug("File tagging completed: {Path}", filePath);
    }

    /// <summary>
    /// Qobuz sends the description as HTML: line breaks as tags, accents and © as entities
    /// (<c>&amp;copy;</c>), which a tag reader would otherwise show verbatim.
    /// </summary>
    public static string BuildComment(string template, string? description) =>
        template
            .Replace("%description%", WebUtility.HtmlDecode(description ?? ""))
            .Replace("<br/>", Environment.NewLine)
            .Replace("<br />", Environment.NewLine);

    private void SetFlacTags(TagLib.File file, TagLib.Ogg.XiphComment tags, QobuzAlbum album, QobuzTrack track, AppSettings s)
    {
        if (s.TrackTag) file.Tag.Track = (uint)track.TrackNumber;
        if (s.IsrcTag) tags.SetField("ISRC", track.ISRC ?? "");
        if (s.TypeTag) tags.SetField("MEDIATYPE", album.ProductType.ToUpper());
        if (s.UpcTag) tags.SetField("BARCODE", album.UPC ?? "");
        if (s.LabelTag) tags.SetField("LABEL", album.Label?.Name ?? "");
        if (s.ExplicitTag) tags.SetField("ITUNESADVISORY", track.ParentalWarning ? "1" : "0");

        if (s.CommentTag && !string.IsNullOrEmpty(s.CommentText))
        {
            tags.SetField("COMMENT", new[] { BuildComment(s.CommentText, album.Description) });
        }

        if (s.UrlTag)
        {
            var url = ResolveQobuzUrl(album, track);
            if (!string.IsNullOrWhiteSpace(url))
                tags.SetField("Qobuz URL", new[] { url });
        }

        if (s.ReleaseDateTag || s.YearTag)
        {
            var date = (!string.IsNullOrWhiteSpace(track.Album?.ReleaseDateOriginal) ? track.Album.ReleaseDateOriginal : album.ReleaseDateOriginal)?.Trim();
            if (!string.IsNullOrEmpty(date))
            {
                if (s.ReleaseDateTag) tags.SetField("DATE", date);
                if (s.YearTag && date.Length >= 4) tags.SetField("YEAR", date[..4]);
            }
        }
    }

    private void SetMp3Tags(TagLib.File file, Id3v2Tag mp3Tag, QobuzAlbum album, QobuzTrack track, AppSettings s)
    {
        if (s.UpcTag) UserTextInformationFrame.Get(mp3Tag, "BARCODE", true).Text = new[] { album.UPC ?? "" };
        if (s.ExplicitTag) UserTextInformationFrame.Get(mp3Tag, "ITUNESADVISORY", true).Text = new[] { track.ParentalWarning ? "1" : "0" };
        if (s.IsrcTag) mp3Tag.SetTextFrame("TSRC", track.ISRC ?? "");
        if (s.LabelTag) mp3Tag.SetTextFrame("TPUB", album.Label?.Name ?? "");
        if (s.TypeTag)
        {
            var pt = album.ProductType;
            mp3Tag.SetTextFrame("TMED", string.IsNullOrEmpty(pt) ? "" : char.ToUpper(pt[0]) + pt[1..].ToLower());
        }

        if (s.CommentTag && !string.IsNullOrEmpty(s.CommentText))
        {
            mp3Tag.Comment = BuildComment(s.CommentText, album.Description);
        }

        if (s.TrackTag)
        {
            mp3Tag.SetNumberFrame("TRCK", (uint)track.TrackNumber, file.Tag.TrackCount);
        }

        if (s.UrlTag)
        {
            var url = ResolveQobuzUrl(album, track);
            if (!string.IsNullOrWhiteSpace(url))
            {
                var data = new ByteVector {
                    0, ByteVector.FromString("Qobuz URL", StringType.UTF8),
                    0, ByteVector.FromString(url, StringType.UTF8)
                };
                mp3Tag.AddFrame(new UnknownFrame("WXXX", data));
            }
        }

        if (s.ReleaseDateTag || s.YearTag)
        {
            var date = album.ReleaseDateOriginal?.Trim();
            if (!string.IsNullOrEmpty(date))
            {
                if (s.YearTag && date.Length >= 4 && uint.TryParse(date[..4], out var y))
                    mp3Tag.Year = y;
                if (s.ReleaseDateTag)
                    mp3Tag.SetTextFrame("TDRL", date);
            }
        }
    }

    private static string? ResolveQobuzUrl(QobuzAlbum album, QobuzTrack track)
    {
        var url = !string.IsNullOrWhiteSpace(track.Url) ? track.Url : album.Url;
        return url == null ? null : LocaleSegmentRegex.Replace(url, "");
    }

    /// <summary>Ajoute le marqueur au titre : devant (0) ou derrière (1). Les espaces du marqueur sont
    /// conservés tels quels, c'est à celui qui le saisit de prévoir « [E] » ou « (Explicit) ».</summary>
    public static string ApplyTitleTag(string title, string tag, int prefixOrSuffix) =>
        prefixOrSuffix == 0 ? tag + title : title + tag;

    private void SetCommonTags(TagLib.File file, QobuzAlbum album, QobuzTrack track, AppSettings s)
    {
        if (s.GenreTag) file.Tag.Genres = new[] { album.Genre?.Name ?? "" };
        if (s.AlbumTag) file.Tag.Album = album.Version == null ? album.Title : $"{album.Title.TrimEnd()} ({album.Version})";
        if (s.ComposerTag) file.Tag.Composers = new[] { track.Composer?.Name ?? "" };
        if (s.TotalTracksTag) file.Tag.TrackCount = (uint)album.TracksCount;
        if (s.DiscTag) file.Tag.Disc = (uint)track.MediaNumber;
        if (s.TotalDiscsTag) file.Tag.DiscCount = (uint)album.MediaCount;
        if (s.CopyrightTag) file.Tag.Copyright = album.Copyright ?? "";

        if (s.TrackTitleTag)
        {
            var title = _templates.FormatTrackTitle(track);

            if (track.ParentalWarning && s.ExplicitTagInTitle && !string.IsNullOrWhiteSpace(s.ExplicitTitleTagText))
                title = ApplyTitleTag(title, s.ExplicitTitleTagText, s.ExplicitTitleTagPrefixOrSuffix);
            else if (!track.ParentalWarning && s.CleanTagInTitle && !string.IsNullOrWhiteSpace(s.CleanTitleTagText))
                title = ApplyTitleTag(title, s.CleanTitleTagText, s.CleanTitleTagPrefixOrSuffix);

            file.Tag.Title = title;
        }

        if (s.ArtistTag)
        {
            file.Tag.Performers = new[] { track.Performer?.Name ?? "" };
        }
    }

    private void SetAlbumArtists(TagLib.File file, QobuzAlbum album, AppSettings s)
    {
        if (!s.AlbumArtistTag) return;

        var mainArtists = album.Artists?.Where(a => a.Roles.Contains("main-artist")).ToList();
        if (mainArtists != null && mainArtists.Count > 1)
        {
            var allButLast = string.Join(", ", mainArtists.Take(mainArtists.Count - 1).Select(a => a.Name));
            file.Tag.AlbumArtists = new[] { $"{allButLast} & {mainArtists.Last().Name}" };
        }
        else
        {
            file.Tag.AlbumArtists = new[] { album.Artist?.Name ?? "" };
        }
    }

    private void EmbedArtwork(TagLib.File file, string? artworkPath, AppSettings s)
    {
        if (!s.ImageTag || string.IsNullOrEmpty(artworkPath) || !System.IO.File.Exists(artworkPath)) return;

        try
        {
            var pic = new AttachmentFrame
            {
                TextEncoding = StringType.Latin1,
                MimeType = System.Net.Mime.MediaTypeNames.Image.Jpeg,
                Type = PictureType.FrontCover,
                Data = ByteVector.FromPath(artworkPath)
            };
            file.Tag.Pictures = new IPicture[] { pic };
        }
        catch (Exception ex)
        {
            _logger.LogError("Failed to embed artwork: {Error}", ex.Message);
        }
    }
}
