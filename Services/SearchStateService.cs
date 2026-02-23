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
}
