using Newtonsoft.Json;
using QobuzDLX.Web.Models;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class PlaylistTrackIdTests
{
    [Fact]
    public static void Playlist_track_ids_above_int32_are_read()
    {
        // Real value returned by Qobuz for a playlist entry (found while testing against the live service)
        var track = JsonConvert.DeserializeObject<QobuzTrack>("{\"id\":140682305,\"title\":\"Mission\",\"playlist_track_id\":12424235026,\"position\":1}")!;
        H.Check("playlist_track_id above Int32 is read", track.PlaylistTrackId == 12424235026L);

        var playlist = JsonConvert.DeserializeObject<QobuzPlaylist>("{\"id\":71588272,\"name\":\"x\",\"tracks\":{\"total\":1,\"items\":[{\"id\":1,\"title\":\"t\",\"playlist_track_id\":12424235026}]}}")!;
        H.Check("a whole playlist with such an entry deserializes", playlist.Tracks!.Items[0].PlaylistTrackId == 12424235026L);
    }
}
