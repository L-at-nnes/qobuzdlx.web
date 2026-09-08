using QobuzDLX.Web.Models;

namespace QobuzDLX.Web.Services;

public class SearchStateService
{
    public string Query { get; set; } = "";

    /// <summary>Requête de la dernière recherche exécutée : « Voir plus » doit prolonger celle-là,
    /// pas ce qui a été tapé depuis dans le champ.</summary>
    public string LastQuery { get; set; } = "";
    public string LastSearchType { get; set; } = "albums";
    public List<QobuzAlbum>? Albums { get; set; }
    public List<QobuzTrack>? Tracks { get; set; }
    public List<QobuzArtist>? Artists { get; set; }

    // Nombre total de résultats côté Qobuz, pour savoir s'il reste des pages.
    public int AlbumsTotal { get; set; }
    public int TracksTotal { get; set; }
    public int ArtistsTotal { get; set; }

    public bool HasMoreAlbums => Albums != null && Albums.Count < AlbumsTotal;
    public bool HasMoreTracks => Tracks != null && Tracks.Count < TracksTotal;
    public bool HasMoreArtists => Artists != null && Artists.Count < ArtistsTotal;

    // Les listes sont remplacées et jamais modifiées : cet objet est partagé par tous les circuits
    // ouverts, et l'un d'eux peut être en train d'énumérer l'ancienne liste pendant l'ajout.
    public void AppendAlbums(IEnumerable<QobuzAlbum> page, int total)
    {
        Albums = Append(Albums, page);
        AlbumsTotal = total;
    }

    public void AppendTracks(IEnumerable<QobuzTrack> page, int total)
    {
        Tracks = Append(Tracks, page);
        TracksTotal = total;
    }

    public void AppendArtists(IEnumerable<QobuzArtist> page, int total)
    {
        Artists = Append(Artists, page);
        ArtistsTotal = total;
    }

    private static List<T> Append<T>(List<T>? current, IEnumerable<T> page) =>
        (current ?? new List<T>()).Concat(page).ToList();
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
