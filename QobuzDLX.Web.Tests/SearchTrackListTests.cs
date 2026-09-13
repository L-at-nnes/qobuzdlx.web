using QobuzDLX.Web.Models;
using QobuzDLX.Web.Services;
using Xunit;

namespace QobuzDLX.Web.Tests;

public class SearchTrackListTests
{
    [Fact]
    public void A_row_alternates_between_expanded_and_collapsed()
    {
        var state = new SearchStateService();
        var key = SearchStateService.AlbumKey("abc");

        Assert.False(state.IsExpanded(key));
        Assert.True(state.ToggleExpanded(key));
        Assert.True(state.IsExpanded(key));
        Assert.False(state.ToggleExpanded(key));
        Assert.False(state.IsExpanded(key));
    }

    [Fact]
    public void An_album_and_a_playlist_with_the_same_number_do_not_share_a_row()
    {
        var state = new SearchStateService();

        state.ToggleExpanded(SearchStateService.AlbumKey("42"));

        Assert.False(state.IsExpanded(SearchStateService.PlaylistKey(42)));
    }

    [Fact]
    public void Loaded_tracks_are_kept_when_the_row_is_collapsed_and_opened_again()
    {
        var state = new SearchStateService();
        var key = SearchStateService.PlaylistKey(7);
        var tracks = new List<QobuzTrack> { new() { Id = 1, Title = "One" }, new() { Id = 2, Title = "Two" } };

        Assert.Null(state.TracksOf(key));
        state.SetTracks(key, tracks);
        state.ToggleExpanded(key);
        state.ToggleExpanded(key);

        Assert.Equal(new[] { "One", "Two" }, state.TracksOf(key)!.Select(t => t.Title));
    }

    [Fact]
    public void A_new_search_forgets_the_open_rows_and_their_tracks()
    {
        var state = new SearchStateService();
        var key = SearchStateService.AlbumKey("abc");
        state.ToggleExpanded(key);
        state.SetTracks(key, new List<QobuzTrack> { new() { Id = 1, Title = "One" } });

        state.ForgetTrackLists();

        Assert.False(state.IsExpanded(key));
        Assert.Null(state.TracksOf(key));
    }

    [Theory]
    [InlineData(0, "0:00")]
    [InlineData(9, "0:09")]
    [InlineData(249, "4:09")]
    [InlineData(3600, "1:00:00")]
    [InlineData(3723, "1:02:03")]
    [InlineData(-5, "0:00")]
    public void Durations_are_shown_as_minutes_and_seconds(int seconds, string expected)
        => Assert.Equal(expected, TrackDuration.Format(seconds));
}
