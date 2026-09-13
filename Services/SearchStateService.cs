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
    public List<QobuzPlaylist>? Playlists { get; set; }

    /// <summary>Masque ce qui n'est pas Hi-Res. Conservé ici comme les tris : il survit à la navigation.</summary>
    public bool HiResOnly { get; set; }

    // Nombre total de résultats côté Qobuz, pour savoir s'il reste des pages.
    public int AlbumsTotal { get; set; }
    public int TracksTotal { get; set; }
    public int ArtistsTotal { get; set; }
    public int PlaylistsTotal { get; set; }

    public bool HasMoreAlbums => Albums != null && Albums.Count < AlbumsTotal;
    public bool HasMoreTracks => Tracks != null && Tracks.Count < TracksTotal;
    public bool HasMoreArtists => Artists != null && Artists.Count < ArtistsTotal;
    public bool HasMorePlaylists => Playlists != null && Playlists.Count < PlaylistsTotal;

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

    public void AppendPlaylists(IEnumerable<QobuzPlaylist> page, int total)
    {
        Playlists = Append(Playlists, page);
        PlaylistsTotal = total;
    }

    private static List<T> Append<T>(List<T>? current, IEnumerable<T> page) =>
        (current ?? new List<T>()).Concat(page).ToList();

    // ─── Pistes dépliées (albums et playlists) ───
    //
    // Cet objet est partagé par les onglets d'un même appareil : les deux collections sont donc
    // protégées, et les listes de pistes stockées ne sont jamais modifiées après coup.
    private readonly object _trackListsLock = new();
    private readonly HashSet<string> _expanded = new();
    private readonly Dictionary<string, IReadOnlyList<QobuzTrack>> _trackLists = new();

    public static string AlbumKey(string albumId) => "album:" + albumId;
    public static string PlaylistKey(long playlistId) => "playlist:" + playlistId;

    public bool IsExpanded(string key)
    {
        lock (_trackListsLock) return _expanded.Contains(key);
    }

    /// <summary>Déplie ou replie la ligne ; vrai quand elle est dépliée après l'appel.</summary>
    public bool ToggleExpanded(string key)
    {
        lock (_trackListsLock) return _expanded.Remove(key) ? false : _expanded.Add(key);
    }

    /// <summary>Les pistes déjà chargées de cet album ou de cette playlist, ou null.</summary>
    public IReadOnlyList<QobuzTrack>? TracksOf(string key)
    {
        lock (_trackListsLock) return _trackLists.GetValueOrDefault(key);
    }

    public void SetTracks(string key, IReadOnlyList<QobuzTrack> tracks)
    {
        lock (_trackListsLock) _trackLists[key] = tracks;
    }

    /// <summary>Une nouvelle recherche remplace les résultats : les lignes dépliées n'ont plus lieu d'être.</summary>
    public void ForgetTrackLists()
    {
        lock (_trackListsLock)
        {
            _expanded.Clear();
            _trackLists.Clear();
        }
    }

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
