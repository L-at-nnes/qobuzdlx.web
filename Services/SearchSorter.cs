using QobuzDLX.Web.Models;

namespace QobuzDLX.Web.Services;

/// <summary>
/// Sort options shared by the search result lists. Each list only exposes the subset that makes
/// sense for it (e.g. <see cref="CountDesc"/> means "most tracks" for albums and "most albums"
/// for artists).
/// </summary>
public enum SearchSort
{
    Relevance,
    TitleAsc,
    TitleDesc,
    ArtistAsc,
    ArtistDesc,
    DateDesc,
    DateAsc,
    QualityDesc,
    CountDesc,
    CountAsc,
    LabelAsc,
    AlbumAsc,
    DurationAsc,
    DurationDesc
}

/// <summary>
/// Client-side sorting of already-fetched search results (the Qobuz search endpoints expose no
/// sort parameter). All methods return a new sequence and never mutate the source list, so
/// <see cref="SearchSort.Relevance"/> always restores the exact order returned by the API.
/// LINQ's OrderBy is stable, so ties keep their original relevance ranking.
/// </summary>
public static class SearchSorter
{
    private static readonly StringComparer TextComparer = StringComparer.CurrentCultureIgnoreCase;

    public static IEnumerable<QobuzAlbum> SortAlbums(IEnumerable<QobuzAlbum> albums, SearchSort sort) => sort switch
    {
        SearchSort.TitleAsc => albums.OrderBy(a => a.Title ?? "", TextComparer),
        SearchSort.TitleDesc => albums.OrderByDescending(a => a.Title ?? "", TextComparer),
        SearchSort.ArtistAsc => albums.OrderBy(a => a.Artist?.Name ?? "", TextComparer),
        SearchSort.ArtistDesc => albums.OrderByDescending(a => a.Artist?.Name ?? "", TextComparer),
        SearchSort.LabelAsc => albums.OrderBy(a => a.Label?.Name ?? "", TextComparer),
        SearchSort.DateDesc => ByDateDesc(albums, a => a.ReleaseDateOriginal),
        SearchSort.DateAsc => ByDateAsc(albums, a => a.ReleaseDateOriginal),
        SearchSort.QualityDesc => albums
            .OrderByDescending(a => a.MaximumBitDepth)
            .ThenByDescending(a => a.MaximumSamplingRate),
        SearchSort.CountDesc => albums.OrderByDescending(a => a.TracksCount),
        SearchSort.CountAsc => albums.OrderBy(a => a.TracksCount),
        _ => albums
    };

    public static IEnumerable<QobuzTrack> SortTracks(IEnumerable<QobuzTrack> tracks, SearchSort sort) => sort switch
    {
        SearchSort.TitleAsc => tracks.OrderBy(t => t.Title ?? "", TextComparer),
        SearchSort.TitleDesc => tracks.OrderByDescending(t => t.Title ?? "", TextComparer),
        SearchSort.ArtistAsc => tracks.OrderBy(t => t.Performer?.Name ?? "", TextComparer),
        SearchSort.ArtistDesc => tracks.OrderByDescending(t => t.Performer?.Name ?? "", TextComparer),
        SearchSort.AlbumAsc => tracks.OrderBy(t => t.Album?.Title ?? "", TextComparer),
        SearchSort.DateDesc => ByDateDesc(tracks, t => t.Album?.ReleaseDateOriginal),
        SearchSort.DateAsc => ByDateAsc(tracks, t => t.Album?.ReleaseDateOriginal),
        SearchSort.QualityDesc => tracks
            .OrderByDescending(t => t.MaximumBitDepth)
            .ThenByDescending(t => t.MaximumSamplingRate),
        SearchSort.DurationAsc => tracks.OrderBy(t => t.Duration),
        SearchSort.DurationDesc => tracks.OrderByDescending(t => t.Duration),
        _ => tracks
    };

    public static IEnumerable<QobuzArtist> SortArtists(IEnumerable<QobuzArtist> artists, SearchSort sort) => sort switch
    {
        SearchSort.TitleAsc => artists.OrderBy(a => a.Name ?? "", TextComparer),
        SearchSort.TitleDesc => artists.OrderByDescending(a => a.Name ?? "", TextComparer),
        SearchSort.CountDesc => artists.OrderByDescending(a => a.AlbumsCount),
        SearchSort.CountAsc => artists.OrderBy(a => a.AlbumsCount),
        _ => artists
    };

    // ─── Filtre Hi-Res ───

    /// <summary>
    /// Hi-Res au sens usuel de Qobuz : plus que le CD (16 bits, 44,1 kHz), par la profondeur ou par la
    /// fréquence. Une valeur absente (0) ne passe pas pour du Hi-Res.
    /// </summary>
    public static bool IsHiRes(QobuzAlbum album) => IsHiRes(album.MaximumBitDepth, album.MaximumSamplingRate);

    public static bool IsHiRes(QobuzTrack track) => IsHiRes(track.MaximumBitDepth, track.MaximumSamplingRate);

    private static bool IsHiRes(int bitDepth, double samplingRate) => bitDepth > 16 || samplingRate > 44.1;

    // ─── Date helpers ───
    // Release dates are ISO strings ("YYYY-MM-DD") and are sometimes missing. Sorting on a first
    // key "has no date" keeps undated entries at the bottom in both directions.

    private static IOrderedEnumerable<T> ByDateDesc<T>(IEnumerable<T> source, Func<T, string?> selector) =>
        source.OrderBy(x => ParseDate(selector(x)).HasValue ? 0 : 1)
              .ThenByDescending(x => ParseDate(selector(x)) ?? DateTime.MinValue);

    private static IOrderedEnumerable<T> ByDateAsc<T>(IEnumerable<T> source, Func<T, string?> selector) =>
        source.OrderBy(x => ParseDate(selector(x)).HasValue ? 0 : 1)
              .ThenBy(x => ParseDate(selector(x)) ?? DateTime.MaxValue);

    private static DateTime? ParseDate(string? value) =>
        DateTime.TryParse(value, System.Globalization.CultureInfo.InvariantCulture,
            System.Globalization.DateTimeStyles.None, out var date)
            ? date
            : null;
}
