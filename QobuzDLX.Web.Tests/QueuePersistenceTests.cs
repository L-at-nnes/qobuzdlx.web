using Newtonsoft.Json;
using QobuzDLX.Web.Services;
using Xunit;

namespace QobuzDLX.Web.Tests;

public static class QueuePersistenceTests
{
    private const string AlbumUrl = "https://play.qobuz.com/album/alb";

    private static string QueueFile(Fixtures fx) => Path.Combine(fx.ConfigDir, "queue.json");

    private static void WriteQueue(string configDir, params PersistedQueueItem[] items)
    {
        Directory.CreateDirectory(configDir);
        File.WriteAllText(Path.Combine(configDir, "queue.json"), JsonConvert.SerializeObject(items));
    }

    private static PersistedQueueItem Stored(string id, QueueItemStatus status, string url = AlbumUrl) => new()
    {
        Id = id, Url = url, Kind = "album", RefId = url[(url.LastIndexOf('/') + 1)..], Label = "Alb", Status = status, EnqueuedAt = DateTime.UtcNow
    };

    [Fact]
    public static void A_finished_item_and_its_outcome_survive_a_restart()
    {
        var cdn = FakeBackend.StartCdn();
        try
        {
            var (fx, qobuz, _) = FakeBackend.NewLibrary();
            qobuz.Track2Works = true;
            fx.Download.Enqueue(AlbumUrl, label: "Alb", subLabel: "Art");
            var finished = FakeBackend.WaitFinished(fx, AlbumUrl);
            Assert.Equal(QueueItemStatus.Completed, finished.Status);
            Assert.True(File.Exists(QueueFile(fx)), "the queue is written as soon as it changes");

            var restarted = new Fixtures(fx.ConfigDir);   // same configuration folder: a new process
            var item = Assert.Single(restarted.Download.QueueSnapshot());

            Assert.Equal(finished.Id, item.Id);
            Assert.Equal(QueueItemStatus.Completed, item.Status);
            Assert.Equal((finished.Downloaded, finished.Skipped), (item.Downloaded, item.Skipped));
            Assert.Equal(("Alb", "Art"), (item.Label, item.SubLabel));
            Assert.True(restarted.Download.IsDone(AlbumUrl), "the search page keeps its check mark");
        }
        finally { cdn.StopAsync().GetAwaiter().GetResult(); }
    }

    [Fact]
    public static void Failures_are_restored_with_their_reason_and_can_be_retried()
    {
        var cdn = FakeBackend.StartCdn();
        try
        {
            var (fx, _, _) = FakeBackend.NewLibrary();   // track 2 is refused by the fake
            fx.Download.Enqueue(AlbumUrl);
            var finished = FakeBackend.WaitFinished(fx, AlbumUrl);
            Assert.Single(finished.Failures);

            var restarted = new Fixtures(fx.ConfigDir);
            var item = Assert.Single(restarted.Download.QueueSnapshot());

            var failure = Assert.Single(item.Failures);
            Assert.Equal((2, "Two"), (failure.TrackId, failure.Title));
            Assert.Contains("Track restricted", failure.Error);
        }
        finally { cdn.StopAsync().GetAwaiter().GetResult(); }
    }

    [Fact]
    public static void An_interrupted_item_is_pending_again_and_waits_for_a_session()
    {
        var configDir = Path.Combine(Path.GetTempPath(), "qbdlx-queue-" + Guid.NewGuid().ToString("N")[..6]);
        var running = Stored("run", QueueItemStatus.Running);
        running.StartedAt = DateTime.UtcNow;
        running.Downloaded = 2;   // counted before the process stopped: the item restarts from scratch
        WriteQueue(configDir, running, Stored("pend", QueueItemStatus.Pending, "https://play.qobuz.com/album/other"));

        var fx = new Fixtures(configDir);
        var items = fx.Download.QueueSnapshot();

        Assert.All(items, item => Assert.Equal(QueueItemStatus.Pending, item.Status));
        Assert.Equal(0, items.First(i => i.Id == "run").Downloaded);
        Assert.True(fx.Download.HasPending);
        Assert.False(fx.Download.IsRunning, "nothing starts without a Qobuz session");
        Assert.Contains(fx.Download.LogSnapshot(), line => line.Contains(fx.Loc.T("log.queueRestored", 2)));
    }

    [Fact]
    public static void Opening_a_session_resumes_the_restored_queue()
    {
        var cdn = FakeBackend.StartCdn();
        try
        {
            var configDir = Path.Combine(Path.GetTempPath(), "qbdlx-queue-" + Guid.NewGuid().ToString("N")[..6]);
            WriteQueue(configDir, Stored("wait", QueueItemStatus.Pending));

            var (fx, qobuz, downloads) = FakeBackend.NewLibrary(configDir, signIn: false);
            qobuz.Track2Works = true;
            Assert.Equal(QueueItemStatus.Pending, Assert.Single(fx.Download.QueueSnapshot()).Status);

            FakeBackend.SignIn(fx);
            var item = FakeBackend.WaitFinished(fx, AlbumUrl);

            Assert.Equal(QueueItemStatus.Completed, item.Status);
            Assert.Equal(3, item.Downloaded);
            Assert.Equal(3, Directory.GetFiles(downloads, "*.flac", SearchOption.AllDirectories).Length);
        }
        finally { cdn.StopAsync().GetAwaiter().GetResult(); }
    }

    [Fact]
    public static void An_unreadable_queue_file_is_set_aside_and_the_queue_starts_empty()
    {
        var configDir = Path.Combine(Path.GetTempPath(), "qbdlx-queue-" + Guid.NewGuid().ToString("N")[..6]);
        Directory.CreateDirectory(configDir);
        File.WriteAllText(Path.Combine(configDir, "queue.json"), "{ not json");

        var fx = new Fixtures(configDir);

        Assert.Empty(fx.Download.QueueSnapshot());
        Assert.Single(Directory.GetFiles(configDir, "queue.json.corrupt-*"));
        Assert.False(File.Exists(Path.Combine(configDir, "queue.json")));
    }

    [Fact]
    public static void Only_the_most_recent_finished_items_are_kept()
    {
        var path = Path.Combine(Path.GetTempPath(), "qbdlx-queue-" + Guid.NewGuid().ToString("N")[..6], "queue.json");
        var store = new QueueStore(path, Microsoft.Extensions.Logging.Abstractions.NullLogger.Instance);

        var items = Enumerable.Range(0, QueueStore.MaxFinishedItems + 5)
            .Select(i => Stored($"done{i}", QueueItemStatus.Completed, $"https://play.qobuz.com/album/a{i}"))
            .Append(Stored("waiting", QueueItemStatus.Pending, "https://play.qobuz.com/album/w"))
            .ToList();
        store.Save(items);

        var restored = store.Load();
        Assert.Equal(QueueStore.MaxFinishedItems + 1, restored.Count);
        Assert.DoesNotContain(restored, i => i.Id == "done0");                       // the oldest went
        Assert.Contains(restored, i => i.Id == $"done{QueueStore.MaxFinishedItems + 4}");
        Assert.Contains(restored, i => i.Id == "waiting");                            // what waits is never dropped
    }
}
