using Newtonsoft.Json;

namespace QobuzDLX.Web.Models;

public class QobuzImage
{
    [JsonProperty("small")] public string Small { get; set; } = "";
    [JsonProperty("thumbnail")] public string Thumbnail { get; set; } = "";
    [JsonProperty("large")] public string Large { get; set; } = "";
    [JsonProperty("back")] public string? Back { get; set; }
}

public class QobuzGenre
{
    [JsonProperty("id")] public int Id { get; set; }
    [JsonProperty("name")] public string Name { get; set; } = "";
}

public class QobuzLabel
{
    [JsonProperty("id")] public int Id { get; set; }
    [JsonProperty("name")] public string Name { get; set; } = "";
    [JsonProperty("albums_count")] public int AlbumsCount { get; set; }
    [JsonProperty("albums")] public QobuzItemList<QobuzAlbum>? Albums { get; set; }
}

public class QobuzComposer
{
    [JsonProperty("id")] public int Id { get; set; }
    [JsonProperty("name")] public string? Name { get; set; }
}

public class QobuzPerformer
{
    [JsonProperty("id")] public int Id { get; set; }
    [JsonProperty("name")] public string? Name { get; set; }
}

public class QobuzArtistRole
{
    [JsonProperty("id")] public int Id { get; set; }
    [JsonProperty("name")] public string Name { get; set; } = "";
    [JsonProperty("roles")] public List<string> Roles { get; set; } = new();
}

public class QobuzArtist
{
    [JsonProperty("id")] public int Id { get; set; }
    [JsonProperty("name")] public string Name { get; set; } = "";
    [JsonProperty("picture")] public string? Picture { get; set; }
    [JsonProperty("albums_count")] public int AlbumsCount { get; set; }
    [JsonProperty("albums")] public QobuzItemList<QobuzAlbum>? Albums { get; set; }
}

public class QobuzItemList<T>
{
    [JsonProperty("offset")] public int Offset { get; set; }
    [JsonProperty("limit")] public int Limit { get; set; }
    [JsonProperty("total")] public int Total { get; set; }
    [JsonProperty("items")] public List<T> Items { get; set; } = new();
    [JsonProperty("has_more")] public bool HasMore { get; set; }
}

public class QobuzGoody
{
    [JsonProperty("id")] public int Id { get; set; }
    [JsonProperty("file_format_id")] public int FileFormatId { get; set; }
    [JsonProperty("description")] public string? Description { get; set; }
    [JsonProperty("url")] public string? Url { get; set; }
}

public class QobuzAlbum
{
    [JsonProperty("id")] public string Id { get; set; } = "";
    [JsonProperty("title")] public string Title { get; set; } = "";
    [JsonProperty("version")] public string? Version { get; set; }
    [JsonProperty("url")] public string? Url { get; set; }
    [JsonProperty("artist")] public QobuzPerformer Artist { get; set; } = new();
    [JsonProperty("artists")] public List<QobuzArtistRole> Artists { get; set; } = new();
    [JsonProperty("image")] public QobuzImage Image { get; set; } = new();
    [JsonProperty("genre")] public QobuzGenre Genre { get; set; } = new();
    [JsonProperty("label")] public QobuzLabel Label { get; set; } = new();
    [JsonProperty("composer")] public QobuzComposer? Composer { get; set; }
    [JsonProperty("release_date_original")] public string? ReleaseDateOriginal { get; set; }
    [JsonProperty("upc")] public string? UPC { get; set; }
    [JsonProperty("copyright")] public string? Copyright { get; set; }
    [JsonProperty("description")] public string? Description { get; set; }
    [JsonProperty("product_type")] public string ProductType { get; set; } = "album";
    [JsonProperty("streamable")] public bool Streamable { get; set; } = true;
    [JsonProperty("parental_warning")] public bool ParentalWarning { get; set; }
    [JsonProperty("tracks_count")] public int TracksCount { get; set; }
    [JsonProperty("media_count")] public int MediaCount { get; set; } = 1;
    [JsonProperty("maximum_bit_depth")] public int MaximumBitDepth { get; set; }
    [JsonProperty("maximum_sampling_rate")] public double MaximumSamplingRate { get; set; }
    [JsonProperty("tracks")] public QobuzItemList<QobuzTrack>? Tracks { get; set; }
    [JsonProperty("goodies")] public List<QobuzGoody>? Goodies { get; set; }
}

public class QobuzTrack
{
    [JsonProperty("id")] public int Id { get; set; }
    [JsonProperty("title")] public string Title { get; set; } = "";
    [JsonProperty("version")] public string? Version { get; set; }
    [JsonProperty("url")] public string? Url { get; set; }
    [JsonProperty("track_number")] public int TrackNumber { get; set; }
    [JsonProperty("media_number")] public int MediaNumber { get; set; } = 1;
    [JsonProperty("position")] public int Position { get; set; }
    [JsonProperty("duration")] public int Duration { get; set; }
    [JsonProperty("streamable")] public bool Streamable { get; set; } = true;
    [JsonProperty("parental_warning")] public bool ParentalWarning { get; set; }
    [JsonProperty("isrc")] public string? ISRC { get; set; }
    [JsonProperty("maximum_bit_depth")] public int MaximumBitDepth { get; set; }
    [JsonProperty("maximum_sampling_rate")] public double MaximumSamplingRate { get; set; }
    [JsonProperty("performer")] public QobuzPerformer? Performer { get; set; }
    [JsonProperty("composer")] public QobuzComposer? Composer { get; set; }
    [JsonProperty("performers")] public string? Performers { get; set; }
    [JsonProperty("album")] public QobuzAlbum? Album { get; set; }
    [JsonProperty("playlist_track_id")] public int? PlaylistTrackId { get; set; }
}

public class QobuzPlaylist
{
    [JsonProperty("id")] public long Id { get; set; }
    [JsonProperty("name")] public string Name { get; set; } = "";
    [JsonProperty("description")] public string? Description { get; set; }
    [JsonProperty("tracks_count")] public int TracksCount { get; set; }
    [JsonProperty("images150")] public List<string>? Images150 { get; set; }
    [JsonProperty("image_rectangle")] public List<string>? ImageRectangle { get; set; }
    [JsonProperty("tracks")] public QobuzItemList<QobuzTrack>? Tracks { get; set; }
}

public class QobuzStreamUrl
{
    [JsonProperty("url")] public string? Url { get; set; }
    [JsonProperty("format_id")] public int FormatId { get; set; }
    [JsonProperty("mime_type")] public string? MimeType { get; set; }
    [JsonProperty("sampling_rate")] public double SamplingRate { get; set; }
    [JsonProperty("bit_depth")] public int BitDepth { get; set; }

    /// <summary>Vrai quand l'URL ne donne que l'extrait de 30 s et non la piste entière.</summary>
    [JsonProperty("sample")] public bool Sample { get; set; }
}

public class QobuzUserInfo
{
    [JsonProperty("id")] public int Id { get; set; }
    [JsonProperty("display_name")] public string? DisplayName { get; set; }
    [JsonProperty("email")] public string? Email { get; set; }
    [JsonProperty("country_code")] public string? CountryCode { get; set; }
    [JsonProperty("avatar")] public string? Avatar { get; set; }
    [JsonProperty("credential")] public QobuzCredential? Credential { get; set; }
}

public class QobuzCredential
{
    [JsonProperty("parameters")] public QobuzCredentialParams? Parameters { get; set; }
}

public class QobuzCredentialParams
{
    [JsonProperty("short_label")] public string? ShortLabel { get; set; }
    [JsonProperty("lossy_streaming")] public bool LossyStreaming { get; set; }
    [JsonProperty("lossless_streaming")] public bool LosslessStreaming { get; set; }
    [JsonProperty("hires_streaming")] public bool HiresStreaming { get; set; }
    [JsonProperty("hires_purchases_streaming")] public bool HiresPurchasesStreaming { get; set; }
    [JsonProperty("expiration_date")] public string? ExpirationDate { get; set; }
}

public class QobuzLoginResponse
{
    [JsonProperty("user_auth_token")] public string? UserAuthToken { get; set; }
    [JsonProperty("user")] public QobuzUserInfo? User { get; set; }
}

public class QobuzSearchAlbumResult
{
    [JsonProperty("albums")] public QobuzItemList<QobuzAlbum> Albums { get; set; } = new();
}

public class QobuzSearchTrackResult
{
    [JsonProperty("tracks")] public QobuzItemList<QobuzTrack> Tracks { get; set; } = new();
}

public class QobuzSearchArtistResult
{
    [JsonProperty("artists")] public QobuzItemList<QobuzArtist> Artists { get; set; } = new();
}

public class QobuzFavorites
{
    [JsonProperty("albums")] public QobuzItemList<QobuzAlbum>? Albums { get; set; }
    [JsonProperty("tracks")] public QobuzItemList<QobuzTrack>? Tracks { get; set; }
    [JsonProperty("artists")] public QobuzItemList<QobuzArtist>? Artists { get; set; }
}

public class QobuzAppId
{
    public string AppId { get; set; } = "";
    public string AppSecret { get; set; } = "";
}
