using QobuzDLX.Web.Models;

namespace QobuzDLX.Web.Services;

public class SearchStateService
{
    public string Query { get; set; } = "";
    public string LastSearchType { get; set; } = "albums";
    public List<QobuzAlbum>? Albums { get; set; }
    public List<QobuzTrack>? Tracks { get; set; }
    public List<QobuzArtist>? Artists { get; set; }
    public int ExpandedArtistId { get; set; }
    public List<QobuzAlbum>? ArtistAlbums { get; set; }

    // Sort order of each result list. Kept here (like the results themselves) so it survives
    // navigating away from the search page and back.
    public SearchSort AlbumSort { get; set; } = SearchSort.Relevance;
    public SearchSort TrackSort { get; set; } = SearchSort.Relevance;
    public SearchSort ArtistSort { get; set; } = SearchSort.Relevance;
    // A discography reads best chronologically.
    public SearchSort ArtistAlbumSort { get; set; } = SearchSort.DateAsc;
}
