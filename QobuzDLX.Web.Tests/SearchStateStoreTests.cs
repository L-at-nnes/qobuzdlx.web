using System.Security.Claims;
using Microsoft.AspNetCore.Components.Authorization;
using QobuzDLX.Web.Services;
using Xunit;

namespace QobuzDLX.Web.Tests;

public class SearchStateStoreTests
{
    private sealed class FixedAuthentication(ClaimsPrincipal user) : AuthenticationStateProvider
    {
        public override Task<AuthenticationState> GetAuthenticationStateAsync() => Task.FromResult(new AuthenticationState(user));
    }

    [Fact]
    public void A_device_keeps_the_same_state_and_two_devices_do_not_share_one()
    {
        var store = new SearchStateStore();

        var phone = store.For("phone");
        phone.Query = "miles davis";

        Assert.Same(phone, store.For("phone"));
        Assert.NotSame(phone, store.For("laptop"));
        Assert.Equal("", store.For("laptop").Query);
    }

    [Fact]
    public void Without_a_device_everyone_shares_a_single_state()
    {
        var store = new SearchStateStore();

        Assert.Same(store.For(null), store.For(null));
        Assert.NotSame(store.For(null), store.For("phone"));
    }

    [Fact]
    public void The_least_recently_used_device_is_forgotten_beyond_the_limit()
    {
        var store = new SearchStateStore();
        var first = store.For("device-0");
        first.Query = "kept?";

        for (var i = 1; i < SearchStateStore.MaxDevices; i++) store.For($"device-{i}");
        store.For("device-0");                       // used again: no longer the oldest
        store.For("one-too-many");

        Assert.Same(first, store.For("device-0"));
        Assert.Equal("", store.For("device-1").Query);   // device-1 was the oldest and was dropped, so this one is new
    }

    [Fact]
    public void The_device_id_is_read_from_the_access_cookie_claim()
    {
        var withDevice = new ClaimsPrincipal(new ClaimsIdentity(new[] { new Claim(AppAccessGate.DeviceClaim, "abc123") }, "test"));
        var withoutDevice = new ClaimsPrincipal(new ClaimsIdentity(new[] { new Claim(ClaimTypes.Name, "qbdlx") }, "test"));

        Assert.Equal("abc123", SearchStateStore.DeviceIdOf(new FixedAuthentication(withDevice)));
        Assert.Null(SearchStateStore.DeviceIdOf(new FixedAuthentication(withoutDevice)));
        Assert.Null(SearchStateStore.DeviceIdOf(new FixedAuthentication(new ClaimsPrincipal(new ClaimsIdentity()))));
    }
}
