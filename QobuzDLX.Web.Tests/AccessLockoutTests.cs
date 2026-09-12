using QobuzDLX.Web.Services;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class AccessLockoutTests
{
    private sealed class FakeTime : TimeProvider
    {
        public DateTimeOffset Now = new(2026, 10, 12, 12, 0, 0, TimeSpan.Zero);
        public override DateTimeOffset GetUtcNow() => Now;
        public void Advance(TimeSpan span) => Now += span;
    }

    private static void Fail(AppAccessGate gate, int times)
    {
        for (var i = 0; i < times; i++) gate.RegisterFailure();
    }

    [Fact]
    public static void Failed_unlocks_lock_progressively_and_the_level_decays()
    {
        // escalation unchanged while the attack goes on
        var time = new FakeTime();
        var gate = new AppAccessGate(time);
        Fail(gate, 4);
        H.Check("4 failures: not locked", !gate.IsLockedOut);
        Fail(gate, 1);
        H.Check("5th failure locks", gate.IsLockedOut);
        H.Check("first lockout is 30 s", gate.RemainingSeconds is >= 30 and <= 31);
        time.Advance(TimeSpan.FromSeconds(31));
        H.Check("lock expires", !gate.IsLockedOut);
        Fail(gate, 5);
        H.Check("second lockout is 1 min", gate.RemainingSeconds is >= 60 and <= 61);
        time.Advance(TimeSpan.FromSeconds(61));
        Fail(gate, 5);
        H.Check("third lockout is 5 min (still escalating)", gate.RemainingSeconds is >= 300 and <= 301);

        // an hour of calm per level brings the level back down
        time.Advance(TimeSpan.FromMinutes(5) + TimeSpan.FromHours(3) + TimeSpan.FromSeconds(1));
        Fail(gate, 5);
        H.Check("after 3 h of calm the level is back to 30 s", gate.RemainingSeconds is >= 30 and <= 31);

        // less than an hour of calm: no decay
        time = new FakeTime();
        gate = new AppAccessGate(time);
        Fail(gate, 5); time.Advance(TimeSpan.FromSeconds(31));
        Fail(gate, 5); time.Advance(TimeSpan.FromSeconds(61));
        time.Advance(TimeSpan.FromMinutes(30));
        Fail(gate, 5);
        H.Check("30 min of calm: no decay (5 min)", gate.RemainingSeconds is >= 300 and <= 301);

        // isolated failures are forgotten after an hour
        time = new FakeTime();
        gate = new AppAccessGate(time);
        Fail(gate, 3);
        time.Advance(TimeSpan.FromHours(2));
        Fail(gate, 2);
        H.Check("stale failures are forgotten", !gate.IsLockedOut);

        // success resets everything
        time = new FakeTime();
        gate = new AppAccessGate(time);
        Fail(gate, 5);
        gate.RegisterSuccess();
        H.Check("success clears the lock", !gate.IsLockedOut && gate.RemainingSeconds == 0);
    }
}
